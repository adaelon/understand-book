import { defineConfig, configDefaults } from 'vitest/config';
import vue from '@vitejs/plugin-vue';
import { fileURLToPath } from 'node:url';
import { pdfOcrAssets } from '@understand-book/web/build/pdf-ocr-assets.mjs';
import { boardPreviewAssets } from '@understand-book/web/build/board-preview-assets.mjs';
import { readerEdition } from '@understand-book/web/build/reader-edition.mjs';

const backend = process.env.UNDERSTAND_BOOK_ADDR ? `http://${process.env.UNDERSTAND_BOOK_ADDR}` : 'http://127.0.0.1:8787';

export default defineConfig({
  resolve: { alias: { '/reader-src': fileURLToPath(new URL('../../packages/web/src', import.meta.url)) } },
  publicDir: fileURLToPath(new URL('../../packages/web/public', import.meta.url)),
  define: { 'import.meta.env.VITE_MULTI_USER': JSON.stringify('1') },
  plugins: [vue(), pdfOcrAssets(), boardPreviewAssets(), readerEdition(true), {
    name: 'reader-startup-styles', apply: 'build',
    transformIndexHtml: { order: 'post', handler: (html, context) => context.path === '/home.html' ? html
      : html.replace(/<link\b[^>]*rel="stylesheet"[^>]*>/g, tag => tag.replace('rel="stylesheet"', 'rel="preload" as="style" data-app-styles')) },
  }],
  build: { emptyOutDir: true, rollupOptions: { input: {
    reader: fileURLToPath(new URL('./index.html', import.meta.url)),
    home: fileURLToPath(new URL('./home.html', import.meta.url)),
  } } },
  test: { exclude: [...configDefaults.exclude, 'playwright/**'], setupFiles: ['./test-setup.ts'] },
  server: { proxy: { '/api': { target: backend, changeOrigin: true, rewrite: path => path.replace(/^\/api/, '') } } },
});
