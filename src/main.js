import * as Ably from "ably";
import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";

import { I18N } from "./i18n.js";

const ABLY_API_KEY = "aGkPAA.1VHkjw:Bai-67g05FcqHdfVOMiSfjYlK3aLz8wOzj5WeTgz4cw";
const LOBBY_CHANNEL_NAME = "minecraft-lobby";
const POLL_INTERVAL_MS = 1500;
const SAFE_RELEASE_STATES = new Set(["initialized", "detached", "failed"]);
const SAFE_SKIP_STATES = new Set(["detached", "failed", "suspended"]);
const SETTINGS_THEME_KEY = "minecraft-p2p-theme";
const SETTINGS_LANGUAGE_KEY = "minecraft-p2p-language";
const SETTINGS_PROFILE_KEY = "minecraft-p2p-profile";

const modalEl = document.querySelector("#host-modal");
const openHostModalEl = document.querySelector("#open-host-modal");
const closeModalEl = document.querySelector("#close-modal");
const closeModalSecondaryEl = document.querySelector("#close-modal-secondary");
const requirePasswordEl = document.querySelector("#require-password");
const useCloudflareEl = document.querySelector("#use-cloudflare");
const passwordFieldGroupEl = document.querySelector("#password-field-group");
const roomNameEl = document.querySelector("#room-name");
const roomPasswordEl = document.querySelector("#room-password");
const localGamePortEl = document.querySelector("#local-game-port");
const hostButtonEl = document.querySelector("#host-button");
const stopButtonEl = document.querySelector("#stop-button");
const refreshLobbyEl = document.querySelector("#refresh-lobby");
const copyLogsEl = document.querySelector("#copy-logs");
const copyDiagnosticsEl = document.querySelector("#copy-diagnostics");
const yggstackStatusEl = document.querySelector("#yggstack-status");
const prepareYggstackEl = document.querySelector("#prepare-yggstack");
const startYggstackEl = document.querySelector("#start-yggstack");
const retryYggstackEl = document.querySelector("#retry-yggstack");
const stopYggstackEl = document.querySelector("#stop-yggstack");
const copySelectedEndpointEl = document.querySelector("#copy-selected-endpoint");
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
const hideOverlayButtonEl = document.querySelector("#hide-overlay-button");
const openProfileModalEl = document.querySelector("#open-profile-modal");
const overlayShortcutChipEl = document.querySelector("#overlay-shortcut-chip");
const overlayStatusLineEl = document.querySelector("#overlay-status-line");
const topbarProfileAvatarEl = document.querySelector("#topbar-profile-avatar");
const topbarProfileNameEl = document.querySelector("#topbar-profile-name");
const settingsNicknameEl = document.querySelector("#settings-nickname");
const settingsAvatarInputEl = document.querySelector("#settings-avatar-input");
const settingsAvatarPreviewEl = document.querySelector("#settings-avatar-preview");
const settingsProfileNameEl = document.querySelector("#settings-profile-name");
const settingsProfileSubtitleEl = document.querySelector("#settings-profile-subtitle");
const settingsShortcutInputEl = document.querySelector("#settings-shortcut-input");
const saveProfileButtonEl = document.querySelector("#save-profile-button");
const clearAvatarButtonEl = document.querySelector("#clear-avatar-button");
const profileModalEl = document.querySelector("#profile-modal");
const closeProfileModalEl = document.querySelector("#close-profile-modal");
const closeProfileModalSecondaryEl = document.querySelector("#close-profile-modal-secondary");
const saveProfileModalEl = document.querySelector("#save-profile-modal");
const profileNicknameInputEl = document.querySelector("#profile-nickname-input");
const profileAvatarInputEl = document.querySelector("#profile-avatar-input");
const profileShortcutInputEl = document.querySelector("#profile-shortcut-input");
const profileAvatarPreviewEl = document.querySelector("#profile-avatar-preview");

const appWindow = getCurrentWindow();

const hostSession = {
  active: false,
  roomName: "",
  hasPassword: false,
  peerId: null,
  listenAddrs: [],
  peerAddr: null,
  localPort: 25565,
  minecraftVersion: null,
  presencePayload: null,
  presenceEntered: false,
  useCloudflare: false,
  yggEnabled: false,
  yggAddress: null,
  yggPublicKey: null,
  yggSubnet: null,
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
  pendingTransportFlow: null,
  tunnelReady: false,
  activeTunnelTransport: null,
  lastPreflight: null,
  testServerInfo: null,
  yggstackInfo: null,
  page: "home",
  preferences: loadPreferences(),
};

function ensureClientId() {
  const key = "minecraft-p2p-client-id";
  const existing = localStorage.getItem(key);
  if (existing) return existing;
  const created = `mc-${crypto.randomUUID().slice(0, 8)}`;
  localStorage.setItem(key, created);
  return created;
}

function loadPreferences() {
  const storedProfile = safeParseJson(localStorage.getItem(SETTINGS_PROFILE_KEY));
  return {
    theme: localStorage.getItem(SETTINGS_THEME_KEY) || "oled",
    language: localStorage.getItem(SETTINGS_LANGUAGE_KEY) || "ru",
    profile: {
      nickname: storedProfile?.nickname || "",
      avatarDataUrl: storedProfile?.avatarDataUrl || null,
      overlayShortcut: storedProfile?.overlayShortcut || "SHIFT+TAB",
    },
  };
}

function safeParseJson(value) {
  try {
    return value ? JSON.parse(value) : null;
  } catch {
    return null;
  }
}

function savePreference(key, value) {
  localStorage.setItem(key, value);
}

function t(key, variables = {}) {
  const dictionary = I18N[state.preferences.language] ?? I18N.ru;
  const template = dictionary[key] ?? I18N.ru[key] ?? key;
  return template.replaceAll(/\{(\w+)\}/g, (_, name) => String(variables[name] ?? `{${name}}`));
}

function escapeHtml(value) {
  return String(value)
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll('"', "&quot;")
    .replaceAll("'", "&#039;");
}

