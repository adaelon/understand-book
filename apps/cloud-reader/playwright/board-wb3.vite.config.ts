import { defineConfig } from 'vite';
import vue from '@vitejs/plugin-vue';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { pdfOcrAssets } from '@understand-book/web/build/pdf-ocr-assets.mjs';

// HTTP proxy streams SSE while the controlled model awaits a browser receipt.
// Playwright route.fetch would buffer that stream and deadlock presentation.act.
const fixture = JSON.parse(readFileSync(fileURLToPath(new URL('../../../tmp/board-wb3/fixture.json', import.meta.url)), 'utf8'));
const upstream = {
  target: process.env.WB3_BACKEND_URL ?? 'http://127.0.0.1:4220',
  changeOrigin: false,
  headers: { host: 'reader.example', origin: 'https://reader.example', cookie: fixture.cookie },
};

export default defineConfig({
  resolve: { alias: { '/reader-src': fileURLToPath(new URL('../../../packages/web/src', import.meta.url)) } },
  plugins: [vue(), pdfOcrAssets()],
  server: { host: '127.0.0.1', port: 4222, strictPort: true, proxy: { '/api': upstream, '/books': upstream } },
});
