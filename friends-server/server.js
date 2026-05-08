import express from "express";
import cors from "cors";
import { WebSocketServer } from "ws";
import { createServer } from "http";
import Database from "better-sqlite3";
import { v4 as uuidv4 } from "uuid";
import { randomBytes } from "crypto";
import { createWriteStream, mkdirSync, existsSync } from "fs";
import { join, extname } from "path";
import multer from "multer";

const PORT = process.env.PORT || 3478;
const DB_PATH = process.env.DB_PATH || "./friends.db";
const AVATARS_DIR = process.env.AVATARS_DIR || "./avatars";
const AVATARS_BASE_URL = process.env.AVATARS_BASE_URL || `http://2.26.54.53:${PORT}/avatars`;

// Ensure avatars directory exists
if (!existsSync(AVATARS_DIR)) mkdirSync(AVATARS_DIR, { recursive: true });

// ── Multer (avatar upload) ────────────────────────────────────────
const avatarStorage = multer.diskStorage({
  destination: (_req, _file, cb) => cb(null, AVATARS_DIR),
  filename: (req, _file, cb) => {
    // filename = userId.ext — overwrites previous avatar automatically
    const ext = ".jpg"; // we convert to jpg on client side
    cb(null, `${req.authUser.id}${ext}`);
  },
});
const upload = multer({
  storage: avatarStorage,
  limits: { fileSize: 2 * 1024 * 1024 }, // 2 MB max
  fileFilter: (_req, file, cb) => {
    if (file.mimetype.startsWith("image/")) cb(null, true);
    else cb(new Error("Only images allowed"));
  },
});

// ── Database ─────────────────────────────────────────────────────
const db = new Database(DB_PATH);
db.pragma("journal_mode = WAL");
db.pragma("foreign_keys = ON");

db.exec(`
  CREATE TABLE IF NOT EXISTS users (
    id TEXT PRIMARY KEY,
    device_id TEXT UNIQUE NOT NULL,
    nickname TEXT NOT NULL DEFAULT 'Player',
    friend_code TEXT UNIQUE NOT NULL,
    minecraft_nickname TEXT,
    avatar_url TEXT,
    bio TEXT DEFAULT '',
    created_at INTEGER NOT NULL
  );

  CREATE TABLE IF NOT EXISTS friendships (
    id TEXT PRIMARY KEY,
    sender_id TEXT NOT NULL REFERENCES users(id),
    receiver_id TEXT NOT NULL REFERENCES users(id),
    status TEXT NOT NULL CHECK(status IN ('pending', 'accepted', 'blocked')),
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
  );

  CREATE UNIQUE INDEX IF NOT EXISTS idx_friendship_pair
    ON friendships(MIN(sender_id, receiver_id), MAX(sender_id, receiver_id));

  CREATE TABLE IF NOT EXISTS follows (
    follower_id TEXT NOT NULL REFERENCES users(id),
    following_id TEXT NOT NULL REFERENCES users(id),
    created_at INTEGER NOT NULL,
    PRIMARY KEY (follower_id, following_id)
  );

  CREATE TABLE IF NOT EXISTS presence (
    user_id TEXT PRIMARY KEY REFERENCES users(id),
    online INTEGER NOT NULL DEFAULT 0,
    hosting INTEGER NOT NULL DEFAULT 0,
    host_data TEXT,
    last_heartbeat INTEGER NOT NULL
  );
`);

// Migrate: add bio column if missing (safe for existing DBs)
try { db.exec("ALTER TABLE users ADD COLUMN bio TEXT DEFAULT ''"); } catch {}

