import { defineConfig } from '@playwright/test';

export default defineConfig({
  testDir: './playwright',
  testMatch: 'interactive-teaching-om6.spec.ts',
  workers: 1,
  use: { baseURL: 'http://127.0.0.1:4195' },
  webServer: {
    command: 'pnpm dev --host 127.0.0.1 --port 4195',
    url: 'http://127.0.0.1:4195/',
    reuseExistingServer: false,
  },
});
