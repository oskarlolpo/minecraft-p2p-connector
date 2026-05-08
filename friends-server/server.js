import express from "express";
import cors from "cors";
import { WebSocketServer } from "ws";
import { createServer } from "http";
import Database from "better-sqlite3";
import { v4 as uuidv4 } from "uuid";
import { randomBytes } from "crypto";

const PORT = process.env.PORT || 3478;
const DB_PATH = process.env.DB_PATH || "./friends.db";

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

  CREATE TABLE IF NOT EXISTS presence (
    user_id TEXT PRIMARY KEY REFERENCES users(id),
    online INTEGER NOT NULL DEFAULT 0,
    hosting INTEGER NOT NULL DEFAULT 0,
    host_data TEXT,
    last_heartbeat INTEGER NOT NULL
  );
`);

// ── Prepared Statements ──────────────────────────────────────────
const stmts = {
  getUserByDevice: db.prepare("SELECT * FROM users WHERE device_id = ?"),
  getUserById: db.prepare("SELECT * FROM users WHERE id = ?"),
  getUserByCode: db.prepare("SELECT * FROM users WHERE friend_code = ?"),
  createUser: db.prepare(
    "INSERT INTO users (id, device_id, nickname, friend_code, created_at) VALUES (?, ?, ?, ?, ?)"
  ),
  updateNickname: db.prepare("UPDATE users SET nickname = ?, minecraft_nickname = ? WHERE id = ?"),

  getFriends: db.prepare(`
    SELECT u.id, u.nickname, u.friend_code, u.minecraft_nickname, u.avatar_url,
           f.status, f.sender_id, f.receiver_id, f.id as friendship_id,
           p.online, p.hosting, p.host_data, p.last_heartbeat
    FROM friendships f
    JOIN users u ON (u.id = CASE WHEN f.sender_id = ?1 THEN f.receiver_id ELSE f.sender_id END)
    LEFT JOIN presence p ON p.user_id = u.id
    WHERE (f.sender_id = ?1 OR f.receiver_id = ?1)
    ORDER BY f.status ASC, p.online DESC, u.nickname ASC
  `),

  getPendingRequests: db.prepare(`
    SELECT u.id, u.nickname, u.friend_code, f.id as friendship_id, f.created_at
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

  upsertPresence: db.prepare(`
    INSERT INTO presence (user_id, online, hosting, host_data, last_heartbeat)
    VALUES (?1, ?2, ?3, ?4, ?5)
    ON CONFLICT(user_id) DO UPDATE SET
      online = ?2, hosting = ?3, host_data = ?4, last_heartbeat = ?5
  `),

  setOffline: db.prepare("UPDATE presence SET online = 0, hosting = 0, host_data = NULL WHERE user_id = ?"),

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

function nowMs() {
  return Date.now();
}

function authenticateRequest(req, res) {
  const deviceId = req.headers["x-device-id"];
  if (!deviceId || deviceId.length < 8) {
    res.status(401).json({ error: "Missing or invalid X-Device-Id header" });
    return null;
  }
  let user = stmts.getUserByDevice.get(deviceId);
  if (!user) {
    const id = uuidv4();
    const friendCode = generateFriendCode();
    stmts.createUser.run(id, deviceId, "Player", friendCode, nowMs());
    user = stmts.getUserByDevice.get(deviceId);
  }
  return user;
}

// ── Express ──────────────────────────────────────────────────────
const app = express();
app.use(cors());
app.use(express.json());

// Health check
app.get("/health", (_req, res) => {
  res.json({ status: "ok", time: nowMs() });
});

// Get/create user profile
app.get("/api/me", (req, res) => {
  const user = authenticateRequest(req, res);
  if (!user) return;
  const presence = db.prepare("SELECT * FROM presence WHERE user_id = ?").get(user.id);
  res.json({ ...user, presence });
});

// Update profile
app.patch("/api/me", (req, res) => {
  const user = authenticateRequest(req, res);
  if (!user) return;
  const { nickname, minecraftNickname } = req.body;
  if (nickname) {
    stmts.updateNickname.run(
      nickname.slice(0, 32),
      minecraftNickname?.slice(0, 32) || user.minecraft_nickname,
      user.id
    );
  }
  res.json(stmts.getUserById.get(user.id));
});

// Get friends list
app.get("/api/friends", (req, res) => {
  const user = authenticateRequest(req, res);
  if (!user) return;
  const friends = stmts.getFriends.all(user.id);
  const pending = stmts.getPendingRequests.all(user.id);
  res.json({ friends, pendingRequests: pending });
});

// Add friend by code
app.post("/api/friends/add", (req, res) => {
  const user = authenticateRequest(req, res);
  if (!user) return;

  const { friendCode } = req.body;
  if (!friendCode) return res.status(400).json({ error: "friendCode required" });

  const target = stmts.getUserByCode.get(friendCode.toUpperCase().replace(/[^A-Z0-9-]/g, ""));
  if (!target) return res.status(404).json({ error: "User with this code not found" });
  if (target.id === user.id) return res.status(400).json({ error: "Cannot add yourself" });

  const existing = stmts.findFriendship.get(user.id, target.id);
  if (existing) {
    if (existing.status === "accepted") return res.status(409).json({ error: "Already friends" });
    if (existing.status === "pending") return res.status(409).json({ error: "Request already sent" });
  }

  const id = uuidv4();
  const now = nowMs();
  stmts.createFriendship.run(id, user.id, target.id, now, now);

  // Notify target via WebSocket
  broadcastToUser(target.id, {
    type: "friend_request",
    from: { id: user.id, nickname: user.nickname, friendCode: user.friend_code },
  });

  res.json({ success: true, friendshipId: id });
});

