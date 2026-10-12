import { defineConfig } from '@playwright/test';

export default defineConfig({
  testDir: './playwright',
  testMatch: 'tutor-building-blocks-tb5.spec.ts',
  outputDir: '../../docs/performance/tutor-building-blocks-tb5/playwright-results',
  workers: 1,
  timeout: 90_000,
  use: { baseURL: 'http://127.0.0.1:4186', headless: true },
  webServer: {
    command: 'pnpm dev --host 127.0.0.1 --port 4186',
    url: 'http://127.0.0.1:4186',
    reuseExistingServer: true,
  },
});
