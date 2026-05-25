const fs = require('fs');
let code = fs.readFileSync('main.js', 'utf8');
code = code.replace(/([a-zA-Z0-9_]+El)\.addEventListener/g, '$1?.addEventListener');
fs.writeFileSync('main.js', code);
console.log('Fixed all El.addEventListener');
