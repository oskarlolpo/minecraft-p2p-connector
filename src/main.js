import * as Ably from "ably";
import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";

import { I18N } from "./i18n.js";

const ABLY_API_KEY = "aGkPAA.1VHkjw:Bai-67g05FcqHdfVOMiSfjYlK3aLz8wOzj5WeTgz4cw";
const LOBBY_CHANNEL_NAME = "minecraft-lobby";
const POLL_INTERVAL_MS = 1500;
const SAFE_RELEASE_STATES = new Set(["initialized", "detached", "failed"]);
const SAFE_SKIP_STATES = new Set(["detached", "failed", "suspended"]);
const SETTINGS_THEME_KEY = "minecraft-p2p-theme";
const SETTINGS_LANGUAGE_KEY = "minecraft-p2p-language";

const modalEl = document.querySelector("#host-modal");
const openHostModalEl = document.querySelector("#open-host-modal");
const closeModalEl = document.querySelector("#close-modal");
const closeModalSecondaryEl = document.querySelector("#close-modal-secondary");
const requirePasswordEl = document.querySelector("#require-password");
const passwordFieldGroupEl = document.querySelector("#password-field-group");
const roomNameEl = document.querySelector("#room-name");
const roomPasswordEl = document.querySelector("#room-password");
const localGamePortEl = document.querySelector("#local-game-port");
const autoDetectPortEl = document.querySelector("#auto-detect-port");
const enableGeyserEl = document.querySelector("#enable-geyser");
const geyserPortFieldEl = document.querySelector("#geyser-port-field");
const geyserPortEl = document.querySelector("#geyser-port");
const enableE4mcEl = document.querySelector("#enable-e4mc");
const hostButtonEl = document.querySelector("#host-button");
const stopButtonEl = document.querySelector("#stop-button");
const refreshLobbyEl = document.querySelector("#refresh-lobby");
const copyLogsEl = document.querySelector("#copy-logs");
const copyDiagnosticsEl = document.querySelector("#copy-diagnostics");
const copySelectedEndpointEl = document.querySelector("#copy-selected-endpoint");
const copySelectedBedrockEndpointEl = document.querySelector("#copy-selected-bedrock-endpoint");
const connectSelectedEl = document.querySelector("#connect-selected");
const runPreflightEl = document.querySelector("#run-preflight");
const startTestServerEl = document.querySelector("#start-test-server");
const stopTestServerEl = document.querySelector("#stop-test-server");
const probeTestServerEl = document.querySelector("#probe-test-server");
const testServerPortEl = document.querySelector("#test-server-port");
const serverListEl = document.querySelector("#server-list");
const logsEl = document.querySelector("#logs");
const peerListEl = document.querySelector("#peer-list");
const connectionStateEl = document.querySelector("#connection-state");
const ablyStateEl = document.querySelector("#ably-state");
const lobbyCountEl = document.querySelector("#lobby-count");
const publicEndpointEl = document.querySelector("#public-endpoint");
const selectedServerEl = document.querySelector("#selected-server");
const selectedEndpointEl = document.querySelector("#selected-endpoint");
const selectedBedrockEndpointEl = document.querySelector("#selected-bedrock-endpoint");
const selectedMetaEl = document.querySelector("#selected-meta");
const statusNoteEl = document.querySelector("#status-note");
const peerCountEl = document.querySelector("#peer-count");
const minecraftTargetHintEl = document.querySelector("#minecraft-target-hint");
const activeHostCardEl = document.querySelector("#active-host-card");
const currentVersionEl = document.querySelector("#current-version");
const sessionModeEl = document.querySelector("#session-mode");
const hostLockNoteEl = document.querySelector("#host-lock-note");
const hostSectionTitleEl = document.querySelector("#host-section-title");
const navHomeEl = document.querySelector("#nav-home");
const navSettingsEl = document.querySelector("#nav-settings");
const pageHomeEl = document.querySelector("#page-home");
const pageSettingsEl = document.querySelector("#page-settings");
const portHelpEl = document.querySelector("#port-help");
const brandUserNameEl = document.querySelector("#brand-user-name");
const brandAvatarImageEl = document.querySelector("#brand-avatar-image");
const brandAvatarFallbackEl = document.querySelector("#brand-avatar-fallback");
const profileMenuTriggerEl = document.querySelector("#profile-menu-trigger");
const profileMenuEl = document.querySelector("#profile-menu");
const profileNicknameEl = document.querySelector("#profile-nickname");
const profileAvatarFileEl = document.querySelector("#profile-avatar-file");
const chooseAvatarEl = document.querySelector("#choose-avatar");
const saveProfileEl = document.querySelector("#save-profile");
const settingsVersionEl = document.querySelector("#settings-version");
const checkUpdatesEl = document.querySelector("#check-updates");
const installUpdateEl = document.querySelector("#install-update");
const updateStatusEl = document.querySelector("#update-status");
const externalHostAddressFieldEl = document.querySelector("#external-host-address-field");
const externalHostAddressEl = document.querySelector("#external-host-address");

const portChoiceModalEl = document.querySelector("#port-choice-modal");
const closePortModalEl = document.querySelector("#close-port-modal");
const closePortModalSecondaryEl = document.querySelector("#close-port-modal-secondary");
const detectedPortsListEl = document.querySelector("#detected-ports-list");
const clearIgnoredPortsEl = document.querySelector("#clear-ignored-ports");
const ignoredPortsListEl = document.querySelector("#ignored-ports-list");

const PROFILE_STORAGE_KEY = "minecraft-p2p-profile-v1";
const EXTERNAL_SERVERS_STORAGE_KEY = "minecraft-p2p-external-servers-v1";
const E4MC_FALLBACK_STORAGE_KEY = "minecraft-p2p-enable-e4mc";
const IGNORED_PORTS_STORAGE_KEY = "minecraft-p2p-ignored-ports-v1";
const CP1251_EXTRA_ENCODE_MAP = {
  "Ђ": 0x80, "Ѓ": 0x81, "‚": 0x82, "ѓ": 0x83, "„": 0x84, "…": 0x85, "†": 0x86, "‡": 0x87,
  "€": 0x88, "‰": 0x89, "Љ": 0x8a, "‹": 0x8b, "Њ": 0x8c, "Ќ": 0x8d, "Ћ": 0x8e, "Џ": 0x8f,
  "ђ": 0x90, "‘": 0x91, "’": 0x92, "“": 0x93, "”": 0x94, "•": 0x95, "–": 0x96, "—": 0x97,
  "™": 0x99, "љ": 0x9a, "›": 0x9b, "њ": 0x9c, "ќ": 0x9d, "ћ": 0x9e, "џ": 0x9f,
  "Ў": 0xa1, "ў": 0xa2, "Ј": 0xa3, "¤": 0xa4, "Ґ": 0xa5, "¦": 0xa6, "§": 0xa7,
  "Ё": 0xa8, "©": 0xa9, "Є": 0xaa, "«": 0xab, "¬": 0xac, "­": 0xad, "®": 0xae, "Ї": 0xaf,
  "°": 0xb0, "±": 0xb1, "І": 0xb2, "і": 0xb3, "ґ": 0xb4, "µ": 0xb5, "¶": 0xb6, "·": 0xb7,
  "ё": 0xb8, "№": 0xb9, "є": 0xba, "»": 0xbb, "ј": 0xbc, "Ѕ": 0xbd, "ѕ": 0xbe, "ї": 0xbf,
};

const hostSession = {
  active: false,
  roomName: "",
  hasPassword: false,
  peerId: null,
  listenAddrs: [],
  peerAddr: null,
  localPort: 25565,
  maxPlayers: 30,
  minecraftVersion: null,
  publicJoinAddress: null,
  e4mcDomain: null,
  e4mcVerified: false,
  presencePayload: null,
  presenceEntered: false,
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
  pendingConnects: new Set(),
  pendingKicks: new Set(),
  tunnelReady: false,
  activeTunnelTransport: null,
  lastPreflight: null,
  testServerInfo: null,
  page: "home",
  preferences: loadPreferences(),
  profile: loadProfile(),
  externalServers: loadExternalServers(),
  peerProfiles: new Map(),
  updateInfo: null,
  detectedMinecraftNickname: null,
  runtimeFingerprint: null,
  localWorldPlayers: [],
  lastLocalPlayerSyncAt: 0,
  e4mcEnabled: loadE4mcPreference(),
  ignoredPorts: loadIgnoredPorts(),
};

function loadE4mcPreference() {
  const value = localStorage.getItem(E4MC_FALLBACK_STORAGE_KEY);
  return value == null ? true : value === "true";
}

function saveE4mcPreference(value) {
  localStorage.setItem(E4MC_FALLBACK_STORAGE_KEY, String(Boolean(value)));
}

function ensureClientId() {
  const key = "minecraft-p2p-client-id";
  const existing = localStorage.getItem(key);
  if (existing) return existing;
  const created = `mc-${crypto.randomUUID().slice(0, 8)}`;
  localStorage.setItem(key, created);
  return created;
}

function loadProfile() {
  try {
    const raw = localStorage.getItem(PROFILE_STORAGE_KEY);
    const parsed = raw ? JSON.parse(raw) : {};
    const nickname = String(parsed?.nickname || "Player").trim() || "Player";
    const avatarDataUrl = typeof parsed?.avatarDataUrl === "string" ? parsed.avatarDataUrl : null;
    return { nickname, avatarDataUrl };
  } catch {
    return { nickname: "Player", avatarDataUrl: null };
  }
}

function saveProfileState() {
  localStorage.setItem(PROFILE_STORAGE_KEY, JSON.stringify(state.profile));
}

function loadExternalServers() {
  try {
    const raw = localStorage.getItem(EXTERNAL_SERVERS_STORAGE_KEY);
    const parsed = raw ? JSON.parse(raw) : [];
    return Array.isArray(parsed) ? parsed : [];
  } catch {
    return [];
  }
}

function saveExternalServers() {
  localStorage.setItem(EXTERNAL_SERVERS_STORAGE_KEY, JSON.stringify(state.externalServers));
}

function loadPreferences() {
  return {
    theme: localStorage.getItem(SETTINGS_THEME_KEY) || "oled",
    language: localStorage.getItem(SETTINGS_LANGUAGE_KEY) || "ru",
  };
}

function savePreference(key, value) {
  localStorage.setItem(key, value);
}

function loadIgnoredPorts() {
  try {
    const raw = localStorage.getItem(IGNORED_PORTS_STORAGE_KEY);
    const parsed = raw ? JSON.parse(raw) : [];
    return Array.isArray(parsed) ? parsed : [];
  } catch {
    return [];
  }
}

function saveIgnoredPorts() {
  localStorage.setItem(IGNORED_PORTS_STORAGE_KEY, JSON.stringify(state.ignoredPorts));
}

function t(key, variables = {}) {
  const dictionary = I18N[state.preferences.language] ?? I18N.ru;
  const template = dictionary[key] ?? I18N.ru[key] ?? key;
  return template.replaceAll(/\{(\w+)\}/g, (_, name) => String(variables[name] ?? `{${name}}`));
}

