import { defineConfig } from '@playwright/test';
const port = Number(process.env.FP6_PORT ?? 4194);
export default defineConfig({ testDir: './playwright', testMatch: 'focus-pdf.spec.ts', timeout: 300_000,
  workers: 1, fullyParallel: false, retries: 0, outputDir: '../../tmp/focus-reading-fp6/test-results',
  use: { baseURL: `http://127.0.0.1:${port}`, viewport: { width: 1440, height: 900 }, headless: true },
  webServer: { command: `pnpm exec vite preview --config vite.focus-pdf.config.ts --host 127.0.0.1 --port ${port} --strictPort`,
    url: `http://127.0.0.1:${port}/playwright/fixtures/focus-pdf.html`, reuseExistingServer: false },
});
