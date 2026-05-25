const fs = require('fs');
const mainJs = fs.readFileSync('main.js', 'utf8');
const indexHtml = fs.readFileSync('index.html', 'utf8');
const regex = /document\.querySelector\(['"]([^'"]+)['"]\)/g;
let match;
const notFound = [];
while ((match = regex.exec(mainJs)) !== null) {
  const selector = match[1];
  if (selector.startsWith('#')) {
    const id = selector.slice(1);
    if (!indexHtml.includes(`id="${id}"`) && !indexHtml.includes(`id='${id}'`)) {
      notFound.push(selector);
    }
  }
}
console.log('Selectors not found in HTML:', [...new Set(notFound)]);
