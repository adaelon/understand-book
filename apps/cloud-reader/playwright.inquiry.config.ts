import { defineConfig } from '@playwright/test';
export default defineConfig({
  testDir: './playwright', testMatch: 'inquiry-course.spec.ts', timeout: 90_000, workers: 1,
  use: { baseURL: 'http://127.0.0.1:4264', viewport: { width: 1440, height: 1050 } },
  webServer: { command: 'pnpm dev --host 127.0.0.1 --port 4264', url: 'http://127.0.0.1:4264', reuseExistingServer: true },
});
