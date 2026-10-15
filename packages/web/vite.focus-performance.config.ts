import { defineConfig } from 'vite';
import vue from '@vitejs/plugin-vue';
import { fileURLToPath } from 'node:url';

// Isolated production bundle; it never changes the shipped app's entry points.
export default defineConfig({
  plugins: [vue()],
  publicDir: '../../tmp/focus-reading-fp1/public',
  build: {
    outDir: '../../tmp/focus-reading-fp1/dist',
    emptyOutDir: true,
    rollupOptions: { input: fileURLToPath(new URL('./playwright/fixtures/focus-reading.html', import.meta.url)) },
  },
});
