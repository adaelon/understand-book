import { defineConfig } from '@playwright/test';

// Start tests::board_wb3_browser_host, then Vite using board-wb3.vite.config.ts.
export default defineConfig({
  testDir: '.', testMatch: 'board-wb3.spec.ts', workers: 1, retries: 0,
  timeout: 300_000,
  outputDir: '../../../tmp/board-wb3/playwright-results',
  use: { baseURL: process.env.WB3_ENTRY_URL ?? 'http://127.0.0.1:4222',
    viewport: { width: 1500, height: 1050 }, trace: 'retain-on-failure' },
});