function toCp1251Byte(char) {
  const code = char.charCodeAt(0);
  if (code <= 0x7f) return code;
  if (code >= 0x0410 && code <= 0x044f) return code - 0x350;
  if (code === 0x0401) return 0xa8;
  if (code === 0x0451) return 0xb8;
  return CP1251_EXTRA_ENCODE_MAP[char];
}

function decodeMojibakeIfNeeded(value) {
  const text = String(value ?? "");
  if (!text) return text;
  if (!/(?:[РС][\u0400-\u045f]){3,}/.test(text)) return text;

  const bytes = [];
  for (const char of text) {
    const byte = toCp1251Byte(char);
    if (byte == null) {
      return text;
    }
    bytes.push(byte);
  }

  try {
    const decoded = new TextDecoder("utf-8", { fatal: false }).decode(Uint8Array.from(bytes));
    return /[А-Яа-яЁё]/.test(decoded) ? decoded : text;
  } catch {
    return text;
  }
}

function escapeHtml(value) {
  return String(value)
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll('"', "&quot;")
    .replaceAll("'", "&#039;");
}

function applyTranslations() {
  document.title = t("appTitle");
  document.querySelectorAll("[data-i18n]").forEach((element) => {
    const key = element.dataset.i18n;
    if (key) element.textContent = t(key);
  });
  document.querySelectorAll("[data-i18n-placeholder]").forEach((element) => {
    const key = element.dataset.i18nPlaceholder;
    if (key) element.placeholder = t(key);
  });
  navHomeEl.setAttribute("aria-label", t("homeAria"));
  navSettingsEl.setAttribute("aria-label", t("settingsAria"));
  closeModalEl.setAttribute("aria-label", t("closeAria"));
  profileMenuTriggerEl?.setAttribute("aria-label", t("avatarButtonAria"));
  portHelpEl.title = t("portHelpTitle");
  document.documentElement.lang = state.preferences.language;
  syncExternalHostMode();
  renderProfile();
}

function syncGeyserField() {
  geyserPortFieldEl.classList.toggle("hidden", !enableGeyserEl.checked);
}

function syncExternalHostMode() {
  const external = Boolean(externalHostModeEl?.checked);
  externalHostAddressFieldEl?.classList.toggle("hidden", !external);
  localGamePortEl?.closest(".field")?.classList.toggle("hidden", external);
  requirePasswordEl?.closest(".checkbox-row")?.classList.toggle("hidden", external);
  passwordFieldGroupEl?.classList.toggle("hidden", external || !requirePasswordEl.checked);
  enableGeyserEl?.closest(".checkbox-row")?.classList.toggle("hidden", external);
  geyserPortFieldEl?.classList.toggle("hidden", external || !enableGeyserEl.checked);
  enableE4mcEl?.closest(".checkbox-row")?.classList.toggle("hidden", external);
  hostButtonEl.textContent = t(external ? "modalExternalButton" : "modalHostButton");
}

function renderProfile() {
  const nickname = state.profile.nickname?.trim() || "Player";
  brandUserNameEl.textContent = nickname;
  profileNicknameEl.value = nickname;

  if (state.profile.avatarDataUrl) {
    brandAvatarImageEl.src = state.profile.avatarDataUrl;
    brandAvatarImageEl.classList.remove("hidden");
    brandAvatarFallbackEl.classList.add("hidden");
  } else {
    brandAvatarImageEl.removeAttribute("src");
    brandAvatarImageEl.classList.add("hidden");
    brandAvatarFallbackEl.classList.remove("hidden");
    brandAvatarFallbackEl.textContent = nickname.slice(0, 1).toUpperCase();
  }
}

async function detectMinecraftNickname() {
  try {
    const detection = await invoke("detect_minecraft_nickname_command");
    if (!detection?.nickname) return;
    state.detectedMinecraftNickname = detection.nickname;
    renderProfile();
    addLog(t("autoNicknameDetected", { nickname: detection.nickname }));
  } catch {
    state.detectedMinecraftNickname = null;
    renderProfile();
  }
}

async function detectRuntimeFingerprint() {
  try {
    const runtime = await invoke("detect_client_runtime_info_command");
    state.runtimeFingerprint = runtime ?? null;
    if (runtime?.launcher || runtime?.minecraftVersion || runtime?.modLoader) {
      addLog(
        `Runtime fingerprint: launcher=${runtime?.launcher ?? "unknown"}, version=${runtime?.minecraftVersion ?? "unknown"}, modloader=${runtime?.modLoader ?? "vanilla"}`,
      );
    } else {
      addLog("Runtime fingerprint incomplete: launcher/version/modloader were not detected.");
    }
  } catch (error) {
    state.runtimeFingerprint = null;
    addLog(`Runtime fingerprint not found: ${String(error)}`);
  }
}

async function autoDetectLocalGamePort() {
  try {
    autoDetectPortEl.disabled = true;
    const ports = await invoke("get_available_lan_ports_command", { ignoredPorts: state.ignoredPorts });
    
    if (!ports || ports.length === 0) {
      addLog(t("autoPortFailed", { error: "No ports found" }));
      return;
    }

    if (ports.length === 1) {
      const detection = ports[0];
      localGamePortEl.value = String(detection.port);
      addLog(t("autoPortDetected", { port: detection.port, path: detection.sourcePath }));
      await autofillRoomNameFromLocalServer();
    } else {
      openPortChoiceModal(ports);
    }
  } catch (error) {
    addLog(t("autoPortFailed", { error: String(error) }));
  } finally {
    autoDetectPortEl.disabled = false;
  }
}

function openPortChoiceModal(ports) {
  detectedPortsListEl.innerHTML = "";
  ports.forEach((det) => {
    const item = document.createElement("div");
    item.className = "port-item";
    item.innerHTML = `
      <div class="port-item-info">
        <strong>Port ${det.port}</strong>
        <span title="${det.source_path}">${t("portChoiceSource", { source: det.source_path })}</span>
      </div>
      <button class="primary-button mini-button" type="button">${t("portChoiceSelect")}</button>
      <button class="ghost-button mini-button danger-button" type="button" title="${t("portChoiceIgnore")}">${t("portChoiceIgnore")}</button>
    `;
    
    // Select button
    item.querySelector(".primary-button").onclick = () => {
      localGamePortEl.value = String(det.port);
      addLog(t("autoPortDetected", { port: det.port, path: det.source_path }));
      autofillRoomNameFromLocalServer();
      closePortChoiceModal();
    };

    // Ignore button
    item.querySelector(".danger-button").onclick = () => {
      if (!state.ignoredPorts.includes(det.port)) {
        state.ignoredPorts.push(det.port);
        saveIgnoredPorts();
        renderIgnoredPorts();
      }
      // Re-detect or just remove from this list? Let's re-detect to handle remaining items
      autoDetectLocalGamePort();
      closePortChoiceModal();
    };

    detectedPortsListEl.appendChild(item);
  });

  portChoiceModalEl.classList.remove("hidden");
  portChoiceModalEl.setAttribute("aria-hidden", "false");
}

function closePortChoiceModal() {
  portChoiceModalEl.classList.add("hidden");
  portChoiceModalEl.setAttribute("aria-hidden", "true");
}

function renderIgnoredPorts() {
  if (!ignoredPortsListEl) return;
  if (state.ignoredPorts.length === 0) {
    ignoredPortsListEl.innerHTML = `<div class="empty-state">${t("noServers")}</div>`;
    return;
  }

  ignoredPortsListEl.innerHTML = state.ignoredPorts.map(port => `
    <div class="tag-item">
      <span>${port}</span>
      <button class="remove-tag" data-port="${port}" type="button">×</button>
    </div>
  `).join("");

  ignoredPortsListEl.querySelectorAll(".remove-tag").forEach(btn => {
    btn.onclick = () => {
      const port = Number(btn.dataset.port);
      state.ignoredPorts = state.ignoredPorts.filter(p => p !== port);
      saveIgnoredPorts();
      renderIgnoredPorts();
    };
  });
}

async function autofillRoomNameFromLocalServer() {
  const localPort = Number(localGamePortEl.value || 25565);
  if (!localPort) return;
  try {
    const probe = await invoke("query_external_server", { host: "127.0.0.1", port: localPort });
    const detectedName = String(probe?.roomName || "").trim();
    if (!detectedName) return;
    if (!roomNameEl.value.trim() || roomNameEl.dataset.autofilled === "true") {
      roomNameEl.value = detectedName;
      roomNameEl.dataset.autofilled = "true";
    }
  } catch {
    // Ignore: local world metadata is optional.
  }
}

function renderSettingsOptions() {
  document.querySelectorAll("[data-theme-value]").forEach((button) => {
    button.classList.toggle("active", button.dataset.themeValue === state.preferences.theme);
  });
  document.querySelectorAll("[data-language-value]").forEach((button) => {
    button.classList.toggle("active", button.dataset.languageValue === state.preferences.language);
  });
}

function applyTheme(theme) {
  state.preferences.theme = theme;
  document.body.dataset.theme = theme;
  savePreference(SETTINGS_THEME_KEY, theme);
  renderSettingsOptions();
}

function applyLanguage(language) {
  state.preferences.language = language;
  savePreference(SETTINGS_LANGUAGE_KEY, language);
  applyTranslations();
  rerender();
}

function setPage(page) {
  state.page = page;
  pageHomeEl.classList.toggle("page-active", page === "home");
  pageSettingsEl.classList.toggle("page-active", page === "settings");
  navHomeEl.classList.toggle("nav-button-active", page === "home");
  navSettingsEl.classList.toggle("nav-button-active", page === "settings");
}

async function loadAppInfo() {
  try {
    const info = await invoke("get_app_info");
    settingsVersionEl.textContent = info.version;
  } catch {
    settingsVersionEl.textContent = "unknown";
  }
}

async function checkForUpdates() {
  updateStatusEl.textContent = t("updatesChecking");
  installUpdateEl.classList.add("hidden");
  try {
    const info = await invoke("check_for_updates");
    state.updateInfo = info;
    if (info.available) {
      updateStatusEl.textContent = t("updatesAvailable", { version: info.latestVersion });
      installUpdateEl.classList.remove("hidden");
    } else {
      updateStatusEl.textContent = t("updatesNone");
    }
  } catch (error) {
    updateStatusEl.textContent = t("updatesError", { error: String(error) });
  }
}

async function installUpdate() {
  if (!state.updateInfo?.available) return;
  installUpdateEl.disabled = true;
  try {
    const result = await invoke("install_update");
    updateStatusEl.textContent = result.message ?? state.updateInfo.downloadUrl ?? "";
  } catch (error) {
    updateStatusEl.textContent = t("updatesError", { error: String(error) });
  } finally {
    installUpdateEl.disabled = false;
  }
}

function toggleProfileMenu(force) {
  ensureProfileMenuPortal();
  const open = typeof force === "boolean" ? force : profileMenuEl.classList.contains("hidden");
  profileMenuEl.classList.toggle("hidden", !open);
  if (open) requestAnimationFrame(positionProfileMenu);
}

