import { defineConfig } from '@playwright/test';
const port = Number(process.env.FP4_PORT ?? 4192);
export default defineConfig({
  testDir: './playwright', testMatch: 'focus-turn.spec.ts', timeout: 300_000,
  workers: 1, fullyParallel: false, retries: 0, outputDir: '../../tmp/focus-reading-fp4/test-results',
  use: { baseURL: `http://127.0.0.1:${port}`, viewport: { width: 1440, height: 900 }, headless: true },
  webServer: { command: `pnpm exec vite preview --config vite.focus-turn.config.ts --host 127.0.0.1 --port ${port} --strictPort`, url: `http://127.0.0.1:${port}/playwright/fixtures/focus-turn.html`, reuseExistingServer: false },
});
