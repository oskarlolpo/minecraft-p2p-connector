const fs = require('fs');
const path = require('path');

const mainJsPath = path.join(__dirname, 'src', 'main.js');
let content = fs.readFileSync(mainJsPath, 'utf8');
if (content.charCodeAt(0) === 0xFEFF || content.includes('\0')) {
  content = fs.readFileSync(mainJsPath, 'utf16le');
}

if (!content.includes('host-theme-badge')) {
  // Inject the theme badge next to the server name
  // Original is: <strong>${escapeHtml(server.name || "Minecraft Server")}</strong>
  content = content.replace(/<strong>\$\{escapeHtml\(server\.name\s*\|\|\s*["']Minecraft Server["']\)\}<\/strong>/, `<strong>\${escapeHtml(server.name || "Minecraft Server")}</strong>
              \${server.theme && server.theme !== 'all' ? \`<span class="host-theme-badge theme-\${server.theme}">\${escapeHtml(server.theme)}</span>\` : ''}`);
}

const originalExt = fs.readFileSync(mainJsPath);
if (originalExt[0] === 0xFF && originalExt[1] === 0xFE) {
   const buffer = Buffer.from(content, 'utf16le');
   const bom = Buffer.from([0xFF, 0xFE]);
   fs.writeFileSync(mainJsPath, Buffer.concat([bom, buffer]));
} else {
   fs.writeFileSync(mainJsPath, content, 'utf8');
}
console.log("Patched theme badge");
