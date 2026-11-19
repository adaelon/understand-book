import { defineConfig } from '@playwright/test';
const browser = process.env.MU8_BROWSER === 'webkit' ? 'webkit' : 'chromium';
export default defineConfig({
  testDir: './playwright', testMatch: 'multi-user-reader.spec.ts', workers: 1,
  expect: { timeout: 15000 },
  timeout: 60000, use: { viewport: { width: 1280, height: 900 }, ignoreHTTPSErrors: true },
  webServer: { command: 'node playwright/mu8-host.mjs', url: 'https://localhost:4189/', ignoreHTTPSErrors: true },
  projects: [{ name: browser, use: { browserName: browser } }],
});
