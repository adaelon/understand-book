import { copyFileSync, mkdirSync, readFileSync } from 'node:fs';
import path from 'node:path';
import { repoRoot } from './platform.mjs';

export function exportInstaller({ appRoot, filename, debug = false }) {
  const config = JSON.parse(readFileSync(path.join(appRoot, 'src-tauri/tauri.conf.json'), 'utf8'));
  const targetRoot = process.env.CARGO_TARGET_DIR ? path.resolve(repoRoot, process.env.CARGO_TARGET_DIR) : path.join(repoRoot, 'target');
  const target = process.env.CARGO_BUILD_TARGET;
  const profileRoot = path.join(targetRoot, ...(target ? [target] : []), debug ? 'debug' : 'release');
  const architecture = { x64: 'x64', arm64: 'arm64' }[process.arch];
  if (process.platform !== 'win32' || !architecture) throw new Error('NSIS export must run on the Windows target architecture.');
  const source = path.join(profileRoot, 'bundle/nsis', `${config.productName}_${config.version}_${architecture}-setup.exe`);
  const output = path.join(repoRoot, 'dist', `${filename}${debug ? '-debug' : ''}.exe`);
  mkdirSync(path.dirname(output), { recursive: true });
  copyFileSync(source, output);
  console.log(output);
  return output;
}
