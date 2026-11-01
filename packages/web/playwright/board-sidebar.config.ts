import { defineConfig } from '@playwright/test';

export default defineConfig({
  testDir: '.',
  testMatch: 'board-workspace.spec.ts',
  grep: /sidebar composer|fullscreen/,
  timeout: 45_000,
  workers: 1,
  use: { baseURL: 'http://127.0.0.1:4187', channel: 'msedge', screenshot: 'only-on-failure', trace: 'retain-on-failure' },
  outputDir: '../../../docs/performance/board-freeform-sidebar-20261030/browser',
  webServer: {
    command: 'pnpm dev --host 127.0.0.1 --port 4187 --strictPort',
    url: 'http://127.0.0.1:4187/board-workspace-visual.html',
    reuseExistingServer: true,
    timeout: 120_000,
  },
});
