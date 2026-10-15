import { defineConfig } from '@playwright/test';
const port = Number(process.env.FP5_PORT ?? 4193);
process.env.FOCUS_EVIDENCE_DIR ??= '../../docs/performance/focus-reading-fp5-20261015/results';
export default defineConfig({ testDir: './playwright', testMatch: ['focus-operations.spec.ts', 'focus-turn.spec.ts'], timeout: 300_000,
  workers: 1, fullyParallel: false, retries: 0, outputDir: '../../tmp/focus-reading-fp5/test-results',
  use: { baseURL: `http://127.0.0.1:${port}`, viewport: { width: 1440, height: 900 }, headless: true },
  webServer: { command: `pnpm exec vite preview --config vite.focus-operations.config.ts --host 127.0.0.1 --port ${port} --strictPort`, url: `http://127.0.0.1:${port}/playwright/fixtures/focus-turn.html`, reuseExistingServer: false },
});
