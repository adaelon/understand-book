import { fileURLToPath } from 'node:url';
import { exportInstaller } from '../../../scripts/desktop/export-installer.mjs';

exportInstaller({
  appRoot: fileURLToPath(new URL('../', import.meta.url)),
  filename: 'UnderstandBookSetup',
  debug: process.argv.includes('--debug'),
});
