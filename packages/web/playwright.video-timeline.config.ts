import { defineConfig } from '@playwright/test';

export default defineConfig({
  testDir: './playwright', testMatch: 'video-timeline.spec.ts', timeout: 60000,
  use: { baseURL: 'http://127.0.0.1:4216', channel: 'chrome', viewport: { width: 1280, height: 900 } },
  webServer: { command: 'pnpm dev --host 127.0.0.1 --port 4216 --strictPort', url: 'http://127.0.0.1:4216', reuseExistingServer: true },
});
