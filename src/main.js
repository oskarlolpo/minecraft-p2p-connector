async function copyTextToClipboard(text) {
  if (navigator.clipboard && window.isSecureContext) {
    try {
      await navigator.clipboard.writeText(text);
      return;
    } catch (err) {}
  }
  const textArea = document.createElement('textarea');
  textArea.value = text;
  textArea.style.position = 'fixed';
  textArea.style.top = '0';
  textArea.style.left = '0';
  document.body.appendChild(textArea);
  textArea.focus();
  textArea.select();
  try {
    document.execCommand('copy');
  } catch (err) { console.error(err); }
  document.body.removeChild(textArea);
}

import * as Ably from "ably";
import { listen as tauriListen } from "@tauri-apps/api/event";
import { invoke as tauriInvoke } from "@tauri-apps/api/core";
import { supabase } from "./supabase.js";

const isTauri = !!(window.__TAURI_INTERNALS__ || (window.__tauri && window.__tauri.invoke));
const listen = isTauri ? tauriListen : () => new Promise(() => {});
const invoke = isTauri ? tauriInvoke : async (cmd, args) => {
  console.warn(`[Tauri Mock] invoke("${cmd}") called outside Tauri environment`, args);
  if (cmd === "get_status") {
    return {
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
      note: "Запущено вне Tauri",
      lastError: null,
      signalingServer: "Mock",
      logs: [],
    };
  }
  return null;
};

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
const activeSessionPanelEl = document.querySelector("#active-session-panel");
const navHomeEl = document.querySelector("#nav-home");
const navSettingsEl = document.querySelector("#nav-settings");
const pageHomeEl = document.querySelector("#page-home");
const pageSettingsEl = document.querySelector("#page-settings");
const pageProfileEl = document.querySelector("#page-profile");
const portHelpEl = document.querySelector("#port-help");
const brandUserNameEl = document.querySelector("#brand-user-name");
const brandAvatarImageEl = document.querySelector("#brand-avatar-image");
const brandAvatarFallbackEl = document.querySelector("#brand-avatar-fallback");
const profileMenuTriggerEl = document.querySelector("#profile-menu-trigger");
const profileNicknameInputEl = document.querySelector("#profile-nickname-input");
const profileAvatarFileEl = document.querySelector("#profile-avatar-file");
const profileBannerFileEl = document.querySelector("#profile-banner-file");
const editBannerBtnEl = document.querySelector("#edit-banner-btn");
const editAvatarBtnEl = document.querySelector("#edit-avatar-btn");
const editNameBtnEl = document.querySelector("#edit-name-btn");
const socialBannerImgEl = document.querySelector("#social-banner-img");
const settingsAuthEmailEl = document.querySelector("#settings-auth-email");
const settingsAuthProviderEl = document.querySelector("#settings-auth-provider");
const settingsVersionEl = document.querySelector("#settings-version");
const checkUpdatesEl = document.querySelector("#check-updates");
const installUpdateEl = document.querySelector("#install-update");
const updateStatusEl = document.querySelector("#update-status");
const externalHostModeEl = document.querySelector("#external-host-mode");
const externalHostAddressFieldEl = document.querySelector("#external-host-address-field");
const externalHostAddressEl = document.querySelector("#external-host-address");
const hostProgressContainerEl = document.querySelector("#host-progress-container");
const hostProgressTextEl = document.querySelector("#host-progress-text");

const playerModalEl = document.querySelector("#player-modal");
const closePlayerModalEl = document.querySelector("#close-player-modal");
const closePlayerModalSecondaryEl = document.querySelector("#close-player-modal-secondary");
const kickButtonEl = document.querySelector("#kick-button");
const pingCanvas = document.querySelector("#ping-graph");
const ctx = pingCanvas?.getContext("2d");

const authOverlayEl = document.querySelector("#auth-overlay");
const authFormEl = document.querySelector("#auth-form");
const authEmailEl = document.querySelector("#auth-email");
const authPasswordEl = document.querySelector("#auth-password");
const btnAuthSubmitEl = document.querySelector("#btn-auth-submit");
const btnToggleModeEl = document.querySelector("#btn-toggle-mode");
const btnGoogleEl = document.querySelector("#btn-google");
const btnGuestEl = document.querySelector("#btn-guest");
let originalGoogleBtnHtml = btnGoogleEl ? btnGoogleEl.innerHTML : "";
const authTitleEl = document.querySelector("#auth-title");
const authSubtitleEl = document.querySelector("#auth-subtitle");
const authErrorEl = document.querySelector("#auth-error-message");
const profileAvatarPreviewEl = document.querySelector("#profile-avatar-preview");
const profileAvatarLetterEl = document.querySelector("#profile-avatar-letter");
const profileDisplayNameEl = document.querySelector("#profile-display-name");
const profileUserIdEl = document.querySelector("#profile-user-id");
const profileIdInputEl = document.querySelector("#profile-id-input");
const editIdBtnEl = document.querySelector("#edit-id-btn");
const nameConfirmActionsEl = document.querySelector("#name-confirm-actions");
const idConfirmActionsEl = document.querySelector("#id-confirm-actions");
const saveNameBtnEl = document.querySelector("#save-name-btn");
const cancelNameBtnEl = document.querySelector("#cancel-name-btn");
const saveIdBtnEl = document.querySelector("#save-id-btn");
const cancelIdBtnEl = document.querySelector("#cancel-id-btn");
const profileAuthEmailEl = document.querySelector("#profile-auth-email");
const profileAuthProviderEl = document.querySelector("#profile-auth-provider");
const btnLogoutEl = document.querySelector("#btn-logout");

// Store the real display ID (not from textContent which may show "Скопировано!")
let storedDisplayId = null;
let currentEditingField = null; // 'name' | 'id' | null

let activeModalPeerId = null;

const portChoiceModalEl = document.querySelector("#port-choice-modal");
const closePortModalEl = document.querySelector("#close-port-modal");
const closePortModalSecondaryEl = document.querySelector("#close-port-modal-secondary");
const detectedPortsListEl = document.querySelector("#detected-ports-list");
const clearIgnoredPortsEl = document.querySelector("#clear-ignored-ports");
const ignoredPortsListEl = document.querySelector("#ignored-ports-list");

