import {defineConfig} from '@playwright/test';
import base from './playwright.config';
export default defineConfig({...base,
  use:{...base.use,baseURL:'http://127.0.0.1:4184'},
  webServer:{command:'pnpm dev --host 127.0.0.1 --port 4184',url:'http://127.0.0.1:4184/agent-presentation-visual.html',reuseExistingServer:true},
});
