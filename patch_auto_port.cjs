const fs = require('fs');
const path = require('path');

const mainJsPath = path.join(__dirname, 'src', 'main.js');
let content = fs.readFileSync(mainJsPath, 'utf8');
if (content.charCodeAt(0) === 0xFEFF || content.includes('\0')) {
  content = fs.readFileSync(mainJsPath, 'utf16le');
}

if (!content.includes('// AUTO PORT INJECTED')) {
  const hostModalOpenRegex = /openHostModalEl\.addEventListener\(\s*["']click["']\s*,\s*\(\)\s*=>\s*\{/g;
  content = content.replace(hostModalOpenRegex, `openHostModalEl.addEventListener("click", () => {
    // AUTO PORT INJECTED
    setTimeout(() => {
       const autoDetectBtn = document.getElementById('auto-detect-port');
       if (autoDetectBtn) {
         autoDetectBtn.click();
       }
    }, 100);
  `);
}

const originalExt = fs.readFileSync(mainJsPath);
if (originalExt[0] === 0xFF && originalExt[1] === 0xFE) {
   const buffer = Buffer.from(content, 'utf16le');
   const bom = Buffer.from([0xFF, 0xFE]);
   fs.writeFileSync(mainJsPath, Buffer.concat([bom, buffer]));
} else {
   fs.writeFileSync(mainJsPath, content, 'utf8');
}
console.log("Patched auto port");
