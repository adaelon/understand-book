import { execFileSync } from 'node:child_process';
import { mkdirSync, readFileSync } from 'node:fs';
import { createRequire } from 'node:module';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

/** Compiled drivers re-enter their own command instead of addressing source .ts files. */
export function videoExecutorArgs(command: string, script: string, args: string[]) {
  return process.env.UNDERSTAND_BOOK_SIDECAR_SELF
    ? [command, ...args]
    : ['--import', pathToFileURL(createRequire(import.meta.url).resolve('tsx')).href, script, ...args];
}

/** Keep resumable CLI databases with the build, which may live on another drive. */
export function videoExecutorStorageArgs(workspace: string) {
  const directory = path.resolve(workspace, '.build/codex-state');
  mkdirSync(directory, { recursive: true });
  return ['-c', 'sqlite_home=' + JSON.stringify(directory)];
}

export function videoExecutorPrompt() {
  const sidecar = process.env.UNDERSTAND_BOOK_SIDECAR_SELF;
  return sidecar
    ? execFileSync(sidecar, ['prompt', 'automatic-build-dispatch-executor.md'], { encoding: 'utf8', windowsHide: true })
    : readFileSync(new URL('../../agents/automatic-build-dispatch-executor.md', import.meta.url), 'utf8');
}