function saveProfile() {
  localStorage.setItem(
    SETTINGS_PROFILE_KEY,
    JSON.stringify(state.preferences.profile),
  );
}

function currentNickname() {
  return state.preferences.profile.nickname?.trim() || localClientId;
}

function normalizedShortcut(value) {
  return String(value || "SHIFT+TAB")
    .trim()
    .replace(/\s+/g, "")
    .replaceAll("+", "+")
    .toUpperCase();
}

function avatarMarkup(label = currentNickname(), avatarDataUrl = state.preferences.profile.avatarDataUrl) {
  if (avatarDataUrl) {
    return `<img class="host-avatar-image" src="${avatarDataUrl}" alt="${escapeHtml(label)}" />`;
  }
  const initials = String(label || "MC")
    .replace(/§.|&./g, "")
    .trim()
    .split(/\s+/)
    .slice(0, 2)
    .map((part) => part[0]?.toUpperCase() ?? "")
    .join("") || "MC";
  return `<span>${escapeHtml(initials)}</span>`;
}

const MINECRAFT_COLOR_MAP = {
  0: "#000000",
  1: "#0000aa",
  2: "#00aa00",
  3: "#00aaaa",
  4: "#aa0000",
  5: "#aa00aa",
  6: "#ffaa00",
  7: "#aaaaaa",
  8: "#555555",
  9: "#5555ff",
  a: "#55ff55",
  b: "#55ffff",
  c: "#ff5555",
  d: "#ff55ff",
  e: "#ffff55",
  f: "#ffffff",
};

function renderMinecraftFormattedText(raw) {
  const value = String(raw ?? "");
  const fragments = [];
  let style = { color: null, bold: false, italic: false, underline: false, strike: false };
  let buffer = "";

  function flush() {
    if (!buffer) return;
    const rules = [];
    if (style.color) rules.push(`color:${style.color}`);
    if (style.bold) rules.push("font-weight:800");
    if (style.italic) rules.push("font-style:italic");
    if (style.underline || style.strike) {
      rules.push(
        `text-decoration:${[style.underline ? "underline" : null, style.strike ? "line-through" : null]
          .filter(Boolean)
          .join(" ")}`,
      );
    }
    fragments.push(
      `<span${rules.length ? ` style="${rules.join(";")}"` : ""}>${escapeHtml(buffer)}</span>`,
    );
    buffer = "";
  }

  for (let index = 0; index < value.length; index += 1) {
    const symbol = value[index];
    const next = value[index + 1]?.toLowerCase();
    if ((symbol === "§" || symbol === "&") && next) {
      flush();
      if (MINECRAFT_COLOR_MAP[next]) {
        style = { color: MINECRAFT_COLOR_MAP[next], bold: false, italic: false, underline: false, strike: false };
      } else if (next === "l") {
        style.bold = true;
      } else if (next === "o") {
        style.italic = true;
      } else if (next === "n") {
        style.underline = true;
      } else if (next === "m") {
        style.strike = true;
      } else if (next === "r") {
        style = { color: null, bold: false, italic: false, underline: false, strike: false };
      }
      index += 1;
      continue;
    }
    buffer += symbol;
  }
  flush();
  return fragments.join("") || escapeHtml(value);
}

function renderAvatarTarget(element, label = currentNickname(), avatarDataUrl = state.preferences.profile.avatarDataUrl) {
  if (!element) return;
  element.innerHTML = avatarMarkup(label, avatarDataUrl);
}

function syncProfileSurface() {
  const nickname = currentNickname();
  const shortcut = normalizedShortcut(state.preferences.profile.overlayShortcut);
  state.preferences.profile.overlayShortcut = shortcut;
  if (overlayShortcutChipEl) overlayShortcutChipEl.textContent = shortcut;
  if (topbarProfileNameEl) topbarProfileNameEl.textContent = nickname;
  if (settingsProfileNameEl) settingsProfileNameEl.textContent = nickname;
  if (settingsProfileSubtitleEl) {
    settingsProfileSubtitleEl.textContent = `Оверлей открывается по ${shortcut}.`;
  }
  if (settingsNicknameEl) settingsNicknameEl.value = state.preferences.profile.nickname ?? "";
  if (settingsShortcutInputEl) settingsShortcutInputEl.value = shortcut;
  if (profileNicknameInputEl) profileNicknameInputEl.value = state.preferences.profile.nickname ?? "";
  if (profileShortcutInputEl) profileShortcutInputEl.value = shortcut;
  renderAvatarTarget(topbarProfileAvatarEl, nickname);
  renderAvatarTarget(settingsAvatarPreviewEl, nickname);
  renderAvatarTarget(profileAvatarPreviewEl, nickname);
}

async function fileToDataUrl(file) {
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onerror = () => reject(new Error("Не удалось прочитать файл аватара."));
    reader.onload = () => resolve(String(reader.result ?? ""));
    reader.readAsDataURL(file);
  });
}

async function buildProfilePayload(nicknameEl, shortcutEl, avatarInputEl) {
  const nickname = nicknameEl?.value?.trim() || "";
  const overlayShortcut = normalizedShortcut(shortcutEl?.value || "SHIFT+TAB");
  let avatarDataUrl = state.preferences.profile.avatarDataUrl ?? null;
  const file = avatarInputEl?.files?.[0];
  if (file) {
    avatarDataUrl = await fileToDataUrl(file);
  }
  return {
    nickname,
    avatarDataUrl,
    overlayShortcut,
    theme: state.preferences.theme,
    language: state.preferences.language,
  };
}

function openProfileModal(required = false) {
  profileModalEl?.classList.remove("hidden");
  profileModalEl?.setAttribute("aria-hidden", "false");
  if (profileModalEl) profileModalEl.dataset.required = required ? "true" : "false";
  setTimeout(() => profileNicknameInputEl?.focus(), 40);
}

function closeProfileModal() {
  if (profileModalEl?.dataset.required === "true" && !state.preferences.profile.nickname?.trim()) {
    return;
  }
  profileModalEl?.classList.add("hidden");
  profileModalEl?.setAttribute("aria-hidden", "true");
}

