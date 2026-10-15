import { defineConfig } from '@playwright/test';

export default defineConfig({
  testDir: './playwright',
  testMatch: 'focus-reading-performance.spec.ts',
  timeout: 180_000,
  workers: 1,
  fullyParallel: false,
  retries: 0,
  outputDir: '../../tmp/focus-reading-fp1/test-results',
  use: { baseURL: 'http://127.0.0.1:4188', viewport: { width:1440,height:900 }, headless: true },
  webServer: {
    command: 'pnpm exec vite preview --config vite.focus-performance.config.ts --host 127.0.0.1 --port 4188 --strictPort',
    url: 'http://127.0.0.1:4188/playwright/fixtures/focus-reading.html',
    reuseExistingServer: false,
  },
});
