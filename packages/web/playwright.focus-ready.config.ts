import { defineConfig } from '@playwright/test';
const port = Number(process.env.FP3_PORT ?? 4190);
export default defineConfig({
  testDir: './playwright', testMatch: 'focus-ready.spec.ts', timeout: 300_000,
  workers: 1, fullyParallel: false, retries: 0, outputDir: '../../tmp/focus-reading-fp3/test-results',
  use: { baseURL: `http://127.0.0.1:${port}`, viewport: { width: 1440, height: 900 }, headless: true },
  webServer: { command: `pnpm exec vite preview --config vite.focus-ready.config.ts --host 127.0.0.1 --port ${port} --strictPort`, url: `http://127.0.0.1:${port}/playwright/fixtures/focus-ready.html`, reuseExistingServer: false },
});
