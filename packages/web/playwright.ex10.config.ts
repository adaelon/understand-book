import {defineConfig} from '@playwright/test';
import base from './playwright.config';
const port=process.env.PRESENTATION_WEB_TEST_PORT || '4184';
export default defineConfig({...base,
  use:{...base.use,baseURL:`http://127.0.0.1:${port}`},
  webServer:{command:`pnpm dev --host 127.0.0.1 --port ${port}`,url:`http://127.0.0.1:${port}/agent-presentation-visual.html`,reuseExistingServer:true},
});