function ensureProfileMenuPortal() {
  if (!profileMenuEl || profileMenuEl.parentElement === document.body) return;
  document.body.appendChild(profileMenuEl);
}

function positionProfileMenu() {
  if (profileMenuEl.classList.contains("hidden")) return;
  const triggerRect = profileMenuTriggerEl.getBoundingClientRect();
  const menuRect = profileMenuEl.getBoundingClientRect();
  const margin = 12;
  const spacing = 8;

  let left = triggerRect.left;
  let top = triggerRect.bottom + spacing;

  const maxLeft = window.innerWidth - menuRect.width - margin;
  left = Math.min(Math.max(margin, left), Math.max(margin, maxLeft));

  const maxTop = window.innerHeight - menuRect.height - margin;
  if (top > maxTop) {
    top = triggerRect.top - menuRect.height - spacing;
  }
  top = Math.min(Math.max(margin, top), Math.max(margin, maxTop));

  profileMenuEl.style.left = `${Math.round(left)}px`;
  profileMenuEl.style.top = `${Math.round(top)}px`;
}

async function pickAvatarFile() {
  profileAvatarFileEl.click();
}

async function handleAvatarChosen() {
  const file = profileAvatarFileEl.files?.[0];
  if (!file) return;
  const reader = new FileReader();
  reader.onload = () => {
    state.profile.avatarDataUrl = String(reader.result || "");
    renderProfile();
  };
  reader.readAsDataURL(file);
}

function saveProfileFromInputs() {
  state.profile.nickname = profileNicknameEl.value.trim() || "Player";
  saveProfileState();
  renderProfile();
  addLog(t("profileSaved"));
  toggleProfileMenu(false);
}

function addLog(message) {
  const normalizedMessage = decodeMojibakeIfNeeded(String(message ?? ""));
  const stamp = new Date().toLocaleTimeString(state.preferences.language === "ru" ? "ru-RU" : "en-US", {
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
  });
  state.logBuffer.unshift(`[${stamp}] ${normalizedMessage}`);
  state.logBuffer = state.logBuffer.slice(0, 120);
  renderLogs();
}

function currentLogLines() {
  const combined = [...state.logBuffer];
  if (state.status?.logs?.length) combined.push(...state.status.logs.map((line) => decodeMojibakeIfNeeded(line)));
  return [...new Set(combined)].slice(0, 120);
}

function renderLogs() {
  const lines = currentLogLines();
  logsEl.innerHTML = lines.length
    ? lines.map((entry) => `<div class="log-entry">${escapeHtml(entry)}</div>`).join("")
    : `<div class="empty-state">${escapeHtml(t("logEmpty"))}</div>`;
}

function setMinecraftHint(text, active = false) {
  minecraftTargetHintEl.textContent = text;
  minecraftTargetHintEl.classList.toggle("active", active);
}

function syncPasswordField() {
  passwordFieldGroupEl.classList.toggle("hidden", !requirePasswordEl.checked);
  if (!requirePasswordEl.checked) roomPasswordEl.value = "";
}

function openModal() {
  if (!canOpenHostModal()) return;
  if (enableE4mcEl) enableE4mcEl.checked = Boolean(state.e4mcEnabled);
  modalEl.classList.remove("hidden");
  modalEl.setAttribute("aria-hidden", "false");
  void autofillRoomNameFromLocalServer();
  setTimeout(() => roomNameEl.focus(), 30);
}

function closeModal() {
  modalEl.classList.add("hidden");
  modalEl.setAttribute("aria-hidden", "true");
}

function canOpenHostModal() {
  const mode = state.status?.mode ?? "idle";
  const currentState = state.status?.state ?? "idle";
  return mode !== "client" && mode !== "host" && !["starting", "connecting", "waitingForPeer", "punching"].includes(currentState);
}

function isClientLocked() {
  return state.status?.mode === "client" && !["idle", "error"].includes(state.status?.state ?? "idle");
}

function formatState(value) {
  const mapping = {
    idle: t("stateIdle"),
    starting: t("stateStarting"),
    waitingForPeer: t("stateWaitingForPeer"),
    punching: t("statePunching"),
    connecting: t("stateConnecting"),
    hosting: t("stateHosting"),
    connected: t("stateConnected"),
    error: t("stateError"),
  };
  return mapping[value] ?? value ?? t("stateIdle");
}

function formatMode(mode) {
  const mapping = { idle: t("modeIdle"), host: t("modeHost"), client: t("modeClient") };
  return mapping[mode] ?? t("modeSummaryUnknown");
}

function formatTransportLabel(transport) {
  if (transport === "relay-circuit" || transport === "relay-reservation") return "Circuit Relay v2";
  if (transport === "direct-hole-punch") return "DCUtR hole punch";
  if (transport === "direct" || transport === "direct-quic") return "Direct libp2p";
  if (transport === "e4mc-public") return "e4mc";
  return transport ?? "unknown transport";
}

function buildPeerSummaryLines(peer, profile = null) {
  return [
    `${t("profileNicknameLabel")}: ${profile?.nickname || peer.peerId || "n/a"}`,
    `${t("profileMinecraftNicknameLabel")}: ${profile?.minecraftNickname || "n/a"}`,
    `Launcher: ${profile?.launcher || "unknown"}`,
    `Version: ${profile?.minecraftVersion || "unknown"}`,
    `Mod loader: ${profile?.modLoader || "unknown"}`,
    `Address: ${peer.addr || "n/a"}`,
    `Transport: ${formatTransportLabel(peer.transport)}`,
    `Ping: ${peer.pingMs == null ? "n/a" : `${peer.pingMs} ms`}`,
  ].join("\n");
}

function normalizeIdentity(value) {
  return String(value ?? "").trim().toLowerCase();
}

function collectIdentityVariants(value) {
  const normalized = normalizeIdentity(value);
  if (!normalized) return [];
  const variants = new Set([normalized]);
  const withoutTrailingDigits = normalized.replace(/\d+$/u, "");
  if (withoutTrailingDigits.length >= 3 && withoutTrailingDigits !== normalized) {
    variants.add(withoutTrailingDigits);
  }
  return [...variants];
}

function getOwnIdentityNames() {
  const identities = new Set();
  [state.detectedMinecraftNickname, state.runtimeFingerprint?.nickname, state.profile.nickname].forEach((value) => {
    collectIdentityVariants(value).forEach((variant) => identities.add(variant));
  });
  return identities;
}

function getTrackedMinecraftNames(peers) {
  const names = new Set();
  getOwnIdentityNames().forEach((value) => names.add(value));
  peers.forEach((peer) => {
    const profile = state.peerProfiles.get(peer.peerId) ?? null;
    [profile?.minecraftNickname, profile?.nickname, peer.peerId].filter(Boolean).forEach((value) => {
      collectIdentityVariants(value).forEach((variant) => names.add(variant));
    });
  });
  return names;
}

function buildInferredPlayers(peers) {
  if (state.status?.mode !== "host") return [];
  const trackedNames = getTrackedMinecraftNames(peers);
  return (state.localWorldPlayers || [])
    .filter((name) => !trackedNames.has(normalizeIdentity(name)))
    .map((name) => ({
      peerId: `minecraft:${name}`,
      addr: "minecraft-status",
      connected: true,
      pingMs: null,
      transport: state.status?.e4mcDomain ? "e4mc-public" : "unknown transport",
      inferred: true,
      inferredName: name,
    }));
}

async function refreshLocalWorldPlayers(force = false, statusSnapshot = state.status) {
  const now = Date.now();
  if (!force && now - state.lastLocalPlayerSyncAt < 5000) return;
  if (statusSnapshot?.mode !== "host" || !statusSnapshot?.localGamePort) {
    state.localWorldPlayers = [];
    state.lastLocalPlayerSyncAt = now;
    return;
  }
  try {
    const snapshot = await invoke("get_local_player_snapshot_command", { port: Number(statusSnapshot.localGamePort) });
    state.localWorldPlayers = Array.isArray(snapshot?.sampleNames) ? snapshot.sampleNames : [];
    state.lastLocalPlayerSyncAt = now;
  } catch (error) {
    state.lastLocalPlayerSyncAt = now;
    addLog(`Local player snapshot unavailable: ${String(error)}`);
  }
}

function getSelectedServer() {
  return state.servers.find((server) => server.clientId === state.selectedServerId) ?? null;
}

function isLikelyPublicEndpoint(value) {
  if (!value) return false;
  const normalized = String(value).trim().toLowerCase();
  if (!normalized) return false;
  if (normalized.includes("/p2p-circuit")) return true;
  if (normalized.includes("bore.pub")) return true;
  if (normalized.includes("/dns4/") || normalized.includes("/dns6/")) return true;
  if (normalized.includes("/ip4/127.") || normalized.includes("/ip4/10.") || normalized.includes("/ip4/192.168.")) {
    return false;
  }
  if (normalized.includes("/ip4/172.")) {
    return !/\/ip4\/172\.(1[6-9]|2\d|3[0-1])\./.test(normalized);
  }
  if (normalized.includes("localhost") || normalized.includes("127.0.0.1")) return false;
  if (/^10\./.test(normalized) || /^192\.168\./.test(normalized)) return false;
  if (/^172\.(1[6-9]|2\d|3[0-1])\./.test(normalized)) return false;
  return true;
}

function sortAdvertisedAddrs(addrs) {
  return [...addrs].sort((left, right) => Number(isLikelyPublicEndpoint(right)) - Number(isLikelyPublicEndpoint(left)));
}

function normalizeToMultiaddr(value) {
  if (!value) return null;
  const trimmed = String(value).trim();
  if (!trimmed) return null;
  if (trimmed.startsWith("/")) return trimmed;

  const separator = trimmed.lastIndexOf(":");
  if (separator <= 0 || separator === trimmed.length - 1) return trimmed;

  const host = trimmed.slice(0, separator).replace(/^\[|\]$/g, "");
  const port = trimmed.slice(separator + 1);
  if (!/^\d+$/.test(port)) return trimmed;

  if (/^\d{1,3}(\.\d{1,3}){3}$/.test(host)) {
    return `/ip4/${host}/tcp/${port}`;
  }
  if (host.includes(":")) {
    return `/ip6/${host}/tcp/${port}`;
  }
  return `/dns4/${host}/tcp/${port}`;
}

function collectAdvertisedAddrs(bootstrap, status) {
  const values = [
    ...(bootstrap?.listenAddrs ?? []),
    status?.publicUdpAddr ?? null,
    status?.udpBindAddr ?? null,
  ]
    .map(normalizeToMultiaddr)
    .filter(Boolean);
  return sortAdvertisedAddrs([...new Set(values)]);
}

function advertisedEndpoint(addrs, explicitEndpoint = null) {
  return normalizeToMultiaddr(explicitEndpoint) ?? addrs.find((addr) => isLikelyPublicEndpoint(addr)) ?? addrs[0] ?? null;
}

