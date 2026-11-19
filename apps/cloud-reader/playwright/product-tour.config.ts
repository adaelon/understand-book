import { defineConfig } from '@playwright/test';
import { fileURLToPath } from 'node:url';

export default defineConfig({
  testDir: '.', testMatch: ['product-tour.spec.ts', 'reader-header.spec.ts'], workers: 1, retries: 0, timeout: 90_000,
  outputDir: '../../../.tmp/product-tour-browser',
  expect: { timeout: 10_000 },
  use: { baseURL: 'http://127.0.0.1:4197', channel: 'msedge', actionTimeout: 10_000, trace: 'retain-on-failure', screenshot: 'only-on-failure', reducedMotion: 'reduce' },
  // Uses the isolated personal_notes_browser_host backend on 4230/4231.
  webServer: { command: 'pnpm exec vite preview --outDir dist-product-tour --host 127.0.0.1 --port 4197 --strictPort', cwd: fileURLToPath(new URL('..', import.meta.url)), url: 'http://127.0.0.1:4197' },
});
