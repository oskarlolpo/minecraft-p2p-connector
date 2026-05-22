const fs = require('fs');
const path = require('path');

const mainJsPath = path.join(__dirname, 'src', 'main.js');
let content = fs.readFileSync(mainJsPath, 'utf8');
if (content.charCodeAt(0) === 0xFEFF || content.includes('\0')) {
  // Try utf16le if we see null bytes or BOM
  content = fs.readFileSync(mainJsPath, 'utf16le');
}

// 1. Settings Tabs Logic
if (!content.includes('settingsTabs.forEach')) {
  content += `\n
// Settings Tabs Logic
document.addEventListener("DOMContentLoaded", () => {
  const settingsTabs = document.querySelectorAll('.settings-tab');
  const settingsTabContents = document.querySelectorAll('.settings-tab-content');

  settingsTabs.forEach(tab => {
    tab.addEventListener('click', () => {
      settingsTabs.forEach(t => t.classList.remove('active'));
      settingsTabContents.forEach(c => c.classList.remove('active'));
      tab.classList.add('active');
      const targetId = 'tab-' + tab.dataset.tab;
      const targetContent = document.getElementById(targetId);
      if (targetContent) {
        targetContent.classList.add('active');
      }
    });
  });
});
`;
}

// 2. Filter Modal Logic
if (!content.includes('open-filter-modal')) {
  content += `\n
// Filter Modal Logic
document.addEventListener("DOMContentLoaded", () => {
  const filterModal = document.getElementById('filter-modal');
  const openFilterBtn = document.getElementById('open-filter-modal');
  const closeFilterBtn = document.getElementById('close-filter-modal');
  const applyFiltersBtn = document.getElementById('apply-filters-button');
  
  if (openFilterBtn) {
    openFilterBtn.addEventListener('click', () => {
      filterModal.classList.remove('hidden');
    });
  }
  
  if (closeFilterBtn) {
    closeFilterBtn.addEventListener('click', () => {
      filterModal.classList.add('hidden');
    });
  }
  
  if (applyFiltersBtn) {
    applyFiltersBtn.addEventListener('click', () => {
      filterModal.classList.add('hidden');
      if (typeof renderServers === 'function') {
         // trigger render
         renderServers();
      }
    });
  }

  // Also search input
  const searchInput = document.getElementById('server-search-input');
  if (searchInput) {
    searchInput.addEventListener('input', () => {
      if (typeof renderServers === 'function') renderServers();
    });
  }
});
`;
}

// 3. Patch renderServers to include filtering
if (!content.includes('// FILTER INJECTED')) {
  const renderServersStart = content.indexOf('function renderServers() {');
  if (renderServersStart !== -1) {
    const listUpdateStart = content.indexOf('serverListEl.innerHTML = state.servers', renderServersStart);
    if (listUpdateStart !== -1) {
      const injectString = `
    // FILTER INJECTED
    const searchInput = document.getElementById('server-search-input');
    const filterTheme = document.getElementById('filter-theme');
    const searchTerm = searchInput ? searchInput.value.toLowerCase() : '';
    const themeFilter = filterTheme ? filterTheme.value : 'all';
    
    let filteredServers = state.servers.filter(server => {
      const nameMatch = server.name && server.name.toLowerCase().includes(searchTerm);
      const themeMatch = themeFilter === 'all' || server.theme === themeFilter;
      return nameMatch && themeMatch;
    });

    lobbyCountEl.textContent = t("lobbyCount", { count: filteredServers.length });
    if (!filteredServers.length) {
      serverListEl.innerHTML = \`<div class="empty-state">\${escapeHtml(t("noServers"))}</div>\`;
      return;
    }

    serverListEl.innerHTML = filteredServers
      `;
      
      // replace "serverListEl.innerHTML = state.servers" with the injectString
      content = content.replace('serverListEl.innerHTML = state.servers', injectString.trim());
    }
  }
}

// 4. Patch host button to have creating state
if (!content.includes('// HOST BUTTON INJECTED')) {
  // The host button click listener is likely around here:
  // hostButtonEl.addEventListener("click", ...
  const hostClickRegex = /hostButtonEl\.addEventListener\(\s*["']click["']\s*,\s*async\s*\(\)\s*=>\s*\{/g;
  content = content.replace(hostClickRegex, `hostButtonEl.addEventListener("click", async () => {
      // HOST BUTTON INJECTED
      const originalText = hostButtonEl.innerHTML;
      hostButtonEl.disabled = true;
      hostButtonEl.innerHTML = '<span class="spinner" style="display:inline-block; margin-right:8px; border:2px solid var(--text-base); border-top-color:transparent; border-radius:50%; width:14px; height:14px; animation: spin 1s linear infinite;"></span>Создание...';
      try {
  `);
  
  // also need to restore button state
  const hostHostCall = content.indexOf('await invoke("host_session"');
  if (hostHostCall !== -1) {
    // find the end of the try block or just patch the end of the function.
    // simpler: intercept the startHosting call completely or add it to catch.
    // Instead of complex AST, let's just restore it blindly after 2 seconds or on error.
    content += `\n
// Restore host button automatically after modal closes
document.addEventListener("DOMContentLoaded", () => {
  const hostModal = document.getElementById('host-modal');
  const observer = new MutationObserver((mutations) => {
    mutations.forEach((mutation) => {
      if (mutation.attributeName === 'class') {
        if (hostModal.classList.contains('hidden')) {
          const hostBtn = document.getElementById('host-button');
          if (hostBtn && hostBtn.disabled) {
            hostBtn.disabled = false;
            hostBtn.innerHTML = t("modalHostButton") || "Запустить хост";
          }
        }
      }
    });
  });
  if (hostModal) observer.observe(hostModal, { attributes: true });
});
`;
  }
}

// 5. Host configuration theme parameter
if (!content.includes('theme: document.getElementById("room-theme")?.value')) {
  content = content.replace(/name:\s*roomNameEl\.value\.trim\(\)/, `name: roomNameEl.value.trim(),\n          theme: document.getElementById("room-theme")?.value || "survival"`);
}

// write back in the same encoding
const originalExt = fs.readFileSync(mainJsPath);
if (originalExt[0] === 0xFF && originalExt[1] === 0xFE) {
   // utf16le bom
   const buffer = Buffer.from(content, 'utf16le');
   const bom = Buffer.from([0xFF, 0xFE]);
   fs.writeFileSync(mainJsPath, Buffer.concat([bom, buffer]));
} else {
   fs.writeFileSync(mainJsPath, content, 'utf8');
}
console.log("Successfully patched main.js");
