import { invoke } from "@tauri-apps/api/core";

const views = Object.fromEntries(
  [...document.querySelectorAll("[data-view]")].map((element) => [element.dataset.view, element]),
);

const roomCodeEl = document.querySelector("#room-code");
const peerCountEl = document.querySelector("#peer-count");
const peerListEl = document.querySelector("#peer-list");
const hostNoteEl = document.querySelector("#host-note");
const hostEndpointEl = document.querySelector("#host-endpoint");
const connectStageEl = document.querySelector("#connect-stage");
const connectNoteEl = document.querySelector("#connect-note");
const connectEndpointEl = document.querySelector("#connect-endpoint");
const statusChipEl = document.querySelector("#status-chip");
const signalingServerEl = document.querySelector("#signaling-server");
const errorTextEl = document.querySelector("#error-text");
const roomCodeInputEl = document.querySelector("#room-code-input");

const hostButton = document.querySelector("#host-button");
const connectButton = document.querySelector("#connect-button");
const copyRoomCodeButton = document.querySelector("#copy-room-code");
const connectSubmitButton = document.querySelector("#connect-submit");

let activeView = "main";
let statusPollHandle = null;

const stageLabel = {
  idle: "Idle",
  starting: "Starting",
  waitingForPeer: "Waiting For Peer",
  punching: "Punching NAT",
  connecting: "Connecting",
  hosting: "Hosting",
  connected: "Connected",
  error: "Error",
};

function showView(name) {
  activeView = name;
  Object.entries(views).forEach(([viewName, element]) => {
    element.classList.toggle("hidden", viewName !== name);
  });
}

function formatStage(state) {
  return stageLabel[state] ?? state ?? "Idle";
}

function peerMarkup(peer) {
  const latency = peer.pingMs == null ? "n/a" : `${peer.pingMs} ms`;
  const state = peer.connected ? "Online" : "Waiting";

  return `
    <div class="rounded-2xl border border-white/8 bg-white/4 px-4 py-4">
      <div class="flex flex-wrap items-start justify-between gap-3">
        <div>
          <p class="text-sm font-semibold text-white">${peer.peerId}</p>
          <p class="mt-1 text-sm text-white/45">${peer.addr}</p>
        </div>
        <div class="text-right">
          <p class="text-sm font-medium ${peer.connected ? "text-cyan-300" : "text-white/55"}">${state}</p>
          <p class="mt-1 text-sm text-amber-300/90">${latency}</p>
        </div>
      </div>
    </div>
  `;
}

function renderPeers(peers) {
  if (!peers.length) {
    peerListEl.innerHTML = `
      <div class="rounded-2xl border border-dashed border-white/12 px-4 py-8 text-center text-sm text-white/45">
        Пока никого нет. Room code уже можно отдавать другу.
      </div>
    `;
    return;
  }

  peerListEl.innerHTML = peers.map(peerMarkup).join("");
}

function renderStatus(status) {
  statusChipEl.textContent = formatStage(status.state);
  signalingServerEl.textContent = status.signalingServer ?? "n/a";
  errorTextEl.textContent = status.lastError ?? "None";

  roomCodeEl.textContent = status.roomCode ?? "------";
  peerCountEl.textContent = String(status.peerCount ?? 0);
  hostNoteEl.textContent = status.note ?? "Ожидание peer'ов.";
  hostEndpointEl.textContent = `UDP: ${status.publicUdpAddr ?? status.udpBindAddr ?? "n/a"}`;

  connectStageEl.textContent = formatStage(status.state);
  connectNoteEl.textContent = status.note ?? "Приложение ждёт room code.";
  connectEndpointEl.textContent = `UDP: ${status.publicUdpAddr ?? status.udpBindAddr ?? "n/a"}`;

  renderPeers(status.peers ?? []);

  if (status.mode === "host") {
    showView("host");
  } else if (status.mode === "client") {
    showView("connect");
  } else if (activeView !== "connect") {
    showView("main");
  }
}

async function pollStatus() {
  try {
    const status = await invoke("get_status");
    renderStatus(status);
  } catch (error) {
    errorTextEl.textContent = String(error);
  }
}

async function startHosting() {
  hostButton.disabled = true;
  try {
    showView("host");
    const roomCode = await invoke("start_hosting");
    roomCodeEl.textContent = roomCode;
    await pollStatus();
  } catch (error) {
    errorTextEl.textContent = String(error);
  } finally {
    hostButton.disabled = false;
  }
}

async function connectToHost() {
  const roomCode = roomCodeInputEl.value.trim().toUpperCase();
  if (!roomCode) {
    roomCodeInputEl.focus();
    return;
  }

  connectSubmitButton.disabled = true;
  try {
    showView("connect");
    await invoke("connect_to_host", { roomCode });
    await pollStatus();
  } catch (error) {
    errorTextEl.textContent = String(error);
  } finally {
    connectSubmitButton.disabled = false;
  }
}

hostButton.addEventListener("click", startHosting);
connectButton.addEventListener("click", () => showView("connect"));
connectSubmitButton.addEventListener("click", connectToHost);
copyRoomCodeButton.addEventListener("click", async () => {
  const value = roomCodeEl.textContent?.trim();
  if (value && value !== "------") {
    await navigator.clipboard.writeText(value);
  }
});

roomCodeInputEl.addEventListener("keydown", (event) => {
  if (event.key === "Enter") {
    connectToHost();
  }
});

document.querySelectorAll("[data-back]").forEach((button) => {
  button.addEventListener("click", () => showView("main"));
});

statusPollHandle = window.setInterval(pollStatus, 1000);
window.addEventListener("beforeunload", () => {
  if (statusPollHandle != null) {
    window.clearInterval(statusPollHandle);
  }
});

pollStatus();

