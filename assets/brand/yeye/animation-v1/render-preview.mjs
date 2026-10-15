import fs from 'node:fs';
import path from 'node:path';
import { createRequire } from 'node:module';
import { fileURLToPath } from 'node:url';

// Render the authored Lottie scenes with the official player's Skottie engine.
// Usage: node render-preview.mjs <official-player-folder> <temporary-frame-folder>
const here = path.dirname(fileURLToPath(import.meta.url));
const authoring = path.resolve(process.argv[2]);
const output = path.resolve(process.argv[3]);
const require = createRequire(path.join(authoring, 'package.json'));
const canvasKitPath = require.resolve('canvaskit-wasm/full');
const init = require(canvasKitPath);
const ck = await init({ wasmBinary: fs.readFileSync(path.join(path.dirname(canvasKitPath), 'canvaskit.wasm')) });
const surface = ck.MakeSurface(1080, 372);
const canvas = surface.getCanvas();
const scenes = ['idle', 'reading', 'success'].map(id => {
  const json = fs.readFileSync(path.join(here, id, 'lottie.json'), 'utf8');
  const data = JSON.parse(json);
  const assets = Object.fromEntries(data.assets.filter(a => a.p).map(a => {
    const bytes = fs.readFileSync(path.join(here, id, a.p));
    return [a.p, bytes.buffer.slice(bytes.byteOffset, bytes.byteOffset + bytes.byteLength)];
  }));
  return { id, data, animation: ck.MakeManagedAnimation(json, assets) };
});
fs.mkdirSync(output, { recursive: true });
const frames = 100; // 5 seconds at 20fps; shorter one-shot scenes hold their end pose.
for (let f = 0; f < frames; f++) {
  canvas.clear(ck.Color(247, 243, 235, 1));
  scenes.forEach(({ data, animation }, col) => {
    animation.seekFrame(Math.min(data.op - 1, f * data.fr / 20));
    animation.render(canvas, ck.LTRBRect(col * 360, 6, col * 360 + 360, 366));
  });
  surface.flush();
  const snapshot = surface.makeImageSnapshot();
  const png = snapshot.encodeToBytes();
  fs.writeFileSync(path.join(output, `${String(f).padStart(3, '0')}.png`), png);
  snapshot.delete();
}
for (const { animation } of scenes) animation.delete();
surface.dispose();
console.log(`Rendered ${frames} frames to ${output}`);