// ── Prepared Statements ──────────────────────────────────────────
const stmts = {
  getUserByDevice: db.prepare("SELECT * FROM users WHERE device_id = ?"),
  getUserById: db.prepare("SELECT * FROM users WHERE id = ?"),
  getUserByCode: db.prepare("SELECT * FROM users WHERE friend_code = ?"),
  getUserByNick: db.prepare("SELECT * FROM users WHERE LOWER(nickname) = LOWER(?)"),
  searchUsers: db.prepare(
    "SELECT id, nickname, friend_code, avatar_url, bio FROM users WHERE LOWER(nickname) LIKE LOWER(?) LIMIT 30"
  ),

  createUser: db.prepare(
    "INSERT INTO users (id, device_id, nickname, friend_code, created_at) VALUES (?, ?, ?, ?, ?)"
  ),
  updateProfile: db.prepare(
    "UPDATE users SET nickname = ?, minecraft_nickname = ?, bio = ? WHERE id = ?"
  ),
  updateAvatar: db.prepare("UPDATE users SET avatar_url = ? WHERE id = ?"),

  getFriends: db.prepare(`
    SELECT u.id, u.nickname, u.friend_code, u.minecraft_nickname, u.avatar_url, u.bio,
           f.status, f.sender_id, f.receiver_id, f.id as friendship_id,
           p.online, p.hosting, p.host_data, p.last_heartbeat
    FROM friendships f
    JOIN users u ON (u.id = CASE WHEN f.sender_id = ?1 THEN f.receiver_id ELSE f.sender_id END)
    LEFT JOIN presence p ON p.user_id = u.id
    WHERE (f.sender_id = ?1 OR f.receiver_id = ?1)
    ORDER BY f.status ASC, p.online DESC, u.nickname ASC
  `),

  getPendingRequests: db.prepare(`
    SELECT u.id, u.nickname, u.friend_code, u.avatar_url, f.id as friendship_id, f.created_at
    FROM friendships f
    JOIN users u ON u.id = f.sender_id
    WHERE f.receiver_id = ? AND f.status = 'pending'
    ORDER BY f.created_at DESC
  `),

  findFriendship: db.prepare(`
    SELECT * FROM friendships
    WHERE (sender_id = ?1 AND receiver_id = ?2) OR (sender_id = ?2 AND receiver_id = ?1)
  `),
  createFriendship: db.prepare(
    "INSERT INTO friendships (id, sender_id, receiver_id, status, created_at, updated_at) VALUES (?, ?, ?, 'pending', ?, ?)"
  ),
  acceptFriendship: db.prepare(
    "UPDATE friendships SET status = 'accepted', updated_at = ? WHERE id = ? AND receiver_id = ?"
  ),
  deleteFriendship: db.prepare("DELETE FROM friendships WHERE id = ?"),

  // Follows
  follow: db.prepare(
    "INSERT OR IGNORE INTO follows (follower_id, following_id, created_at) VALUES (?, ?, ?)"
  ),
  unfollow: db.prepare("DELETE FROM follows WHERE follower_id = ? AND following_id = ?"),
  getFollowers: db.prepare(`
    SELECT u.id, u.nickname, u.avatar_url FROM follows fl
    JOIN users u ON u.id = fl.follower_id WHERE fl.following_id = ?
  `),
  getFollowing: db.prepare(`
    SELECT u.id, u.nickname, u.avatar_url FROM follows fl
    JOIN users u ON u.id = fl.following_id WHERE fl.follower_id = ?
  `),
  isFollowing: db.prepare(
    "SELECT 1 FROM follows WHERE follower_id = ? AND following_id = ?"
  ),
  countFollowers: db.prepare("SELECT COUNT(*) as cnt FROM follows WHERE following_id = ?"),
  countFollowing: db.prepare("SELECT COUNT(*) as cnt FROM follows WHERE follower_id = ?"),

  // Presence
  upsertPresence: db.prepare(`
    INSERT INTO presence (user_id, online, hosting, host_data, last_heartbeat)
    VALUES (?1, ?2, ?3, ?4, ?5)
    ON CONFLICT(user_id) DO UPDATE SET
      online = ?2, hosting = ?3, host_data = ?4, last_heartbeat = ?5
  `),
  setOffline: db.prepare(
    "UPDATE presence SET online = 0, hosting = 0, host_data = NULL WHERE user_id = ?"
  ),
  cleanupStale: db.prepare(`
    UPDATE presence SET online = 0, hosting = 0, host_data = NULL
    WHERE last_heartbeat < ? AND online = 1
  `),
};