function toSocketEndpoint(value) {
  if (!value) return null;
  const trimmed = String(value).trim();
  if (!trimmed) return null;
  if (!trimmed.startsWith("/")) return trimmed;
  const normalized = normalizeToMultiaddr(trimmed);
  if (!normalized?.startsWith("/")) return normalized;
  const parts = normalized.split("/");
  if (parts.length >= 5 && parts[1] === "ip4" && parts[3] === "tcp") {
    return `${parts[2]}:${parts[4]}`;
  }
  if (parts.length >= 5 && parts[1] === "ip6" && parts[3] === "tcp") {
    return `[${parts[2]}]:${parts[4]}`;
  }
  if (parts.length >= 5 && parts[1] === "dns4" && parts[3] === "tcp") {
    return `${parts[2]}:${parts[4]}`;
  }
  return trimmed;
}

function deriveBedrockEndpoint(endpoint, bedrockPort) {
  if (!endpoint || !bedrockPort) return null;
  const socket = toSocketEndpoint(endpoint);
  if (!socket) return null;
  const match = socket.match(/^\[([^\]]+)\]:(\d+)$/);
  if (match) {
    return `[${match[1]}]:${bedrockPort}`;
  }
  const separator = socket.lastIndexOf(":");
  if (separator <= 0) return null;
  return `${socket.slice(0, separator)}:${bedrockPort}`;
}

function canAdvertiseHost() {
  return Boolean(
    hostSession.active &&
      hostSession.peerId &&
      (getAdvertisableJoinAddress() || advertisedEndpoint(hostSession.listenAddrs, hostSession.peerAddr)),
  );
}

function isE4mcVerified(status = state.status) {
  return Boolean(status?.e4mcVerified ?? hostSession.e4mcVerified);
}

function getVisibleE4mcDomain(status = state.status) {
  return hostSession.e4mcDomain ?? status?.e4mcDomain ?? null;
}

function getVerifiedPublicJoinAddress(status = state.status) {
  if (!isE4mcVerified(status)) {
    return status?.publicJoinAddress ?? hostSession.publicJoinAddress ?? null;
  }
  return hostSession.publicJoinAddress ?? status?.publicJoinAddress ?? getVisibleE4mcDomain(status) ?? null;
}

function getDirectAdvertisedEndpoint(status = state.status) {
  const endpoint = advertisedEndpoint(hostSession.listenAddrs, hostSession.peerAddr);
  return toSocketEndpoint(endpoint) ?? endpoint ?? status?.publicUdpAddr ?? status?.udpBindAddr ?? null;
}

function getAdvertisableJoinAddress(status = state.status) {
  return getVerifiedPublicJoinAddress(status) ?? getDirectAdvertisedEndpoint(status);
}

function formatPrimaryEndpoint(status = state.status) {
  const endpoint = getAdvertisableJoinAddress(status);
  if (endpoint) return endpoint;
  const domain = getVisibleE4mcDomain(status);
  if (domain) return `${domain} (${t("e4mcPendingShort")})`;
  return status?.publicUdpAddr ?? status?.udpBindAddr ?? "n/a";
}

function renderSelectedServer() {
  const selected = getSelectedServer();
  const bedrockEndpoint =
    selected?.bedrockEndpoint ?? (selected ? deriveBedrockEndpoint(selected.peerAddr, selected.bedrockPort) : null);
  const javaEndpoint =
    selected?.publicJoinAddress ??
    selected?.joinAddress ??
    (selected?.peerAddr ? toSocketEndpoint(selected.peerAddr) ?? selected.peerAddr : null);
  selectedServerEl.textContent = selected ? selected.roomName : t("noSelection");
  selectedEndpointEl.textContent = javaEndpoint ?? "n/a";
  selectedBedrockEndpointEl.textContent = bedrockEndpoint ?? "n/a";
  selectedMetaEl.textContent = selected
    ? t("selectedMetaTemplate", {
        host: `${selected.hostName}${selected.clientId === localClientId ? ` (${t("selfHostLabel")})` : ""} · ${selected.peerId}`,
        version: selected.minecraftVersion ?? t("serverUnknownVersion"),
        slots: selected.slots,
        password: selected.hasPassword ? t("selectedMetaPassword") : "",
        bedrock:
          selected.geyserEnabled && bedrockEndpoint
            ? t("selectedMetaBedrock", { endpoint: bedrockEndpoint })
            : "",
      })
    : t("selectedMetaEmpty");
}

function syncButtons() {
  const mode = state.status?.mode ?? "idle";
  const currentState = state.status?.state ?? "idle";
  const busy = ["starting", "connecting", "punching", "waitingForPeer"].includes(currentState);
  const selected = getSelectedServer();
  const clientLocked = isClientLocked();

  openHostModalEl.disabled = busy || mode === "host" || clientLocked;
  openHostModalEl.classList.toggle("hidden", clientLocked);
  hostButtonEl.disabled = busy || mode === "host" || clientLocked;
  hostButtonEl.textContent = busy
    ? t("connectBusyButton")
    : externalHostModeEl?.checked
      ? t("modalExternalButton")
      : t("modalHostButton");
  stopButtonEl.disabled = mode === "idle";
  stopButtonEl.textContent = mode === "client" ? t("disconnectButton") : t("stopButton");
  stopButtonEl.classList.toggle("danger-active", mode !== "idle");
  refreshLobbyEl.disabled = state.realtime?.connection.state === "connecting";
  connectSelectedEl.disabled =
    !selected ||
    selected.clientId === localClientId ||
    busy ||
    clientLocked ||
    mode === "host" ||
    state.pendingConnects.has(selected.clientId);
  connectSelectedEl.textContent = state.pendingConnects.has(selected?.clientId ?? "")
    ? t("connectBusyButton")
    : t("connectButton");
  copySelectedEndpointEl.disabled = !(selected?.publicJoinAddress || selected?.joinAddress || selected?.peerAddr);
  copySelectedBedrockEndpointEl.disabled = !deriveBedrockEndpoint(selected?.peerAddr, selected?.bedrockPort);
  hostLockNoteEl.textContent = clientLocked
    ? t("hostNoteClientLocked")
    : mode === "host"
      ? t("hostNoteHosting")
      : t("hostNoteIdle");

  if (clientLocked) closeModal();
}

function renderSessionCard() {
  const status = state.status;
  const mode = status?.mode ?? "idle";
  const version = status?.minecraftVersion ?? hostSession.minecraftVersion ?? t("serverUnknownVersion");
  const publicJavaEndpoint = getAdvertisableJoinAddress(status);
  const hostBedrockEndpoint = deriveBedrockEndpoint(status?.publicUdpAddr, status?.bedrockPort);
  const online = Math.max(1, (status?.peerCount ?? 0) + 1);
  const maxPlayers = Math.max(online, hostSession.maxPlayers ?? 30);
  const e4mcDomain = getVisibleE4mcDomain(status);
  const e4mcLabel = e4mcDomain
    ? isE4mcVerified(status)
      ? t("hostCardE4mcReady", { domain: e4mcDomain })
      : t("hostCardE4mcPending", { domain: e4mcDomain })
    : null;

  if (mode === "client") {
    hostSectionTitleEl.textContent = t("clientSessionTitle");
    activeHostCardEl.className = "active-host-card";
    activeHostCardEl.innerHTML = `
    <div class="active-host-layout">
      <div class="host-avatar">⇄</div>
      <div class="host-details">
        <h3>${escapeHtml(t("clientCardTitle"))}</h3>
          <p>${escapeHtml(t("clientCardDescription", { peer: status?.peers?.[0]?.addr ?? "n/a" }))}</p>
          <div class="host-meta-row">
            <span class="host-meta-pill">${escapeHtml(t("clientCardReady"))}</span>
            <span class="host-meta-pill">${escapeHtml(status?.peers?.[0]?.pingMs == null ? "Ping: n/a" : `Ping: ${status.peers[0].pingMs} ms`)}</span>
          </div>
        </div>
      </div>
    `;
    return;
  }

  hostSectionTitleEl.textContent = t("activeHostTitle");
  if (!hostSession.active) {
    activeHostCardEl.className = "active-host-card empty";
    activeHostCardEl.innerHTML = `
      <div class="active-host-layout">
        <div class="host-avatar">MC</div>
        <div class="host-details">
          <h3>${escapeHtml(t("hostEmptyTitle"))}</h3>
          <p>${escapeHtml(t("hostEmptyDescription"))}</p>
        </div>
      </div>
    `;
    return;
  }

  activeHostCardEl.className = "active-host-card";
  activeHostCardEl.innerHTML = `
    <div class="active-host-layout">
      <div class="host-avatar">MC</div>
      <div class="host-details">
        <h3>${escapeHtml(hostSession.roomName)}</h3>
        <p>${escapeHtml(t("hostCardPlayers", { count: `${online}/${maxPlayers}` }))}</p>
        <div class="host-meta-row">
          <span class="host-meta-pill">${escapeHtml(t("hostCardVersion", { version }))}</span>
          <span class="host-meta-pill">${escapeHtml(t("hostCardPort", { port: hostSession.localPort }))}</span>
          <span class="host-meta-pill">${escapeHtml(hostSession.hasPassword ? t("hostCardPasswordOn") : t("hostCardPasswordOff"))}</span>
          ${
            e4mcLabel
              ? `<span class="host-meta-pill">${escapeHtml(e4mcLabel)}</span>`
              : ""
          }
          ${
            status?.geyserEnabled && hostBedrockEndpoint
              ? `<span class="host-meta-pill">${escapeHtml(t("hostCardGeyser", { endpoint: hostBedrockEndpoint }))}</span>`
              : ""
          }
        </div>
        <div class="active-host-tools">
          ${publicJavaEndpoint ? `<button class="ghost-button" type="button" data-copy-host-java="${escapeHtml(publicJavaEndpoint)}">${escapeHtml(t("copyIpButton"))}</button>` : ""}
          ${hostBedrockEndpoint ? `<button class="ghost-button" type="button" data-copy-host-bedrock="${escapeHtml(hostBedrockEndpoint)}">${escapeHtml(t("copyBedrockIpButton"))}</button>` : ""}
        </div>
      </div>
    </div>
  `;
}

