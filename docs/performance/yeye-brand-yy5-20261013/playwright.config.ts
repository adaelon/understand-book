import { defineConfig } from '@playwright/test';
import { fileURLToPath } from 'node:url';

export default defineConfig({
  testDir: '../../../packages/web/playwright',
  testMatch: 'product-home.spec.ts',
  outputDir: '../../../tmp/yy5-playwright',
  timeout: 30_000,
  workers: 2,
  use: { baseURL: 'http://127.0.0.1:4176' },
  webServer: {
    command: 'pnpm preview --host 127.0.0.1 --port 4176 --strictPort',
    cwd: fileURLToPath(new URL('../../../packages/web/', import.meta.url)),
    url: 'http://127.0.0.1:4176/home.html',
    reuseExistingServer: false,
  },
});