// ── Helper ────────────────────────────────────────────────────────
function generateFriendCode() {
  const chars = "ABCDEFGHJKLMNPQRSTUVWXYZ23456789";
  let code = "";
  const bytes = randomBytes(8);
  for (let i = 0; i < 8; i++) {
    code += chars[bytes[i] % chars.length];
    if (i === 3) code += "-";
  }
  return code;
}
const nowMs = () => Date.now();

function getOrCreateUser(deviceId) {
  let user = stmts.getUserByDevice.get(deviceId);
  if (!user) {
    const id = uuidv4();
    stmts.createUser.run(id, deviceId, "Player", generateFriendCode(), nowMs());
    user = stmts.getUserByDevice.get(deviceId);
  }
  return user;
}

function authenticateRequest(req, res) {
  const deviceId = req.headers["x-device-id"];
  if (!deviceId || deviceId.length < 8) {
    res.status(401).json({ error: "Missing or invalid X-Device-Id header" });
    return null;
  }
  return getOrCreateUser(deviceId);
}

// Middleware to attach user to req
function authMiddleware(req, res, next) {
  const user = authenticateRequest(req, res);
  if (!user) return;
  req.authUser = user;
  next();
}

// ── Express ──────────────────────────────────────────────────────
const app = express();
app.use(cors());
app.use(express.json({ limit: "1mb" }));

// Serve avatars statically
app.use("/avatars", express.static(AVATARS_DIR, {
  maxAge: "1d",
  setHeaders: (res) => res.setHeader("Cross-Origin-Resource-Policy", "cross-origin"),
}));

// Health
app.get("/health", (_req, res) => res.json({ status: "ok", time: nowMs() }));

// ── Profile ──────────────────────────────────────────────────────

// GET /api/me — own profile
app.get("/api/me", authMiddleware, (req, res) => {
  const user = req.authUser;
  const presence = db.prepare("SELECT * FROM presence WHERE user_id = ?").get(user.id);
  const followers = stmts.countFollowers.get(user.id)?.cnt ?? 0;
  const following = stmts.countFollowing.get(user.id)?.cnt ?? 0;
  const friends = stmts.getFriends.all(user.id).filter(f => f.status === "accepted").length;
  res.json({ ...user, presence, followers, following, friends });
});

// PATCH /api/me — update nickname, bio, minecraft_nickname
app.patch("/api/me", authMiddleware, (req, res) => {
  const user = req.authUser;
  const { nickname, minecraftNickname, bio } = req.body;
  stmts.updateProfile.run(
    (nickname ?? user.nickname).slice(0, 32),
    (minecraftNickname ?? user.minecraft_nickname ?? "").slice(0, 32),
    (bio ?? user.bio ?? "").slice(0, 200),
    user.id
  );
  res.json(stmts.getUserById.get(user.id));
});

// POST /api/me/avatar — upload avatar image
app.post("/api/me/avatar", authMiddleware, (req, res, next) => {
  // attach user to req so multer filename can use it
  upload.single("avatar")(req, res, (err) => {
    if (err) return res.status(400).json({ error: err.message });
    if (!req.file) return res.status(400).json({ error: "No file uploaded" });

    const avatarUrl = `${AVATARS_BASE_URL}/${req.authUser.id}.jpg`;
    stmts.updateAvatar.run(avatarUrl, req.authUser.id);

    // Notify friends about profile update
    broadcastToFriends(req.authUser.id, {
      type: "profile_update",
      userId: req.authUser.id,
      avatarUrl,
    });

    res.json({ avatarUrl });
  });
});

// GET /api/users/search?q=nickname — search all users
app.get("/api/users/search", authMiddleware, (req, res) => {
  const q = (req.query.q || "").trim();
  if (q.length < 2) return res.json({ users: [] });

  const rows = stmts.searchUsers.all(`%${q}%`);
  const myId = req.authUser.id;

  const users = rows
    .filter(u => u.id !== myId)
    .map(u => {
      const friendship = stmts.findFriendship.get(myId, u.id);
      const isFollowing = !!stmts.isFollowing.get(myId, u.id);
      return { ...u, friendship: friendship || null, isFollowing };
    });

  res.json({ users });
});

