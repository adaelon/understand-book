import { defineConfig } from '@playwright/test';
const port = Number(process.env.FP2_PORT ?? 4189);
export default defineConfig({
  testDir: './playwright', testMatch: 'focus-pagination.spec.ts', timeout: 180_000,
  workers: 1, fullyParallel: false, retries: 0, outputDir: '../../tmp/focus-reading-fp2/test-results',
  use: { baseURL: `http://127.0.0.1:${port}`, viewport: { width: 1440, height: 900 }, headless: true },
  webServer: { command: `pnpm exec vite preview --config vite.focus-pagination.config.ts --host 127.0.0.1 --port ${port} --strictPort`, url: `http://127.0.0.1:${port}/playwright/fixtures/focus-pagination.html`, reuseExistingServer: false },
});