async function persistProfileFromFields(nicknameEl, shortcutEl, avatarInputEl, { closeAfter = false } = {}) {
  const payload = await buildProfilePayload(nicknameEl, shortcutEl, avatarInputEl);
  if (!payload.nickname) {
    nicknameEl?.focus();
    return;
  }

  state.preferences.profile.nickname = payload.nickname;
  state.preferences.profile.avatarDataUrl = payload.avatarDataUrl;
  state.preferences.profile.overlayShortcut = payload.overlayShortcut;
  saveProfile();

  await invoke("save_user_profile", {
    profile: {
      nickname: payload.nickname,
      avatarDataUrl: payload.avatarDataUrl,
      theme: state.preferences.theme,
      language: state.preferences.language,
      overlayShortcut: payload.overlayShortcut,
    },
  });

  if (settingsAvatarInputEl) settingsAvatarInputEl.value = "";
  if (profileAvatarInputEl) profileAvatarInputEl.value = "";
  syncProfileSurface();
  rerender();
  addLog(`Профиль сохранён. Overlay shortcut: ${payload.overlayShortcut}.`);
  if (closeAfter) closeProfileModal();
}

async function hydrateProfile() {
  try {
    const profile = await invoke("get_user_profile");
    if (profile?.nickname) state.preferences.profile.nickname = profile.nickname;
    if (profile?.avatarDataUrl) state.preferences.profile.avatarDataUrl = profile.avatarDataUrl;
    if (profile?.overlayShortcut) state.preferences.profile.overlayShortcut = profile.overlayShortcut;
    if (profile?.theme) state.preferences.theme = profile.theme;
    if (profile?.language) state.preferences.language = profile.language;
  } catch (error) {
    addLog(`Не удалось загрузить профиль из backend: ${String(error)}`);
  }

  document.body.dataset.theme = state.preferences.theme;
  saveProfile();
  syncProfileSurface();

  if (!state.preferences.profile.nickname?.trim()) {
    openProfileModal(true);
  }
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
  portHelpEl.title = t("portHelpTitle");
  document.documentElement.lang = state.preferences.language;
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
  syncProfileSurface();
  rerender();
}

function setPage(page) {
  state.page = page;
  pageHomeEl.classList.toggle("page-active", page === "home");
  pageSettingsEl.classList.toggle("page-active", page === "settings");
  navHomeEl.classList.toggle("nav-button-active", page === "home");
  navSettingsEl.classList.toggle("nav-button-active", page === "settings");
}

function addLog(message) {
  const stamp = new Date().toLocaleTimeString(state.preferences.language === "ru" ? "ru-RU" : "en-US", {
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
  });
  state.logBuffer.unshift(`[${stamp}] ${message}`);
  state.logBuffer = state.logBuffer.slice(0, 120);
  renderLogs();
}