// GET /api/users/:id — public profile
app.get("/api/users/:id", authMiddleware, (req, res) => {
  const target = stmts.getUserById.get(req.params.id);
  if (!target) return res.status(404).json({ error: "User not found" });

  const myId = req.authUser.id;
  const friendship = stmts.findFriendship.get(myId, target.id);
  const isFollowing = !!stmts.isFollowing.get(myId, target.id);
  const presence = db.prepare("SELECT * FROM presence WHERE user_id = ?").get(target.id);
  const followers = stmts.countFollowers.get(target.id)?.cnt ?? 0;
  const following = stmts.countFollowing.get(target.id)?.cnt ?? 0;
  const friendsList = stmts.getFriends.all(target.id)
    .filter(f => f.status === "accepted")
    .map(f => ({ id: f.id, nickname: f.nickname, avatar_url: f.avatar_url }));

  // Hide device_id from public view
  const { device_id: _d, ...publicUser } = target;

  res.json({
    ...publicUser,
    presence,
    friendship: friendship || null,
    isFollowing,
    followers,
    following,
    friends: friendsList,
  });
});

// ── Friends ──────────────────────────────────────────────────────

app.get("/api/friends", authMiddleware, (req, res) => {
  const friends = stmts.getFriends.all(req.authUser.id);
  const pending = stmts.getPendingRequests.all(req.authUser.id);
  res.json({ friends, pendingRequests: pending });
});

// POST /api/friends/add — send request by userId (search result click)
app.post("/api/friends/add", authMiddleware, (req, res) => {
  const user = req.authUser;
  const { userId, friendCode } = req.body;

  let target = null;
  if (userId) target = stmts.getUserById.get(userId);
  else if (friendCode)
    target = stmts.getUserByCode.get(friendCode.toUpperCase().replace(/[^A-Z0-9-]/g, ""));

  if (!target) return res.status(404).json({ error: "User not found" });
  if (target.id === user.id) return res.status(400).json({ error: "Cannot add yourself" });

  const existing = stmts.findFriendship.get(user.id, target.id);
  if (existing) {
    if (existing.status === "accepted") return res.status(409).json({ error: "Already friends" });
    if (existing.status === "pending") return res.status(409).json({ error: "Request already sent" });
  }

  const id = uuidv4();
  const now = nowMs();
  stmts.createFriendship.run(id, user.id, target.id, now, now);

  broadcastToUser(target.id, {
    type: "friend_request",
    friendshipId: id,
    from: { id: user.id, nickname: user.nickname, friendCode: user.friend_code, avatarUrl: user.avatar_url },
  });

  res.json({ success: true, friendshipId: id });
});

app.post("/api/friends/accept", authMiddleware, (req, res) => {
  const user = req.authUser;
  const { friendshipId } = req.body;
  if (!friendshipId) return res.status(400).json({ error: "friendshipId required" });

  const result = stmts.acceptFriendship.run(nowMs(), friendshipId, user.id);
  if (result.changes === 0) return res.status(404).json({ error: "Request not found" });

  const friendship = db.prepare("SELECT * FROM friendships WHERE id = ?").get(friendshipId);
  if (friendship) {
    broadcastToUser(friendship.sender_id, {
      type: "friend_accepted",
      by: { id: user.id, nickname: user.nickname, avatarUrl: user.avatar_url },
    });
  }
  res.json({ success: true });
});

app.delete("/api/friends/:friendshipId", authMiddleware, (req, res) => {
  const user = req.authUser;
  const friendship = db.prepare("SELECT * FROM friendships WHERE id = ?").get(req.params.friendshipId);
  if (!friendship) return res.status(404).json({ error: "Not found" });
  if (friendship.sender_id !== user.id && friendship.receiver_id !== user.id)
    return res.status(403).json({ error: "Not your friendship" });

  stmts.deleteFriendship.run(req.params.friendshipId);
  res.json({ success: true });
});

// ── Follows ──────────────────────────────────────────────────────

app.post("/api/users/:id/follow", authMiddleware, (req, res) => {
  const myId = req.authUser.id;
  if (myId === req.params.id) return res.status(400).json({ error: "Cannot follow yourself" });
  stmts.follow.run(myId, req.params.id, nowMs());
  res.json({ success: true });
});

