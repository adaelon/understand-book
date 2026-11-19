import { defineConfig } from '@playwright/test';

export default defineConfig({
  testDir: './playwright',
  testMatch: ['registration-entry.spec.ts', 'presentation-immediate.spec.ts'],
  timeout: 60_000, workers: 1,
  use: { baseURL: 'http://127.0.0.1:4268', screenshot: 'only-on-failure' },
  webServer: { command: 'pnpm dev --host 127.0.0.1 --port 4268 --strictPort',
    url: 'http://127.0.0.1:4268', reuseExistingServer: false },
});