function renderPeers(peers) {
  const hostMode = state.status?.mode === "host";
  const inferredPlayers = buildInferredPlayers(peers);
  const hostVirtualCount = hostMode && hostSession.active ? 1 : 0;
  peerCountEl.textContent = t("peerCount", { count: (peers?.length ?? 0) + inferredPlayers.length + hostVirtualCount });

  if (!peers?.length && !hostMode) {
    peerListEl.innerHTML = `<div class="empty-state">${escapeHtml(t("noPeers"))}</div>`;
    return;
  }

  const rows = [];
  if (hostMode && hostSession.active) {
    const nickname = state.profile.nickname?.trim() || "Player";
    const minecraftNick = state.detectedMinecraftNickname ? ` · ${state.detectedMinecraftNickname}` : "";
    const hostTransport = state.status?.e4mcDomain
      ? `${formatTransportLabel(state.status?.transportPath)} + e4mc`
      : formatTransportLabel(state.status?.transportPath);
    const hostSummary = [
      `${t("profileNicknameLabel")}: ${nickname}`,
      `${t("profileMinecraftNicknameLabel")}: ${state.detectedMinecraftNickname ?? "n/a"}`,
      `Launcher: ${state.runtimeFingerprint?.launcher ?? "unknown"}`,
      `Version: ${state.runtimeFingerprint?.minecraftVersion ?? state.status?.minecraftVersion ?? "unknown"}`,
      `Mod loader: ${state.runtimeFingerprint?.modLoader ?? "unknown"}`,
      `Address: ${hostSession.peerAddr ? toSocketEndpoint(hostSession.peerAddr) ?? hostSession.peerAddr : "n/a"}`,
      `Transport: ${hostTransport || "n/a"}`,
      `e4mc: ${state.status?.e4mcDomain ?? "n/a"}`,
    ].join("\n");
    rows.push(`
      <div class="player-row">
        <div class="player-main">
          <strong>${escapeHtml(nickname)} <span class="row-chip">${escapeHtml(t("hostBadge"))}</span> <span class="row-chip">${escapeHtml(hostTransport)}</span></strong>
          <span>${escapeHtml(`${t("profileMinecraftNicknameLabel")}: ${state.detectedMinecraftNickname ?? "n/a"}`)}</span>
          <span>${escapeHtml(hostSession.peerAddr ? toSocketEndpoint(hostSession.peerAddr) ?? hostSession.peerAddr : "n/a")}</span>
          <span>${escapeHtml(`online${minecraftNick}`)}</span>
        </div>
        <div class="player-actions">
          <button class="help-badge" type="button" title="${escapeHtml(hostSummary)}">?</button>
          <div class="row-action-button">${escapeHtml(t("playerPassiveAction"))}</div>
        </div>
      </div>
    `);
  }

  rows.push(
    ...[...peers, ...inferredPlayers].map((peer) => {
      const profile = state.peerProfiles.get(peer.peerId) ?? null;
      const canKick = hostMode && peer.connected && !peer.inferred;
      const label = state.pendingKicks.has(peer.peerId)
        ? t("kickPendingButton")
        : canKick
          ? t("kickButton")
          : t("playerPassiveAction");

      return `
        <div class="player-row">
          <div class="player-main">
          <strong>${escapeHtml(peer.inferredName || profile?.nickname || peer.peerId)} <span class="row-chip">${escapeHtml(peer.inferred ? "External" : formatTransportLabel(peer.transport))}</span></strong>
          ${(peer.inferredName || profile?.minecraftNickname) ? `<span>${escapeHtml(peer.inferredName || profile?.minecraftNickname)}</span>` : ""}
          <span>${escapeHtml(peer.addr)}</span>
          <span>${peer.connected ? "online" : "pending"} · ${peer.pingMs == null ? "n/a" : `${peer.pingMs} ms`}</span>
        </div>
        <div class="player-actions">
            <button class="help-badge" type="button" title="${escapeHtml(buildPeerSummaryLines(peer, profile))}">?</button>
            ${
              canKick
                ? `<button class="secondary-button row-action-button" data-kick-peer="${escapeHtml(peer.peerId)}" ${
                    state.pendingKicks.has(peer.peerId) ? "disabled" : ""
                  }>${escapeHtml(label)}</button>`
                : `<div class="row-action-button">${escapeHtml(label)}</div>`
            }
          </div>
        </div>
      `;
    }),
  );

  peerListEl.innerHTML = rows.join("");
}

function renderServers() {
  lobbyCountEl.textContent = t("lobbyCount", { count: state.servers.length });
  if (!state.servers.length) {
    serverListEl.innerHTML = `<div class="empty-state">${escapeHtml(t("noServers"))}</div>`;
    renderSelectedServer();
    return;
  }

  serverListEl.innerHTML = state.servers
    .map((server) => {
      const isSelected = state.selectedServerId === server.clientId;
      const isLocal = server.clientId === localClientId;
      const isExternal = Boolean(server.external);
      const isConnecting = state.pendingConnects.has(server.clientId);
      const buttonLabel = isLocal
        ? t("hostingButton")
        : isExternal && isConnecting
          ? t("joinCopiedButton")
          : isConnecting
            ? t("connectBusyButton")
            : t("joinButton");
      const endpointText = isExternal
        ? server.joinAddress ?? (server.peerAddr ? toSocketEndpoint(server.peerAddr) ?? server.peerAddr : t("serverNoEndpoint"))
        : server.publicJoinAddress ?? server.peerAddr ?? t("serverNoEndpoint");

      return `
        <article class="server-row ${isSelected ? "active" : ""}" data-select-server="${escapeHtml(server.clientId)}">
          <div class="server-main">
            <div class="server-main-top">
              <strong>${escapeHtml(server.roomName)}</strong>
              <span class="row-chip">${escapeHtml(server.hasPassword ? t("serverLockedChip") : t("serverOpenChip"))}</span>
              ${isExternal ? `<span class="row-chip external-chip">${escapeHtml(t("serverExternalChip"))}</span>` : ""}
              ${server.geyserEnabled && server.bedrockPort ? `<span class="row-chip">Bedrock ${escapeHtml(String(server.bedrockPort))}</span>` : ""}
            </div>
            <span>${escapeHtml(server.hostName)}${isLocal ? ` · ${escapeHtml(t("selfHostLabel"))}` : ""}</span>
            ${isExternal ? `<span class="server-submeta">${escapeHtml(t("externalAddedBy", { name: server.addedBy ?? "Player" }))}</span>` : ""}
          </div>
          <div class="server-main">
            <strong>${escapeHtml(server.minecraftVersion ?? t("serverUnknownVersion"))}</strong>
            <span>${escapeHtml(endpointText)}</span>
          </div>
          <div class="server-main">
            <strong>${escapeHtml(server.slots)}</strong>
          </div>
          <div class="player-actions">
            <button
              class="${isLocal ? "secondary-button" : "gradient-button"} row-action-button"
              data-connect-server="${escapeHtml(server.clientId)}"
              ${isLocal || (!isExternal && isConnecting) || isClientLocked() || state.status?.mode === "host" ? "disabled" : ""}
            >
              ${escapeHtml(buttonLabel)}
            </button>
          </div>
        </article>
      `;
    })
    .join("");

  renderSelectedServer();
}
function mergeServers(presenceServers) {
  const externalServers = state.externalServers.map((server) => ({ ...server, external: true }));
  const merged = [...presenceServers];
  for (const server of externalServers) {
    if (!merged.some((item) => item.clientId === server.clientId)) {
      merged.push(server);
    }
  }
  state.servers = merged;
}

async function refreshExternalServers() {
  if (!state.externalServers.length) return;
  const refreshed = await Promise.all(
    state.externalServers.map(async (server) => {
      try {
        const probe = await invoke("query_external_server", {
          host: server.hostName,
          port: Number(server.localPort || 25565),
        });
        return {
          ...server,
          roomName: probe.roomName || server.roomName,
          hostName: probe.hostName || server.hostName,
          slots: `${probe.onlinePlayers}/${probe.maxPlayers}`,
          minecraftVersion: probe.version || server.minecraftVersion || null,
          pingMs: probe.pingMs ?? server.pingMs ?? null,
        };
      } catch {
        return server;
      }
    }),
  );
  state.externalServers = refreshed;
  saveExternalServers();
}

function hydrateServers(members) {
  const presenceServers = members
    .map((member) => {
      const data = member.data ?? {};
      const peerAddrs = sortAdvertisedAddrs(
        Array.isArray(data.listen_addrs) ? data.listen_addrs.map(normalizeToMultiaddr).filter(Boolean) : [],
      );
      const endpoint = normalizeToMultiaddr(data.endpoint) ?? advertisedEndpoint(peerAddrs);
      const e4mcVerified = Boolean(data.e4mc_verified ?? (data.e4mc_domain && data.public_join_address === data.e4mc_domain));
      return {
        clientId: member.clientId,
        roomName: data.room_name ?? "Unnamed room",
        hostName: data.host_name ?? member.clientId,
        slots: data.slots ?? "1/30",
        hasPassword: Boolean(data.has_password),
        peerId: data.peer_id ?? null,
        peerAddrs,
        peerAddr: endpoint ?? null,
        localPort: data.local_port ?? 25565,
        minecraftVersion: data.minecraft_version ?? null,
        transport: data.transport ?? null,
        publicJoinAddress: data.public_join_address ?? data.socket_endpoint ?? null,
        e4mcDomain: e4mcVerified ? data.e4mc_domain ?? null : null,
        e4mcVerified,
        geyserEnabled: Boolean(data.geyser_enabled),
        bedrockPort: data.bedrock_port ?? null,
        bedrockEndpoint: data.bedrock_endpoint ?? null,
        minecraftNickname: data.minecraft_nickname ?? null,
        launcher: data.launcher ?? null,
        minecraftVersionRuntime: data.client_minecraft_version ?? null,
        modLoader: data.mod_loader ?? null,
        external: false,
      };
    })
    .filter((server) => Boolean(server.peerId) && (server.peerAddrs.length > 0 || Boolean(server.peerAddr)));

  mergeServers(presenceServers);

  if (state.selectedServerId && !state.servers.find((server) => server.clientId === state.selectedServerId)) {
    state.selectedServerId = null;
  }
  if (!state.selectedServerId && state.servers.length === 1) {
    state.selectedServerId = state.servers[0].clientId;
  }
  renderServers();
}

function buildExternalServerId(host, port) {
  return `external:${String(host).trim().toLowerCase()}:${Number(port)}`;
}

function parseServerAddress(input) {
  const value = String(input || "").trim();
  if (!value) throw new Error("Укажите адрес внешнего сервера.");

  if (value.startsWith("/dns4/") || value.startsWith("/ip4/")) {
    const parts = value.split("/").filter(Boolean);
    const host = parts[1];
    const tcpIndex = parts.findIndex((p) => p === "tcp" || p === "udp");
    const port = tcpIndex >= 0 ? Number(parts[tcpIndex + 1]) : 25565;
    if (!host || Number.isNaN(port) || port < 1 || port > 65535) {
      throw new Error("Некорректный адрес внешнего сервера.");
    }
    return { host, port };
  }

  if (value.startsWith("[")) {
    const match = value.match(/^\[([^\]]+)\](?::(\d+))?$/);
    if (!match) throw new Error("Некорректный IPv6 адрес.");
    const port = Number(match[2] || 25565);
    if (port < 1 || port > 65535) throw new Error("Некорректный порт.");
    return { host: match[1], port };
  }

  const separator = value.lastIndexOf(":");
  if (separator > 0 && value.indexOf(":") === separator) {
    const maybePort = Number(value.slice(separator + 1));
    if (!Number.isNaN(maybePort)) {
      if (maybePort < 1 || maybePort > 65535) throw new Error("Некорректный порт.");
      return { host: value.slice(0, separator), port: maybePort };
    }
  }

  return { host: value, port: 25565 };
}

