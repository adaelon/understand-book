import { defineConfig } from '@playwright/test';
export default defineConfig({
  testDir: './playwright', testMatch: 'multi-user-login.spec.ts', workers: 1,
  timeout: 30_000, use: { browserName: 'chromium', viewport: { width: 390, height: 844 } },
});
