import { defineConfig } from '@playwright/test';
export default defineConfig({
  testDir: './playwright', testMatch: 'book-covers.spec.ts', workers: 1,
  timeout: 45000, use: { baseURL: 'http://127.0.0.1:4192', viewport: { width: 1280, height: 900 } },
  webServer: { command: 'pnpm dev --host 127.0.0.1 --port 4192', url: 'http://127.0.0.1:4192',
    env: { VITE_MULTI_USER: '1' }, reuseExistingServer: true },
});
