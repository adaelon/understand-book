import { fileURLToPath } from 'node:url';

process.argv[2] ??= fileURLToPath(new URL('../src-tauri/resources/music/verovio', import.meta.url));
await import('../../../scripts/desktop/prepare-music-resources.mjs');
