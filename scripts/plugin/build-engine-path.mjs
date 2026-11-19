import path from 'node:path';
import { desktopTarget, repoRoot } from '../desktop/platform.mjs';

// The public packager and plugin checks consume the same platform artifact.
export function buildEnginePath() {
  if (process.env.UNDERSTAND_BOOK_BUILD_EXE) return path.resolve(process.env.UNDERSTAND_BOOK_BUILD_EXE);
  const target = desktopTarget();
  const directory = process.env.UNDERSTAND_BOOK_SIDECAR_OUTPUT_DIR
    ? path.resolve(process.env.UNDERSTAND_BOOK_SIDECAR_OUTPUT_DIR)
    : path.join(repoRoot, 'dist/build-engine', target.triple);
  return path.join(directory, `understand-book-build-${target.triple}${target.suffix}`);
}
