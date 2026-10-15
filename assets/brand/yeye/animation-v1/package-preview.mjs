import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const here = path.dirname(fileURLToPath(import.meta.url));
const scenes = Object.fromEntries(['idle', 'reading', 'success'].map(id => {
  const data = JSON.parse(fs.readFileSync(path.join(here, id, 'lottie.json'), 'utf8'));
  for (const asset of data.assets.filter(a => a.p)) {
    asset.p = `data:image/png;base64,${fs.readFileSync(path.join(here, id, asset.p)).toString('base64')}`;
    asset.e = 1;
  }
  return [id, data];
}));
const runtime = fs.readFileSync(path.join(here, 'vendor/lottie-5.13.0.min.js'), 'utf8');
const license = fs.readFileSync(path.join(here, 'licenses/lottie-web-MIT.txt'), 'utf8');
const app = fs.readFileSync(path.join(here, 'preview.js'), 'utf8')
  .replace('path: `${id}/lottie.json`', 'animationData: YEYE_DATA[id]');
const html = fs.readFileSync(path.join(here, 'index.html'), 'utf8')
  .replace('<script src="vendor/lottie-5.13.0.min.js"></script>',
    () => `<!-- Lottie Web 5.13.0\n${license}\n-->\n<script>${runtime}</script>\n<script>const YEYE_DATA=${JSON.stringify(scenes)};</script>`)
  .replace('<script type="module" src="preview.js"></script>', () => `<script type="module">${app}</script>`);
fs.writeFileSync(path.join(here, 'preview.html'), html);
console.log('Created standalone preview.html with embedded runtime and PNG assets.');
