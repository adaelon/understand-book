import { realpathSync } from 'node:fs';
import { dirname } from 'node:path';
import { fileURLToPath } from 'node:url';
import { defineConfig, mergeConfig } from 'vite';
import base from '../vite.config';

// Worktree development shares dependencies with the primary checkout. Include
// their real path so the browser loads the actual app fonts during visual checks.
export default defineConfig(env => mergeConfig(typeof base === 'function' ? base(env) : base, {
  server: { fs: { allow: [fileURLToPath(new URL('../../../', import.meta.url)),
    dirname(realpathSync(fileURLToPath(new URL('../../../node_modules', import.meta.url))))] } },
}));
