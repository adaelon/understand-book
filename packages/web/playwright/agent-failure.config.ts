import { defineConfig } from '@playwright/test';

export default defineConfig({
  testDir: '.', testMatch: 'agent-failure.spec.ts', timeout: 30_000, workers: 1,
  use: { baseURL: 'http://127.0.0.1:4193', channel: 'msedge', screenshot: 'only-on-failure' },
  webServer: { command: 'pnpm dev --host 127.0.0.1 --port 4193', url: 'http://127.0.0.1:4193', reuseExistingServer: true },
});