function currentLogLines() {
  const combined = [...state.logBuffer];
  if (state.status?.logs?.length) combined.push(...state.status.logs);
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
  modalEl.classList.remove("hidden");
  modalEl.setAttribute("aria-hidden", "false");
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
  if (transport === "cloudflare-webrtc") return "Cloudflare TURN/WebRTC";
  if (transport === "ably-relay") return "Ably MQTT relay";
  if (transport === "relay-circuit" || transport === "relay-reservation") return "Circuit Relay v2";
  if (transport === "direct-hole-punch") return "DCUtR hole punch";
  if (transport === "direct" || transport === "direct-quic") return "Direct QUIC";
  return transport ?? "неизвестный транспорт";
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

function canAdvertiseHost() {
  return Boolean(hostSession.active && hostSession.peerId && advertisedEndpoint(hostSession.listenAddrs, hostSession.peerAddr));
}

function renderSelectedServer() {
  const selected = getSelectedServer();
  selectedServerEl.innerHTML = selected ? renderMinecraftFormattedText(selected.roomName) : escapeHtml(t("noSelection"));
  selectedEndpointEl.textContent = selected?.peerAddr ?? "n/a";
  selectedMetaEl.textContent = selected
    ? t("selectedMetaTemplate", {
        host: `${selected.hostName}${selected.clientId === localClientId ? " (you)" : ""} · ${selected.peerId}`,
        version: selected.minecraftVersion ?? t("serverUnknownVersion"),
        slots: selected.slots,
        password: `${selected.hasPassword ? t("selectedMetaPassword") : ""}${selected.cloudflareEnabled ? " · Cloudflare" : ""}`,
      })
    : t("selectedMetaEmpty");
}

function renderYggstackRuntimeLegacy(info) {
  state.yggstackInfo = info ?? null;
  if (!yggstackStatusEl) return;
  if (!info) {
    yggstackStatusEl.textContent = "Yggstack: runtime ещё не проверен.";
    return;
  }

  const stateLabel = info.running ? "sidecar запущен" : info.ready ? "runtime готов" : "runtime не готов";
  yggstackStatusEl.textContent = `Yggstack: ${stateLabel}. ${info.note ?? ""}`.trim();
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
  hostButtonEl.textContent = busy ? t("connectBusyButton") : t("modalHostButton");
  stopButtonEl.disabled = mode === "idle";
  stopButtonEl.textContent = mode === "client" ? t("disconnectButton") : t("stopButton");
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
  copySelectedEndpointEl.disabled = !selected?.peerAddr;
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
        <div class="host-avatar">${avatarMarkup(hostSession.roomName)}</div>
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
      <div class="host-avatar">${avatarMarkup(hostSession.roomName)}</div>
      <div class="host-details">
        <h3>${renderMinecraftFormattedText(hostSession.roomName)}</h3>
        <p>${escapeHtml(t("hostCardPlayers", { count: status?.peerCount ?? 0 }))}</p>
        <div class="host-meta-row">
          <span class="host-meta-pill">${escapeHtml(t("hostCardVersion", { version }))}</span>
          <span class="host-meta-pill">${escapeHtml(t("hostCardPort", { port: hostSession.localPort }))}</span>
          <span class="host-meta-pill">${escapeHtml(hostSession.hasPassword ? t("hostCardPasswordOn") : t("hostCardPasswordOff"))}</span>
        </div>
      </div>
    </div>
  `;
}

function renderPeers(peers) {
  peerCountEl.textContent = t("peerCount", { count: peers?.length ?? 0 });
  if (!peers?.length) {
    peerListEl.innerHTML = `<div class="empty-state">${escapeHtml(t("noPeers"))}</div>`;
    return;
  }

  const hostMode = state.status?.mode === "host";
  peerListEl.innerHTML = peers
    .map((peer) => {
      const canKick = hostMode && peer.connected;
      const label = state.pendingKicks.has(peer.peerId)
        ? t("kickPendingButton")
        : canKick
          ? t("kickButton")
          : t("playerPassiveAction");

      return `
        <div class="player-row">
          <div class="player-main">
            <strong>${escapeHtml(peer.peerId)}</strong>
            <span>${escapeHtml(peer.addr)}</span>
            <span>${peer.connected ? "online" : "pending"} · ${peer.pingMs == null ? "n/a" : `${peer.pingMs} ms`}</span>
          </div>
          <div class="player-actions">
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
    })
    .join("");
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
      const isConnecting = state.pendingConnects.has(server.clientId);
      const buttonLabel = isLocal ? t("hostingButton") : isConnecting ? t("connectBusyButton") : t("joinButton");

      return `
        <article class="server-row ${isSelected ? "active" : ""}" data-select-server="${escapeHtml(server.clientId)}">
          <div class="server-main">
            <div class="server-main-top">
              <strong class="minecraft-name">${renderMinecraftFormattedText(server.roomName)}</strong>
              <span class="row-chip">${server.cloudflareEnabled ? "☁" : server.hasPassword ? "🔒" : "⚔"}</span>
            </div>
            <span>${escapeHtml(server.hostName)}${isLocal ? ` · ${escapeHtml(t("selfHostLabel"))}` : ""}</span>
          </div>
          <div class="server-main">
            <strong>${escapeHtml(server.minecraftVersion ?? t("serverUnknownVersion"))}</strong>
            <span>${escapeHtml(server.peerAddr ?? t("serverNoEndpoint"))}</span>
          </div>
          <div class="server-main">
            <strong>${escapeHtml(server.slots)}</strong>
          </div>
          <div class="player-actions">
            <button
              class="${isLocal ? "secondary-button" : "gradient-button"} row-action-button"
              data-connect-server="${escapeHtml(server.clientId)}"
              ${isLocal || isConnecting || isClientLocked() || state.status?.mode === "host" ? "disabled" : ""}
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

function hydrateServers(members) {
  state.servers = members
    .map((member) => {
      const data = member.data ?? {};
      const peerAddrs = sortAdvertisedAddrs(
        Array.isArray(data.listen_addrs) ? data.listen_addrs.map(normalizeToMultiaddr).filter(Boolean) : [],
      );
      const endpoint = normalizeToMultiaddr(data.endpoint) ?? advertisedEndpoint(peerAddrs);
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
        transportPreference: data.transport_preference ?? "direct",
        cloudflareEnabled: Boolean(data.cloudflare_enabled),
        cloudflareTurnReady: Boolean(data.cloudflare_turn_ready),
        cloudflareTurnEndpoint: data.cloudflare_turn_endpoint ?? null,
        yggReady: Boolean(data.ygg_ready),
        yggAddress: data.ygg_address ?? null,
        yggPublicKey: data.ygg_public_key ?? null,
        yggSubnet: data.ygg_subnet ?? null,
      };
    })
    .filter((server) => Boolean(server.peerId) && (server.peerAddrs.length > 0 || Boolean(server.peerAddr)));

  if (state.selectedServerId && !state.servers.find((server) => server.clientId === state.selectedServerId)) {
    state.selectedServerId = null;
  }
  if (!state.selectedServerId && state.servers.length === 1) {
    state.selectedServerId = state.servers[0].clientId;
  }
  renderServers();
}

function buildPresencePayload(status) {
  const endpoint = advertisedEndpoint(hostSession.listenAddrs, hostSession.peerAddr);
  return {
    room_name: hostSession.roomName,
    host_name: currentNickname(),
    slots: `${Math.max(1, (status?.peerCount ?? 0) + 1)}/30`,
    has_password: hostSession.hasPassword,
    peer_id: hostSession.peerId ?? localClientId,
    listen_addrs: hostSession.listenAddrs,
    endpoint,
    peer_addr: hostSession.peerAddr,
    local_port: hostSession.localPort,
    minecraft_version: hostSession.minecraftVersion ?? status?.minecraftVersion ?? null,
    transport: status?.transportPath ?? state.activeTunnelTransport ?? null,
    transport_preference: hostSession.useCloudflare ? "cloudflare" : "direct",
    cloudflare_enabled: hostSession.useCloudflare,
    cloudflare_turn_ready: Boolean(status?.cloudflareTurnReady),
    cloudflare_turn_endpoint: status?.cloudflareTurnEndpoint ?? null,
    ygg_ready: Boolean(hostSession.yggEnabled && hostSession.yggAddress),
    ygg_address: hostSession.yggAddress ?? null,
    ygg_public_key: hostSession.yggPublicKey ?? null,
    ygg_subnet: hostSession.yggSubnet ?? null,
  };
}

function syncHostSessionFromStatus(status) {
  if (status.mode === "host") {
    hostSession.active = true;
    hostSession.roomName = status.roomCode ?? hostSession.roomName;
    hostSession.hasPassword = Boolean(status.passwordProtected);
    hostSession.useCloudflare = Boolean(status.cloudflareEnabled ?? hostSession.useCloudflare);
    hostSession.peerAddr =
      advertisedEndpoint(hostSession.listenAddrs, status.publicUdpAddr ?? status.udpBindAddr) ?? hostSession.peerAddr;
    hostSession.localPort = status.localGamePort ?? hostSession.localPort;
    hostSession.minecraftVersion = status.minecraftVersion ?? hostSession.minecraftVersion;
    return;
  }
  hostSession.active = false;
  hostSession.peerId = null;
  hostSession.listenAddrs = [];
  hostSession.peerAddr = null;
  hostSession.minecraftVersion = null;
  hostSession.presenceEntered = false;
  hostSession.useCloudflare = false;
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
  publicEndpointEl.textContent = status.publicUdpAddr ?? status.udpBindAddr ?? "n/a";
  sessionModeEl.textContent = formatMode(status.mode);
  currentVersionEl.textContent = status.minecraftVersion ?? t("serverUnknownVersion");
  statusNoteEl.textContent = status.note ?? t("modeIdle");
  renderPeers(status.peers ?? []);
  renderSessionCard();
  renderLogs();
  renderSelectedServer();
  updateHintFromStatus(status);
  if (overlayStatusLineEl) {
    overlayStatusLineEl.textContent =
      status.mode === "client" && state.tunnelReady
        ? `Туннель активен: ${formatTransportLabel(status.transportPath ?? state.activeTunnelTransport)}. Подключайтесь к localhost:25565.`
        : status.mode === "host"
          ? `Комната ${status.roomCode ?? "без имени"} опубликована. Ждём подключения игроков.`
          : `Оверлей скрыт в трее. Открывайте его по ${normalizedShortcut(state.preferences.profile.overlayShortcut)}.`;
  }
  syncProfileSurface();
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
      const relaySessionId = message.data?.relay_session_id ?? null;
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

    await state.privateChannel.subscribe("cloudflare-offer", async (message) => {
      const requester = message.data?.client_id ?? message.clientId ?? "unknown";
      const sessionId = message.data?.session_id;
      const offerJson = message.data?.offer_json;
      const peerAddr = message.data?.peer_addr ?? "unknown";
      if (!sessionId || !offerJson) return;

      try {
        addLog(`Cloudflare offer получен от ${requester}.`);
        const answerJson = await invoke("cloudflare_accept_offer", {
          sessionId,
          offerJson,
          peerAddr,
        });
        await state.realtime.channels.get(`lobby:${requester}`).publish("cloudflare-answer", {
          client_id: localClientId,
          session_id: sessionId,
          answer_json: answerJson,
          peer_addr: hostSession.peerAddr,
        });
        addLog(`Cloudflare answer отправлен клиенту ${requester}.`);
      } catch (error) {
        addLog(`Cloudflare host answer failed: ${String(error)}`);
      }
    });

    await state.privateChannel.subscribe("cloudflare-answer", async (message) => {
      const sessionId = message.data?.session_id;
      const answerJson = message.data?.answer_json;
      if (!sessionId || !answerJson) return;

      try {
        addLog("Cloudflare answer получен от хоста.");
        await invoke("cloudflare_finish_client_answer", { sessionId, answerJson });
      } catch (error) {
        addLog(`Cloudflare client answer apply failed: ${String(error)}`);
        if (state.pendingTransportFlow) {
          await startRelayFallback(state.pendingTransportFlow);
        }
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
    if (shouldEnter) {
      await state.lobbyChannel.presence.enter(payload);
      addLog(t("hostStartedPresence", { room: hostSession.roomName, addr: hostSession.peerAddr }));
      hostSession.presenceEntered = true;
    } else {
      await state.lobbyChannel.presence.update(payload);
      addLog(`Presence updated for ${hostSession.roomName} (${payload.endpoint ?? "n/a"}).`);
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
      `Preflight ${report.reachable ? "OK" : "FAIL"} для 127.0.0.1:${report.localPort}. ${
        report.minecraftVersion ? `Версия: ${report.minecraftVersion}.` : "Версия не определилась."
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
    addLog("Диагностический сервер нельзя запускать на том же порту, что и Minecraft. Используйте отдельный порт, например 25566.");
    return;
  }
  try {
    const info = await invoke("start_test_server", { port });
    state.testServerInfo = info;
    addLog(`Тестовый сервер запущен на ${info.bindAddr}. Протокол: ${info.protocol}.`);
  } catch (error) {
    addLog(`Не удалось запустить тестовый сервер: ${String(error)}`);
  }
}

