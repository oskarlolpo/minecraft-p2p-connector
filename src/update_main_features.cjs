const fs = require('fs');
let code = fs.readFileSync('G:/oskarlolpo project/minecraftjava/01_Active/p2p/src/main.js', 'utf8');

// 1. Add Network Stats Graph Logic
const networkStatsCode = `
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
    networkStatsDownEl.textContent = \`D: \${(currentBytesIn / 1024).toFixed(1)} KB/s\`;
    networkStatsUpEl.textContent = \`U: \${(currentBytesOut / 1024).toFixed(1)} KB/s\`;
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
`;

if (!code.includes('network_stats')) {
    code = code.replace(/await listen\(\"peer_connected\",/, `await listen("network_stats", (event) => {
  if (event.payload.bytesIn) currentBytesIn += event.payload.bytesIn;
  if (event.payload.bytesOut) currentBytesOut += event.payload.bytesOut;
});\n\nawait listen("peer_connected",`);
    
    code = code + '\n' + networkStatsCode;
}

// 2. Add Host Profiles Logic
const hostProfilesCode = `
// Host Profiles
const hostProfileSelect = document.querySelector("#host-profile-select");
const saveHostProfileBtn = document.querySelector("#save-host-profile");

function loadHostProfiles() {
    try {
        return JSON.parse(localStorage.getItem("host_profiles") || "{}");
    } catch { return {}; }
}
function saveHostProfiles(profiles) {
    localStorage.setItem("host_profiles", JSON.stringify(profiles));
}
function updateProfileSelect() {
    if (!hostProfileSelect) return;
    const profiles = loadHostProfiles();
    const currentVal = hostProfileSelect.value;
    hostProfileSelect.innerHTML = '<option value="">По умолчанию</option>';
    for (const name of Object.keys(profiles)) {
        const opt = document.createElement("option");
        opt.value = name;
        opt.textContent = name;
        hostProfileSelect.appendChild(opt);
    }
    hostProfileSelect.value = profiles[currentVal] ? currentVal : "";
}

hostProfileSelect?.addEventListener("change", () => {
    const val = hostProfileSelect.value;
    if (!val) return;
    const profiles = loadHostProfiles();
    const p = profiles[val];
    if (p) {
        if (hostRoomNameEl) hostRoomNameEl.value = p.roomName || "";
        if (hostPasswordEl) hostPasswordEl.value = p.password || "";
        if (hostPortEl) hostPortEl.value = p.port || "";
        if (hostBedrockEnabledEl) hostBedrockEnabledEl.checked = Boolean(p.bedrock);
        if (hostBedrockPortEl) hostBedrockPortEl.value = p.bedrockPort || "";
        saveHostState();
    }
});

saveHostProfileBtn?.addEventListener("click", () => {
    const name = prompt("Введите имя профиля:");
    if (!name) return;
    const profiles = loadHostProfiles();
    profiles[name] = {
        roomName: hostRoomNameEl?.value || "",
        password: hostPasswordEl?.value || "",
        port: hostPortEl?.value || "",
        bedrock: hostBedrockEnabledEl?.checked || false,
        bedrockPort: hostBedrockPortEl?.value || ""
    };
    saveHostProfiles(profiles);
    updateProfileSelect();
    hostProfileSelect.value = name;
    addLog("Профиль сохранен: " + name);
});

// Call on startup
updateProfileSelect();
`;

if (!code.includes('hostProfileSelect')) {
    code = code + '\n' + hostProfilesCode;
}

fs.writeFileSync('G:/oskarlolpo project/minecraftjava/01_Active/p2p/src/main.js', code);
console.log('main.js updated for Network Stats and Host Profiles.');
