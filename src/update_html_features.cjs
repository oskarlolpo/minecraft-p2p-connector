const fs = require('fs');
let html = fs.readFileSync('G:/oskarlolpo project/minecraftjava/01_Active/p2p/src/index.html', 'utf8');

// Add network stats graph to Network Snapshot
if (!html.includes('network-stats-graph')) {
    html = html.replace(/<strong id="peer-count">0<\/strong>\s*<\/div>\s*<\/div>/, `<strong id="peer-count">0</strong>
                  </div>
                </div>
                <div class="network-stats-graph-container" style="margin-top: 16px; height: 60px;">
                  <canvas id="network-stats-graph"></canvas>
                  <div style="display: flex; justify-content: space-between; font-size: 0.75rem; color: var(--text-secondary); margin-top: 4px;">
                    <span id="network-stats-down">D: 0 KB/s</span>
                    <span id="network-stats-up">U: 0 KB/s</span>
                  </div>
                </div>`);
}

// Add Host Profiles UI
if (!html.includes('host-profile-select')) {
    html = html.replace(/<span class="eyebrow" data-i18n="modalLabel">ПАРАМЕТРЫ ХОСТА<\/span>\s*<h2 id="host-modal-title" data-i18n="modalTitle">Создать хост<\/h2>/, `<span class="eyebrow" data-i18n="modalLabel">ПАРАМЕТРЫ ХОСТА</span>
             <div style="display: flex; justify-content: space-between; align-items: center;"><h2 id="host-modal-title" data-i18n="modalTitle">Создать хост</h2>
<select id="host-profile-select" class="field" style="width: 150px; padding: 4px; margin-left: auto; margin-right: 8px;" data-i18n-title="hostProfiles"><option value="" data-i18n="profileDefault">По умолчанию</option></select>
<button id="save-host-profile" class="ghost-button" data-i18n="profileSaveAction">Сохранить профиль</button></div>`);
}

fs.writeFileSync('G:/oskarlolpo project/minecraftjava/01_Active/p2p/src/index.html', html);
console.log('index.html updated for Network Stats and Host Profiles.');