app.delete("/api/users/:id/follow", authMiddleware, (req, res) => {
  stmts.unfollow.run(req.authUser.id, req.params.id);
  res.json({ success: true });
});

app.get("/api/users/:id/followers", authMiddleware, (req, res) => {
  res.json({ followers: stmts.getFollowers.all(req.params.id) });
});

app.get("/api/users/:id/following", authMiddleware, (req, res) => {
  res.json({ following: stmts.getFollowing.all(req.params.id) });
});

// ── WebSocket (Presence + Real-time) ─────────────────────────────
const server = createServer(app);
const wss = new WebSocketServer({ server, path: "/ws" });

/** @type {Map<string, Set<import('ws').WebSocket>>} */
const userSockets = new Map();

function broadcastToUser(userId, data) {
  const sockets = userSockets.get(userId);
  if (!sockets) return;
  const msg = JSON.stringify(data);
  for (const ws of sockets) {
    if (ws.readyState === 1) ws.send(msg);
  }
}

function broadcastToFriends(userId, data) {
  const friends = stmts.getFriends.all(userId);
  for (const f of friends) {
    if (f.status === "accepted") broadcastToUser(f.id, data);
  }
}

wss.on("connection", (ws, req) => {
  const url = new URL(req.url, `http://${req.headers.host}`);
  const deviceId = url.searchParams.get("deviceId");
  if (!deviceId) { ws.close(4001, "Missing deviceId"); return; }

  const user = getOrCreateUser(deviceId);

  if (!userSockets.has(user.id)) userSockets.set(user.id, new Set());
  userSockets.get(user.id).add(ws);

  stmts.upsertPresence.run(user.id, 1, 0, null, nowMs());
  broadcastToFriends(user.id, { type: "presence", userId: user.id, online: true, hosting: false });

  ws.send(JSON.stringify({
    type: "init",
    user,
    friends: stmts.getFriends.all(user.id),
    pendingRequests: stmts.getPendingRequests.all(user.id),
  }));

  ws.on("message", (raw) => {
    try { handleWsMessage(user, ws, JSON.parse(raw)); } catch {}
  });

  ws.on("close", () => {
    const sockets = userSockets.get(user.id);
    if (sockets) {
      sockets.delete(ws);
      if (sockets.size === 0) {
        userSockets.delete(user.id);
        stmts.setOffline.run(user.id);
        broadcastToFriends(user.id, { type: "presence", userId: user.id, online: false, hosting: false });
      }
    }
  });

  ws.isAlive = true;
  ws.on("pong", () => { ws.isAlive = true; });
});

function handleWsMessage(user, ws, msg) {
  switch (msg.type) {
    case "heartbeat":
      stmts.upsertPresence.run(
        user.id, 1, msg.hosting ? 1 : 0,
        msg.hostData ? JSON.stringify(msg.hostData) : null, nowMs()
      );
      if (msg.hosting) {
        broadcastToFriends(user.id, {
          type: "presence", userId: user.id, online: true, hosting: true, hostData: msg.hostData,
        });
      }
      break;
    case "stop_hosting":
      stmts.upsertPresence.run(user.id, 1, 0, null, nowMs());
      broadcastToFriends(user.id, { type: "presence", userId: user.id, online: true, hosting: false });
      break;
    case "ping":
      ws.send(JSON.stringify({ type: "pong", time: msg.time }));
      break;
  }
}

// Ping dead connections every 30s
const heartbeatInterval = setInterval(() => {
  wss.clients.forEach((ws) => {
    if (!ws.isAlive) return ws.terminate();
    ws.isAlive = false;
    ws.ping();
  });
  stmts.cleanupStale.run(nowMs() - 60_000);
}, 30_000);

wss.on("close", () => clearInterval(heartbeatInterval));

// ── Start ────────────────────────────────────────────────────────
server.listen(PORT, "0.0.0.0", () => {
  console.log(`🎮 P2P Friends Server running on http://0.0.0.0:${PORT}`);
  console.log(`   Avatars: ${AVATARS_BASE_URL}`);
  console.log(`   Database: ${DB_PATH}`);
});
