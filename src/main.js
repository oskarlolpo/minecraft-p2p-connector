import * as Ably from "ably";
import { invoke } from "@tauri-apps/api/core";

const ABLY_API_KEY = "aGkPAA.1VHkjw:Bai-67g05FcqHdfVOMiSfjYlK3aLz8wOzj5WeTgz4cw";
const LOBBY_CHANNEL_NAME = "minecraft-lobby";
const DEFAULT_SLOTS = "1/30";
const POLL_INTERVAL_MS = 1500;

const roomNameEl = document.querySelector("#room-name");
const roomPasswordEl = document.querySelector("#room-password");
const hostButtonEl = document.querySelector("#host-button");
const stopButtonEl = document.querySelector("#stop-button");
const refreshLobbyEl = document.querySelector("#refresh-lobby");
const copyLogsEl = document.querySelector("#copy-logs");
const copySelectedEndpointEl = document.querySelector("#copy-selected-endpoint");
const connectSelectedEl = document.querySelector("#connect-selected");
const serverListEl = document.querySelector("#server-list");
const logsEl = document.querySelector("#logs");
const peerListEl = document.querySelector("#peer-list");
const connectionStateEl = document.querySelector("#connection-state");
const ablyStateEl = document.querySelector("#ably-state");
const lobbyCountEl = document.querySelector("#lobby-count");
const publicEndpointEl = document.querySelector("#public-endpoint");
const selectedServerEl = document.querySelector("#selected-server");
const selectedEndpointEl = document.querySelector("#selected-endpoint");
const statusNoteEl = document.querySelector("#status-note");
const peerCountEl = document.querySelector("#peer-count");

const hostSession = {
  active: false,
  roomName: "",
  hasPassword: false,
  peerAddr: null,
  presencePayload: null,
};

const localClientId = ensureClientId();
const state = {
  servers: [],
  selectedServerId: null,
  status: null,
  realtime: null,
  lobbyChannel: null,
  privateChannel: null,
  logBuffer: [],
  syncingPresence: false,
  channelHandlersBound: false,
};

function ensureClientId() {
  const key = "blood-paradise-client-id";
  const existing = localStorage.getItem(key);
  if (existing) {
    return existing;
  }
  const created = `bp-${crypto.randomUUID().slice(0, 8)}`;
  localStorage.setItem(key, created);
  return created;
}

function addLog(message) {
  const stamp = new Date().toLocaleTimeString("ru-RU", {
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
  });
  state.logBuffer.unshift(`[${stamp}] ${message}`);
  state.logBuffer = state.logBuffer.slice(0, 100);
  renderLogs();
}

function currentLogLines() {
  const combined = [...state.logBuffer];
  if (state.status?.logs?.length) {
    combined.push(...state.status.logs);
  }
  return [...new Set(combined)].slice(0, 80);
}

function renderLogs() {
  const lines = currentLogLines();
  logsEl.innerHTML = lines.length
    ? lines.map((entry) => `<div class="log-entry">${escapeHtml(entry)}</div>`).join("")
    : `<div class="log-entry text-white/35">Лог пока пуст.</div>`;
}

function syncButtons() {
  const mode = state.status?.mode ?? "idle";
  const busy = ["starting", "connecting", "punching", "waitingForPeer"].includes(
    state.status?.state ?? "idle",
  );
  const isHostMode = mode === "host";
  const selectedServer = getSelectedServer();

  hostButtonEl.disabled = isHostMode || busy;
  stopButtonEl.disabled = mode === "idle";
  connectSelectedEl.disabled =
    !selectedServer || selectedServer.clientId === localClientId || busy || mode === "host";
  copySelectedEndpointEl.disabled = !selectedServer?.peerAddr;
}

function getSelectedServer() {
  return state.servers.find((server) => server.clientId === state.selectedServerId) ?? null;
}

function renderSelectedServer() {
  const selected = getSelectedServer();
  selectedServerEl.textContent = selected ? selected.roomName : "No selection";
  selectedEndpointEl.textContent = selected?.peerAddr ?? "n/a";
  syncButtons();
}

