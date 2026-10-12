import { defineConfig } from '@playwright/test';

export default defineConfig({
  testDir: './playwright',
  testMatch: 'tutor-building-blocks-tb4.spec.ts',
  workers: 1,
  timeout: 1_200_000,
  use: { baseURL: 'http://127.0.0.1:4184', headless: true },
  webServer: {
    command: 'pnpm dev --host 127.0.0.1 --port 4184',
    url: 'http://127.0.0.1:4184/agent-presentation-visual.html',
    reuseExistingServer: true,
  },
});
