import { existsSync, readFileSync, writeFileSync, renameSync, unlinkSync, statSync, mkdirSync } from "node:fs";
import { serialize, deserialize } from "node:v8";
import { gzipSync, gunzipSync } from "node:zlib";
import { randomUUID } from "node:crypto";
import path from "node:path";

export function receiptStamp(file: string): string | undefined {
  if (!existsSync(file)) return;
  const stat = statSync(file);
  return `${stat.size}/${stat.mtimeMs}`;
}

/** Disposable progress cache. The original receipts remain the source of truth. */
export function readTeachingRouteCache<T>(file: string, basis: string): T | undefined {
  if (!existsSync(file)) return;
  try {
    const saved = deserialize(gunzipSync(readFileSync(file)));
    if (saved.version !== 1 || saved.basis !== basis
      || saved.receipts.some(([name, stamp]: [string, string]) => receiptStamp(name) !== stamp)) return;
    return saved.value as T;
  } catch { return; } // A missing/interrupted cache is rebuilt from immutable receipts.
}

export function writeTeachingRouteCache<T>(file: string, basis: string, value: T, receipts: string[]): void {
  mkdirSync(path.dirname(file), { recursive: true });
  const temporary = file + "." + randomUUID() + ".tmp";
  try {
    // V8 preserves shared proposal/source references across historical tasks.
    const bytes = gzipSync(serialize({ version: 1, basis, value,
      receipts: receipts.map(name => [name, receiptStamp(name)]) }), { level: 1 });
    writeFileSync(temporary, bytes); renameSync(temporary, file);
  } catch (error) {
    if ((error as NodeJS.ErrnoException).code !== "ENOSPC") throw error;
    // Cache exhaustion cannot invalidate a committed action or its previous checkpoint.
  } finally { if (existsSync(temporary)) unlinkSync(temporary); }
}
