import { defineConfig } from 'vite';
import vue from '@vitejs/plugin-vue';
import { fileURLToPath } from 'node:url';
export default defineConfig({ plugins: [vue()], publicDir: '../../tmp/focus-reading-fp1/public', build: {
  outDir: '../../tmp/focus-reading-fp3/dist', emptyOutDir: true,
  rollupOptions: { input: fileURLToPath(new URL('./playwright/fixtures/focus-ready.html', import.meta.url)) },
} });
