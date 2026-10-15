import { defineConfig } from '@playwright/test';
export default defineConfig({
  testDir: './playwright',
  testMatch: 'reading-resumption.spec.ts',
  timeout: 60000,
  use: { baseURL: 'http://127.0.0.1:4185' },
  webServer: {
    command: 'pnpm dev --host 127.0.0.1 --port 4185',
    url: 'http://127.0.0.1:4185',
    env: { VITE_MULTI_USER: '1' },
    reuseExistingServer: false,
  },
});
