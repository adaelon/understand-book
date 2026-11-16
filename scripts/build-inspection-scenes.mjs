// Author reads complete, offline HTML. Only orbit and framing are shared.
import fs from 'node:fs';
import {
  fileURLToPath
} from 'node:url';
import path from 'node:path';
const base = fileURLToPath(new URL('../skills/presentation/methods/inspect-space/v1/', import.meta.url));
const stage = fs.readFileSync(path.join(base, 'stage.js'), 'utf8').trim();
for (const dir of ['example', 'sections']) {
  const source = fs.readFileSync(path.join(base, dir, 'scene.html'), 'utf8');
  if (!source.includes('/* INSPECTION_STAGE */')) throw Error('Missing orbit insertion in ' + dir);
  fs.writeFileSync(path.join(base, dir, 'index.html'), source.replace('/* INSPECTION_STAGE */', () => stage));
}
