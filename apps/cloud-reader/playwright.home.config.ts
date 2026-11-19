import { defineConfig } from '@playwright/test';

export default defineConfig({
  testDir: './playwright',
  testMatch: 'product-home.spec.ts',
  timeout: 30_000,
  use: { baseURL: 'http://127.0.0.1:5197', reducedMotion: 'reduce' },
  webServer: {
    command: 'pnpm dev --host 127.0.0.1 --port 5197 --strictPort',
    url: 'http://127.0.0.1:5197/home.html',
    reuseExistingServer: true,
  },
});