async function addExternalServerFromModal(roomName) {
  const { host, port } = parseServerAddress(externalHostAddressEl.value);

  const probe = await invoke("query_external_server", { host, port });
  const clientId = buildExternalServerId(host, port);
  const joinAddress = `${host}:${port}`;
  const externalServer = {
    clientId,
    roomName: roomName || probe.roomName || host,
    hostName: probe.hostName || host,
    minecraftNickname: null,
    slots: `${probe.onlinePlayers}/${probe.maxPlayers}`,
    hasPassword: false,
    peerId: clientId,
    peerAddrs: [normalizeToMultiaddr(`${host}:${port}`)].filter(Boolean),
    peerAddr: normalizeToMultiaddr(`${host}:${port}`),
    joinAddress,
    addedBy: state.profile.nickname,
    localPort: port,
    minecraftVersion: probe.version || null,
    transport: "external-java",
    geyserEnabled: false,
    bedrockPort: null,
    bedrockEndpoint: null,
    pingMs: probe.pingMs ?? null,
    external: true,
  };

  state.externalServers = [
    ...state.externalServers.filter((server) => server.clientId !== clientId),
    externalServer,
  ];
  saveExternalServers();
  mergeServers(state.servers.filter((server) => !server.external));
  state.selectedServerId = clientId;
  renderServers();
  addLog(t("externalAddSuccess"));
}

function buildPresencePayload(status) {
  const endpoint = advertisedEndpoint(hostSession.listenAddrs, hostSession.peerAddr);
  const socketEndpoint = toSocketEndpoint(endpoint);
  const e4mcVerified = isE4mcVerified(status);
  const publicJoinAddress = getAdvertisableJoinAddress(status);
  const e4mcDomain = e4mcVerified ? getVisibleE4mcDomain(status) : null;
  const online = Math.max(1, (status?.peerCount ?? 0) + 1);
  const maxPlayers = Math.max(online, hostSession.maxPlayers ?? 30);
  return {
    room_name: hostSession.roomName,
    host_name: state.profile.nickname,
    minecraft_nickname: state.detectedMinecraftNickname ?? null,
    launcher: state.runtimeFingerprint?.launcher ?? null,
    client_minecraft_version: state.runtimeFingerprint?.minecraftVersion ?? null,
    mod_loader: state.runtimeFingerprint?.modLoader ?? null,
    slots: `${online}/${maxPlayers}`,
    has_password: hostSession.hasPassword,
    peer_id: hostSession.peerId ?? localClientId,
    listen_addrs: hostSession.listenAddrs,
    endpoint,
    socket_endpoint: socketEndpoint,
    public_join_address: publicJoinAddress,
    e4mc_domain: e4mcDomain,
    e4mc_verified: e4mcVerified,
    peer_addr: hostSession.peerAddr,
    local_port: hostSession.localPort,
    minecraft_version: hostSession.minecraftVersion ?? status?.minecraftVersion ?? null,
    transport: status?.transportPath ?? state.activeTunnelTransport ?? null,
    geyser_enabled: Boolean(status?.geyserEnabled),
    bedrock_port: status?.bedrockPort ?? null,
    bedrock_endpoint: deriveBedrockEndpoint(socketEndpoint, status?.bedrockPort ?? null),
  };
}

function syncHostSessionFromStatus(status) {
  if (status.mode === "host") {
    hostSession.active = true;
    hostSession.roomName = status.roomCode ?? hostSession.roomName;
    hostSession.hasPassword = Boolean(status.passwordProtected);
    hostSession.peerAddr =
      advertisedEndpoint(hostSession.listenAddrs, status.publicUdpAddr ?? status.udpBindAddr) ?? hostSession.peerAddr;
    hostSession.localPort = status.localGamePort ?? hostSession.localPort;
    hostSession.minecraftVersion = status.minecraftVersion ?? hostSession.minecraftVersion;
    hostSession.publicJoinAddress = status.publicJoinAddress ?? null;
    hostSession.e4mcDomain = status.e4mcDomain ?? null;
    hostSession.e4mcVerified = Boolean(status.e4mcVerified);
    return;
  }
  hostSession.active = false;
  hostSession.peerId = null;
  hostSession.listenAddrs = [];
  hostSession.peerAddr = null;
  hostSession.minecraftVersion = null;
  hostSession.publicJoinAddress = null;
  hostSession.e4mcDomain = null;
  hostSession.e4mcVerified = false;
  hostSession.presenceEntered = false;
}

function updateHintFromStatus(status) {
  if (state.tunnelReady || (status.mode === "client" && status.state === "connected")) {
    setMinecraftHint(t("hintConnected"), true);
  } else if (status.mode === "host") {
    setMinecraftHint(t("hintHostReady"), false);
  } else if (["starting", "connecting", "waitingForPeer", "punching"].includes(status.state)) {
    setMinecraftHint(t("hintConnecting"), false);
  } else if (status.state === "error") {
    setMinecraftHint(t("hintFailed"), false);
  } else {
    setMinecraftHint(t("hintWaiting"), false);
  }
}

function renderStatus(status) {
  state.status = status;
  if (["connected", "error", "idle", "hosting"].includes(status.state)) {
    state.pendingConnects.clear();
  }
  state.activeTunnelTransport = status.transportPath ?? state.activeTunnelTransport;
  syncHostSessionFromStatus(status);
  connectionStateEl.textContent = formatState(status.state);
  connectionStateEl.dataset.state = status.state ?? "idle";
  ablyStateEl.textContent = state.realtime?.connection.state ?? "offline";
  publicEndpointEl.textContent = formatPrimaryEndpoint(status);
  sessionModeEl.textContent = formatMode(status.mode);
  currentVersionEl.textContent = status.minecraftVersion ?? t("serverUnknownVersion");
  statusNoteEl.textContent = decodeMojibakeIfNeeded(status.note ?? t("modeIdle"));
  renderPeers(status.peers ?? []);
  renderSessionCard();
  renderLogs();
  renderSelectedServer();
  updateHintFromStatus(status);
  syncButtons();
}

function safeShouldSkip(channel) {
  return !channel || SAFE_SKIP_STATES.has(channel.state);
}

async function safePresenceLeave(channel) {
  if (!channel || channel.state !== "attached") return;
  try {
    await channel.presence.leave();
    addLog("Presence left.");
  } catch (error) {
    addLog(`Presence leave skipped: ${String(error)}`);
  }
}

async function safeDetachChannel(channel) {
  if (!channel || SAFE_SKIP_STATES.has(channel.state) || channel.state === "initialized") return;
  try {
    if (channel.state !== "detached") await channel.detach();
  } catch (error) {
    addLog(`Channel detach skipped: ${String(error)}`);
  }
}

function safeReleaseChannel(name) {
  const channel = state.realtime?.channels.get(name);
  if (!channel || !SAFE_RELEASE_STATES.has(channel.state)) return;
  try {
    state.realtime.channels.release(name);
  } catch (error) {
    addLog(`Channel release skipped: ${String(error)}`);
  }
}

async function ensureChannels() {
  if (!state.realtime) return;
  state.lobbyChannel ??= state.realtime.channels.get(LOBBY_CHANNEL_NAME);
  state.privateChannel ??= state.realtime.channels.get(`lobby:${localClientId}`);
}

async function bindChannelHandlers() {
  await ensureChannels();
  if (!state.lobbyChannel || !state.privateChannel) return;

  if (!state.lobbyChannel.__mcp2pPresenceBound) {
    await state.lobbyChannel.presence.subscribe("enter", () => void refreshLobby());
    await state.lobbyChannel.presence.subscribe("update", () => void refreshLobby());
    await state.lobbyChannel.presence.subscribe("leave", () => void refreshLobby());
    state.lobbyChannel.__mcp2pPresenceBound = true;
  }

  if (!state.privateChannel.__mcp2pHandshakeBound) {
    await state.privateChannel.subscribe("connect-request", async (message) => {
      const peerAddr = message.data?.peer_addr;
      const requester = message.data?.client_id ?? message.clientId ?? "unknown";
      const nickname = message.data?.nickname ?? requester;
      const minecraftNickname = message.data?.minecraft_nickname ?? null;
      const launcher = message.data?.launcher ?? null;
      const minecraftVersion = message.data?.client_minecraft_version ?? null;
      const modLoader = message.data?.mod_loader ?? null;
      const relaySessionId = message.data?.relay_session_id ?? null;
      state.peerProfiles.set(requester, { nickname, minecraftNickname, launcher, minecraftVersion, modLoader });
      addLog(t("incomingHandshake", { peer: requester, addr: peerAddr ?? "n/a" }));
      if (!peerAddr) return;

      try {
        await invoke("connect_to_peer", {
          peerId: requester,
          peerAddrs: [peerAddr],
          relaySessionId,
        });
        addLog(t("hostPunchSent", { addr: peerAddr }));
      } catch (error) {
        addLog(`Punch error: ${String(error)}`);
      }
    });
    state.privateChannel.__mcp2pHandshakeBound = true;
  }
}

async function refreshLobby() {
  await ensureChannels();
  if (!state.lobbyChannel || !state.realtime) return;

  try {
    if (state.realtime.connection.state !== "connected") {
      addLog(t("lobbyRefreshPostponed"));
      return;
    }
    if (!safeShouldSkip(state.lobbyChannel) && state.lobbyChannel.state !== "attached") {
      await state.lobbyChannel.attach();
    }
    const members = await state.lobbyChannel.presence.get();
    await refreshExternalServers();
    hydrateServers(members);
    addLog(`Lobby refresh: ${members.length} presence members.`);
  } catch (error) {
    addLog(t("lobbyRefreshFailed", { error: String(error) }));
  }
}

async function syncPresence(status, { force = false, enter = false } = {}) {
  await ensureChannels();
  if (
    !canAdvertiseHost() ||
    !state.lobbyChannel ||
    state.syncingPresence ||
    state.realtime?.connection.state !== "connected"
  ) {
    return;
  }

  const payload = buildPresencePayload(status);
  const serialized = JSON.stringify(payload);
  if (!force && !enter && serialized === hostSession.presencePayload) return;

  state.syncingPresence = true;
  try {
    if (!safeShouldSkip(state.lobbyChannel) && state.lobbyChannel.state !== "attached") {
      await state.lobbyChannel.attach();
    }
    const shouldEnter = enter || !hostSession.presenceEntered;
    const advertisedAddress = payload.public_join_address ?? payload.endpoint ?? "n/a";
    if (shouldEnter) {
      await state.lobbyChannel.presence.enter(payload);
      addLog(t("hostStartedPresence", { room: hostSession.roomName, addr: advertisedAddress }));
      hostSession.presenceEntered = true;
    } else {
      await state.lobbyChannel.presence.update(payload);
      addLog(`Presence updated for ${hostSession.roomName} (${advertisedAddress}).`);
    }
    hostSession.presencePayload = serialized;
  } catch (error) {
    addLog(t("presenceSyncFailed", { error: String(error) }));
  } finally {
    state.syncingPresence = false;
  }
}

async function setupAbly() {
  state.realtime = new Ably.Realtime({ key: ABLY_API_KEY, clientId: localClientId });
  state.realtime.connection.on(async (change) => {
    ablyStateEl.textContent = change.current;
    addLog(`Ably connection: ${change.previous ?? "none"} -> ${change.current}`);
    if (change.current === "connected") {
      await bindChannelHandlers();
      await syncPresence(state.status, { force: true, enter: !hostSession.presenceEntered });
      await refreshLobby();
    }
  });
  await new Promise((resolve) => state.realtime.connection.once("connected", resolve));
  await bindChannelHandlers();
  await refreshLobby();
}

