import { defineConfig } from 'vite';
import vue from '@vitejs/plugin-vue';
import { fileURLToPath } from 'node:url';
export default defineConfig({ plugins: [vue()], publicDir: '../../tmp/focus-reading-fp1/public', build: {
  outDir: '../../tmp/focus-reading-fp4/dist', emptyOutDir: true,
  rollupOptions: { input: fileURLToPath(new URL('./playwright/fixtures/focus-turn.html', import.meta.url)) },
} });