async function stopEmbeddedTestServer() {
  try {
    await invoke("stop_test_server");
    state.testServerInfo = null;
    addLog("Тестовый сервер остановлен.");
  } catch (error) {
    addLog(`Не удалось остановить тестовый сервер: ${String(error)}`);
  }
}

async function copyDiagnosticsSnapshot() {
  try {
    const localPort = Number(localGamePortEl.value || 25565);
    await invoke("run_network_self_check_command");
    const snapshot = await invoke("export_diagnostics_snapshot", { localPort });
    await navigator.clipboard.writeText(JSON.stringify(snapshot, null, 2));
    addLog("Полная диагностика скопирована в буфер обмена.");
  } catch (error) {
    addLog(`Не удалось выгрузить диагностику: ${String(error)}`);
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
    addLog(`Тестовый сервер ответил с 127.0.0.1:${port}: ${response || "<empty>"}`);
  } catch (error) {
    addLog(`Не удалось подключиться к тестовому серверу на 127.0.0.1:${port}: ${String(error)}`);
  }
}

async function refreshYggstackRuntime({ silent = true } = {}) {
  try {
    const info = await invoke("get_yggstack_runtime_info");
    renderYggstackRuntime(info);
    if (!silent) addLog(`Yggstack status: ${info.note}`);
  } catch (error) {
    if (!silent) addLog(`Не удалось получить статус Yggstack: ${String(error)}`);
  }
}

async function prepareYggstackRuntimeActionLegacy() {
  try {
    const info = await invoke("prepare_yggstack_runtime");
    renderYggstackRuntime(info);
    addLog(`Yggstack runtime подготовлен. Бинарник: ${info.binaryPath ?? "n/a"}`);
  } catch (error) {
    addLog(`Не удалось подготовить Yggstack runtime: ${String(error)}`);
  }
}

