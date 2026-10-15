import { defineConfig } from '@playwright/test';
import { fileURLToPath } from 'node:url';

process.env.YEYE_EVIDENCE_DIR = fileURLToPath(new URL('./', import.meta.url));
export default defineConfig({
  testDir: '../../../packages/web/playwright',
  testMatch: ['yeye-integration.spec.ts', 'product-home.spec.ts', 'brand-release.spec.ts'],
  outputDir: '../../../tmp/logo-brand-release-20261013/browser-results',
  timeout: 45_000, workers: 2,
  use: { baseURL: 'http://127.0.0.1:4188' },
  webServer: {
    command: 'pnpm preview --outDir dist-multi --host 127.0.0.1 --port 4188 --strictPort',
    cwd: fileURLToPath(new URL('../../../packages/web/', import.meta.url)),
    url: 'http://127.0.0.1:4188/home.html', reuseExistingServer: false,
  },
});
