import { fileURLToPath } from 'node:url';

// Preserve existing operator/plugin commands while the implementation is public.
process.env.UNDERSTAND_BOOK_SIDECAR_OUTPUT_DIR ??= fileURLToPath(new URL('../src-tauri/binaries', import.meta.url));
await import('../../../scripts/desktop/build-sidecar.mjs');
