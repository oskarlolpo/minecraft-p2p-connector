const fs = require('fs');
let html = fs.readFileSync('G:/oskarlolpo project/minecraftjava/01_Active/p2p/src/index.html', 'utf8');

const friendsAllStr = `<section class="panel">
                <div class="panel-head">
                  <h2 data-i18n="friendsAll">`;
const settingsPageStr = `<section id="page-settings"`;

const startIndex = html.indexOf('<section class="panel">', html.indexOf('friendsAll') - 100);
const endIndex = html.indexOf(settingsPageStr);

if (startIndex !== -1 && endIndex !== -1 && startIndex < endIndex) {
    html = html.substring(0, startIndex) + html.substring(endIndex);
    fs.writeFileSync('G:/oskarlolpo project/minecraftjava/01_Active/p2p/src/index.html', html);
    console.log('Fixed index.html structure by removing hanging sections.');
} else {
    console.log('Could not find the exact bounds.', {startIndex, endIndex});
}
