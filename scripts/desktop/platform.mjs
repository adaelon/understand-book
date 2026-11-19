import { existsSync } from 'node:fs';
import { spawnSync } from 'node:child_process';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

export const repoRoot = fileURLToPath(new URL('../../', import.meta.url));

// Native Node and Canvas are packaged on the OS/architecture that will run them.
export function desktopTarget(platform = process.platform, arch = process.arch) {
  const targets = {
    'win32-x64': { triple: 'x86_64-pc-windows-msvc', bun: 'bun-windows-x64', canvas: '@napi-rs/canvas-win32-x64-msvc', suffix: '.exe' },
    'darwin-arm64': { triple: 'aarch64-apple-darwin', bun: 'bun-darwin-arm64', canvas: '@napi-rs/canvas-darwin-arm64', suffix: '' },
    'darwin-x64': { triple: 'x86_64-apple-darwin', bun: 'bun-darwin-x64', canvas: '@napi-rs/canvas-darwin-x64', suffix: '' },
  };
  const target = targets[`${platform}-${arch}`];
  if (!target) throw new Error(`Desktop resources do not support ${platform}/${arch}. Build on Windows x64 or macOS arm64/x64.`);
  return { platform, arch, ...target };
}

export function bunExecutable() {
  if (process.env.BUN_BINARY) return process.env.BUN_BINARY;
  const name = process.platform === 'win32' ? 'bun.exe' : 'bun';
  for (const directory of (process.env.PATH ?? '').split(path.delimiter)) {
    for (const candidate of [path.join(directory, name), path.join(directory, 'node_modules/bun/bin', name)]) {
      if (existsSync(candidate)) return candidate;
    }
  }
  throw new Error('Bun was not found; install Bun as a build tool or set BUN_BINARY.');
}

export function run(program, args, options = {}) {
  const result = spawnSync(program, args, { cwd: repoRoot, stdio: 'inherit', windowsHide: true, ...options });
  if (result.error) throw result.error;
  if (result.status !== 0) throw new Error(`${path.basename(program)} failed (${result.status ?? 'terminated'})`);
}

export function nodeLicense() {
  const candidates = [
    process.env.UNDERSTAND_BOOK_NODE_LICENSE,
    path.join(path.dirname(process.execPath), 'LICENSE'),
    path.resolve(path.dirname(process.execPath), '../LICENSE'),
    path.resolve(path.dirname(process.execPath), '../share/doc/node/LICENSE'),
  ].filter(Boolean);
  const license = candidates.find(existsSync);
  if (!license) throw new Error('Set UNDERSTAND_BOOK_NODE_LICENSE to the LICENSE distributed with the build-time Node runtime.');
  return license;
}
