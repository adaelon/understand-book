import { defineConfig } from '@playwright/test';
import { fileURLToPath } from 'node:url';

process.env.YEYE_EVIDENCE_DIR = fileURLToPath(new URL('./', import.meta.url));

export default defineConfig({
  testDir: '../../../packages/web/playwright',
  testMatch: ['yeye-integration.spec.ts'],
  outputDir: '../../../tmp/yy7-playwright',
  timeout: 45_000,
  workers: 2,
  use: { baseURL: 'http://127.0.0.1:4177' },
  webServer: {
    command: 'pnpm preview --host 127.0.0.1 --port 4177 --strictPort',
    cwd: fileURLToPath(new URL('../../../packages/web/', import.meta.url)),
    url: 'http://127.0.0.1:4177/home.html',
    reuseExistingServer: false,
  },
});
