import { defineConfig } from '@playwright/test';

export default defineConfig({
  testDir: '.', testMatch: ['thought-window.spec.ts', 'thought-placements.spec.ts'], timeout: 45_000, workers: 1,
  use: { baseURL: 'http://127.0.0.1:4186', channel: 'msedge', screenshot: 'only-on-failure' },
  webServer: { command: 'pnpm dev --mode multi --host 127.0.0.1 --port 4186', url: 'http://127.0.0.1:4186', reuseExistingServer: true },
});
