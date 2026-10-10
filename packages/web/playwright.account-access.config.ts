import { defineConfig } from '@playwright/test';

export default defineConfig({
  testDir: './playwright',
  testMatch: 'account-access-layout.spec.ts',
  workers: 1,
  use: { baseURL: 'http://127.0.0.1:4197' },
  webServer: {
    command: 'pnpm dev --host 127.0.0.1 --port 4197',
    env: { VITE_MULTI_USER: '1' },
    url: 'http://127.0.0.1:4197',
  },
});