async function waitForStatus(predicate, timeoutMs = 6000) {
  const started = Date.now();
  while (Date.now() - started < timeoutMs) {
    const status = await invoke("get_status");
    renderStatus(status);
    if (predicate(status)) return status;
    await new Promise((resolve) => setTimeout(resolve, 250));
  }
  throw new Error("Timed out while waiting for backend status.");
}

async function runPreflightCheck({ silent = false } = {}) {
  const localPort = Number(localGamePortEl.value || 25565);
  const report = await invoke("run_preflight_and_store", { localPort });
  state.lastPreflight = report;

  if (!silent) {
    addLog(
      `Preflight ${report.reachable ? "OK" : "FAIL"} for 127.0.0.1:${report.localPort}. ${
        report.minecraftVersion ? `Version: ${report.minecraftVersion}.` : "Version not detected."
      }`,
    );
    addLog(report.recommendedHostAction);
    if (report.note) addLog(report.note);
  }

  return report;
}

async function startEmbeddedTestServer() {
  const port = Number(testServerPortEl.value || 25566);
  if (port === Number(localGamePortEl.value || 25565)) {
    addLog("Diagnostics server cannot use the same port as Minecraft. Use a separate port, e.g. 25566.");
    return;
  }
  try {
    const info = await invoke("start_test_server", { port });
    state.testServerInfo = info;
    addLog(`Test server started at ${info.bindAddr}. Protocol: ${info.protocol}.`);
  } catch (error) {
    addLog(`Failed to start test server: ${String(error)}`);
  }
}

async function stopEmbeddedTestServer() {
  try {
    await invoke("stop_test_server");
    state.testServerInfo = null;
    addLog("Test server stopped.");
  } catch (error) {
    addLog(`Failed to stop test server: ${String(error)}`);
  }
}

async function copyDiagnosticsSnapshot() {
  try {
    const localPort = Number(localGamePortEl.value || 25565);
    const snapshot = await invoke("export_diagnostics_snapshot", { localPort });
    await navigator.clipboard.writeText(JSON.stringify(snapshot, null, 2));
    addLog("Full diagnostics copied to clipboard.");
  } catch (error) {
    addLog(`Failed to export diagnostics: ${String(error)}`);
  }
}

async function probeEmbeddedTestServer() {
  const port = Number(testServerPortEl.value || 25566);
  try {
    const payload = `diagnostic-ping:${Date.now()}`;
    const response = await invoke("probe_test_server_command", {
      port,
      payload,
    });
    addLog(`Test server response from 127.0.0.1:${port}: ${response || "<empty>"}`);
  } catch (error) {
    addLog(`Failed to connect test server 127.0.0.1:${port}: ${String(error)}`);
  }
}

async function startHosting() {
  if (!canOpenHostModal()) return;
  await detectMinecraftNickname();
  await autofillRoomNameFromLocalServer();
  const roomName = roomNameEl.value.trim();
  if (!roomName) {
    roomNameEl.focus();
    return;
  }

  if (externalHostModeEl.checked) {
    hostButtonEl.disabled = true;
    try {
      await addExternalServerFromModal(roomName);
      closeModal();
    } catch (error) {
      addLog(t("externalAddFailed", { error: String(error) }));
    } finally {
      hostButtonEl.disabled = false;
    }
    return;
  }

  const localPort = Number(localGamePortEl.value || 25565);
  const password = requirePasswordEl.checked ? roomPasswordEl.value.trim() || null : null;
  const enableGeyser = Boolean(enableGeyserEl.checked);
  state.e4mcEnabled = Boolean(enableE4mcEl?.checked);
  saveE4mcPreference(state.e4mcEnabled);
  const geyserPort = Number(geyserPortEl.value || 19132);
  hostButtonEl.disabled = true;
  state.tunnelReady = false;
  setMinecraftHint(t("hintWaiting"), false);

  try {
    const preflight = await runPreflightCheck({ silent: false });
    if (!preflight.reachable) {
      addLog("Host not started: local Minecraft is unavailable. Open LAN world or start a local server first.");
      return;
    }

    const bootstrap = await invoke("start_hosting", {
      roomName,
      password,
      localPort,
      enableGeyser,
      geyserPort,
      enableE4mc: state.e4mcEnabled,
    });
    const status = await waitForStatus(
      (snapshot) => snapshot.mode === "host" && ["waitingForPeer", "hosting", "connected", "error"].includes(snapshot.state),
      22000,
    );
    renderStatus(status);
    hostSession.active = true;
    hostSession.roomName = roomName;
    hostSession.hasPassword = Boolean(password);
    hostSession.peerId = bootstrap.peerId || localClientId;
    hostSession.listenAddrs = collectAdvertisedAddrs(bootstrap, status);
    hostSession.peerAddr = advertisedEndpoint(hostSession.listenAddrs, status.publicUdpAddr ?? status.udpBindAddr);
    hostSession.localPort = localPort;
    hostSession.minecraftVersion = status.minecraftVersion ?? null;
    hostSession.publicJoinAddress = status.publicJoinAddress ?? null;
    hostSession.e4mcDomain = status.e4mcDomain ?? null;
    hostSession.e4mcVerified = Boolean(status.e4mcVerified);
    try {
      const localMeta = await invoke("query_external_server", { host: "127.0.0.1", port: localPort });
      if (localMeta?.roomName) {
        hostSession.roomName = localMeta.roomName;
        roomNameEl.value = localMeta.roomName;
        roomNameEl.dataset.autofilled = "true";
      }
      const maxPlayers = Number(localMeta?.maxPlayers ?? 0);
      hostSession.maxPlayers = maxPlayers > 0 ? maxPlayers : 30;
    } catch {
      hostSession.maxPlayers = 30;
    }
    hostSession.presencePayload = null;
    hostSession.presenceEntered = false;
    if (canAdvertiseHost()) {
      await syncPresence(status, { force: true, enter: true });
    } else {
      addLog("Presence delayed: waiting for public endpoint.");
    }
    await refreshLobby();
    closeModal();
  } catch (error) {
    addLog(t("hostStartFailed", { error: String(error) }));
  } finally {
    syncButtons();
  }
}

async function stopSession() {
  stopButtonEl.disabled = true;
  state.pendingConnects.clear();
  state.pendingKicks.clear();
  state.tunnelReady = false;
  state.activeTunnelTransport = null;
  setMinecraftHint(t("hintWaiting"), false);

  try {
    await safePresenceLeave(state.lobbyChannel);
    await invoke("stop_hosting");
  } catch (error) {
    addLog(`Stop failed: ${String(error)}`);
  } finally {
    hostSession.active = false;
    hostSession.roomName = "";
    hostSession.hasPassword = false;
    hostSession.peerId = null;
    hostSession.listenAddrs = [];
    hostSession.peerAddr = null;
    hostSession.localPort = 25565;
    hostSession.maxPlayers = 30;
    hostSession.minecraftVersion = null;
    hostSession.publicJoinAddress = null;
    hostSession.e4mcDomain = null;
    hostSession.e4mcVerified = false;
    hostSession.presencePayload = null;
    hostSession.presenceEntered = false;
    state.selectedServerId = null;
    const status = await invoke("get_status");
    renderStatus(status);
    await refreshLobby();
    addLog(t("hostStopped"));
  }
}

async function connectToServer(server) {
  await detectMinecraftNickname();
  if (server.external) {
    const externalJoin = server.joinAddress ?? (server.peerAddr ? toSocketEndpoint(server.peerAddr) ?? server.peerAddr : null);
    if (externalJoin) {
      await navigator.clipboard.writeText(externalJoin);
      state.pendingConnects.add(server.clientId);
      renderServers();
      addLog(t("copiedIp"));
      setMinecraftHint(`${t("selectedAddressLabel")}: ${externalJoin}`, true);
      setTimeout(() => {
        state.pendingConnects.delete(server.clientId);
        renderServers();
      }, 1400);
    }
    return;
  }

  if (server.clientId === localClientId) {
    addLog(t("ownHostBlocked"));
    return;
  }
  if (state.pendingConnects.has(server.clientId)) {
    addLog(t("repeatConnectBlocked"));
    return;
  }
  if (isClientLocked()) {
    addLog(t("hostNoteClientLocked"));
    return;
  }

  state.selectedServerId = server.clientId;
  state.pendingConnects.add(server.clientId);
  state.tunnelReady = false;
  setMinecraftHint(t("hintConnecting"), false);
  renderServers();

  if (server.hasPassword) {
    const provided = window.prompt(t("passwordPrompt", { room: server.roomName }));
    if (provided == null) {
      state.pendingConnects.delete(server.clientId);
      renderServers();
      return;
    }
  }

  try {
    addLog(t("connectProgress", { room: server.roomName, addr: server.peerAddr }));
    const relaySessionId = `relay-${crypto.randomUUID()}`;
    const peerAddrs = sortAdvertisedAddrs(
      [...new Set([...(server.peerAddrs ?? []), normalizeToMultiaddr(server.peerAddr)].filter(Boolean))],
    );
    await invoke("connect_to_peer", {
      peerId: server.peerId,
      peerAddrs,
      relaySessionId,
    });
    const status = await waitForStatus(
      (snapshot) =>
        snapshot.mode === "client" &&
        Boolean(snapshot.publicUdpAddr ?? snapshot.udpBindAddr) &&
        ["waitingForPeer", "connecting", "connected"].includes(snapshot.state),
      8000,
    );
    renderStatus(status);
      await state.realtime.channels.get(`lobby:${server.clientId}`).publish("connect-request", {
        client_id: localClientId,
        nickname: state.profile.nickname,
        minecraft_nickname: state.detectedMinecraftNickname ?? null,
        launcher: state.runtimeFingerprint?.launcher ?? null,
        client_minecraft_version: state.runtimeFingerprint?.minecraftVersion ?? null,
        mod_loader: state.runtimeFingerprint?.modLoader ?? null,
        room_name: server.roomName,
        peer_addr: status.publicUdpAddr ?? status.udpBindAddr,
        relay_session_id: relaySessionId,
      });
    addLog(t("connectRequestSent", { host: server.clientId }));
  } catch (error) {
    state.pendingConnects.delete(server.clientId);
    setMinecraftHint(t("hintFailed"), false);
    addLog(t("connectFailed", { error: String(error) }));
    if (server.publicJoinAddress) {
      await navigator.clipboard.writeText(server.publicJoinAddress);
      addLog(t("connectFallbackCopied", { address: server.publicJoinAddress }));
      setMinecraftHint(`${t("selectedAddressLabel")}: ${server.publicJoinAddress}`, true);
    }
    renderServers();
  }
}

async function kickPeer(peerId) {
  if (state.pendingKicks.has(peerId)) return;
  state.pendingKicks.add(peerId);
  renderPeers(state.status?.peers ?? []);
  try {
    await invoke("kick_peer", { peerId });
  } catch (error) {
    addLog(t("kickFailed", { error: String(error) }));
  } finally {
    state.pendingKicks.delete(peerId);
    const status = await invoke("get_status");
    renderStatus(status);
  }
}

