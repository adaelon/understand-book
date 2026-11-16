import { defineConfig } from '@playwright/test';
export default defineConfig({
  testDir:'./playwright', testMatch:'learning-entry.spec.ts', timeout:180_000, workers:1,
  expect:{timeout:25_000},
  use:{baseURL:'http://127.0.0.1:4264',viewport:{width:1440,height:1050}},
  webServer:{command:'pnpm exec vite preview --outDir dist-multi --host 127.0.0.1 --port 4264',url:'http://127.0.0.1:4264',reuseExistingServer:true},
});
