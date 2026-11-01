import { defineConfig } from '@playwright/test';
import thoughtWindow from './thought-window.config';

export default defineConfig({
  ...thoughtWindow,
  grep: /mobile keyboard|mobile note/,
  outputDir: '../test-results/note-mobile-webkit',
  use: { ...thoughtWindow.use, channel: undefined, browserName: 'webkit', isMobile: true, hasTouch: true, deviceScaleFactor: 2 },
});
