import { defineConfig } from '@playwright/test';

// Start tests::board_follow_up_browser_host and Vite on 4222 before running.
export default defineConfig({
  testDir: '.',
  testMatch: 'board-follow-up.spec.ts',
  timeout: 180_000,
  workers: 1,
  retries: 0,
  outputDir: '../.wb2-live-results',
  use: {
    baseURL: process.env.WB2_ENTRY_URL ?? 'http://127.0.0.1:4222',
    viewport: { width: 1440, height: 1000 },
    trace: 'retain-on-failure',
  },
});