async function startYggstackSidecarActionLegacy() {
  try {
    const info = await invoke("start_yggstack_sidecar");
    renderYggstackRuntime(info);
    addLog(`Yggstack sidecar запущен. Лог: ${info.logPath ?? "n/a"}`);
  } catch (error) {
    addLog(`Не удалось запустить Yggstack sidecar: ${String(error)}`);
  }
}

async function stopYggstackSidecarActionLegacy() {
  try {
    const info = await invoke("stop_yggstack_sidecar");
    renderYggstackRuntime(info);
    addLog("Yggstack sidecar остановлен.");
  } catch (error) {
    addLog(`Не удалось остановить Yggstack sidecar: ${String(error)}`);
  }
}

function renderYggstackRuntime(info) {
  state.yggstackInfo = info ?? null;
  if (!yggstackStatusEl) return;
  if (!info) {
    yggstackStatusEl.textContent = "Yggstack: встроенный runtime ещё не проверен.";
    return;
  }

  const embedded = info.binaryPath === "embedded://yggstackbridge";
  let stateLabel = "runtime не готов";
  if (info.running) {
    stateLabel = embedded ? "встроенный bridge запущен" : "runtime запущен";
  } else if (info.ready) {
    stateLabel = embedded ? "встроенный bridge готов" : "runtime готов";
  }

  yggstackStatusEl.textContent = `Yggstack: ${stateLabel}. ${info.note ?? ""}`.trim();
}

async function prepareYggstackRuntimeAction() {
  try {
    const info = await invoke("prepare_yggstack_runtime");
    renderYggstackRuntime(info);
    addLog(`Yggstack готов. Источник: ${info.binaryPath ?? "embedded"}`);
  } catch (error) {
    addLog(`Не удалось подготовить Yggstack: ${String(error)}`);
  }
}

async function startYggstackSidecarAction() {
  try {
    const info = await invoke("start_yggstack_sidecar");
    renderYggstackRuntime(info);
    addLog(
      info.binaryPath === "embedded://yggstackbridge"
        ? "Встроенный Yggstack bridge запущен."
        : `Yggstack runtime запущен. Лог: ${info.logPath ?? "n/a"}`,
    );
  } catch (error) {
    addLog(`Не удалось запустить Yggstack: ${String(error)}`);
  }
}

async function retryYggstackPeersAction() {
  try {
    const info = await invoke("retry_yggstack_peers");
    renderYggstackRuntime(info);
    addLog("Yggstack: инициирован повторный peer-discovery.");
  } catch (error) {
    addLog(`Не удалось обновить peer'ы Yggstack: ${String(error)}`);
  }
}

async function stopYggstackSidecarAction() {
  try {
    const info = await invoke("stop_yggstack_sidecar");
    renderYggstackRuntime(info);
    addLog(
      info.binaryPath === "embedded://yggstackbridge"
        ? "Встроенный Yggstack bridge остановлен."
        : "Yggstack runtime остановлен.",
    );
  } catch (error) {
    addLog(`Не удалось остановить Yggstack: ${String(error)}`);
  }
}

async function ensureYggstackReady({ autoStart = false, silent = false } = {}) {
  try {
    let info = await invoke("get_yggstack_runtime_info");
    renderYggstackRuntime(info);

    if (autoStart && info.ready && !info.running) {
      info = await invoke("start_yggstack_sidecar");
      renderYggstackRuntime(info);
      if (!silent) {
        addLog(
          info.binaryPath === "embedded://yggstackbridge"
            ? "Встроенный Yggstack bridge автоматически запущен."
            : "Yggstack runtime автоматически запущен.",
        );
      }
    }

    return info;
  } catch (error) {
    if (!silent) addLog(`Не удалось проверить Yggstack runtime: ${String(error)}`);
    return null;
  }
}