function renderServers() {
  lobbyCountEl.textContent = `${state.servers.length} servers`;
  if (!state.servers.length) {
    serverListEl.innerHTML =
      '<div class="log-entry text-white/35">В lobby пока нет активных хостов.</div>';
    renderSelectedServer();
    return;
  }

  serverListEl.innerHTML = state.servers
    .map((server) => {
      const isSelected = state.selectedServerId === server.clientId;
      const isLocal = server.clientId === localClientId;
      return `
        <article class="server-card ${isSelected ? "server-card-active" : ""}">
          <div class="flex items-start justify-between gap-3">
            <div>
              <p class="text-base font-semibold text-white">${escapeHtml(server.roomName)}</p>
              <p class="mt-1 text-xs text-white/45">
                Host: ${escapeHtml(server.hostName)}${isLocal ? " (you)" : ""}
              </p>
              <p class="mt-1 break-all text-[11px] text-white/35">${escapeHtml(server.peerAddr ?? "n/a")}</p>
            </div>
            <div class="text-right">
              <p class="text-xs uppercase tracking-[0.18em] text-white/45">${escapeHtml(server.slots)}</p>
              <p class="mt-2 text-xl">${server.hasPassword ? "🔒" : "⚔"}</p>
            </div>
          </div>
          <div class="mt-4 flex gap-3">
            <button class="ghost-button flex-1" data-select-server="${escapeHtml(server.clientId)}">Select</button>
            <button
              class="${isLocal ? "ghost-button" : "primary-button"} flex-1"
              data-connect-server="${escapeHtml(server.clientId)}"
              ${isLocal ? "disabled" : ""}
            >
              ${isLocal ? "Hosting" : "Connect"}
            </button>
          </div>
        </article>
      `;
    })
    .join("");

  renderSelectedServer();
}

function renderPeers(peers) {
  peerCountEl.textContent = `${peers?.length ?? 0} peers`;
  if (!peers?.length) {
    peerListEl.innerHTML = '<div class="log-entry text-white/35">Нет активных peer-соединений.</div>';
    return;
  }

  peerListEl.innerHTML = peers
    .map((peer) => {
      const ping = peer.pingMs == null ? "n/a" : `${peer.pingMs} ms`;
      return `
        <div class="peer-card">
          <div>
            <p class="text-sm font-semibold text-white">${escapeHtml(peer.peerId)}</p>
            <p class="mt-1 break-all text-xs text-white/45">${escapeHtml(peer.addr)}</p>
          </div>
          <div class="text-right">
            <p class="text-xs uppercase tracking-[0.18em] ${peer.connected ? "text-red-300" : "text-white/35"}">
              ${peer.connected ? "online" : "pending"}
            </p>
            <p class="mt-1 text-xs text-white/60">${ping}</p>
          </div>
        </div>
      `;
    })
    .join("");
}

function renderStatus(status) {
  state.status = status;
  connectionStateEl.textContent = formatState(status.state);
  ablyStateEl.textContent = state.realtime?.connection.state ?? "offline";
  publicEndpointEl.textContent = status.publicUdpAddr ?? status.udpBindAddr ?? "n/a";
  statusNoteEl.textContent = status.note ?? "Idle";
  renderPeers(status.peers ?? []);
  renderLogs();
  syncButtons();
}

function formatState(value) {
  const labels = {
    idle: "Idle",
    starting: "Booting",
    waitingForPeer: "Waiting",
    punching: "Punching",
    connecting: "Connecting",
    hosting: "Hosting",
    connected: "Connected",
    error: "Error",
  };
  return labels[value] ?? value ?? "Idle";
}

function hydrateServers(members) {
  state.servers = members
    .map((member) => {
      const data = member.data ?? {};
      return {
        clientId: member.clientId,
        roomName: data.room_name ?? "Unnamed room",
        hostName: data.host_name ?? member.clientId,
        slots: data.slots ?? DEFAULT_SLOTS,
        hasPassword: Boolean(data.has_password),
        peerAddr: data.peer_addr ?? null,
      };
    })
    .filter((server) => Boolean(server.peerAddr));

  if (state.selectedServerId && !state.servers.find((server) => server.clientId === state.selectedServerId)) {
    state.selectedServerId = null;
  }

  if (!state.selectedServerId && state.servers.length === 1) {
    state.selectedServerId = state.servers[0].clientId;
  }

  renderServers();
}

async function recreateChannels() {
  if (!state.realtime) {
    return;
  }

  state.channelHandlersBound = false;
  state.realtime.channels.release(LOBBY_CHANNEL_NAME);
  state.realtime.channels.release(`lobby:${localClientId}`);
  state.lobbyChannel = state.realtime.channels.get(LOBBY_CHANNEL_NAME);
  state.privateChannel = state.realtime.channels.get(`lobby:${localClientId}`);
  await bindChannelHandlers();
}

