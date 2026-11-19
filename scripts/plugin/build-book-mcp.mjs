import { copyFileSync, mkdirSync } from 'node:fs';
import path from 'node:path';
import { desktopTarget, repoRoot, run } from '../desktop/platform.mjs';

const target = desktopTarget();
const targetRoot = process.env.CARGO_TARGET_DIR
  ? path.resolve(repoRoot, process.env.CARGO_TARGET_DIR)
  : path.join(repoRoot, 'target');
const outputDir = process.env.UNDERSTAND_BOOK_MCP_OUTPUT_DIR
  ? path.resolve(process.env.UNDERSTAND_BOOK_MCP_OUTPUT_DIR)
  : path.join(repoRoot, 'dist/book-mcp', target.triple);
const output = path.join(outputDir, `book-mcp-${target.triple}${target.suffix}`);

run(process.env.CARGO ?? 'cargo', [
  'build', '--locked', '--release', '-p', 'reader-service', '--bin', 'book_mcp',
]);
mkdirSync(outputDir, { recursive: true });
copyFileSync(path.join(targetRoot, 'release', `book_mcp${target.suffix}`), output);
console.log(output);
