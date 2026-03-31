import * as Ably from "ably";
import { invoke } from "@tauri-apps/api/core";

const ABLY_API_KEY = "aGkPAA.1VHkjw:Bai-67g05FcqHdfVOMiSfjYlK3aLz8wOzj5WeTgz4cw";
const LOBBY_CHANNEL_NAME = "minecraft-lobby";
const DEFAULT_SLOTS = "1/30";

const roomNameEl = document.querySelector("#room-name");
const roomPasswordEl = document.querySelector("#room-password");
const hostButtonEl = document.querySelector("#host-button");
const stopButtonEl = document.querySelector("#stop-button");
const refreshLobbyEl = document.querySelector("#refresh-lobby");
const serverListEl = document.querySelector("#server-list");
const logsEl = document.querySelector("#logs");
const peerListEl = document.querySelector("#peer-list");
const connectionStateEl = document.querySelector("#connection-state");
const ablyStateEl = document.querySelector("#ably-state");
const lobbyCountEl = document.querySelector("#lobby-count");
const publicEndpointEl = document.querySelector("#public-endpoint");
const selectedServerEl = document.querySelector("#selected-server");
const statusNoteEl = document.querySelector("#status-note");
const peerCountEl = document.querySelector("#peer-count");

const hostSession = {
  active: false,
  roomName: "",
  hasPassword: false,
  peerAddr: null,
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
  state.logBuffer = state.logBuffer.slice(0, 80);
  renderLogs();
}

function renderLogs() {
  const combined = [...state.logBuffer];
  if (state.status?.logs?.length) {
    combined.push(...state.status.logs);
  }

  const unique = [...new Set(combined)].slice(0, 50);
  logsEl.innerHTML = unique.length
    ? unique.map((entry) => `<div class="log-entry">${escapeHtml(entry)}</div>`).join("")
    : `<div class="log-entry text-white/35">Лог пока пуст.</div>`;
}

function renderServers() {
  lobbyCountEl.textContent = `${state.servers.length} servers`;
  if (!state.servers.length) {
    serverListEl.innerHTML =
      '<div class="log-entry text-white/35">В lobby пока нет активных хостов.</div>';
    return;
  }

  serverListEl.innerHTML = state.servers
    .map((server) => {
      const isSelected = state.selectedServerId === server.clientId;
      return `
        <article class="server-card ${isSelected ? "server-card-active" : ""}">
          <div class="flex items-start justify-between gap-3">
            <div>
              <p class="text-base font-semibold text-white">${escapeHtml(server.roomName)}</p>
              <p class="mt-1 text-xs text-white/45">Host: ${escapeHtml(server.hostName)}</p>
              <p class="mt-1 break-all text-[11px] text-white/35">${escapeHtml(server.peerAddr ?? "n/a")}</p>
            </div>
            <div class="text-right">
              <p class="text-xs uppercase tracking-[0.18em] text-white/45">${escapeHtml(server.slots)}</p>
              <p class="mt-2 text-xl">${server.hasPassword ? "🔒" : "⚔"}</p>
            </div>
          </div>
          <div class="mt-4 flex gap-3">
            <button class="ghost-button flex-1" data-select-server="${escapeHtml(server.clientId)}">Select</button>
            <button class="primary-button flex-1" data-connect-server="${escapeHtml(server.clientId)}">Connect</button>
          </div>
        </article>
      `;
    })
    .join("");
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
    .filter((member) => member.clientId !== localClientId)
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
    selectedServerEl.textContent = "No selection";
  }

  renderServers();
}

async function refreshLobby() {
  if (!state.lobbyChannel) {
    return;
  }
  const members = await state.lobbyChannel.presence.get();
  hydrateServers(members);
  addLog(`Lobby refresh: ${members.length} presence members.`);
}

async function setupAbly() {
  const realtime = new Ably.Realtime({
    key: ABLY_API_KEY,
    clientId: localClientId,
  });
  state.realtime = realtime;

  realtime.connection.on((change) => {
    ablyStateEl.textContent = change.current;
    addLog(`Ably connection: ${change.previous ?? "none"} -> ${change.current}`);
  });

  await new Promise((resolve) => realtime.connection.once("connected", resolve));
  state.lobbyChannel = realtime.channels.get(LOBBY_CHANNEL_NAME);
  state.privateChannel = realtime.channels.get(`lobby:${localClientId}`);

  await state.lobbyChannel.presence.subscribe("enter", refreshLobby);
  await state.lobbyChannel.presence.subscribe("update", refreshLobby);
  await state.lobbyChannel.presence.subscribe("leave", refreshLobby);

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

  await refreshLobby();
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
    await waitForStatus((status) => Boolean(status.publicUdpAddr));
    const status = await invoke("get_status");
    renderStatus(status);

    hostSession.active = true;
    hostSession.roomName = roomName;
    hostSession.hasPassword = Boolean(password);
    hostSession.peerAddr = status.publicUdpAddr ?? status.udpBindAddr;

    await state.lobbyChannel.presence.enter({
      room_name: roomName,
      host_name: localClientId,
      slots: DEFAULT_SLOTS,
      has_password: Boolean(password),
      peer_addr: hostSession.peerAddr,
    });

    addLog(`Presence entered for room "${roomName}" (${hostSession.peerAddr}).`);
    await refreshLobby();
  } catch (error) {
    addLog(`Host start failed: ${String(error)}`);
  } finally {
    hostButtonEl.disabled = false;
  }
}

async function stopHosting() {
  stopButtonEl.disabled = true;
  try {
    await invoke("stop_hosting");
    if (hostSession.active) {
      await state.lobbyChannel.presence.leave();
      addLog("Presence left. Host session stopped.");
    }
    hostSession.active = false;
    hostSession.roomName = "";
    hostSession.hasPassword = false;
    hostSession.peerAddr = null;
    await pollStatus();
    await refreshLobby();
  } catch (error) {
    addLog(`Stop failed: ${String(error)}`);
  } finally {
    stopButtonEl.disabled = false;
  }
}

async function connectToServer(server) {
  state.selectedServerId = server.clientId;
  selectedServerEl.textContent = server.roomName;
  renderServers();

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

    if (hostSession.active && hostSession.peerAddr && state.lobbyChannel) {
      await state.lobbyChannel.presence.update({
        room_name: hostSession.roomName,
        host_name: localClientId,
        slots: `${Math.max(1, (status.peerCount ?? 0) + 1)}/30`,
        has_password: hostSession.hasPassword,
        peer_addr: hostSession.peerAddr,
      });
    }
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
refreshLobbyEl.addEventListener("click", refreshLobby);

serverListEl.addEventListener("click", async (event) => {
  const selectId = event.target.closest("[data-select-server]")?.dataset.selectServer;
  if (selectId) {
    state.selectedServerId = selectId;
    const selected = state.servers.find((server) => server.clientId === selectId);
    selectedServerEl.textContent = selected?.roomName ?? "No selection";
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
}, 1200);

await setupAbly();
await pollStatus();
