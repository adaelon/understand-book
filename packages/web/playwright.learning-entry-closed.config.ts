import { defineConfig } from '@playwright/test';

export default defineConfig({
  testDir: './playwright', testMatch: 'closed-book.spec.ts', timeout: 90_000, workers: 1,
  outputDir: '../../.tmp/learning-entry-20261116/closed-book-ui-results',
  use: { baseURL: 'http://127.0.0.1:4266', screenshot: 'only-on-failure' },
  webServer: { command: 'pnpm dev --host 127.0.0.1 --port 4266 --strictPort', url: 'http://127.0.0.1:4266', reuseExistingServer: true },
});
