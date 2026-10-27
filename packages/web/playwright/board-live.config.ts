import { defineConfig } from '@playwright/test';
export default defineConfig({
  testDir: '.', testMatch: 'board-live.spec.ts', timeout: 90_000, workers: 1,
  outputDir: '../.wb1-live-results',
  use: { baseURL: process.env.WB1_ENTRY_URL ?? 'http://127.0.0.1:4219', viewport: { width: 1440, height: 1000 } },
});
