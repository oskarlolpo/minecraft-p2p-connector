const fs = require('fs');
const https = require('https');

const icons = [
  'x', 'help-circle', 'tree-pine', 'sword', 'palette', 'gamepad-2', 'skull', 'settings', 'globe', 'chevron-down'
];

async function fetchIcon(name) {
  return new Promise((resolve, reject) => {
    https.get(`https://api.iconify.design/lucide/${name}.svg`, (res) => {
      let data = '';
      res.on('data', chunk => data += chunk);
      res.on('end', () => {
        // change width/height to 1em
        let svg = data.replace(/width="[^"]+"/, 'width="1em"').replace(/height="[^"]+"/, 'height="1em"');
        resolve(svg);
      });
    }).on('error', reject);
  });
}

async function run() {
  const svgs = {};
  for (const icon of icons) {
    svgs[icon] = await fetchIcon(icon);
  }

  let html = fs.readFileSync('src/index.html', 'utf8');

  // Replace basic icons
  html = html.replace(/>×</g, `>${svgs['x']}<`);
  html = html.replace(/>\?</g, `>${svgs['help-circle']}<`);

  // Define themes
  const themeData = [
    { value: 'all', icon: svgs['globe'], label: 'Все темы' },
    { value: 'vanilla', icon: svgs['tree-pine'], label: 'Ванилла' },
    { value: 'survival', icon: svgs['sword'], label: 'Выживание' },
    { value: 'creative', icon: svgs['palette'], label: 'Творческий' },
    { value: 'minigames', icon: svgs['gamepad-2'], label: 'Мини-игры' },
    { value: 'anarchy', icon: svgs['skull'], label: 'Анархия' },
    { value: 'modded', icon: svgs['settings'], label: 'С модами' },
  ];

  function buildCustomDropdown(id) {
    const isFilter = id === 'filter-theme';
    const initialOptions = isFilter ? themeData : themeData.slice(1);
    
    let optionsHtml = initialOptions.map(o => `
      <div class="custom-select-option" data-value="\${o.value}">
        <span class="custom-select-icon">\${o.icon}</span>
        <span class="custom-select-label">\${o.label}</span>
      </div>`).join('');

    return `
    <div class="custom-select-wrapper" id="\${id}-wrapper">
      <select id="\${id}" class="hidden" aria-hidden="true" onchange="document.getElementById('\${id}-wrapper').querySelector('.custom-select-value').innerHTML = this.options[this.selectedIndex].innerHTML">
        \${initialOptions.map(o => \`<option value="\${o.value}"><span class="custom-select-icon">\${o.icon}</span><span class="custom-select-label">\${o.label}</span></option>\`).join('')}
      </select>
      <div class="custom-select" tabindex="0">
        <div class="custom-select-trigger">
          <div class="custom-select-value">
            <span class="custom-select-icon">\${initialOptions[0].icon}</span>
            <span class="custom-select-label">\${initialOptions[0].label}</span>
          </div>
          <span class="custom-select-arrow">\${svgs['chevron-down']}</span>
        </div>
        <div class="custom-select-dropdown">
          \${optionsHtml}
        </div>
      </div>
    </div>`;
  }

  // Replace <select id="room-theme" ... </select>
  html = html.replace(/<select id="room-theme"[\s\S]*?<\/select>/, buildCustomDropdown('room-theme'));
  // Replace <select id="filter-theme" ... </select>
  html = html.replace(/<select id="filter-theme"[\s\S]*?<\/select>/, buildCustomDropdown('filter-theme'));

  fs.writeFileSync('src/index.html', html);

  // Add CSS for custom dropdown
  let css = fs.readFileSync('src/styles.css', 'utf8');
  if (!css.includes('.custom-select-wrapper')) {
    css += `\n
/* CUSTOM SELECT DROPDOWN */
.custom-select-wrapper {
  position: relative;
  width: 100%;
}
.custom-select-wrapper select.hidden {
  display: none !important;
}
.custom-select {
  position: relative;
  width: 100%;
  border-radius: var(--radius-sm);
  border: 1px solid var(--line);
  background: var(--surface);
  color: var(--text-base);
  cursor: pointer;
  outline: none;
  user-select: none;
}
.custom-select:focus {
  border-color: var(--accent);
}
.custom-select-trigger {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 10px 12px;
}
.custom-select-value {
  display: flex;
  align-items: center;
  gap: 8px;
}
.custom-select-icon {
  display: flex;
  align-items: center;
  font-size: 1.1em;
}
.custom-select-arrow {
  display: flex;
  align-items: center;
  transition: transform 0.2s;
  color: var(--text-muted);
}
.custom-select.open .custom-select-arrow {
  transform: rotate(180deg);
}
.custom-select-dropdown {
  position: absolute;
  top: calc(100% + 4px);
  left: 0;
  width: 100%;
  background: var(--surface-raised);
  border: 1px solid var(--line);
  border-radius: var(--radius-sm);
  box-shadow: 0 4px 12px rgba(0,0,0,0.2);
  z-index: 100;
  opacity: 0;
  pointer-events: none;
  transform: translateY(-5px);
  transition: all 0.2s;
  max-height: 250px;
  overflow-y: auto;
}
.custom-select.open .custom-select-dropdown {
  opacity: 1;
  pointer-events: auto;
  transform: translateY(0);
}
.custom-select-option {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 10px 12px;
  transition: background 0.15s;
}
.custom-select-option:hover {
  background: color-mix(in srgb, var(--accent) 15%, transparent);
}
.custom-select-option.selected {
  background: color-mix(in srgb, var(--accent) 25%, transparent);
  color: var(--accent);
}
`;
    fs.writeFileSync('src/styles.css', css);
  }

  // Add JS to main.js for custom selects
  let js = fs.readFileSync('src/main.js', 'utf8');
  if (!js.includes('document.querySelectorAll(".custom-select")')) {
    js += `\n
document.querySelectorAll(".custom-select-wrapper").forEach(wrapper => {
  const select = wrapper.querySelector("select");
  const customSelect = wrapper.querySelector(".custom-select");
  const trigger = wrapper.querySelector(".custom-select-trigger");
  const valueContainer = wrapper.querySelector(".custom-select-value");
  const options = wrapper.querySelectorAll(".custom-select-option");

  // set initial
  options.forEach(opt => {
    if (opt.dataset.value === select.value) {
      opt.classList.add("selected");
      valueContainer.innerHTML = opt.innerHTML;
    }
  });

  trigger.addEventListener("click", () => {
    customSelect.classList.toggle("open");
  });

  customSelect.addEventListener("blur", () => {
    customSelect.classList.remove("open");
  });

  options.forEach(opt => {
    opt.addEventListener("click", (e) => {
      e.stopPropagation();
      options.forEach(o => o.classList.remove("selected"));
      opt.classList.add("selected");
      valueContainer.innerHTML = opt.innerHTML;
      select.value = opt.dataset.value;
      select.dispatchEvent(new Event("change"));
      customSelect.classList.remove("open");
    });
  });
});
`;
    fs.writeFileSync('src/main.js', js);
  }

  console.log('HTML, CSS, and JS updated');
}

run().catch(console.error);
