import { defineConfig } from '@playwright/test';
import { fileURLToPath } from 'node:url';
export default defineConfig({
  testDir: '../../../packages/web/playwright',
  testMatch: ['mobile-learning-preview.spec.ts', 'mobile-chat-interactions.spec.ts', 'mobile-workspace.spec.ts', 'conversation-learning-design.spec.ts', 'conversation-learning-entry.spec.ts', 'display-font-share.spec.ts'],
  outputDir: '../../../tmp/logo-brand-release-20261013/mobile-results',
  timeout: 45_000, workers: 2,
  use: { baseURL: 'http://127.0.0.1:4189' },
  webServer: {
    command: 'pnpm dev --host 127.0.0.1 --port 4189 --strictPort',
    cwd: fileURLToPath(new URL('../../../packages/web/', import.meta.url)),
    url: 'http://127.0.0.1:4189/mobile-learning-preview.html', reuseExistingServer: false,
  },
});
