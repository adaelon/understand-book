import { defineConfig } from '@playwright/test';
const port = 4195;
const suite = process.env.FP7_SUITE ?? 'prose';
const engine = process.env.FP7_ENGINE ?? 'chromium';
process.env.FOCUS_ENTRY_URL = `http://127.0.0.1:${port}/`;
process.env.FOCUS_EVIDENCE_DIR = `../../docs/performance/focus-reading-fp7-20261015/results/${engine}/${suite}${process.env.FP7_FRAME_EVENTS_ONLY === '1' ? '-frames' : ''}`;
export default defineConfig({
  testDir: './playwright',
  testMatch: suite === 'live' ? ['focus-live.spec.ts'] : suite === 'pdf' ? ['focus-pdf.spec.ts'] : ['focus-turn.spec.ts', 'focus-operations.spec.ts', 'focus-release.spec.ts'],
  timeout: 300_000, workers: 1, fullyParallel: false, retries: 0,
  outputDir: `../../tmp/focus-reading-fp7/test-results/${engine}/${suite}`,
  use: { browserName: engine === 'webkit' ? 'webkit' : 'chromium', baseURL: `http://127.0.0.1:${port}`,
    viewport: { width: 1440, height: 900 }, headless: true },
  webServer: { command: `pnpm exec vite preview --host 127.0.0.1 --port ${port} --strictPort`,
    url: `http://127.0.0.1:${port}`, reuseExistingServer: false },
});
