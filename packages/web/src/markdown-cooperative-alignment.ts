import { runReaderWork } from './reader-cooperative';

export type SourceEdit = { kind: 'equal' | 'delete' | 'insert' };

/** Hirschberg LCS: linear workspace instead of retaining a quadratic Myers trace.
 * Each row yields a checkpoint; the runner yields the main thread every 4ms.
 * Prefix/suffix removal makes already rendered prose and long equal runs cheap.
 */
export function* sourceEdits(source: string, semantic: string, syntax?: Uint8Array): Generator<void, SourceEdit[]> {
  const edits: SourceEdit[] = [];
  // Keep the quadratic inner loop on numeric UTF-16 units instead of string
  // indexing; this reduces WebKit compute time and cooperative task switches.
  const sourceCodes = new Uint16Array(source.length), semanticCodes = new Uint16Array(semantic.length);
  for (let i = 0; i < source.length; i++) sourceCodes[i] = source.charCodeAt(i);
  for (let i = 0; i < semantic.length; i++) semanticCodes[i] = semantic.charCodeAt(i);
  const equal = (a: number, b: number) => !syntax?.[a] && sourceCodes[a] === semanticCodes[b];
  function* row(a0: number, a1: number, b0: number, b1: number, reverse: boolean): Generator<void, Uint32Array> {
    const n = b1 - b0;
    let previous = new Uint32Array(n + 1), current = new Uint32Array(n + 1);
    for (let a = 0; a < a1 - a0; a++) {
      for (let b = 1; b <= n; b++) {
        current[b] = equal(reverse ? a1 - 1 - a : a0 + a, reverse ? b1 - b : b0 + b - 1)
          ? previous[b - 1] + 1 : Math.max(previous[b], current[b - 1]);
      }
      [previous, current] = [current, previous];
      yield;
    }
    return previous;
  }
  function* visit(a0: number, a1: number, b0: number, b1: number): Generator<void, void> {
    while (a0 < a1 && b0 < b1 && equal(a0, b0)) { edits.push({ kind: 'equal' }); a0++; b0++; }
    let suffix = 0;
    while (a0 < a1 && b0 < b1 && equal(a1 - 1, b1 - 1)) { a1--; b1--; suffix++; }
    if (a0 === a1) for (let b = b0; b < b1; b++) edits.push({ kind: 'insert' });
    else if (b0 === b1) for (let a = a0; a < a1; a++) edits.push({ kind: 'delete' });
    else if (a1 - a0 === 1) {
      let match = b0; while (match < b1 && !equal(a0, match)) match++;
      if (match === b1) { edits.push({ kind: 'delete' }); for (let b = b0; b < b1; b++) edits.push({ kind: 'insert' }); }
      else { for (let b = b0; b < match; b++) edits.push({ kind: 'insert' }); edits.push({ kind: 'equal' }); for (let b = match + 1; b < b1; b++) edits.push({ kind: 'insert' }); }
    } else {
      const mid = (a0 + a1) >>> 1;
      // Release both row arrays before recursing; no retained per-depth matrix.
      let left: Uint32Array | null = yield* row(a0, mid, b0, b1, false);
      let right: Uint32Array | null = yield* row(mid, a1, b0, b1, true);
      let split = 0, best = -1;
      for (let b = 0; b <= b1 - b0; b++) {
        const score = left[b] + right[b1 - b0 - b];
        if (score > best) { best = score; split = b; }
      }
      left = null; right = null;
      yield* visit(a0, mid, b0, b0 + split);
      yield* visit(mid, a1, b0 + split, b1);
    }
    while (suffix--) edits.push({ kind: 'equal' });
    yield;
  }
  yield* visit(0, source.length, 0, semantic.length);
  return edits;
}

export const cooperativeSourceEdits = (source: string, semantic: string, syntax?: Uint8Array, checkpoint?: () => Promise<void>) =>
  runReaderWork(sourceEdits(source, semantic, syntax), checkpoint);