async function pollStatus() {
  try {
    const status = await invoke("get_status");
    await refreshLocalWorldPlayers(false, status);
    renderStatus(status);
    await syncPresence(status);
  } catch (error) {
    addLog(t("statusPollFailed", { error: String(error) }));
  }
}

function rerender() {
  applyTranslations();
  renderSettingsOptions();
  renderServers();
  renderIgnoredPorts();
  renderStatus(
    state.status ?? {
      mode: "idle",
      state: "idle",
      roomCode: null,
      udpBindAddr: null,
      publicUdpAddr: null,
      localGamePort: null,
      minecraftVersion: null,
      geyserEnabled: false,
      bedrockPort: null,
      passwordProtected: false,
      peerCount: 0,
      peers: [],
      note: t("modeIdle"),
      lastError: null,
      signalingServer: "Ably Presence + Channels",
      logs: [],
    },
  );
}

await listen("tunnel_established", async (event) => {
  state.pendingConnects.clear();
  state.tunnelReady = true;
  state.activeTunnelTransport = event.payload?.transport ?? state.activeTunnelTransport ?? "direct-quic";
  setMinecraftHint(t("hintConnected"), true);
  addLog(
    `${t("tunnelEstablishedLog", { addr: event.payload?.minecraftAddr ?? "localhost:25565" })} (${formatTransportLabel(
      state.activeTunnelTransport,
    )})`,
  );
  const status = await invoke("get_status");
  renderStatus(status);
  await syncPresence(status, { force: true });
  renderServers();
});

await listen("tunnel_failed", async (event) => {
  state.pendingConnects.clear();
  state.tunnelReady = false;
  setMinecraftHint(t("hintFailed"), false);
  if (event.payload?.reason) {
    addLog(`${t("tunnelFailedLog")} ${String(event.payload.reason)}`);
  } else {
    addLog(t("tunnelFailedLog"));
  }
  const status = await invoke("get_status");
  renderStatus(status);
  renderServers();
});

await listen("reverse_tunnel_ready", async (event) => {
  const endpoint = normalizeToMultiaddr(event.payload?.endpoint ?? null);
  const multiaddr = normalizeToMultiaddr(event.payload?.multiaddr ?? null);
  if (multiaddr) {
    hostSession.listenAddrs = sortAdvertisedAddrs([...new Set([multiaddr, ...hostSession.listenAddrs])]);
  }
  if (endpoint) {
    hostSession.peerAddr = endpoint;
  }
  addLog(`Reverse tunnel ready: ${endpoint ?? multiaddr ?? "n/a"}`);
  const status = await invoke("get_status");
  renderStatus(status);
  await syncPresence(status, { force: true, enter: !hostSession.presenceEntered });
  await refreshLobby();
});

await listen("test_server_started", async (event) => {
  state.testServerInfo = event.payload ?? null;
  addLog(`Test server ready: ${event.payload?.bindAddr ?? "n/a"} (${event.payload?.protocol ?? "unknown"}).`);
});

await listen("test_server_client_closed", async (event) => {
  addLog(`Test server client disconnected: ${event.payload ?? "unknown"}.`);
});

await listen("relay_active", async (event) => {
  addLog(`Relay active: ${event.payload?.relayAddr ?? "n/a"}`);
});

await listen("hole_punch_success", async (event) => {
  state.activeTunnelTransport = "direct-quic";
  addLog(`Hole punch success for ${event.payload?.peerId ?? "peer"}.`);
});

await listen("e4mc_domain_ready", async (event) => {
  const domain = String(event.payload?.domain ?? "").trim();
  if (!domain) return;
  hostSession.e4mcDomain = domain;
  hostSession.e4mcVerified = Boolean(event.payload?.verified);
  hostSession.publicJoinAddress = hostSession.e4mcVerified ? domain : null;
  addLog(
    hostSession.e4mcVerified
      ? t("e4mcVerifiedLog", { domain })
      : t("e4mcPendingLog", { domain, error: String(event.payload?.error ?? "verification pending") }),
  );
  const status = await invoke("get_status");
  renderStatus(status);
  await syncPresence(status, { force: true, enter: !hostSession.presenceEntered });
  await refreshLobby();
});

navHomeEl.addEventListener("click", () => setPage("home"));
navSettingsEl.addEventListener("click", () => setPage("settings"));
openHostModalEl.addEventListener("click", openModal);
closeModalEl.addEventListener("click", closeModal);
closeModalSecondaryEl.addEventListener("click", closeModal);
modalEl.addEventListener("click", (event) => {
  if (event.target instanceof HTMLElement && event.target.dataset.closeModal === "true") closeModal();
});

requirePasswordEl.addEventListener("change", syncPasswordField);
enableGeyserEl.addEventListener("change", syncGeyserField);
roomNameEl.addEventListener("input", () => {
  roomNameEl.dataset.autofilled = "false";
});
hostButtonEl.addEventListener("click", startHosting);
stopButtonEl.addEventListener("click", stopSession);
refreshLobbyEl.addEventListener("click", async () => {
  const status = await invoke("get_status");
  if (canAdvertiseHost()) {
    await syncPresence(status, { force: true, enter: !hostSession.presenceEntered });
  }
  await refreshLobby();
});
copyLogsEl.addEventListener("click", async () => {
  await navigator.clipboard.writeText(currentLogLines().join("\n"));
  addLog(t("copiedLog"));
});
copyDiagnosticsEl.addEventListener("click", async () => {
  await copyDiagnosticsSnapshot();
});
copySelectedEndpointEl.addEventListener("click", async () => {
  const selected = getSelectedServer();
  const endpoint =
    selected?.publicJoinAddress ??
    selected?.joinAddress ??
    (selected?.peerAddr ? toSocketEndpoint(selected.peerAddr) ?? selected.peerAddr : null);
  if (!endpoint) return;
  await navigator.clipboard.writeText(endpoint);
  addLog(t("copiedIp"));
});
copySelectedBedrockEndpointEl.addEventListener("click", async () => {
  const selected = getSelectedServer();
  const endpoint = deriveBedrockEndpoint(selected?.peerAddr, selected?.bedrockPort);
  if (!endpoint) return;
  await navigator.clipboard.writeText(endpoint);
  addLog(t("copiedBedrockIp"));
});
runPreflightEl.addEventListener("click", async () => {
  await runPreflightCheck();
});
autoDetectPortEl.addEventListener("click", async () => {
  await autoDetectLocalGamePort();
});
externalHostModeEl.addEventListener("change", syncExternalHostMode);
startTestServerEl.addEventListener("click", async () => {
  await startEmbeddedTestServer();
});
stopTestServerEl.addEventListener("click", async () => {
  await stopEmbeddedTestServer();
});
probeTestServerEl.addEventListener("click", async () => {
  await probeEmbeddedTestServer();
});
connectSelectedEl.addEventListener("click", async () => {
  const selected = getSelectedServer();
  if (selected) await connectToServer(selected);
});
profileMenuTriggerEl.addEventListener("click", () => {
  toggleProfileMenu();
});
chooseAvatarEl.addEventListener("click", pickAvatarFile);
profileAvatarFileEl.addEventListener("change", handleAvatarChosen);
saveProfileEl.addEventListener("click", saveProfileFromInputs);
checkUpdatesEl.addEventListener("click", checkForUpdates);
installUpdateEl.addEventListener("click", installUpdate);

closePortModalEl.addEventListener("click", closePortChoiceModal);
closePortModalSecondaryEl.addEventListener("click", closePortChoiceModal);
portChoiceModalEl.addEventListener("click", (event) => {
  if (event.target instanceof HTMLElement && event.target.dataset.closeModalPort === "true") closePortChoiceModal();
});
clearIgnoredPortsEl.addEventListener("click", () => {
  state.ignoredPorts = [];
  saveIgnoredPorts();
  renderIgnoredPorts();
});

serverListEl.addEventListener("click", async (event) => {
  const target = event.target;
  if (!(target instanceof HTMLElement)) return;
  const connectId = target.closest("[data-connect-server]")?.dataset.connectServer;
  if (connectId) {
    const server = state.servers.find((item) => item.clientId === connectId);
    if (server) await connectToServer(server);
    return;
  }
  const selectId = target.closest("[data-select-server]")?.dataset.selectServer;
  if (selectId) {
    state.selectedServerId = selectId;
    renderSelectedServer();
    renderServers();
  }
});

activeHostCardEl.addEventListener("click", async (event) => {
  const target = event.target;
  if (!(target instanceof HTMLElement)) return;
  const javaValue = target.closest("[data-copy-host-java]")?.dataset.copyHostJava;
  if (javaValue) {
    await navigator.clipboard.writeText(toSocketEndpoint(javaValue) ?? javaValue);
    addLog(t("copiedIp"));
    return;
  }
  const bedrockValue = target.closest("[data-copy-host-bedrock]")?.dataset.copyHostBedrock;
  if (bedrockValue) {
    await navigator.clipboard.writeText(bedrockValue);
    addLog(t("copiedBedrockIp"));
  }
});

peerListEl.addEventListener("click", async (event) => {
  const target = event.target;
  if (!(target instanceof HTMLElement)) return;
  const peerId = target.closest("[data-kick-peer]")?.dataset.kickPeer;
  if (peerId) await kickPeer(peerId);
});

document.querySelectorAll("[data-theme-value]").forEach((button) => {
  button.addEventListener("click", () => applyTheme(button.dataset.themeValue));
});
document.querySelectorAll("[data-language-value]").forEach((button) => {
  button.addEventListener("click", () => applyLanguage(button.dataset.languageValue));
});

document.addEventListener("keydown", (event) => {
  if (event.key === "Escape") closeModal();
});

document.addEventListener("click", (event) => {
  const target = event.target;
  if (!(target instanceof Node)) return;
  const element = target instanceof HTMLElement ? target : null;
  const button = element?.closest("button");
  if (button && !button.disabled) {
    const label = button.dataset.i18n ? t(button.dataset.i18n) : button.textContent?.trim() || button.id || "button";
    addLog(`UI action: ${label}`);
  }
  if (!profileMenuEl?.contains(target) && !profileMenuTriggerEl?.contains(target)) {
    toggleProfileMenu(false);
  }
});
window.addEventListener("resize", positionProfileMenu);
window.addEventListener("scroll", positionProfileMenu, true);

document.body.dataset.theme = state.preferences.theme;
applyTranslations();
renderSettingsOptions();
syncPasswordField();
syncGeyserField();
syncExternalHostMode();
ensureProfileMenuPortal();
renderProfile();
renderLogs();
renderSelectedServer();
renderSessionCard();
syncButtons();
setPage("home");
renderIgnoredPorts();
await loadAppInfo();
await detectMinecraftNickname();

setInterval(() => {
  void pollStatus();
}, POLL_INTERVAL_MS);

await detectRuntimeFingerprint();
await setupAbly();
await pollStatus();


