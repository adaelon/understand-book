import { defineConfig } from '@playwright/test';

export default defineConfig({
  testDir: '.', testMatch: 'guest-trial.spec.ts', workers: 1,
  timeout: 90_000,
  use: { browserName: 'chromium', headless: true },
  outputDir: '../../../tmp/guest-trial-20261027/browser-results',
});