async function startHosting() {
  if (!canOpenHostModal()) return;
  const roomName = roomNameEl.value.trim();
  if (!roomName) {
    roomNameEl.focus();
    return;
  }

  const localPort = Number(localGamePortEl.value || 25565);
  const password = requirePasswordEl.checked ? roomPasswordEl.value.trim() || null : null;
  let useCloudflare = Boolean(useCloudflareEl.checked);
  hostButtonEl.disabled = true;
  state.tunnelReady = false;
  setMinecraftHint(t("hintWaiting"), false);

  try {
    const preflight = await runPreflightCheck({ silent: false });
    if (!preflight.reachable) {
      addLog("Хост не запущен: локальный Minecraft недоступен. Сначала откройте мир в LAN или запустите тестовый сервер.");
      return;
    }

    const yggInfo = await ensureYggstackReady({ autoStart: true, silent: false });
    hostSession.yggEnabled = Boolean(yggInfo?.ready);
    hostSession.yggAddress = yggInfo?.yggAddress ?? null;
    hostSession.yggPublicKey = yggInfo?.yggPublicKey ?? null;
    hostSession.yggSubnet = yggInfo?.yggSubnet ?? null;

    if (useCloudflare) {
      const runtime = await invoke("get_cloudflare_runtime_info");
      if (!runtime.ready) {
        addLog(`Cloudflare fallback недоступен сейчас: ${runtime.note}`);
        useCloudflare = false;
      } else {
        addLog(`Cloudflare TURN backend готов: ${runtime.turnEndpoint ?? "turn:turn.cloudflare.com:3478"}`);
      }
    }

    const bootstrap = await invoke("start_hosting", { roomName, password, localPort, useCloudflare });
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
    hostSession.useCloudflare = useCloudflare;
    hostSession.presencePayload = null;
    hostSession.presenceEntered = false;
    if (canAdvertiseHost()) {
      await syncPresence(status, { force: true, enter: true });
    } else {
      addLog("Presence отложен: ждём публичный endpoint от relay или reverse tunnel.");
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
    hostSession.minecraftVersion = null;
    hostSession.useCloudflare = false;
    hostSession.yggEnabled = false;
    hostSession.yggAddress = null;
    hostSession.yggPublicKey = null;
    hostSession.yggSubnet = null;
    hostSession.presencePayload = null;
    hostSession.presenceEntered = false;
    state.selectedServerId = null;
    const status = await invoke("get_status");
    renderStatus(status);
    await refreshLobby();
    addLog(t("hostStopped"));
  }
}

async function startCloudflareFallback(flow) {
  if (!flow || flow.cloudflareAttempted) return;
  flow.cloudflareAttempted = true;
  addLog("Direct path не поднялся. Перехожу на Cloudflare TURN/WebRTC.");

  try {
    const runtime = await invoke("get_cloudflare_runtime_info");
    if (!runtime.ready) {
      addLog(`Cloudflare runtime недоступен: ${runtime.note}`);
      await startRelayFallback(flow);
      return;
    }

    const offerJson = await invoke("cloudflare_create_offer", {
      sessionId: flow.cloudflareSessionId,
      peerAddr: flow.server.peerAddr,
    });
    await state.realtime.channels.get(`lobby:${flow.server.clientId}`).publish("cloudflare-offer", {
      client_id: localClientId,
      session_id: flow.cloudflareSessionId,
      peer_addr: flow.server.peerAddr,
      offer_json: offerJson,
    });
    addLog(`Cloudflare offer отправлен хосту ${flow.server.clientId}.`);
  } catch (error) {
    addLog(`Cloudflare fallback failed before answer: ${String(error)}`);
    await startRelayFallback(flow);
  }
}

async function startRelayFallback(flow) {
  if (!flow || flow.relayAttempted) return;
  flow.relayAttempted = true;
  addLog(`Cloudflare не помог. Перехожу на MQTT relay session ${flow.relaySessionId}.`);
  try {
    await invoke("start_relay_fallback", {
      peerId: flow.server.peerId,
      peerAddrs: flow.peerAddrs,
      relaySessionId: flow.relaySessionId,
    });
  } catch (error) {
    addLog(`Relay fallback failed to start: ${String(error)}`);
    state.pendingConnects.delete(flow.server.clientId);
    state.pendingTransportFlow = null;
    renderServers();
  }
}

async function connectToServer(server) {
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
    await ensureYggstackReady({ autoStart: true, silent: true });
    addLog(t("connectProgress", { room: server.roomName, addr: server.peerAddr }));
    const cloudflarePreferred = Boolean(server.cloudflareEnabled && server.cloudflareTurnReady);
    if (server.cloudflareEnabled) {
      addLog(
        `Хост ${server.roomName} помечен как Cloudflare-preferred. Сначала пробуем direct path, затем текущий fallback.`,
      );
    }
    const relaySessionId = `${server.cloudflareEnabled ? "cfrelay" : "relay"}-${crypto.randomUUID()}`;
    const cloudflareSessionId = `cfwebrtc-${crypto.randomUUID()}`;
    const peerAddrs = sortAdvertisedAddrs(
      [...new Set([...(server.peerAddrs ?? []), normalizeToMultiaddr(server.peerAddr)].filter(Boolean))],
    );
    await invoke("connect_to_peer", {
      peerId: server.peerId,
      peerAddrs,
      relaySessionId: cloudflarePreferred ? null : relaySessionId,
      allowRelayFallback: !cloudflarePreferred,
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
      room_name: server.roomName,
      peer_addr: status.publicUdpAddr ?? status.udpBindAddr,
      relay_session_id: relaySessionId,
    });
    state.pendingTransportFlow = {
      server,
      peerAddrs,
      relaySessionId,
      cloudflareSessionId,
      cloudflareAttempted: false,
      relayAttempted: !cloudflarePreferred,
    };
    addLog(t("connectRequestSent", { host: server.clientId }));
  } catch (error) {
    state.pendingConnects.delete(server.clientId);
    setMinecraftHint(t("hintFailed"), false);
    addLog(t("connectFailed", { error: String(error) }));
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
  renderStatus(
    state.status ?? {
      mode: "idle",
      state: "idle",
      roomCode: null,
      udpBindAddr: null,
      publicUdpAddr: null,
      localGamePort: null,
      minecraftVersion: null,
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

await listen("peer_connected", async (event) => {
  state.pendingConnects.clear();
  state.tunnelReady = true;
  state.activeTunnelTransport = event.payload?.relayed ? "relay-circuit" : "direct";
  setMinecraftHint(t("hintConnected"), true);
  addLog(
    `${t("tunnelEstablishedLog", { addr: "localhost:25565" })} (${formatTransportLabel(
      state.activeTunnelTransport,
    )})`,
  );
  const status = await invoke("get_status");
  renderStatus(status);
  await syncPresence(status, { force: true });
  renderServers();
});

await listen("connection_success", async (event) => {
  state.pendingConnects.clear();
  state.tunnelReady = true;
  state.activeTunnelTransport = event.payload?.transport ?? "reverse-tunnel";
  setMinecraftHint(t("hintConnected"), true);
  addLog(
    `${t("tunnelEstablishedLog", { addr: "localhost:25565" })} (${formatTransportLabel(
      state.activeTunnelTransport,
    )})`,
  );
  const status = await invoke("get_status");
  renderStatus(status);
  renderServers();
});

await listen("cloudflare_connecting", async () => {
  addLog("Cloudflare TURN/WebRTC negotiation started.");
});

await listen("cloudflare_connected", async (event) => {
  state.pendingConnects.clear();
  state.tunnelReady = true;
  state.activeTunnelTransport = "cloudflare-webrtc";
  state.pendingTransportFlow = null;
  setMinecraftHint(t("hintConnected"), true);
  addLog(
    `${t("tunnelEstablishedLog", { addr: "localhost:25565" })} (${formatTransportLabel("cloudflare-webrtc")})`,
  );
  const status = await invoke("get_status");
  renderStatus(status);
  renderServers();
});

await listen("cloudflare_failed", async (event) => {
  addLog(`Cloudflare fallback failed: ${event.payload?.reason ?? "unknown error"}`);
  if (state.pendingTransportFlow) {
    await startRelayFallback(state.pendingTransportFlow);
  }
});

await listen("tunnel_failed", async (event) => {
  addLog(`Tunnel failed: ${event.payload?.reason ?? "unknown"}`);
  if (state.pendingTransportFlow?.server?.cloudflareEnabled && state.pendingTransportFlow?.server?.cloudflareTurnReady) {
    await startCloudflareFallback(state.pendingTransportFlow);
    return;
  }
  if (state.pendingTransportFlow) {
    await startRelayFallback(state.pendingTransportFlow);
    return;
  }
  state.pendingConnects.clear();
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
  addLog(`Тестовый сервер готов: ${event.payload?.bindAddr ?? "n/a"} (${event.payload?.protocol ?? "unknown"}).`);
});

await listen("test_server_client_closed", async (event) => {
  addLog(`Тестовый сервер: клиент ${event.payload ?? "unknown"} завершил соединение.`);
});

await listen("relay_active", async (event) => {
  addLog(`Relay active: ${event.payload?.relayAddr ?? "n/a"}`);
});

await listen("hole_punch_success", async (event) => {
  state.activeTunnelTransport = "direct-quic";
  addLog(`Hole punch success for ${event.payload?.peerId ?? "peer"}.`);
});

navHomeEl.addEventListener("click", () => setPage("home"));
navSettingsEl.addEventListener("click", () => setPage("settings"));
openProfileModalEl?.addEventListener("click", () => openProfileModal(false));
openHostModalEl.addEventListener("click", openModal);
closeModalEl.addEventListener("click", closeModal);
closeModalSecondaryEl.addEventListener("click", closeModal);
modalEl.addEventListener("click", (event) => {
  if (event.target instanceof HTMLElement && event.target.dataset.closeModal === "true") closeModal();
});
profileModalEl?.addEventListener("click", (event) => {
  if (event.target instanceof HTMLElement && event.target.dataset.closeProfileModal === "true") {
    closeProfileModal();
  }
});
closeProfileModalEl?.addEventListener("click", closeProfileModal);
closeProfileModalSecondaryEl?.addEventListener("click", closeProfileModal);
hideOverlayButtonEl?.addEventListener("click", async () => {
  await appWindow.hide();
});

requirePasswordEl.addEventListener("change", syncPasswordField);
hostButtonEl.addEventListener("click", startHosting);
stopButtonEl.addEventListener("click", stopSession);
refreshLobbyEl.addEventListener("click", async () => {
  await safeDetachChannel(state.lobbyChannel);
  safeReleaseChannel(LOBBY_CHANNEL_NAME);
  safeReleaseChannel(`lobby:${localClientId}`);
  state.lobbyChannel = null;
  state.privateChannel = null;
  await bindChannelHandlers();
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
  if (!selected?.peerAddr) return;
  await navigator.clipboard.writeText(selected.peerAddr);
  addLog(t("copiedIp"));
});
runPreflightEl.addEventListener("click", async () => {
  await runPreflightCheck();
});
startTestServerEl.addEventListener("click", async () => {
  await startEmbeddedTestServer();
});
stopTestServerEl.addEventListener("click", async () => {
  await stopEmbeddedTestServer();
});
probeTestServerEl.addEventListener("click", async () => {
  await probeEmbeddedTestServer();
});
prepareYggstackEl?.addEventListener("click", async () => {
  await prepareYggstackRuntimeAction();
});
startYggstackEl?.addEventListener("click", async () => {
  await startYggstackSidecarAction();
});
retryYggstackEl?.addEventListener("click", async () => {
  await retryYggstackPeersAction();
});
stopYggstackEl?.addEventListener("click", async () => {
  await stopYggstackSidecarAction();
});
connectSelectedEl.addEventListener("click", async () => {
  const selected = getSelectedServer();
  if (selected) await connectToServer(selected);
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
saveProfileButtonEl?.addEventListener("click", async () => {
  await persistProfileFromFields(settingsNicknameEl, settingsShortcutInputEl, settingsAvatarInputEl);
});
saveProfileModalEl?.addEventListener("click", async () => {
  await persistProfileFromFields(profileNicknameInputEl, profileShortcutInputEl, profileAvatarInputEl, {
    closeAfter: true,
  });
});
clearAvatarButtonEl?.addEventListener("click", async () => {
  state.preferences.profile.avatarDataUrl = null;
  if (settingsAvatarInputEl) settingsAvatarInputEl.value = "";
  if (profileAvatarInputEl) profileAvatarInputEl.value = "";
  saveProfile();
  syncProfileSurface();
  addLog("Аватар очищен.");
});
settingsAvatarInputEl?.addEventListener("change", async () => {
  const file = settingsAvatarInputEl.files?.[0];
  if (!file) return;
  state.preferences.profile.avatarDataUrl = await fileToDataUrl(file);
  syncProfileSurface();
});
profileAvatarInputEl?.addEventListener("change", async () => {
  const file = profileAvatarInputEl.files?.[0];
  if (!file) return;
  state.preferences.profile.avatarDataUrl = await fileToDataUrl(file);
  syncProfileSurface();
});

document.addEventListener("keydown", (event) => {
  if (event.key !== "Escape") return;
  if (!modalEl.classList.contains("hidden")) {
    closeModal();
    return;
  }
  if (profileModalEl && !profileModalEl.classList.contains("hidden")) {
    closeProfileModal();
    return;
  }
  void appWindow.hide();
});

await hydrateProfile();
document.body.dataset.theme = state.preferences.theme;
applyTranslations();
renderSettingsOptions();
syncPasswordField();
syncProfileSurface();
renderLogs();
renderSelectedServer();
renderSessionCard();
syncButtons();
setPage("home");

setInterval(() => {
  void pollStatus();
}, POLL_INTERVAL_MS);

await setupAbly();
await refreshYggstackRuntime({ silent: true });
await pollStatus();