async function bindChannelHandlers() {
  if (!state.lobbyChannel || !state.privateChannel || state.channelHandlersBound) {
    return;
  }

  await state.lobbyChannel.presence.subscribe("enter", () => void refreshLobby(false));
  await state.lobbyChannel.presence.subscribe("update", () => void refreshLobby(false));
  await state.lobbyChannel.presence.subscribe("leave", () => void refreshLobby(false));

  await state.privateChannel.subscribe("connect-request", async (message) => {
    const peerAddr = message.data?.peer_addr;
    const requester = message.data?.client_id ?? "unknown";
    addLog(`Incoming handshake from ${requester}: ${peerAddr ?? "n/a"}`);
    if (!peerAddr) {
      return;
    }

    try {
      await invoke("connect_to_peer", { peerAddr });
      addLog(`Host sent punch packets to ${peerAddr}.`);
    } catch (error) {
      addLog(`Punch error: ${String(error)}`);
    }
  });

  state.channelHandlersBound = true;
}

async function refreshLobby(forceRecover = false) {
  if (!state.lobbyChannel || !state.realtime) {
    return;
  }

  try {
    if (forceRecover || ["suspended", "detached", "failed"].includes(state.lobbyChannel.state)) {
      await recreateChannels();
    }

    if (state.realtime.connection.state !== "connected") {
      addLog("Lobby refresh postponed: Ably not connected.");
      return;
    }

    if (state.lobbyChannel.state !== "attached") {
      await state.lobbyChannel.attach();
    }

    const members = await state.lobbyChannel.presence.get();
    hydrateServers(members);
    addLog(`Lobby refresh: ${members.length} presence members.`);
  } catch (error) {
    addLog(`Lobby refresh failed: ${String(error)}`);
  }
}

function buildPresencePayload(status) {
  return {
    room_name: hostSession.roomName,
    host_name: localClientId,
    slots: `${Math.max(1, (status?.peerCount ?? 0) + 1)}/30`,
    has_password: hostSession.hasPassword,
    peer_addr: hostSession.peerAddr,
  };
}

async function syncPresence(status, { force = false, enter = false } = {}) {
  if (!hostSession.active || !state.lobbyChannel || !hostSession.peerAddr || state.syncingPresence) {
    return;
  }

  if (state.realtime?.connection.state !== "connected") {
    return;
  }

  const payload = buildPresencePayload(status);
  const serialized = JSON.stringify(payload);
  if (!force && !enter && serialized === hostSession.presencePayload) {
    return;
  }

  state.syncingPresence = true;
  try {
    if (["suspended", "detached", "failed"].includes(state.lobbyChannel.state)) {
      await recreateChannels();
    }

    if (state.lobbyChannel.state !== "attached") {
      await state.lobbyChannel.attach();
    }

    if (enter || !hostSession.presencePayload) {
      await state.lobbyChannel.presence.enter(payload);
      addLog(`Presence entered for room "${hostSession.roomName}" (${hostSession.peerAddr}).`);
    } else {
      await state.lobbyChannel.presence.update(payload);
    }

    hostSession.presencePayload = serialized;
  } catch (error) {
    addLog(`Presence sync failed: ${String(error)}`);
  } finally {
    state.syncingPresence = false;
  }
}

async function setupAbly() {
  const realtime = new Ably.Realtime({
    key: ABLY_API_KEY,
    clientId: localClientId,
  });
  state.realtime = realtime;

  realtime.connection.on(async (change) => {
    ablyStateEl.textContent = change.current;
    addLog(`Ably connection: ${change.previous ?? "none"} -> ${change.current}`);

    if (change.current === "connected") {
      await recreateChannels();
      await syncPresence(state.status, { force: true, enter: !hostSession.presencePayload });
      await refreshLobby(false);
    }
  });

  await new Promise((resolve) => realtime.connection.once("connected", resolve));
  await recreateChannels();
  await refreshLobby(false);
}

async function startHosting() {
  const roomName = roomNameEl.value.trim();
  if (!roomName) {
    roomNameEl.focus();
    return;
  }

  const password = roomPasswordEl.value.trim() || null;
  hostButtonEl.disabled = true;

  try {
    await invoke("start_hosting", { roomName, password });
    const status = await waitForStatus((snapshot) => Boolean(snapshot.publicUdpAddr));
    renderStatus(status);

    hostSession.active = true;
    hostSession.roomName = roomName;
    hostSession.hasPassword = Boolean(password);
    hostSession.peerAddr = status.publicUdpAddr ?? status.udpBindAddr;
    hostSession.presencePayload = null;

    await syncPresence(status, { force: true, enter: true });
    await refreshLobby(true);
  } catch (error) {
    addLog(`Host start failed: ${String(error)}`);
  } finally {
    syncButtons();
  }
}

