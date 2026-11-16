import { defineConfig } from '@playwright/test';
import { fileURLToPath } from 'node:url';

// Build with pnpm build:multi first. Reuses the real personal_notes_browser_host fixture.
export default defineConfig({
  testDir: '.', testMatch: 'board-library.spec.ts', workers: 1, retries: 0, timeout: 60_000,
  outputDir: '../../../docs/performance/board-primary-navigation-20261102/browser',
  use: { baseURL: 'http://127.0.0.1:4199', channel: 'msedge', trace: 'retain-on-failure', screenshot: 'only-on-failure' },
  webServer: { command: 'pnpm exec vite preview --outDir dist-multi --host 127.0.0.1 --port 4199 --strictPort', cwd: fileURLToPath(new URL('..', import.meta.url)), url: 'http://127.0.0.1:4199' },
});