// Accept friend request
app.post("/api/friends/accept", (req, res) => {
  const user = authenticateRequest(req, res);
  if (!user) return;

  const { friendshipId } = req.body;
  if (!friendshipId) return res.status(400).json({ error: "friendshipId required" });

  const result = stmts.acceptFriendship.run(nowMs(), friendshipId, user.id);
  if (result.changes === 0) return res.status(404).json({ error: "Request not found or not yours" });

  // Notify sender
  const friendship = db.prepare("SELECT * FROM friendships WHERE id = ?").get(friendshipId);
  if (friendship) {
    broadcastToUser(friendship.sender_id, {
      type: "friend_accepted",
      by: { id: user.id, nickname: user.nickname },
    });
  }

  res.json({ success: true });
});

// Remove friend / reject request
app.delete("/api/friends/:friendshipId", (req, res) => {
  const user = authenticateRequest(req, res);
  if (!user) return;

  const friendship = db.prepare("SELECT * FROM friendships WHERE id = ?").get(req.params.friendshipId);
  if (!friendship) return res.status(404).json({ error: "Not found" });
  if (friendship.sender_id !== user.id && friendship.receiver_id !== user.id) {
    return res.status(403).json({ error: "Not your friendship" });
  }

  stmts.deleteFriendship.run(req.params.friendshipId);
  res.json({ success: true });
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
  if (!deviceId) {
    ws.close(4001, "Missing deviceId");
    return;
  }

  let user = stmts.getUserByDevice.get(deviceId);
  if (!user) {
    const id = uuidv4();
    const friendCode = generateFriendCode();
    stmts.createUser.run(id, deviceId, "Player", friendCode, nowMs());
    user = stmts.getUserByDevice.get(deviceId);
  }

  // Track socket
  if (!userSockets.has(user.id)) userSockets.set(user.id, new Set());
  userSockets.get(user.id).add(ws);

  // Set online
  stmts.upsertPresence.run(user.id, 1, 0, null, nowMs());
  broadcastToFriends(user.id, {
    type: "presence",
    userId: user.id,
    online: true,
    hosting: false,
  });

  // Send initial data
  ws.send(JSON.stringify({
    type: "init",
    user,
    friends: stmts.getFriends.all(user.id),
    pendingRequests: stmts.getPendingRequests.all(user.id),
  }));

  ws.on("message", (raw) => {
    try {
      const msg = JSON.parse(raw);
      handleWsMessage(user, ws, msg);
    } catch {
      // ignore invalid messages
    }
  });

  ws.on("close", () => {
    const sockets = userSockets.get(user.id);
    if (sockets) {
      sockets.delete(ws);
      if (sockets.size === 0) {
        userSockets.delete(user.id);
        stmts.setOffline.run(user.id);
        broadcastToFriends(user.id, {
          type: "presence",
          userId: user.id,
          online: false,
          hosting: false,
        });
      }
    }
  });

  // Heartbeat
  ws.isAlive = true;
  ws.on("pong", () => { ws.isAlive = true; });
});

function handleWsMessage(user, ws, msg) {
  switch (msg.type) {
    case "heartbeat":
      stmts.upsertPresence.run(user.id, 1, msg.hosting ? 1 : 0, msg.hostData ? JSON.stringify(msg.hostData) : null, nowMs());
      if (msg.hosting) {
        broadcastToFriends(user.id, {
          type: "presence",
          userId: user.id,
          online: true,
          hosting: true,
          hostData: msg.hostData,
        });
      }
      break;

    case "stop_hosting":
      stmts.upsertPresence.run(user.id, 1, 0, null, nowMs());
      broadcastToFriends(user.id, {
        type: "presence",
        userId: user.id,
        online: true,
        hosting: false,
      });
      break;

    case "ping":
      ws.send(JSON.stringify({ type: "pong", time: msg.time }));
      break;
  }
}

// Heartbeat interval to detect dead connections
const heartbeatInterval = setInterval(() => {
  wss.clients.forEach((ws) => {
    if (!ws.isAlive) return ws.terminate();
    ws.isAlive = false;
    ws.ping();
  });
  // Cleanup stale presence entries (offline > 60s)
  stmts.cleanupStale.run(nowMs() - 60_000);
}, 30_000);

wss.on("close", () => clearInterval(heartbeatInterval));

// ── Start ────────────────────────────────────────────────────────
server.listen(PORT, "0.0.0.0", () => {
  console.log(`🎮 P2P Friends Server running on http://0.0.0.0:${PORT}`);
  console.log(`   WebSocket: ws://0.0.0.0:${PORT}/ws`);
  console.log(`   Database: ${DB_PATH}`);
});
