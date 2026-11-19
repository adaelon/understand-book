import { fileURLToPath } from 'node:url';
import { exportInstaller } from '../../../scripts/desktop/export-installer.mjs';

exportInstaller({
  appRoot: fileURLToPath(new URL('../', import.meta.url)),
  filename: 'UnderstandBookLocalSetup',
  debug: process.argv.includes('--debug'),
});
