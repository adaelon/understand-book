import { chmodSync, copyFileSync, mkdirSync } from 'node:fs';
import path from 'node:path';
import { desktopTarget, repoRoot, run } from './platform.mjs';

export function buildLearningCli(destination) {
  const { suffix, triple } = desktopTarget();
  const binaries = ['understand-book', 'book_mcp'];
  const targetRoot = path.resolve(repoRoot, process.env.CARGO_TARGET_DIR || 'target');
  const configuredTarget = process.env.CARGO_BUILD_TARGET;
  if (configuredTarget && configuredTarget !== triple) {
    throw new Error(`Local desktop packaging expects the native Rust target ${triple}, got ${configuredTarget}.`);
  }
  run(process.env.CARGO || 'cargo', ['build', '--release', '--locked', '-p', 'reader-service', ...binaries.flatMap(bin => ['--bin', bin])]);
  mkdirSync(destination, { recursive: true });
  for (const name of binaries) {
    const file = `${name}${suffix}`;
    const output = path.join(destination, file);
    copyFileSync(path.join(targetRoot, configuredTarget || '', 'release', file), output);
    if (process.platform !== 'win32') chmodSync(output, 0o755);
  }
}
