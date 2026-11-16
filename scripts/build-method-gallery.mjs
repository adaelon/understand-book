// Keep the repository gallery available offline without a second hand-written directory.
import fs from 'node:fs';
import path from 'node:path';
import {
  fileURLToPath
} from 'node:url';
const dir = fileURLToPath(new URL('../skills/presentation/methods/', import.meta.url)),
  gallery = path.join(dir, 'index.html');
const {
  methods
} = JSON.parse(fs.readFileSync(path.join(dir, 'catalog.json'), 'utf8'));
const entries = methods.map(({
  id,
  revision,
  name,
  obstacle,
  boundary,
  root,
  entrypoint,
  scenes
}) => ({
  id,
  revision,
  name,
  obstacle,
  boundary,
  path: path.relative(dir, path.resolve(dir, '../../..', root)).replaceAll('\\', '/'),
  entrypoint,
  ...scenes && {
    scenes
  }
}));
const html = fs.readFileSync(gallery, 'utf8'),
  pattern = /(<script id="directory" type="application\/json">)[\s\S]*?(<\/script>)/;
if (!pattern.test(html)) throw Error('Missing method directory in gallery');
fs.writeFileSync(gallery, html.replace(pattern, (_, a, b) => a + JSON.stringify(entries).replaceAll('<',
  '\\u003c') + b));