const PROFILE_STORAGE_KEY = "minecraft-p2p-profile-v1";
const EXTERNAL_SERVERS_STORAGE_KEY = "minecraft-p2p-external-servers-v1";
const IGNORED_PORTS_STORAGE_KEY = "minecraft-p2p-ignored-ports-v1";
const SETTINGS_ACCENT_KEY = "minecraft-p2p-accent";

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
  pendingRelayAcks: new Map(),
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
  ignoredPorts: loadIgnoredPorts(),
  pingHistory: new Map(), // For graph
};



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
    const customId = typeof parsed?.customId === "string" ? parsed.customId : null;
    const customBanner = typeof parsed?.customBanner === "string" ? parsed.customBanner : null;
    return { nickname, avatarDataUrl, customId, customBanner };
  } catch {
    return { nickname: "Player", avatarDataUrl: null, customId: null, customBanner: null };
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
    accent: localStorage.getItem(SETTINGS_ACCENT_KEY) || "blue",
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

function parseMinecraftColors(text) {
  if (!text) return "";
  const escaped = escapeHtml(text);
  const colorMap = {
    '0': '#000000', '1': '#0000AA', '2': '#00AA00', '3': '#00AAAA',
    '4': '#AA0000', '5': '#AA00AA', '6': '#FFAA00', '7': '#AAAAAA',
    '8': '#555555', '9': '#5555FF', 'a': '#55FF55', 'b': '#55FFFF',
    'c': '#FF5555', 'd': '#FF55FF', 'e': '#FFFF55', 'f': '#FFFFFF'
  };
  
  let result = "";
  let currentClasses = [];
  let currentColor = null;
  
  const parts = escaped.split(/([&§][0-9a-fklmno r])/gi);
  
  for (let part of parts) {
    if (part.match(/^[&§][0-9a-fklmno r]$/i)) {
      const code = part[1].toLowerCase();
      if (colorMap[code]) {
        currentColor = colorMap[code];
        currentClasses = []; // Colors reset formatting in MC
      } else if (code === 'r') {
        currentColor = null;
        currentClasses = [];
      } else {
        const classMap = { 'l': 'mc-bold', 'm': 'mc-strikethrough', 'n': 'mc-underline', 'o': 'mc-italic', 'k': 'mc-obfuscated' };
        if (classMap[code]) currentClasses.push(classMap[code]);
      }
    } else {
      let style = currentColor ? `color: ${currentColor};` : "";
      let className = currentClasses.join(" ");
      result += `<span style="${style}" class="${className}">${part}</span>`;
    }
  }
  return result;
}

function escapeHtml(text) {
  const div = document.createElement('div');
  div.textContent = text;
  return div.innerHTML;
}

function t(key, variables = {}) {
  const dictionary = I18N[state.preferences.language] ?? I18N.ru;
  const template = dictionary[key] ?? I18N.ru[key] ?? key;
  return template.replaceAll(/\{(\w+)\}/g, (_, name) => String(variables[name] ?? `{${name}}`));
}

function applyTranslations() {
  document.title = t("appTitle");
  document.querySelectorAll("[data-i18n]").forEach((element) => {
    const key = element.dataset.i18n;
    if (key) element.innerHTML = t(key);
  });
  document.querySelectorAll("[data-i18n-placeholder]").forEach((element) => {
    const key = element.dataset.i18nPlaceholder;
    if (key) element.placeholder = t(key);
  });
  if (navHomeEl) navHomeEl.setAttribute("aria-label", t("homeAria"));
  if (navSettingsEl) navSettingsEl.setAttribute("aria-label", t("settingsAria"));
  if (closeModalEl) closeModalEl.setAttribute("aria-label", t("closeAria"));
  profileMenuTriggerEl?.setAttribute("aria-label", t("avatarButtonAria"));
  if (portHelpEl) portHelpEl.title = t("portHelpTitle");
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
  if (hostButtonEl) hostButtonEl.textContent = t(external ? "modalExternalButton" : "modalHostButton");
}

function renderProfile() {
  const nickname = state.profile.nickname?.trim() || "Player";
  if (brandUserNameEl) brandUserNameEl.textContent = nickname;
  
  if (profileNicknameInputEl && currentEditingField !== "name") {
    profileNicknameInputEl.value = nickname;
  }

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

  // Profile Page Elements (for instant loading without flickering)
  if (profileDisplayNameEl) profileDisplayNameEl.textContent = nickname;
  
  const displayId = state.profile.customId || "—";
  if (profileUserIdEl) profileUserIdEl.textContent = displayId !== "—" ? `@${displayId}` : "ID: —";
  
  if (profileIdInputEl && currentEditingField !== "id") {
    profileIdInputEl.value = displayId !== "—" ? displayId : "";
  }
  storedDisplayId = displayId !== "—" ? displayId : null;

  // Large Avatar on profile page
  if (state.profile.avatarDataUrl) {
    if (profileAvatarPreviewEl) {
      profileAvatarPreviewEl.src = state.profile.avatarDataUrl;
      profileAvatarPreviewEl.classList.remove("hidden");
    }
    if (profileAvatarLetterEl) profileAvatarLetterEl.classList.add("hidden");
  } else {
    if (profileAvatarPreviewEl) {
      profileAvatarPreviewEl.removeAttribute("src");
      profileAvatarPreviewEl.classList.add("hidden");
    }
    if (profileAvatarLetterEl) {
      profileAvatarLetterEl.textContent = nickname.slice(0, 1).toUpperCase();
      profileAvatarLetterEl.classList.remove("hidden");
    }
  }

  // Large Banner on profile page
  if (state.profile.customBanner) {
    if (socialBannerImgEl) {
      socialBannerImgEl.src = state.profile.customBanner;
      socialBannerImgEl.classList.remove("hidden");
    }
  } else {
    if (socialBannerImgEl) {
      socialBannerImgEl.removeAttribute("src");
      socialBannerImgEl.classList.add("hidden");
    }
  }
}


async function pickAvatarFile() {
  if (profileAvatarFileEl) profileAvatarFileEl.click();
}

async function handleAvatarChosen() {
  const file = profileAvatarFileEl?.files?.[0];
  if (!file) return;
  const reader = new FileReader();
  reader.onload = () => {
    state.profile.avatarDataUrl = String(reader.result || "");
    renderProfile();
  };
  reader.readAsDataURL(file);
}

function saveProfileFromInputs() {
  if (!profileNicknameEl) return;
  state.profile.nickname = profileNicknameEl.value.trim() || "Player";
  saveProfileState();
  renderProfile();
  addLog(t("profileSaved"));
  toggleProfileMenu(false);
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
    item.querySelector(".danger-button").onclick = (e) => {
      e.stopPropagation();
      if (!state.ignoredPorts.includes(det.port)) {
        state.ignoredPorts.push(det.port);
        saveIgnoredPorts();
      }
      // Instant removal from UI
      item.style.opacity = "0";
      item.style.transform = "translateX(20px)";
      setTimeout(() => {
        item.remove();
        if (detectedPortsListEl.children.length === 0) {
          closePortChoiceModal();
        }
      }, 150);
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
    ignoredPortsListEl.innerHTML = `<div class="empty-state">Список пуст</div>`;
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
    let detectedName = String(probe?.roomName || probe?.description || "").trim();
    if (!detectedName) return;

    // Clean up color codes for the input field
    detectedName = detectedName.replace(/[&§][0-9a-fklmno r]/gi, "").trim();

    // Strip "Nickname - " prefix
    const dashIndex = detectedName.indexOf(" - ");
    if (dashIndex !== -1 && dashIndex < 20) {
      detectedName = detectedName.slice(dashIndex + 3).trim();
    }

    if (!roomNameEl.value.trim() || roomNameEl.dataset.autofilled === "true") {
      roomNameEl.value = detectedName;
      roomNameEl.dataset.autofilled = "true";
    }
  } catch {
    // Ignore
  }
}

function renderSettingsOptions() {
  document.querySelectorAll("[data-theme-value]").forEach((button) => {
    button.classList.toggle("active", button.dataset.themeValue === state.preferences.theme);
  });
  document.querySelectorAll("[data-language-value]").forEach((button) => {
    button.classList.toggle("active", button.dataset.languageValue === state.preferences.language);
  });
  document.querySelectorAll(".accent-btn").forEach((button) => {
    button.classList.toggle("active", button.dataset.accent === state.preferences.accent);
  });
}

function applyTheme(theme) {
  state.preferences.theme = theme;
  document.body.dataset.theme = theme;
  savePreference(SETTINGS_THEME_KEY, theme);
  renderSettingsOptions();
}

function applyAccent(accent) {
  state.preferences.accent = accent;
  document.documentElement.style.setProperty("--accent", `var(--accent-${accent})`);
  document.documentElement.style.setProperty("--accent-strong", `color-mix(in srgb, var(--accent-${accent}) 80%, white)`);
  savePreference(SETTINGS_ACCENT_KEY, accent);
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
  pageHomeEl?.classList.toggle("page-active", page === "home");
  pageSettingsEl?.classList.toggle("page-active", page === "settings");
  pageProfileEl?.classList.toggle("page-active", page === "profile");
  navHomeEl?.classList.toggle("nav-button-active", page === "home");
  navSettingsEl?.classList.toggle("nav-button-active", page === "settings");
}

const getVersion = window.__TAURI_INTERNALS__
  ? () => import("@tauri-apps/api/app").then(m => m.getVersion())
  : (window.__TAURI__?.app?.getVersion || (() => Promise.reject("Not in Tauri")));

async function loadAppInfo() {
  try {
    const version = await getVersion();
    if (settingsVersionEl) settingsVersionEl.textContent = version;
    if (currentVersionEl) currentVersionEl.textContent = version;
  } catch {
    try {
      const info = await invoke("get_app_info");
      if (settingsVersionEl) settingsVersionEl.textContent = info.version;
      if (currentVersionEl) currentVersionEl.textContent = info.version;
    } catch {
      if (settingsVersionEl) settingsVersionEl.textContent = "0.3.38";
      if (currentVersionEl) currentVersionEl.textContent = "0.3.38";
    }
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

window.decodeMojibakeIfNeeded = function(t) { return t; };
function decodeMojibakeIfNeeded(t) { return t; }
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
  if (minecraftTargetHintEl) {
    minecraftTargetHintEl.textContent = text;
    minecraftTargetHintEl.classList.toggle("active", active);
  }
}

function syncPasswordField() {
  passwordFieldGroupEl.classList.toggle("hidden", !requirePasswordEl.checked);
  if (!requirePasswordEl.checked) roomPasswordEl.value = "";
}

function openModal() {
  if (!canOpenHostModal()) return;
  modalEl.classList.remove("hidden");
  modalEl.setAttribute("aria-hidden", "false");
  autoDetectLocalGamePort().then(() => autofillRoomNameFromLocalServer());
  setTimeout(() => roomNameEl.focus(), 30);
}

function closeModal() {
  modalEl.classList.add("hidden");
  modalEl.setAttribute("aria-hidden", "true");
  if (hostProgressContainerEl) hostProgressContainerEl.classList.add("hidden");
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
  return transport ?? "unknown transport";
}

function getTransportIcon(transport) {
  if (transport === "direct" || transport === "direct-quic" || transport === "direct-hole-punch") {
    // Lightning icon for direct
    return `<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" title="${escapeHtml(formatTransportLabel(transport))}"><path d="M13 2L3 14h9l-1 8 10-12h-9l1-8z"></path></svg>`;
  }
  if (transport === "relay-circuit" || transport === "relay-reservation") {
    // Cloud icon for relay
    return `<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" title="${escapeHtml(formatTransportLabel(transport))}"><path d="M17.5 19H9a7 7 0 1 1 6.71-9h1.79a4.5 4.5 0 1 1 0 9Z"></path></svg>`;
  }
  return '';
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
      transport: "unknown transport",
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
    return `/ip4/${host}/udp/${port}/quic-v1`;
  }
  if (host.includes(":")) {
    return `/ip6/${host}/udp/${port}/quic-v1`;
  }
  return `/dns4/${host}/udp/${port}/quic-v1`;
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
  if (parts.length >= 5 && parts[1] === "ip4" && (parts[3] === "tcp" || parts[3] === "udp")) {
    return `${parts[2]}:${parts[4]}`;
  }
  if (parts.length >= 5 && parts[1] === "ip6" && (parts[3] === "tcp" || parts[3] === "udp")) {
    return `[${parts[2]}]:${parts[4]}`;
  }
  if (parts.length >= 5 && parts[1] === "dns4" && (parts[3] === "tcp" || parts[3] === "udp")) {
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



function getVerifiedPublicJoinAddress(status = state.status) {
  return hostSession.publicJoinAddress ?? status?.publicJoinAddress ?? null;
}

function getDirectAdvertisedEndpoint(status = state.status) {
  const endpoint = advertisedEndpoint(hostSession.listenAddrs, hostSession.peerAddr);
  return toSocketEndpoint(endpoint) ?? endpoint ?? status?.publicUdpAddr ?? null;
}

function getAdvertisableJoinAddress(status = state.status) {
  const addr = getVerifiedPublicJoinAddress(status) ?? getDirectAdvertisedEndpoint(status);
  if (addr && String(addr).includes("0.0.0.0")) return null;
  return addr;
}

function formatPrimaryEndpoint(status = state.status) {
  const endpoint = getAdvertisableJoinAddress(status);
  if (endpoint && !endpoint.startsWith("0.0.0.0")) return endpoint;
  
  const fallback = status?.publicUdpAddr ?? status?.udpBindAddr ?? "n/a";
  return fallback.startsWith("0.0.0.0") ? "Только через Relay (не публичный)" : fallback;
}

function renderSelectedServer() {
  const selected = getSelectedServer();
  const bedrockEndpoint =
    selected?.bedrockEndpoint ?? (selected ? deriveBedrockEndpoint(selected.peerAddr, selected.bedrockPort) : null);
  const javaEndpoint =
    selected?.publicJoinAddress ??
    selected?.joinAddress ??
    (selected?.peerAddr ? toSocketEndpoint(selected.peerAddr) ?? selected.peerAddr : null);
  
  if (selectedServerEl) selectedServerEl.textContent = selected ? selected.roomName : t("noSelection");
  if (selectedEndpointEl) selectedEndpointEl.textContent = javaEndpoint ?? "n/a";
  if (selectedBedrockEndpointEl) selectedBedrockEndpointEl.textContent = bedrockEndpoint ?? "n/a";
  if (selectedMetaEl) {
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
}

function syncButtons() {
  const mode = state.status?.mode ?? "idle";
  const currentState = state.status?.state ?? "idle";
  const busy = ["starting", "connecting", "punching", "waitingForPeer"].includes(currentState);
  const selected = getSelectedServer();
  const clientLocked = isClientLocked();

  if (openHostModalEl) {
    openHostModalEl.disabled = busy || mode === "host" || clientLocked;
    openHostModalEl.classList.toggle("hidden", clientLocked);
  }
  if (hostButtonEl) {
    hostButtonEl.disabled = busy || mode === "host" || clientLocked;
    hostButtonEl.textContent = busy
      ? t("connectBusyButton")
      : externalHostModeEl?.checked
        ? t("modalExternalButton")
        : t("modalHostButton");
  }
  if (stopButtonEl) {
    stopButtonEl.disabled = mode === "idle";
    stopButtonEl.textContent = mode === "client" ? t("disconnectButton") : t("stopButton");
    stopButtonEl.classList.toggle("danger-active", mode !== "idle");
  }
  if (activeSessionPanelEl) {
    activeSessionPanelEl.classList.toggle("hidden", mode === "idle");
  }
  if (refreshLobbyEl) {
    refreshLobbyEl.disabled = state.realtime?.connection.state === "connecting";
  }
  if (connectSelectedEl) {
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
  }
  if (copySelectedEndpointEl) {
    copySelectedEndpointEl.disabled = !(selected?.publicJoinAddress || selected?.joinAddress || selected?.peerAddr);
  }
  if (copySelectedBedrockEndpointEl) {
    copySelectedBedrockEndpointEl.disabled = !deriveBedrockEndpoint(selected?.peerAddr, selected?.bedrockPort);
  }
  if (hostLockNoteEl) {
    hostLockNoteEl.textContent = clientLocked
      ? t("hostNoteClientLocked")
      : mode === "host"
        ? t("hostNoteHosting")
        : t("hostNoteIdle");
  }

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

  if (mode === "client") {
    hostSectionTitleEl.textContent = t("clientSessionTitle");
    activeHostCardEl.className = "active-host-card";
    
    // We get the local address from the tunnel, but typically it's localhost:25565
    // and is stored in hint or can just be written
    const isReady = state.tunnelReady || status?.state === "connected";
    
    if (isReady) {
      activeHostCardEl.innerHTML = `
      <div class="active-host-layout" style="background: rgba(0, 255, 0, 0.1); border: 1px solid var(--accent); padding: 16px; border-radius: 8px;">
        <div class="host-avatar" style="background: var(--accent); color: white;">✅</div>
        <div class="host-details">
          <h3 style="margin-top: 0; color: var(--accent);">Вы успешно подключены к другу!</h3>
          <p style="margin-bottom: 12px;">Заходите в Minecraft и подключайтесь по этому адресу:</p>
          <div class="ip-box" style="display: flex; gap: 8px; align-items: center; background: rgba(0,0,0,0.2); padding: 8px; border-radius: 4px;">
              <strong style="font-size: 1.2em; font-family: monospace; user-select: all;">127.0.0.1:25565</strong>
              <button onclick="copyTextToClipboard('127.0.0.1:25565')" class="ghost-button">📋 Копировать</button>
          </div>
          <div class="host-meta-row" style="margin-top: 12px;">
            <span class="host-meta-pill">${escapeHtml(status?.peers?.[0]?.pingMs == null ? "Ping: n/a" : `Ping: ${status.peers[0].pingMs} ms`)}</span>
          </div>
        </div>
      </div>
      `;
    } else {
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
    }
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
        <h3>${parseMinecraftColors(hostSession.roomName)}</h3>
        <p>${escapeHtml(t("hostCardPlayers", { count: `${online}/${maxPlayers}` }))}</p>
        <div class="host-meta-row">
          <span class="host-meta-pill">${escapeHtml(t("hostCardVersion", { version }))}</span>
          <span class="host-meta-pill">${escapeHtml(t("hostCardPort", { port: hostSession.localPort }))}</span>
          <span class="host-meta-pill">${escapeHtml(hostSession.hasPassword ? t("hostCardPasswordOn") : t("hostCardPasswordOff"))}</span>
          ${
            status?.geyserEnabled && hostBedrockEndpoint
              ? `<span class="host-meta-pill">${escapeHtml(t("hostCardGeyser", { endpoint: hostBedrockEndpoint }))}</span>`
              : ""
          }
        </div>
        <div class="active-host-tools">
          <button class="ghost-button" type="button" data-copy-host-java="true">${escapeHtml(t("copyIpButton"))} (local)</button>
          ${publicJavaEndpoint ? `<button class="primary-button" type="button" data-copy-host-public="${escapeHtml(toSocketEndpoint(publicJavaEndpoint) ?? publicJavaEndpoint)}">${escapeHtml(t("copyIpButton"))} (public)</button>` : ""}
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

  const cards = [];
  if (hostMode && hostSession.active) {
    const nickname = state.profile.nickname?.trim() || "Player";
    const mcNick = state.detectedMinecraftNickname ?? "n/a";
    const avatarChar = nickname.slice(0, 1).toUpperCase();
    
    cards.push(`
      <div class="player-card host-card" onclick="openPlayerModal('host')">
        <div class="player-avatar">${avatarChar}</div>
        <div class="player-info">
          <div class="player-name">
            <strong>${parseMinecraftColors(nickname)}</strong>
            <span class="row-chip">${t("hostBadge")}</span>
          </div>
          <div class="player-meta">${parseMinecraftColors(mcNick)}</div>
        </div>
      </div>
    `);
  }

  [...peers, ...inferredPlayers].forEach((peer) => {
    const profile = state.peerProfiles.get(peer.peerId) ?? null;
    const name = peer.inferredName || profile?.nickname || peer.peerId;
    const mcNick = profile?.minecraftNickname || peer.inferredName || "n/a";
    const avatarChar = name.slice(0, 1).toUpperCase();
    const ping = peer.pingMs == null ? "n/a" : `${peer.pingMs} ms`;
    const pingClass = peer.pingMs > 200 ? "danger" : peer.pingMs > 100 ? "warning" : "";
    const transportIcon = peer.transport && !peer.inferred 
        ? `<span class="transport-icon" style="margin-right: 4px; display: inline-flex; vertical-align: text-bottom; opacity: 0.7;">${getTransportIcon(peer.transport)}</span>`
        : "";

    // Track ping for graph
    if (peer.peerId && peer.pingMs != null) {
      if (!state.pingHistory.has(peer.peerId)) state.pingHistory.set(peer.peerId, []);
      const history = state.pingHistory.get(peer.peerId);
      history.push(peer.pingMs);
      if (history.length > 40) history.shift();
    }

    cards.push(`
      <div class="player-card" onclick="openPlayerModal('${peer.peerId}')">
        <div class="player-avatar">${avatarChar}</div>
        <div class="player-info">
          <div class="player-name">
            <strong>${parseMinecraftColors(name)}</strong>
          </div>
          <div class="player-meta">
            ${transportIcon}<span>${parseMinecraftColors(mcNick)}</span>
            <span class="ping-tag ${pingClass}">${ping}</span>
          </div>
        </div>
      </div>
    `);
  });

  peerListEl.innerHTML = cards.join("");
}

function openPlayerModal(peerId) {
  activeModalPeerId = peerId;
  const isHost = peerId === 'host';
  const peer = isHost ? null : (state.status?.peers?.find(p => p.peerId === peerId) || buildInferredPlayers(state.status?.peers || []).find(p => p.peerId === peerId));
  
  const profile = isHost ? {
    nickname: state.profile.nickname,
    minecraftNickname: state.detectedMinecraftNickname,
    launcher: state.runtimeFingerprint?.launcher,
    minecraftVersion: state.runtimeFingerprint?.minecraftVersion || state.status?.minecraftVersion,
    modLoader: state.runtimeFingerprint?.modLoader
  } : state.peerProfiles.get(peerId);

  document.querySelector("#player-modal-title").innerHTML = parseMinecraftColors(isHost ? state.profile.nickname : (profile?.nickname || peerId));
  document.querySelector("#player-modal-avatar").textContent = (isHost ? state.profile.nickname : (profile?.nickname || peerId)).slice(0, 1).toUpperCase();
  
  const mcNick = isHost ? state.detectedMinecraftNickname : (profile?.minecraftNickname || peer?.inferredName);
  document.querySelector("#player-modal-mc-nick").innerHTML = mcNick ? parseMinecraftColors(mcNick) : "—";
  
  document.querySelector("#player-modal-launcher").textContent = profile?.launcher || "—";
  document.querySelector("#player-modal-version").textContent = profile?.minecraftVersion || peer?.version || "—";
  
  const loaderEl = document.querySelector("#player-modal-loader");
  const loader = profile?.modLoader || "—";
  loaderEl.textContent = (loader === "—" || loader.toLowerCase() === "unknown") ? "Нераспознан" : loader;
  loaderEl.className = (loader === "—" || loader.toLowerCase() === "unknown") ? "loader-unknown" : "";

  const pingEl = document.querySelector("#player-modal-ping");
  pingEl.textContent = isHost ? "0 ms" : (peer?.pingMs == null ? "n/a" : `${peer.pingMs} ms`);

  kickButtonEl.classList.toggle("hidden", isHost || !state.status || state.status.mode !== "host" || (peer && peer.inferred));
  
  playerModalEl.classList.remove("hidden");
  
  // Animation for the graph
  if (state.pingGraphInterval) clearInterval(state.pingGraphInterval);
  state.pingGraphInterval = setInterval(() => {
    updatePingGraph(activeModalPeerId);
  }, 100);
}

function mcNickStripped(nick) {
  if (!nick) return null;
  return nick.replace(/[§&][0-9a-fklmno r]/gi, "");
}

function updatePingGraph(peerId) {
  if (!ctx || !pingCanvas) return;
  const history = state.pingHistory.get(peerId) || [];
  
  // Set resolution
  pingCanvas.width = pingCanvas.offsetWidth * window.devicePixelRatio;
  pingCanvas.height = pingCanvas.offsetHeight * window.devicePixelRatio;
  ctx.scale(window.devicePixelRatio, window.devicePixelRatio);

  const w = pingCanvas.offsetWidth;
  const h = pingCanvas.offsetHeight;
  ctx.clearRect(0, 0, w, h);

  if (history.length < 2) {
    ctx.fillStyle = "rgba(255,255,255,0.1)";
    ctx.font = "12px sans-serif";
    ctx.fillText("Waiting for data...", w/2 - 40, h/2);
    return;
  }

  const maxPing = Math.max(...history, 100);
  const padding = 10;
  const step = (w - padding * 2) / (history.length - 1);

  // Gradient — read accent from CSS variable
  const accentHex = getComputedStyle(document.body).getPropertyValue("--accent").trim();
  const _r = parseInt(accentHex.slice(1,3),16), _g = parseInt(accentHex.slice(3,5),16), _b = parseInt(accentHex.slice(5,7),16);
  const grad = ctx.createLinearGradient(0, 0, 0, h);
  grad.addColorStop(0, `rgba(${_r}, ${_g}, ${_b}, 0.4)`);
  grad.addColorStop(1, `rgba(${_r}, ${_g}, ${_b}, 0)`);

  ctx.beginPath();
  ctx.moveTo(padding, h);
  
  for (let i = 0; i < history.length; i++) {
    const x = padding + i * step;
    const y = h - (history[i] / maxPing) * (h - padding * 2) - padding;
    if (i === 0) ctx.lineTo(x, y);
    else {
      // Smooth curve
      const prevX = padding + (i - 1) * step;
      const prevY = h - (history[i-1] / maxPing) * (h - padding * 2) - padding;
      const cpX = (prevX + x) / 2;
      ctx.bezierCurveTo(cpX, prevY, cpX, y, x, y);
    }
  }
  
  ctx.lineTo(padding + (history.length - 1) * step, h);
  ctx.closePath();
  ctx.fillStyle = grad;
  ctx.fill();

  // Stroke
  ctx.beginPath();
  for (let i = 0; i < history.length; i++) {
    const x = padding + i * step;
    const y = h - (history[i] / maxPing) * (h - padding * 2) - padding;
    if (i === 0) ctx.moveTo(x, y);
    else {
      const prevX = padding + (i - 1) * step;
      const prevY = h - (history[i-1] / maxPing) * (h - padding * 2) - padding;
      const cpX = (prevX + x) / 2;
      ctx.bezierCurveTo(cpX, prevY, cpX, y, x, y);
    }
  }
  ctx.strokeStyle = accentHex;
  ctx.lineWidth = 2;
  ctx.stroke();
}

function renderServers() {
  lobbyCountEl.textContent = t("lobbyCount", { count: state.servers.length });
  if (!state.servers.length) {
    serverListEl.innerHTML = `<div class="empty-state">${escapeHtml(t("noServers"))}</div>`;
    renderSelectedServer();
    return;
  }

    const searchInput = document.getElementById("server-search-input");
    const searchTerm = searchInput ? searchInput.value.toLowerCase() : '';
    const activeThemeChips = Array.from(document.querySelectorAll(".filter-chip.active[data-filter-theme]"));
    const activeThemes = activeThemeChips.map(c => c.dataset.filterTheme);
    const themeFilter = activeThemes.length > 0 ? activeThemes : ["all"];
    
    const activeTypeChip = document.querySelector(".filter-chip.active[data-filter-type]");
    const typeFilter = activeTypeChip ? activeTypeChip.dataset.filterType : "all";
    
    let filteredServers = state.servers.filter(server => {
      // Name match
      const nameStr = (server.roomName || server.hostName || "Неизвестный сервер").toLowerCase();
      const nameMatch = nameStr.includes(searchTerm);
      
      // Theme match
      const themeMatch = themeFilter.includes('all') || themeFilter.includes(server.theme);
      
      // Type match (Global == external, Local == !external)
      let typeMatch = true;
      if (typeFilter === 'global') typeMatch = Boolean(server.external);
      if (typeFilter === 'local') typeMatch = !server.external;
      
      return nameMatch && themeMatch && typeMatch;
    });

    lobbyCountEl.textContent = t("lobbyCount", { count: filteredServers.length });
    if (!filteredServers.length) {
      serverListEl.innerHTML = `<div class="empty-state">${escapeHtml(t("noServers"))}</div>`;
      return;
    }

    serverListEl.innerHTML = filteredServers
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

      // Determine Theme Label
      let themeLabel = "Другое";
      const themeKey = server.theme ? "theme" + server.theme.charAt(0).toUpperCase() + server.theme.slice(1) : "";
      if (themeKey && I18N[state.preferences.language]?.[themeKey]) {
         themeLabel = t(themeKey);
      } else if (server.theme === "other") themeLabel = "Другое";

      // Type Label
      const typeLabel = isExternal ? "Глобальный" : "Локальный";
      const typeIcon = isExternal 
        ? '<svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><circle cx="12" cy="12" r="10"/><path d="M12 2a14.5 14.5 0 0 0 0 20 14.5 14.5 0 0 0 0-20"/><path d="M2 12h20"/></svg>'
        : '<svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M20 10c0 6-8 12-8 12s-8-6-8-12a8 8 0 0 1 16 0Z"/><circle cx="12" cy="10" r="3"/></svg>';

      // Theme Icon mapping
      const themeIcons = {
        survival: '<svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M20.59 13.41l-7.17 7.17a2 2 0 0 1-2.83 0L2 12V2h10l8.59 8.59a2 2 0 0 1 0 2.82z"/></svg>',
        vanilla: '<svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M11 20A7 7 0 0 1 9.8 6.1C15.5 5 17 4.48 19 2c1 2 2 4.18 2 8 0 5.5-4.78 10-10 10Z"/></svg>',
        creative: '<svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M18.36 6.64a9 9 0 1 1-12.73 0"/><line x1="12" y1="2" x2="12" y2="12"/></svg>',
        minigames: '<svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><rect x="2" y="6" width="20" height="12" rx="2"/><path d="M6 12h4"/><path d="M8 10v4"/></svg>',
        anarchy: '<svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><circle cx="9" cy="12" r="1"/><circle cx="15" cy="12" r="1"/><path d="M16 20a2 2 0 0 0 1.56-3.25 8 8 0 1 0-11.12 0A2 2 0 0 0 8 20"/></svg>',
        pvp: '<svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="m14.5 17.5-5-5 2-7L4 2l3.5 7.5-2 2 5 5z"/></svg>',
        rpg: '<svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="m15 5 4 4"/><path d="m2 22 5.5-1.5L21 7a2.12 2.12 0 0 0-3-3L4.5 17.5Z"/></svg>',
        war: '<svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M8.5 14.5A2.5 2.5 0 0 0 11 12c0-1.38-.5-2-1-3-1.072-2.143-.224-4.054 2-6 .5 2.5 2 4.9 4 6.5 2 1.6 3 3.5 3 5.5a7 7 0 1 1-14 0c0-1.153.433-2.294 1-3a2.5 2.5 0 0 0 2.5 2.5z"/></svg>',
        horror: '<svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M12 22s-8-4.5-8-11.8A8 8 0 0 1 12 2a8 8 0 0 1 8 8.2c0 7.3-8 11.8-8 11.8z"/></svg>',
        shop: '<svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="m2 7 4.41-4.41A2 2 0 0 1 7.83 2h8.34a2 2 0 0 1 1.42.59L22 7"/><path d="M4 12v8a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2v-8"/></svg>',
        modded: '<svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><circle cx="12" cy="12" r="3"/><path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 0 1-2.83 2.83l-.06-.06a1.65 1.65 0 0 0-2.82 1.51V21a2 2 0 0 1-4 0v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-2.15.33l-.06.06a2 2 0 0 1-2.83-2.83l.06-.06A1.65 1.65 0 0 0 4.68 15H3a2 2 0 0 1 0-4h.09A1.65 1.65 0 0 0 4.6 9 1.65 1.65 0 0 0 4.27 7.18l-.06-.06a2 2 0 0 1 2.83-2.83l.06.06A1.65 1.65 0 0 0 9 4.68V3a2 2 0 0 1 4 0v.09a1.65 1.65 0 0 0 2.82 1.51l.06-.06a2 2 0 0 1 2.83 2.83l-.06.06A1.65 1.65 0 0 0 19.4 9H21a2 2 0 0 1 0 4h-.09a1.65 1.65 0 0 0-1.51 1z"/></svg>',
        other: '<svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><circle cx="12" cy="12" r="10"/><path d="M9.09 9a3 3 0 0 1 5.83 1c0 2-3 3-3 3"/><path d="M12 17h.01"/></svg>'
      };
      const themeIcon = themeIcons[server.theme] || themeIcons.other;

      // Extract initials or use image
      const initials = (server.roomName || "?").substring(0, 2).toUpperCase();
      // Assume a generic ping logic for UI
      // Use a stable pseudo-random ping derived from clientId if actual ping is missing
      let fakePing = 10;
      if (server.clientId) {
        let hash = 0;
        for (let i = 0; i < server.clientId.length; i++) hash += server.clientId.charCodeAt(i);
        fakePing = (hash % 50) + 10;
      }
      const pingVal = server.pingMs ?? server.ping ?? fakePing;
      const pingClass = pingVal > 150 ? "ping-high" : (pingVal > 80 ? "ping-mid" : "");

      return `
        <article class="server-card ${isSelected ? "active" : ""}" data-select-server="${escapeHtml(server.clientId)}">
          <div class="server-card-avatar">
            ${initials}
          </div>
          <div class="server-card-body">
            <div class="server-card-top">
              <span class="server-card-name">${parseMinecraftColors(server.roomName || "Неизвестный сервер")}</span>
              <span class="server-card-slots">
                <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M16 21v-2a4 4 0 0 0-4-4H6a4 4 0 0 0-4 4v2"/><circle cx="9" cy="7" r="4"/><path d="M22 21v-2a4 4 0 0 0-3-3.87"/><path d="M16 3.13a4 4 0 0 1 0 7.75"/></svg>
                ${escapeHtml(server.slots || "0/0")}
              </span>
            </div>
            
            <div class="server-card-desc">
              ${escapeHtml(server.hostName || "Host")}
            </div>

            <div class="server-card-tags">
              <span class="server-tag">
                <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M2 20.5 12 4l10 16.5Z"/><path d="m20.5 10.5-8-6-8 6"/></svg>
                ${escapeHtml(server.minecraftVersion ?? t("serverUnknownVersion"))}
              </span>
              <span class="server-tag">
                ${themeIcon}
                ${escapeHtml(themeLabel)}
              </span>
              <span class="server-tag">
                ${typeIcon}
                ${escapeHtml(typeLabel)}
              </span>
              
              <span class="server-card-ping ${pingClass}">${pingVal}мс</span>
            </div>
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
        theme: data.theme ?? "other",
        transport: data.transport ?? null,
        publicJoinAddress: data.public_join_address ?? data.socket_endpoint ?? null,
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
  const publicJoinAddress = getAdvertisableJoinAddress(status);
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
    peer_addr: hostSession.peerAddr,
    local_port: hostSession.localPort,
    minecraft_version: hostSession.minecraftVersion ?? status?.minecraftVersion ?? null,
    theme: hostSession.theme ?? "other",
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
    return;
  }
  hostSession.active = false;
  hostSession.peerId = null;
  hostSession.listenAddrs = [];
  hostSession.peerAddr = null;
  hostSession.minecraftVersion = null;
  hostSession.publicJoinAddress = null;
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
  if (statusNoteEl) statusNoteEl.textContent = decodeMojibakeIfNeeded(status.note ?? t("modeIdle"));
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
        await state.realtime.channels.get(`lobby:${requester}`).publish("connect-ack", {
          relay_session_id: relaySessionId,
          host_id: localClientId,
        });
      } catch (error) {
        addLog(`Punch error: ${String(error)}`);
        await state.realtime.channels.get(`lobby:${requester}`).publish("connect-reject", {
          relay_session_id: relaySessionId,
          host_id: localClientId,
          error: String(error),
        });
      }
    });
    await state.privateChannel.subscribe("connect-ack", (message) => {
      const relaySessionId = message.data?.relay_session_id ?? null;
      if (!relaySessionId) return;
      state.pendingRelayAcks.get(relaySessionId)?.resolve(message.data ?? null);
    });
    await state.privateChannel.subscribe("connect-reject", (message) => {
      const relaySessionId = message.data?.relay_session_id ?? null;
      if (!relaySessionId) return;
      const error = message.data?.error ? new Error(String(message.data.error)) : new Error("relay connect rejected by host");
      state.pendingRelayAcks.get(relaySessionId)?.reject(error);
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

function waitForRelayAck(relaySessionId, timeoutMs = 12000) {
  return new Promise((resolve, reject) => {
    const timeout = setTimeout(() => {
      state.pendingRelayAcks.delete(relaySessionId);
      reject(new Error(`relay ack timed out after ${timeoutMs}ms for ${relaySessionId}`));
    }, timeoutMs);

    state.pendingRelayAcks.set(relaySessionId, {
      resolve: (payload) => {
        clearTimeout(timeout);
        state.pendingRelayAcks.delete(relaySessionId);
        resolve(payload);
      },
      reject: (error) => {
        clearTimeout(timeout);
        state.pendingRelayAcks.delete(relaySessionId);
        reject(error);
      },
    });
  });
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
    if (copyDiagnosticsEl) {
      copyDiagnosticsEl.disabled = true;
      copyDiagnosticsEl.textContent = "Сбор данных...";
    }
    addLog("Собираем мега-лог для отладки...");
    const localPort = Number(localGamePortEl.value || 25565);
    
    // Parallelize to speed up collection and avoid clipboard API timeout
    const [snapshot, natType, appInfo, publicIpInfo] = await Promise.all([
      invoke("export_diagnostics_snapshot", { localPort }).catch(() => ({})),
      invoke("detect_nat_type_command").catch(() => "Unknown"),
      invoke("get_app_info").catch(() => ({})),
      Promise.race([
        fetch("https://ipapi.co/json/").then(res => res.json()),
        new Promise((_, reject) => setTimeout(() => reject(new Error("Timeout")), 1500))
      ]).catch((e) => ({ error: `Failed: ${e.message}` }))
    ]);

    const megaLog = {
      timestamp: new Date().toISOString(),
      userAgent: navigator.userAgent,
      appInfo,
      publicIpInfo,
      natType,
      snapshot
    };

    const text = JSON.stringify(megaLog, null, 2);
    
    await copyTextToClipboard(text);
    addLog("Мега-лог (JSON) успешно скопирован в буфер обмена!");
  } catch (error) {
    addLog(`Ошибка при формировании лога: ${String(error)}`);
  } finally {
    if (copyDiagnosticsEl) {
      copyDiagnosticsEl.disabled = false;
      copyDiagnosticsEl.textContent = t("copyDiagnosticsButton") || "Скопировать JSON лог";
    }
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
  const originalHostText = hostButtonEl.innerHTML;
  hostButtonEl.disabled = true;
  hostButtonEl.innerHTML = `<svg class="animate-spin" style="animation: spin 1s linear infinite;" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M21 12a9 9 0 1 1-6.219-8.56"></path></svg> <span>Запуск...</span>`;
  hostButtonEl.classList.add('loading-opacity');

  detectMinecraftNickname().catch(console.error);
  await autofillRoomNameFromLocalServer();
  const roomName = roomNameEl.value.trim();
  const roomThemeInput = document.getElementById("room-theme");
  const theme = roomThemeInput ? roomThemeInput.value : "other";
  
  if (!roomName) {
    roomNameEl.focus();
    hostButtonEl.disabled = false;
    hostButtonEl.innerHTML = originalHostText;
    hostButtonEl.classList.remove('loading-opacity');
    if (hostProgressContainerEl) hostProgressContainerEl.classList.add("hidden");
    return;
  }

  if (externalHostModeEl.checked) {
    try {
      await addExternalServerFromModal(roomName);
      closeModal();
    } catch (error) {
      addLog(t("externalAddFailed", { error: String(error) }));
    } finally {
      hostButtonEl.disabled = false;
      hostButtonEl.innerHTML = originalHostText;
      hostButtonEl.classList.remove('loading-opacity');
    }
    return;
  }

  const localPort = Number(localGamePortEl.value || 25565);
  const password = requirePasswordEl.checked ? roomPasswordEl.value.trim() || null : null;
  const enableGeyser = Boolean(enableGeyserEl.checked);
  const geyserPort = Number(geyserPortEl.value || 19132);
  state.tunnelReady = false;
  setMinecraftHint(t("hintWaiting"), false);

  try {
    if (hostProgressContainerEl) hostProgressContainerEl.classList.remove("hidden");
    
    const preflight = await runPreflightCheck({ silent: false });
    if (!preflight.reachable) {
      addLog("Warning: local Minecraft server not detected at 127.0.0.1:" + localPort + ". Attempting to start anyway...");
    }

    const bootstrap = await invoke("start_hosting", {
      roomName,
      password,
      localPort,
      enableGeyser,
      geyserPort,
    });
    const status = await waitForStatus(
      (snapshot) => snapshot.mode === "host" && ["waitingForPeer", "hosting", "connected", "error"].includes(snapshot.state),
      22000,
    );
    renderStatus(status);
    
    if (status.state === "error") {
       throw new Error(status.lastError || "Unknown error during host startup");
    }
    
    hostSession.active = true;
    hostSession.roomName = roomName;
    hostSession.hasPassword = Boolean(password);
    hostSession.peerId = bootstrap.peerId || localClientId;
    hostSession.listenAddrs = collectAdvertisedAddrs(bootstrap, status);
    hostSession.peerAddr = advertisedEndpoint(hostSession.listenAddrs, status.publicUdpAddr ?? status.udpBindAddr);
    hostSession.localPort = localPort;
    hostSession.minecraftVersion = status.minecraftVersion ?? null;
    hostSession.theme = theme;
    hostSession.publicJoinAddress = status.publicJoinAddress ?? null;
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

    // Trigger a UI re-render now that hostSession is fully populated and active
    renderStatus(state.status);

    if (canAdvertiseHost()) {
      await syncPresence(status, { force: true, enter: true });
    } else {
      addLog("Presence delayed: waiting for public endpoint.");
    }
    await refreshLobby();
    hostButtonEl.textContent = '✅ Сервер работает';
    hostButtonEl.classList.remove('loading-opacity');
    
    // Close the modal upon success!
    closeModal();
    
    // Reset back to original text after 2 seconds in case they open the modal again later
    setTimeout(() => {
      if (hostButtonEl) {
        hostButtonEl.innerHTML = originalHostText;
      }
    }, 2000);

  } catch (error) {
    hostButtonEl.textContent = originalHostText;
    hostButtonEl.classList.remove('loading-opacity');
    addLog(t("hostStartFailed", { error: String(error) }));
  } finally {
    syncButtons();
    hostButtonEl.disabled = false;
    hostButtonEl.classList.remove('loading-opacity');
    if (hostProgressContainerEl) hostProgressContainerEl.classList.add("hidden");
    
    closeModal();
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
  if (state.pendingConnects.has(server.clientId)) {
    addLog(t("repeatConnectBlocked"));
    return;
  }
  if (server.clientId === localClientId) {
    addLog(t("ownHostBlocked"));
    return;
  }
  if (isClientLocked()) {
    addLog(t("hostNoteClientLocked"));
    return;
  }

  // Mark as pending synchronously to prevent double clicks
  state.pendingConnects.add(server.clientId);
  renderServers();

  await detectMinecraftNickname();
  
  if (server.external) {
    const externalJoin = server.joinAddress ?? (server.peerAddr ? toSocketEndpoint(server.peerAddr) ?? server.peerAddr : null);
    if (externalJoin) {
      await copyTextToClipboard(externalJoin);
      addLog(t("copiedIp"));
      setMinecraftHint(`${t("selectedAddressLabel")}: ${externalJoin}`, true);
      setTimeout(() => {
        state.pendingConnects.delete(server.clientId);
        renderServers();
      }, 1400);
    } else {
      state.pendingConnects.delete(server.clientId);
      renderServers();
    }
    return;
  }

  state.selectedServerId = server.clientId;
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
    await invoke("prepare_client_connect", {
      peerId: server.peerId,
      peerAddrs,
    });
    const status = await waitForStatus(
      (snapshot) =>
        snapshot.mode === "client" &&
        Boolean(snapshot.publicUdpAddr ?? snapshot.udpBindAddr) &&
        ["waitingForPeer", "starting", "connecting", "connected"].includes(snapshot.state),
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
    addLog(`Waiting for relay ack ${relaySessionId} from host ${server.clientId}.`);
    await waitForRelayAck(relaySessionId, 12000);
    addLog(`Relay ack received for ${relaySessionId}. Starting client tunnel.`);
    await invoke("commit_prepared_client_connect", { relaySessionId });
  } catch (error) {
    state.pendingConnects.delete(server.clientId);
    setMinecraftHint(t("hintFailed"), false);
    addLog(t("connectFailed", { error: String(error) }));
    try {
      const snapshot = await invoke("get_status");
      if (snapshot?.mode === "client" && snapshot?.state !== "connected") {
        await invoke("stop_hosting");
      }
    } catch {
      // no-op: best effort cleanup for prepared client state
    }
    if (server.publicJoinAddress) {
      await copyTextToClipboard(server.publicJoinAddress);
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
  const addr = event.payload?.minecraftAddr ?? "localhost:25565";
  await copyTextToClipboard(addr);
  addLog(
    `${t("tunnelEstablishedLog", { addr })} (${formatTransportLabel(
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

const peerLastBytes = {};
await listen("peer-health", async (event) => {
  const { peer_id, ping_ms, bytesRx, bytesTx } = event.payload;
  const pId = event.payload.peerId || peer_id;
  const pPing = event.payload.pingMs || ping_ms;

  const last = peerLastBytes[pId] || { rx: 0, tx: 0 };
  const deltaRx = Math.max(0, (bytesRx || 0) - last.rx);
  const deltaTx = Math.max(0, (bytesTx || 0) - last.tx);
  
  currentBytesIn += deltaRx;
  currentBytesOut += deltaTx;
  peerLastBytes[pId] = { rx: bytesRx || 0, tx: bytesTx || 0 };

  if (state.status && state.status.peers) {
    const peer = state.status.peers.find(p => p.peer_id === pId);
    if (peer) {
      peer.ping_ms = pPing;
      renderPeers(state.status.peers);
    }
  }
});

await listen("hole_punch_success", async (event) => {
  state.activeTunnelTransport = "direct-quic";
  addLog(`Hole punch success for ${event.payload?.peerId ?? "peer"}.`);
});

navHomeEl?.addEventListener("click", () => setPage("home"));
navSettingsEl?.addEventListener("click", () => setPage("settings"));
openHostModalEl?.addEventListener("click", openModal);
closeModalEl?.addEventListener("click", closeModal);
closeModalSecondaryEl?.addEventListener("click", closeModal);
modalEl?.addEventListener("click", (event) => {
  if (event.target instanceof HTMLElement && event.target.dataset.closeModal === "true") closeModal();
});

requirePasswordEl?.addEventListener("change", syncPasswordField);
enableGeyserEl?.addEventListener("change", syncGeyserField);
roomNameEl?.addEventListener("input", () => {
  roomNameEl.dataset.autofilled = "false";
});

// Settings tabs logic
document.querySelectorAll(".settings-tab").forEach(tab => {
  tab.addEventListener("click", () => {
    document.querySelectorAll(".settings-tab").forEach(t => t.classList.remove("active"));
    document.querySelectorAll(".settings-tab-content").forEach(c => c.classList.remove("active"));
    tab.classList.add("active");
    const targetId = tab.dataset.tab;
    const content = document.getElementById("tab-" + targetId);
    if (content) content.classList.add("active");
  });
});
hostButtonEl?.addEventListener("click", startHosting);
stopButtonEl?.addEventListener("click", stopSession);
refreshLobbyEl?.addEventListener("click", async () => {
  const status = await invoke("get_status");
  if (canAdvertiseHost()) {
    await syncPresence(status, { force: true, enter: !hostSession.presenceEntered });
  }
  await refreshLobby();
});
copyLogsEl?.addEventListener("click", async () => {
  await copyTextToClipboard(currentLogLines().join("\n"));
  addLog(t("copiedLog"));
});
copyDiagnosticsEl?.addEventListener("click", async () => {
  await copyDiagnosticsSnapshot();
});
copySelectedEndpointEl?.addEventListener("click", async () => {
  const selected = getSelectedServer();
  let endpoint = null;
  
  if (state.tunnelReady && state.selectedServerId === selected?.clientId) {
    endpoint = "localhost:25565"; // Default proxy port
  } else {
    endpoint = selected?.publicJoinAddress ??
      selected?.joinAddress ??
      (selected?.peerAddr ? toSocketEndpoint(selected.peerAddr) ?? selected.peerAddr : null);
  }
  
  if (!endpoint) return;
  await copyTextToClipboard(endpoint);
  addLog(t("copiedIp"));
});
copySelectedBedrockEndpointEl?.addEventListener("click", async () => {
  const selected = getSelectedServer();
  const endpoint = deriveBedrockEndpoint(selected?.peerAddr, selected?.bedrockPort);
  if (!endpoint) return;
  await copyTextToClipboard(endpoint);
  addLog(t("copiedBedrockIp"));
});
runPreflightEl?.addEventListener("click", async () => {
  await runPreflightCheck();
});
autoDetectPortEl?.addEventListener("click", async () => {
  await autoDetectLocalGamePort();
});
externalHostModeEl?.addEventListener("change", syncExternalHostMode);
startTestServerEl?.addEventListener("click", async () => {
  await startEmbeddedTestServer();
});
stopTestServerEl?.addEventListener("click", async () => {
  await stopEmbeddedTestServer();
});
probeTestServerEl?.addEventListener("click", async () => {
  await probeEmbeddedTestServer();
});
connectSelectedEl?.addEventListener("click", async () => {
  const selected = getSelectedServer();
  if (selected) await connectToServer(selected);
});
checkUpdatesEl?.addEventListener("click", checkForUpdates);
installUpdateEl?.addEventListener("click", installUpdate);

closePortModalEl?.addEventListener("click", closePortChoiceModal);
closePortModalSecondaryEl?.addEventListener("click", closePortChoiceModal);
portChoiceModalEl?.addEventListener("click", (event) => {
  if (event.target instanceof HTMLElement && event.target.dataset.closeModalPort === "true") closePortChoiceModal();
});
clearIgnoredPortsEl?.addEventListener("click", () => {
  state.ignoredPorts = [];
  saveIgnoredPorts();
  renderIgnoredPorts();
});

closePlayerModalEl?.addEventListener("click", closePlayerModal);
closePlayerModalSecondaryEl?.addEventListener("click", closePlayerModal);
playerModalEl?.addEventListener("click", (event) => {
  if (event.target instanceof HTMLElement && event.target.dataset.closePlayerModal === "true") closePlayerModal();
});

kickButtonEl?.addEventListener("click", async () => {
  if (activeModalPeerId && activeModalPeerId !== 'host') {
    await kickPeer(activeModalPeerId);
    closePlayerModal();
  }
});

document.querySelectorAll(".accent-btn").forEach((button) => {
  button.addEventListener("click", () => applyAccent(button.dataset.accent));
});

function closePlayerModal() {
  playerModalEl.classList.add("hidden");
  if (state.pingGraphInterval) clearInterval(state.pingGraphInterval);
  activeModalPeerId = null;
}

serverListEl?.addEventListener("click", async (event) => {
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

activeHostCardEl?.addEventListener("click", async (event) => {
  const target = event.target;
  if (!(target instanceof HTMLElement)) return;
  const javaValue = target.closest("[data-copy-host-java]")?.dataset.copyHostJava;
  if (javaValue) {
    // For the host: copy localhost:localPort (what players on the same machine use)
    const localAddr = `localhost:${hostSession.localPort || 25565}`;
    await copyTextToClipboard(localAddr);
    addLog(t("copiedIp") + ` (${localAddr})`);
    setMinecraftHint(`${t("selectedAddressLabel")}: ${localAddr}`, true);
    return;
  }
  const copyPublic = target.closest("[data-copy-host-public]")?.dataset.copyHostPublic;
  if (copyPublic) {
    await copyTextToClipboard(copyPublic);
    addLog(t("copiedIp") + ` (${copyPublic})`);
    setMinecraftHint(`${t("selectedAddressLabel")}: ${copyPublic}`, true);
    return;
  }
  const bedrockValue = target.closest("[data-copy-host-bedrock]")?.dataset.copyHostBedrock;
  if (bedrockValue) {
    await copyTextToClipboard(bedrockValue);
    addLog(t("copiedBedrockIp"));
  }
});

peerListEl?.addEventListener("click", async (event) => {
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
});

document.body.dataset.theme = state.preferences.theme;
applyAccent(state.preferences.accent);
applyTranslations();
renderSettingsOptions();
syncPasswordField();
syncGeyserField();
syncExternalHostMode();
renderProfile();
renderLogs();
renderSelectedServer();
renderSessionCard();
syncButtons();
setPage("home");
renderIgnoredPorts();
await loadAppInfo();
await detectMinecraftNickname();
await initAuth();

document.querySelectorAll(".settings-tab").forEach(tab => {
  tab.addEventListener("click", () => {
    document.querySelectorAll(".settings-tab").forEach(t => t.classList.remove("active"));
    document.querySelectorAll(".settings-tab-content").forEach(c => c.classList.remove("active"));
    tab.classList.add("active");
    const targetEl = document.getElementById(`tab-${tab.dataset.tab}`);
    if (targetEl) targetEl.classList.add("active");
  });
});

profileMenuTriggerEl?.addEventListener("click", () => {
  setPage("profile");
});

// Theme chips in host modal
document.querySelectorAll(".theme-chip").forEach(chip => {
  chip.addEventListener("click", () => {
    document.querySelectorAll(".theme-chip").forEach(c => c.classList.remove("active"));
    chip.classList.add("active");
    const roomThemeEl = document.getElementById("room-theme");
    if (roomThemeEl) roomThemeEl.value = chip.dataset.themeChip;
  });
});

// Host Creation Theme Chips logic
const hostThemeChips = document.querySelectorAll("#host-theme-chips .theme-chip");
const roomThemeInput = document.getElementById("room-theme");

hostThemeChips.forEach(chip => {
  chip.addEventListener("click", () => {
    // Remove active from all
    hostThemeChips.forEach(c => c.classList.remove("active"));
    // Add active to clicked
    chip.classList.add("active");
    // Update hidden input
    if (roomThemeInput) {
      roomThemeInput.value = chip.dataset.themeChip;
    }
  });
});



// Filter chips logic
const filterChipsTheme = document.querySelectorAll(".filter-chip[data-filter-theme]");
const filterChipsType = document.querySelectorAll(".filter-chip[data-filter-type]");

filterChipsTheme.forEach(chip => {
  chip.addEventListener("click", () => {
    chip.classList.toggle("active");
    const activeThemes = [...filterChipsTheme].filter(c => c.classList.contains("active")).map(c => c.dataset.filterTheme);
    const filterThemeEl = document.getElementById("filter-theme");
    if (filterThemeEl) filterThemeEl.value = activeThemes.length === 1 ? activeThemes[0] : "all";
  });
});

filterChipsType.forEach(chip => {
  chip.addEventListener("click", () => {
    filterChipsType.forEach(c => c.classList.remove("active"));
    chip.classList.add("active");
    const filterTypeEl = document.getElementById("filter-type");
    if (filterTypeEl) filterTypeEl.value = chip.dataset.filterType;
  });
});

// Filter clear buttons
document.querySelectorAll(".filter-clear-btn").forEach(btn => {
  btn.addEventListener("click", () => {
    const group = btn.dataset.clear;
    if (group === "theme") {
      filterChipsTheme.forEach(c => c.classList.remove("active"));
      const el = document.getElementById("filter-theme"); if (el) el.value = "all";
    } else if (group === "type") {
      filterChipsType.forEach(c => c.classList.remove("active"));
      const el = document.getElementById("filter-type"); if (el) el.value = "all";
    }
    // We don't call renderServers here, wait for Apply button
  });
});

// Reset all filters
document.getElementById("reset-filters-button")?.addEventListener("click", () => {
  filterChipsTheme.forEach(c => c.classList.remove("active"));
  filterChipsType.forEach(c => c.classList.remove("active"));
  const ft = document.getElementById("filter-theme"); if (ft) ft.value = "all";
  const ftype = document.getElementById("filter-type"); if (ftype) ftype.value = "all";
  const si = document.getElementById("server-search-input"); if (si) si.value = "";
  renderServers();
});



setInterval(() => {
  void pollStatus();
}, POLL_INTERVAL_MS);

await detectRuntimeFingerprint();
await setupAbly();
await pollStatus();




// Network Stats Graph
const networkGraphCanvas = document.querySelector("#network-stats-graph");
const networkGraphCtx = networkGraphCanvas?.getContext("2d");
const networkStatsDownEl = document.querySelector("#network-stats-down");
const networkStatsUpEl = document.querySelector("#network-stats-up");

let statsHistoryIn = [];
let statsHistoryOut = [];
let currentBytesIn = 0;
let currentBytesOut = 0;

setInterval(() => {
  statsHistoryIn.push(currentBytesIn);
  statsHistoryOut.push(currentBytesOut);
  if (statsHistoryIn.length > 60) statsHistoryIn.shift();
  if (statsHistoryOut.length > 60) statsHistoryOut.shift();
  
  if (networkStatsDownEl && networkStatsUpEl) {
    networkStatsDownEl.textContent = `D: ${(currentBytesIn / 1024).toFixed(1)} KB/s`;
    networkStatsUpEl.textContent = `U: ${(currentBytesOut / 1024).toFixed(1)} KB/s`;
  }
  
  currentBytesIn = 0;
  currentBytesOut = 0;
  
  if (networkGraphCtx && networkGraphCanvas) {
    networkGraphCanvas.width = networkGraphCanvas.offsetWidth * window.devicePixelRatio;
    networkGraphCanvas.height = networkGraphCanvas.offsetHeight * window.devicePixelRatio;
    
    const w = networkGraphCanvas.width;
    const h = networkGraphCanvas.height;
    networkGraphCtx.clearRect(0, 0, w, h);
    
    const maxVal = Math.max(...statsHistoryIn, ...statsHistoryOut, 1024);
    
    // Draw In
    networkGraphCtx.beginPath();
    networkGraphCtx.strokeStyle = "rgba(46, 204, 113, 0.8)";
    networkGraphCtx.lineWidth = 2 * window.devicePixelRatio;
    for (let i = 0; i < statsHistoryIn.length; i++) {
        const x = (i / 60) * w;
        const y = h - (statsHistoryIn[i] / maxVal) * h;
        if (i === 0) networkGraphCtx.moveTo(x, y);
        else networkGraphCtx.lineTo(x, y);
    }
    networkGraphCtx.stroke();
    
    // Draw Out
    networkGraphCtx.beginPath();
    networkGraphCtx.strokeStyle = "rgba(52, 152, 219, 0.8)";
    networkGraphCtx.lineWidth = 2 * window.devicePixelRatio;
    for (let i = 0; i < statsHistoryOut.length; i++) {
        const x = (i / 60) * w;
        const y = h - (statsHistoryOut[i] / maxVal) * h;
        if (i === 0) networkGraphCtx.moveTo(x, y);
        else networkGraphCtx.lineTo(x, y);
    }
    networkGraphCtx.stroke();
  }
}, 1000);

// ==========================================
// SUPABASE AUTH LOGIC
// ==========================================
let authMode = "register";

function showAuthError(msg) {
  if (authErrorEl) {
    authErrorEl.textContent = msg;
    authErrorEl.classList.remove("hidden");
  }
}

function generateRandomNickname() {
  const adjectives = ["Swift", "Shadow", "Crystal", "Iron", "Storm", "Blaze", "Frost", "Neon", "Void", "Pixel", "Dark", "Lunar", "Solar", "Turbo", "Hyper"];
  const nouns = ["Miner", "Crafter", "Builder", "Archer", "Knight", "Wizard", "Dragon", "Wolf", "Hawk", "Fox", "Bear", "Tiger", "Raven", "Golem", "Phantom"];
  const adj = adjectives[Math.floor(Math.random() * adjectives.length)];
  const noun = nouns[Math.floor(Math.random() * nouns.length)];
  const num = Math.floor(Math.random() * 999);
  return `${adj}${noun}${num}`;
}

function generateShortId(uuid) {
  if (!uuid) return "—";
  return uuid.slice(0, 8).toUpperCase();
}

async function updateProfileData(data) {
  const guestData = localStorage.getItem("p2p-guest-session");
  if (guestData) {
    let sess = JSON.parse(guestData);
    if (!sess.user.user_metadata) sess.user.user_metadata = {};
    sess.user.user_metadata = { ...sess.user.user_metadata, ...data };
    localStorage.setItem("p2p-guest-session", JSON.stringify(sess));
    return { data: sess };
  } else {
    return await supabase.auth.updateUser({ data });
  }
}

function populateProfilePanel(session) {
  if (!session?.user) return;
  const user = session.user;
  const email = user.email || "—";
  const provider = user.app_metadata?.provider || "email";
  const providerLabel = provider === "google" ? "Google" : provider === "email" ? "Email / Пароль" : provider;
  const shortId = generateShortId(user.id);

  const meta = user.user_metadata || {};
  let updateData = {};
  let needsUpdate = false;

  // 1. Sync Nickname
  if (meta.custom_nickname) {
    state.profile.nickname = meta.custom_nickname;
  } else {
    const googleName = meta.full_name || meta.name;
    const localNick = state.profile.nickname;
    if (localNick && localNick !== "Player") {
      state.profile.nickname = localNick;
    } else {
      state.profile.nickname = googleName || generateRandomNickname();
    }
    updateData.custom_nickname = state.profile.nickname;
    needsUpdate = true;
  }

  // 2. Sync Avatar
  const googleAvatar = meta.avatar_url || meta.picture;
  let activeAvatar = meta.custom_avatar;
  if (!activeAvatar) {
    if (state.profile.avatarDataUrl) {
      activeAvatar = state.profile.avatarDataUrl;
      updateData.custom_avatar = activeAvatar;
      needsUpdate = true;
    } else if (googleAvatar) {
      activeAvatar = googleAvatar;
      updateData.custom_avatar = activeAvatar;
      needsUpdate = true;
    }
  }
  state.profile.avatarDataUrl = activeAvatar || null;

  // 3. Sync Custom ID
  let displayId = meta.custom_id;
  if (!displayId) {
    if (state.profile.customId) {
      displayId = state.profile.customId;
    } else {
      displayId = shortId;
    }
    updateData.custom_id = displayId;
    needsUpdate = true;
  }
  storedDisplayId = displayId;
  state.profile.customId = displayId;

  // If there are newly populated fields, push them to Supabase user_metadata immediately
  if (needsUpdate) {
    updateProfileData(updateData)
      .then(({ data, error }) => {
        if (error) console.error("[auth] Failed to auto-sync profile metadata:", error);
        else console.log("[auth] Successfully auto-synced profile metadata to server:", data);
      })
      .catch(err => console.error("[auth] Profile metadata update error:", err));
  }

  // Banner
  if (meta.custom_banner) {
    state.profile.customBanner = meta.custom_banner;
    if (socialBannerImgEl) {
      socialBannerImgEl.src = meta.custom_banner;
      socialBannerImgEl.classList.remove("hidden");
    }
  } else {
    state.profile.customBanner = null;
    if (socialBannerImgEl) {
      socialBannerImgEl.removeAttribute("src");
      socialBannerImgEl.classList.add("hidden");
    }
  }

  // Save synchronized profile state to localStorage
  saveProfileState();

  // Update UI Elements
  if (activeAvatar) {
    if (profileAvatarPreviewEl) {
      profileAvatarPreviewEl.src = activeAvatar;
      profileAvatarPreviewEl.classList.remove("hidden");
    }
    if (profileAvatarLetterEl) profileAvatarLetterEl.classList.add("hidden");
    if (brandAvatarImageEl) {
      brandAvatarImageEl.src = activeAvatar;
      brandAvatarImageEl.classList.remove("hidden");
    }
    if (brandAvatarFallbackEl) brandAvatarFallbackEl.classList.add("hidden");
  } else {
    const letter = state.profile.nickname.charAt(0).toUpperCase();
    if (profileAvatarLetterEl) profileAvatarLetterEl.textContent = letter;
    if (profileAvatarPreviewEl) profileAvatarPreviewEl.classList.add("hidden");
    if (profileAvatarLetterEl) profileAvatarLetterEl.classList.remove("hidden");
  }

  if (profileDisplayNameEl) profileDisplayNameEl.textContent = state.profile.nickname;
  if (profileUserIdEl) profileUserIdEl.textContent = `@${displayId}`;
  if (profileIdInputEl) profileIdInputEl.value = displayId;
  if (settingsAuthEmailEl) settingsAuthEmailEl.textContent = email;
  if (settingsAuthProviderEl) settingsAuthProviderEl.textContent = providerLabel;
  if (brandUserNameEl) brandUserNameEl.textContent = state.profile.nickname;
  if (profileNicknameInputEl) profileNicknameInputEl.value = state.profile.nickname;

  if (brandAvatarFallbackEl && !activeAvatar) {
    brandAvatarFallbackEl.textContent = state.profile.nickname.charAt(0).toUpperCase();
  }
}

async function initAuth() {
  // Listen for local OAuth receiver event (for seamless Google login)
  listen("oauth-login", async (event) => {
    console.log("[auth] Google OAuth event received:", event.payload);
    const { access_token, refresh_token, code } = event.payload;
    try {
      if (code) {
        console.log("[auth] Exchanging auth code for session...");
        const { data, error } = await supabase.auth.exchangeCodeForSession(code);
        if (error) throw error;
        if (data?.session) {
          console.log("[auth] Google session established successfully via PKCE!");
          authOverlayEl?.classList.add("hidden");
          populateProfilePanel(data.session);
        }
      } else if (access_token) {
        console.log("[auth] Setting session via implicit flow...");
        const { data, error } = await supabase.auth.setSession({
          access_token,
          refresh_token: refresh_token || ""
        });
        if (error) throw error;
        if (data?.session) {
          console.log("[auth] Google session established successfully via Implicit!");
          authOverlayEl?.classList.add("hidden");
          populateProfilePanel(data.session);
        }
      }
    } catch (error) {
      showAuthError("Ошибка при входе: " + error.message);
    }
    // Restore button state
    if (btnGoogleEl) {
      btnGoogleEl.disabled = false;
      btnGoogleEl.innerHTML = originalGoogleBtnHtml;
    }
  });

  // 1. Explicitly check for access_token in hash (Implicit Flow) for robust Tauri login
  if (window.location.hash && window.location.hash.includes("access_token=")) {
    try {
      const hashParams = new URLSearchParams(window.location.hash.substring(1));
      const accessToken = hashParams.get("access_token");
      const refreshToken = hashParams.get("refresh_token");
      if (accessToken) {
        const { data, error } = await supabase.auth.setSession({
          access_token: accessToken,
          refresh_token: refreshToken || ""
        });
        if (error) {
          console.error("[auth] Failed to explicitly set session:", error);
        } else if (data?.session) {
          console.log("[auth] Explicitly set session successfully:", data.session);
          authOverlayEl?.classList.add("hidden");
          populateProfilePanel(data.session);
        }
      }
      // Clean up hash from URL to keep it clean and prevent re-triggering
      window.history.replaceState(null, null, window.location.pathname);
    } catch (e) {
      console.error("[auth] Error parsing hash params:", e);
    }
  }

  // Parse error parameters from URL (e.g. from failed Google OAuth redirect)
  const urlParams = new URLSearchParams(window.location.search || (window.location.hash.includes("?") ? window.location.hash.substring(window.location.hash.indexOf("?")) : ""));
  const authError = urlParams.get("error_description") || urlParams.get("error");
  if (authError) {
    let friendlyError = authError.replace(/\+/g, " ");
    if (authError.includes("bad_oauth_state") || authError.includes("OAuth state not found")) {
      friendlyError = "Ошибка авторизации через Google: Не совпадает порт перенаправления. Пожалуйста, зайдите в настройки Supabase Dashboard -> Authentication -> Redirect URLs и добавьте туда адрес '" + window.location.origin + "/'";
    }
    showAuthError(friendlyError);
  }

  // Check current session
  const guestData = localStorage.getItem("p2p-guest-session");
  if (guestData) {
    authOverlayEl?.classList.add("hidden");
    populateProfilePanel(JSON.parse(guestData));
  } else {
    const { data: { session } } = await supabase.auth.getSession();
    if (session) {
      authOverlayEl?.classList.add("hidden");
      populateProfilePanel(session);
    } else {
      authOverlayEl?.classList.remove("hidden");
    }
  }

  // Listen for changes
  supabase.auth.onAuthStateChange((event, session) => {
    if (localStorage.getItem("p2p-guest-session")) return;
    if (session) {
      authOverlayEl?.classList.add("hidden");
      populateProfilePanel(session);
    } else {
      authOverlayEl?.classList.remove("hidden");
    }
  });

  // Profile menu open/close
  profileMenuTriggerEl?.addEventListener("click", (e) => {
    e.stopPropagation();
    setPage("profile");
  });

  // Copy User ID on click (uses storedDisplayId, updates textContent briefly)
  profileUserIdEl?.addEventListener("click", async () => {
    if (!storedDisplayId) return;
    await copyTextToClipboard(storedDisplayId).catch(() => {});
    const orig = profileUserIdEl.textContent;
    profileUserIdEl.textContent = "Скопировано!";
    setTimeout(() => { if (profileUserIdEl.textContent === "Скопировано!") profileUserIdEl.textContent = orig; }, 1500);
  });

  // ===== UNIFIED INLINE EDITING =====
  // Only one field can be edited at a time.
  let currentEditingField = null; // 'name' | 'id' | null

  function closeAllEditing() {
    if (currentEditingField === "name") {
      profileDisplayNameEl?.classList.remove("hidden");
      profileNicknameInputEl?.classList.add("hidden");
      nameConfirmActionsEl?.classList.add("hidden");
      editNameBtnEl?.classList.remove("hidden"); // Show pencil back
    } else if (currentEditingField === "id") {
      profileUserIdEl?.classList.remove("hidden");
      profileIdInputEl?.classList.add("hidden");
      idConfirmActionsEl?.classList.add("hidden");
      editIdBtnEl?.classList.remove("hidden"); // Show pencil back
    }
    currentEditingField = null;
  }

  function openNameEdit() {
    closeAllEditing();
    currentEditingField = "name";
    // Always pre-populate with the currently displayed name to avoid reverting to "Player"
    if (profileNicknameInputEl && profileDisplayNameEl) {
      profileNicknameInputEl.value = profileDisplayNameEl.textContent.trim();
    }
    profileDisplayNameEl?.classList.add("hidden");
    profileNicknameInputEl?.classList.remove("hidden");
    nameConfirmActionsEl?.classList.remove("hidden");
    editNameBtnEl?.classList.add("hidden"); // Hide the pencil icon during edit
    profileNicknameInputEl?.focus();
    if (profileNicknameInputEl) {
      const len = profileNicknameInputEl.value.length;
      profileNicknameInputEl.setSelectionRange(len, len);
    }
  }

  async function saveNameEdit() {
    const newNick = profileNicknameInputEl?.value?.trim();
    if (newNick && newNick !== state.profile.nickname) {
      state.profile.nickname = newNick;
      saveProfileState();
      if (brandUserNameEl) brandUserNameEl.textContent = newNick;
      if (profileDisplayNameEl) profileDisplayNameEl.textContent = newNick;
      await updateProfileData({ custom_nickname: newNick });
    }
    closeAllEditing();
  }

  function openIdEdit() {
    closeAllEditing();
    currentEditingField = "id";
    // Use storedDisplayId — NOT textContent which may say "Скопировано!"
    if (profileIdInputEl) {
      profileIdInputEl.value = storedDisplayId || profileUserIdEl?.textContent?.replace(/^@/, "").trim() || "";
    }
    profileUserIdEl?.classList.add("hidden");
    profileIdInputEl?.classList.remove("hidden");
    idConfirmActionsEl?.classList.remove("hidden");
    editIdBtnEl?.classList.add("hidden"); // Hide the pencil icon during edit
    profileIdInputEl?.focus();
    if (profileIdInputEl) {
      const len = profileIdInputEl.value.length;
      profileIdInputEl.setSelectionRange(len, len);
    }
  }

  async function saveIdEdit() {
    const newId = profileIdInputEl?.value?.trim().replace(/\s+/g, "_").toLowerCase();
    if (newId && newId !== storedDisplayId) {
      storedDisplayId = newId;
      state.profile.customId = newId;
      saveProfileState();
      if (profileUserIdEl) profileUserIdEl.textContent = `@${newId}`;
      await updateProfileData({ custom_id: newId });
    }
    closeAllEditing();
  }

  // Name edit handlers
  profileDisplayNameEl?.addEventListener("click", () => {
    openNameEdit();
  });
  editNameBtnEl?.addEventListener("click", () => {
    currentEditingField === "name" ? saveNameEdit() : openNameEdit();
  });
  saveNameBtnEl?.addEventListener("click", () => saveNameEdit());
  cancelNameBtnEl?.addEventListener("click", () => closeAllEditing());
  profileNicknameInputEl?.addEventListener("keydown", (e) => {
    if (e.key === "Enter") saveNameEdit();
    if (e.key === "Escape") closeAllEditing();
  });

  // ID edit handlers
  editIdBtnEl?.addEventListener("click", () => {
    currentEditingField === "id" ? saveIdEdit() : openIdEdit();
  });
  saveIdBtnEl?.addEventListener("click", () => saveIdEdit());
  cancelIdBtnEl?.addEventListener("click", () => closeAllEditing());
  profileIdInputEl?.addEventListener("keydown", (e) => {
    if (e.key === "Enter") saveIdEdit();
    if (e.key === "Escape") closeAllEditing();
  });

  editAvatarBtnEl?.addEventListener("click", () => profileAvatarFileEl?.click());
  profileAvatarFileEl?.addEventListener("change", () => {
    const file = profileAvatarFileEl.files?.[0];
    if (!file) return;
    const reader = new FileReader();
    reader.onload = async () => {
      const base64 = reader.result;
      if (profileAvatarPreviewEl) {
        profileAvatarPreviewEl.src = base64;
        profileAvatarPreviewEl.classList.remove("hidden");
      }
      if (profileAvatarLetterEl) profileAvatarLetterEl.classList.add("hidden");
      if (brandAvatarImageEl) {
        brandAvatarImageEl.src = base64;
        brandAvatarImageEl.classList.remove("hidden");
      }
      if (brandAvatarFallbackEl) brandAvatarFallbackEl.classList.add("hidden");
      
      state.profile.avatarDataUrl = base64;
      saveProfileState();
      
      // Save to supabase metadata so it persists
      await updateProfileData({ custom_avatar: base64 });
    };
    reader.readAsDataURL(file);
  });

  // Choose banner
  editBannerBtnEl?.addEventListener("click", () => profileBannerFileEl?.click());
  profileBannerFileEl?.addEventListener("change", () => {
    const file = profileBannerFileEl.files?.[0];
    if (!file) return;
    const reader = new FileReader();
    reader.onload = async () => {
      const base64 = reader.result;
      if (socialBannerImgEl) {
        socialBannerImgEl.src = base64;
        socialBannerImgEl.classList.remove("hidden");
      }
      
      state.profile.customBanner = base64;
      saveProfileState();
      
      // Save to supabase metadata
      await updateProfileData({ custom_banner: base64 });
    };
    reader.readAsDataURL(file);
  });

  // Toggle Mode
  btnToggleModeEl?.addEventListener("click", () => {
    if (authMode === "register") {
      authMode = "login";
      if (authTitleEl) authTitleEl.textContent = "Вход";
      if (authSubtitleEl) authSubtitleEl.textContent = "Авторизуйтесь, чтобы продолжить";
      if (btnAuthSubmitEl) btnAuthSubmitEl.textContent = "Войти";
      if (btnToggleModeEl) btnToggleModeEl.textContent = "Нет аккаунта? Зарегистрироваться";
    } else {
      authMode = "register";
      if (authTitleEl) authTitleEl.textContent = "Регистрация";
      if (authSubtitleEl) authSubtitleEl.textContent = "Создайте аккаунт, чтобы продолжить";
      if (btnAuthSubmitEl) btnAuthSubmitEl.textContent = "Зарегистрироваться";
      if (btnToggleModeEl) btnToggleModeEl.textContent = "Уже есть аккаунт? Войти";
    }
    authErrorEl?.classList.add("hidden");
  });

  // Handle Form Submit
  authFormEl?.addEventListener("submit", async (e) => {
    e.preventDefault();
    authErrorEl?.classList.add("hidden");
    const email = authEmailEl.value;
    const password = authPasswordEl.value;
    
    if (authMode === "login") {
      const { error } = await supabase.auth.signInWithPassword({ email, password });
      if (error) showAuthError(error.message);
    } else {
      if (!email || !password || password.length < 6) {
        showAuthError("Пожалуйста, введите email и пароль (минимум 6 символов).");
        return;
      }
      const { error } = await supabase.auth.signUp({ email, password });
      if (error) {
        showAuthError(error.message);
      } else {
        showAuthError("Успешная регистрация! Проверьте email для подтверждения, либо войдите, если подтверждение отключено.");
        authErrorEl.style.background = "rgba(99, 232, 155, 0.15)";
        authErrorEl.style.borderColor = "var(--success)";
        authErrorEl.style.color = "var(--success)";
      }
    }
  });

  // Handle Guest Auth
  btnGuestEl?.addEventListener("click", () => {
    const guestId = "guest_" + Math.random().toString(36).substring(2, 10);
    const guestSession = {
      user: {
        id: guestId,
        email: "guest@local.host",
        app_metadata: { provider: "Гостевой аккаунт" },
        user_metadata: {
          custom_nickname: "Гость " + Math.floor(Math.random() * 1000),
        }
      }
    };
    localStorage.setItem("p2p-guest-session", JSON.stringify(guestSession));
    authOverlayEl?.classList.add("hidden");
    populateProfilePanel(guestSession);
  });

  // Handle Google Auth
  btnGoogleEl?.addEventListener("click", async () => {
    try {
      if (btnGoogleEl) {
        btnGoogleEl.disabled = true;
        btnGoogleEl.innerHTML = `
          <svg style="animation: spin 1s linear infinite;" width="18" height="18" viewBox="0 0 24 24" fill="none" xmlns="http://www.w3.org/2000/svg">
            <style>
              @keyframes spin {
                from { transform: rotate(0deg); }
                to { transform: rotate(360deg); }
              }
            </style>
            <circle cx="12" cy="12" r="10" stroke="currentColor" stroke-width="3" stroke-dasharray="31.4 31.4" fill="none" opacity="0.3"></circle>
            <circle cx="12" cy="12" r="10" stroke="currentColor" stroke-width="3" stroke-dasharray="15 30" fill="none"></circle>
          </svg>
          Ожидание авторизации в браузере...
        `;
      }

      // 1. Start the temporary OAuth server in Rust (fixed port 14235)
      await invoke("start_oauth_server");
      console.log("[auth] Local OAuth server started on port 14235");

      // 2. Initiate Google OAuth via Supabase and get the authorization URL
      // We use the local Rust server as the redirect target
      const { data, error } = await supabase.auth.signInWithOAuth({
        provider: 'google',
        options: {
          redirectTo: 'http://localhost:14235/callback',
          skipBrowserRedirect: true
        }
      });

      if (error) throw error;
      if (!data?.url) throw new Error("Не удалось получить URL для авторизации");

      console.log("[auth] Opening Google OAuth URL in external browser:", data.url);

      // 3. Open the URL in the default browser using Rust open_url command
      await invoke("open_url", { url: data.url });
    } catch (err) {
      console.error("[auth] Google auth initiation failed:", err);
      showAuthError("Не удалось запустить авторизацию: " + err.message);
      if (btnGoogleEl) {
        btnGoogleEl.disabled = false;
        btnGoogleEl.innerHTML = originalGoogleBtnHtml;
      }
    }
  });

  // Handle Logout
  btnLogoutEl?.addEventListener("click", async () => {
    btnLogoutEl.disabled = true;
    const origText = btnLogoutEl.innerHTML;
    btnLogoutEl.textContent = "Выход...";

    try {
      // Try clean signout first
      await supabase.auth.signOut();
    } catch (err) {
      console.warn("[logout] signOut API call failed, forcing local cleanup:", err);
    }

    // Forcefully clean localStorage except preserved keys
    const preservedKeys = [
      "minecraft-p2p-client-id",
      "minecraft-p2p-profile-v1",
      "minecraft-p2p-external-servers-v1",
      "minecraft-p2p-ignored-ports-v1",
      "minecraft-p2p-theme",
      "minecraft-p2p-language",
      "minecraft-p2p-accent"
    ];
    for (let i = localStorage.length - 1; i >= 0; i--) {
      const key = localStorage.key(i);
      if (key && !preservedKeys.includes(key)) {
        localStorage.removeItem(key);
      }
    }

    // Clear any leftover supabase auth tokens specifically
    for (let i = localStorage.length - 1; i >= 0; i--) {
      const key = localStorage.key(i);
      if (key && (key.startsWith("sb-") || key.includes("supabase"))) {
        localStorage.removeItem(key);
      }
    }

    // Clear sessionStorage
    sessionStorage.clear();

    // Reset local profile state
    state.profile = {
      nickname: "Player",
      avatarDataUrl: null,
      customId: null,
      customBanner: null
    };
    storedDisplayId = null;
    saveProfileState();

    // Reset UI to login screen immediately to prevent flickering
    authOverlayEl?.classList.remove("hidden");

    // Reload with a small timeout to let the storage clear operations commit
    setTimeout(() => {
      window.location.reload();
    }, 100);
  });
}

// NOTE: initAuth() is already called at line ~2557 via `await initAuth()`.
// Duplicate call removed to prevent double OAuth listener registration.

// Filter Modal Logic (runs immediately — ES modules are already deferred)
const filterModal = document.getElementById('filter-modal');
const openFilterBtn = document.getElementById('open-filter-modal');
const closeFilterBtn = document.getElementById('close-filter-modal');
const applyFiltersBtn = document.getElementById('apply-filters-button');

if (openFilterBtn && filterModal) {
  openFilterBtn.addEventListener('click', () => filterModal.classList.remove('hidden'));
}
if (closeFilterBtn && filterModal) {
  closeFilterBtn.addEventListener('click', () => filterModal.classList.add('hidden'));
}
document.querySelector("[data-close-filter-modal]")?.addEventListener("click", () => {
  filterModal?.classList.add("hidden");
});

if (applyFiltersBtn && filterModal) {
  applyFiltersBtn.addEventListener('click', () => {
    if (typeof renderServers === 'function') renderServers();
    filterModal.classList.add('hidden');
  });
}

const serverSearchInput = document.getElementById('server-search-input');
if (serverSearchInput) {
  serverSearchInput.addEventListener('input', () => {
    if (typeof renderServers === 'function') renderServers();
  });
}
