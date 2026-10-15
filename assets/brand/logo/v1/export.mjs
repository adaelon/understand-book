// Run with sharp available to Node; exports all icons from the approved vector master.
import { createRequire } from 'node:module';
import { mkdir, writeFile, copyFile } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';
const sharp = createRequire(import.meta.url)('sharp');
const master = new URL('understand-book-logo.svg', import.meta.url);
const web = new URL('../../../../packages/web/', import.meta.url);
await mkdir(new URL('src/assets/brand/', web), { recursive: true });
await mkdir(new URL('public/brand/', web), { recursive: true });
await copyFile(master, new URL('src/assets/brand/understand-book-logo.svg', web));
await copyFile(master, new URL('public/brand/logo-v1.svg', web));
await sharp(fileURLToPath(master)).resize(1024, 1024).png().toFile(fileURLToPath(new URL('understand-book-logo-transparent.png', import.meta.url)));
for (const size of [32, 180, 192, 512]) {
  await sharp(fileURLToPath(master)).resize(size, size)
    .flatten({ background: '#f7f3eb' }).png()
    .toFile(fileURLToPath(new URL(`public/brand/icon-v1-${size}.png`, web)));
}
await writeFile(new URL('public/brand/site-v1.webmanifest', web), JSON.stringify({
  name: 'Understand Book', short_name: 'Understand Book', lang: 'zh-CN', start_url: '/',
  display: 'standalone', background_color: '#f7f3eb', theme_color: '#b65a17',
  icons: [192, 512].map(size => ({ src: `/brand/icon-v1-${size}.png`, sizes: `${size}x${size}`, type: 'image/png', purpose: 'any' })),
}, null, 2) + '\n');
console.log('Exported approved SVG, transparent PNG, and browser/touch icons.');
