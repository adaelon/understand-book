// Run with sharp available to Node (the Codex bundled runtime includes it).
import { createRequire } from 'node:module';
import { mkdir, writeFile } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';

const sharp = createRequire(import.meta.url)('sharp');
const output = new URL('../../../../packages/web/src/assets/yeye/', import.meta.url);
await mkdir(output, { recursive: true });
const assets = [];
for (const [variant, source, file, size] of [
  ['idle', '../v7/yeye-idle.png', 'yeye-idle.webp', 600],
  ['reading', '../v7/yeye-reading-cream.png', 'yeye-reading-cream.webp', 600],
  ['avatar', '../avatar-v1/yeye-avatar.png', 'yeye-avatar.webp', 96],
]) {
  const input = fileURLToPath(new URL(source, import.meta.url));
  const metadata = await sharp(input).metadata();
  const result = await sharp(input).resize(size, size).webp({ lossless: true }).toFile(fileURLToPath(new URL(file, output)));
  assets.push({ variant, source, sourceWidth: metadata.width, sourceHeight: metadata.height,
    output: `packages/web/src/assets/yeye/${file}`, width: result.width, height: result.height, bytes: result.size });
}
await writeFile(new URL('manifest.json', import.meta.url), JSON.stringify({
  created: '2026-10-13', format: 'lossless WebP with source alpha',
  export: 'Lanczos3 downscale; preserve original square composition and transparency',
  display: { fullBodyCssPx: [120, 128, 160, 220, 300], avatarCssPx: [24, 32] }, assets,
}, null, 2) + '\n');
console.log(JSON.stringify(assets, null, 2));
