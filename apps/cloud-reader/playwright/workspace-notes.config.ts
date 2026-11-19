import { defineConfig } from '@playwright/test';

// Reuse the real multi-user board_follow_up_browser_host fixture on 4220/4221.
// Start a fresh copied test executable with the ignored browser host before a run;
// its scene capacity is intentionally small and repeated runs consume scenes.
// Start Vite --config playwright/workspace-notes.vite.config.ts
// --host 127.0.0.1 --port 4339 --strictPort. Requests reach the real HTTP host.
export default defineConfig({
  testDir: '.', testMatch: 'workspace-notes.spec.ts', workers: 1, retries: 0,
  timeout: 180_000, outputDir: '../../../tmp/workspace-notes-browser/results',
  use: { baseURL: process.env.WORKSPACE_NOTES_ENTRY_URL ?? 'http://127.0.0.1:4339',
    viewport: { width: 1440, height: 1000 }, channel: 'msedge', trace: 'retain-on-failure', screenshot: 'only-on-failure' },
});
