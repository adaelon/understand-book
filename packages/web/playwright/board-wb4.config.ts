import { defineConfig } from '@playwright/test';

// Run the ignored board_wb4_browser_host and the streaming Vite proxy first.
export default defineConfig({
  testDir: '.', testMatch: 'board-wb4.spec.ts', workers: 1, retries: 0,
  timeout: 480_000,
  outputDir: '../../../tmp/board-wb4/playwright-results',
  use: { baseURL: process.env.WB4_ENTRY_URL ?? 'http://127.0.0.1:4232',
    viewport: { width: 1500, height: 1050 }, trace: 'retain-on-failure' },
});
