const fs = require('fs');
const js = fs.readFileSync('src/main.js', 'utf8');
const html = fs.readFileSync('src/index.html', 'utf8');

const regex = /getElementById\(['"]([^'"]+)['"]\)/g;
let match;
const missing = new Set();
while ((match = regex.exec(js)) !== null) {
  const id = match[1];
  if (!html.includes('id="' + id + '"') && !html.includes("id='" + id + "'")) {
    missing.add(id);
  }
}
console.log('Missing getElementById:', [...missing]);

const qRegex = /querySelector\(['"]#([^'"]+)['"]\)/g;
while ((match = qRegex.exec(js)) !== null) {
  const id = match[1];
  if (!html.includes('id="' + id + '"') && !html.includes("id='" + id + "'")) {
    missing.add(id);
  }
}
console.log('Missing all IDs:', [...missing]);
