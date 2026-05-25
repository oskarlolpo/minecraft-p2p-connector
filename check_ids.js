const fs = require('fs');

const mainJs = fs.readFileSync('src/main.js', 'utf8');
const indexHtml = fs.readFileSync('src/index.html', 'utf8');

const regex = /document\.querySelector\(['"]#(.*?)['"]\)/g;
let match;
while ((match = regex.exec(mainJs)) !== null) {
  const id = match[1];
  if (!indexHtml.includes(`id="${id}"`)) {
    console.log(`MISSING ID IN HTML: ${id}`);
  }
}
