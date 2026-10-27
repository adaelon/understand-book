import { defineConfig } from 'vite';
import vue from '@vitejs/plugin-vue';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { pdfOcrAssets } from '../pdf-ocr-assets.mjs';

// Forward APIs/SSE unbuffered; only the controlled model belongs to the fixture.
const fixture = JSON.parse(readFileSync(fileURLToPath(new URL('../../../tmp/board-wb4/fixture.json', import.meta.url)), 'utf8'));
const upstream = { target: process.env.WB4_BACKEND_URL ?? 'http://127.0.0.1:4230', changeOrigin: false,
  headers: { host: 'reader.example', origin: 'https://reader.example', cookie: fixture.cookie } };
export default defineConfig({ plugins: [vue(), pdfOcrAssets()], server: {
  host: '127.0.0.1', port: 4232, strictPort: true, proxy: { '/api': upstream, '/books': upstream },
} });