async function stopHosting() {
  stopButtonEl.disabled = true;
  hostButtonEl.disabled = true;

  try {
    if (hostSession.active && state.lobbyChannel && state.realtime?.connection.state === "connected") {
      try {
        if (state.lobbyChannel.state !== "attached") {
          await state.lobbyChannel.attach();
        }
        await state.lobbyChannel.presence.leave();
        addLog("Presence left.");
      } catch (presenceError) {
        addLog(`Presence leave skipped: ${String(presenceError)}`);
      }
    }

    await invoke("stop_hosting");
  } catch (error) {
    addLog(`Stop failed: ${String(error)}`);
  } finally {
    hostSession.active = false;
    hostSession.roomName = "";
    hostSession.hasPassword = false;
    hostSession.peerAddr = null;
    hostSession.presencePayload = null;
    state.selectedServerId = null;

    try {
      const status = await invoke("get_status");
      renderStatus(status);
    } catch {
      renderStatus({
        mode: "idle",
        state: "idle",
        roomCode: null,
        udpBindAddr: null,
        publicUdpAddr: null,
        peerCount: 0,
        peers: [],
        note: "Idle",
        lastError: null,
        signalingServer: "Ably Presence + Channels",
        logs: ["Session cleared."],
      });
    }

    await refreshLobby(true);
    addLog("Host session stopped.");
    syncButtons();
  }
}

async function connectToServer(server) {
  state.selectedServerId = server.clientId;
  renderSelectedServer();

  if (server.clientId === localClientId) {
    addLog("Собственный host выбран. Для подключения нужен второй клиент.");
    return;
  }

  if (server.hasPassword) {
    const provided = window.prompt(`Введите пароль для "${server.roomName}"`);
    if (provided == null) {
      return;
    }
  }

  try {
    addLog(`Connecting to ${server.roomName} via ${server.peerAddr}`);
    await invoke("connect_to_peer", { peerAddr: server.peerAddr });
    const status = await waitForStatus((snapshot) => Boolean(snapshot.publicUdpAddr), 8000);

    await state.realtime.channels
      .get(`lobby:${server.clientId}`)
      .publish("connect-request", {
        client_id: localClientId,
        room_name: server.roomName,
        peer_addr: status.publicUdpAddr ?? status.udpBindAddr,
      });

    addLog(`Handshake request sent to host ${server.clientId}.`);
  } catch (error) {
    addLog(`Connect failed: ${String(error)}`);
  }
}

async function pollStatus() {
  try {
    const status = await invoke("get_status");
    renderStatus(status);
    await syncPresence(status);
  } catch (error) {
    addLog(`Status poll failed: ${String(error)}`);
  }
}

async function waitForStatus(predicate, timeoutMs = 6000) {
  const started = Date.now();
  while (Date.now() - started < timeoutMs) {
    const status = await invoke("get_status");
    renderStatus(status);
    if (predicate(status)) {
      return status;
    }
    await new Promise((resolve) => setTimeout(resolve, 250));
  }
  throw new Error("Timed out while waiting for backend status.");
}

function escapeHtml(value) {
  return String(value)
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll('"', "&quot;")
    .replaceAll("'", "&#039;");
}

hostButtonEl.addEventListener("click", startHosting);
stopButtonEl.addEventListener("click", stopHosting);
refreshLobbyEl.addEventListener("click", () => void refreshLobby(true));
copyLogsEl.addEventListener("click", async () => {
  const text = currentLogLines().join("\n");
  await navigator.clipboard.writeText(text);
  addLog("Debug log copied to clipboard.");
});
copySelectedEndpointEl.addEventListener("click", async () => {
  const selected = getSelectedServer();
  if (!selected?.peerAddr) {
    return;
  }
  await navigator.clipboard.writeText(selected.peerAddr);
  addLog(`Copied endpoint: ${selected.peerAddr}`);
});
connectSelectedEl.addEventListener("click", async () => {
  const selected = getSelectedServer();
  if (selected) {
    await connectToServer(selected);
  }
});

serverListEl.addEventListener("click", async (event) => {
  const selectId = event.target.closest("[data-select-server]")?.dataset.selectServer;
  if (selectId) {
    state.selectedServerId = selectId;
    renderSelectedServer();
    renderServers();
    return;
  }

  const connectId = event.target.closest("[data-connect-server]")?.dataset.connectServer;
  if (!connectId) {
    return;
  }

  const server = state.servers.find((item) => item.clientId === connectId);
  if (server) {
    await connectToServer(server);
  }
});

setInterval(() => {
  void pollStatus();
}, POLL_INTERVAL_MS);

await setupAbly();
await pollStatus();
