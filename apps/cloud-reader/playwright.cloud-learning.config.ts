import { defineConfig } from '@playwright/test';

export default defineConfig({
  testDir: './playwright',
  testMatch: 'cloud-learning.spec.ts',
  workers: 1,
  timeout: 180_000,
  use: { baseURL: 'http://127.0.0.1:4204', browserName: 'chromium', viewport: { width: 1440, height: 1000 } },
  webServer: [
    { command: 'pnpm dev --host 127.0.0.1 --port 4204 --strictPort', url: 'http://127.0.0.1:4204', reuseExistingServer: true, env: { VITE_MULTI_USER: '1' } },
    { command: 'pnpm --filter @understand-book/admin dev --host 127.0.0.1 --port 4205 --strictPort', url: 'http://127.0.0.1:4205/admin/', reuseExistingServer: true },
  ],
});
