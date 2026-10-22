import { createRequire } from 'node:module';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { renameSync } from 'node:fs';
import { pdfOcrAssets } from '../../../packages/web/pdf-ocr-assets.mjs';
const require = createRequire(new URL('../../../packages/web/package.json', import.meta.url));
const { build } = await import(pathToFileURL(require.resolve('vite')).href);
const { default: vue } = await import(pathToFileURL(require.resolve('@vitejs/plugin-vue')).href);
const root = fileURLToPath(new URL('../../../packages/web/', import.meta.url));
const outDir = fileURLToPath(new URL('../local-reader/', import.meta.url));
await build({ root, configFile: false, plugins: [vue(), pdfOcrAssets()], base: './',
  build: { outDir, emptyOutDir: true, rollupOptions: { input: `${root}/desktop.html` } } });
renameSync(`${outDir}/desktop.html`, `${outDir}/index.html`);
