import {defineConfig} from '@playwright/test';
export default defineConfig({testDir:'.',testMatch:'personal-notes.spec.ts',timeout:90_000,workers:1,
  use:{baseURL:'http://127.0.0.1:4175'},webServer:{command:'pnpm dev --mode multi --host 127.0.0.1 --port 4175',url:'http://127.0.0.1:4175',reuseExistingServer:true}});
