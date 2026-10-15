import { defineConfig } from '@playwright/test';
import { fileURLToPath } from 'node:url';

// Exercise the production reader entry and its bundled assets with the existing API fixture.
export default defineConfig({
  testDir: fileURLToPath(new URL('../../../packages/web/playwright/', import.meta.url)),
  testMatch: 'reading-resumption.spec.ts',
  outputDir: fileURLToPath(new URL('../../../tmp/yy3-playwright/', import.meta.url)),
  timeout: 30_000,
  workers: 2,
  use: { baseURL: 'http://127.0.0.1:6195' },
  webServer: {
    command: 'pnpm preview --host 127.0.0.1 --port 6195 --strictPort',
    cwd: fileURLToPath(new URL('../../../packages/web/', import.meta.url)),
    url: 'http://127.0.0.1:6195/',
    reuseExistingServer: false,
  },
});
