import { defineConfig } from '@playwright/test';

export default defineConfig({
  testDir: './playwright', testMatch: 'workspace-recovery.spec.ts', workers: 1,
  timeout: 60000, expect: { timeout: 15000 },
  use: { baseURL: 'http://127.0.0.1:4196' },
  webServer: {
    command: 'pnpm dev --host 127.0.0.1 --port 4196', url: 'http://127.0.0.1:4196',
    env: { VITE_MULTI_USER: '1' }, reuseExistingServer: false,
  },
});
