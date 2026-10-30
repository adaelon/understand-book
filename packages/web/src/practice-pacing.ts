// Pacing for practice offers (ADR-0178 §1, slice P3). Offers never spend model
// quota by themselves: a problem is generated only after the reader accepts.
// Everything here is a per-device convenience; failures fall back to defaults.

/** Continuous visible reading in one chapter before a margin offer may appear. */
export const MARGIN_READING_MS = 5 * 60 * 1000;
/** Ignored margin offers in one day after which no more appear that day. */
export const MARGIN_DAILY_IGNORES = 2;
/** Problems in one warm-up sequence. */
export const WARMUP_LIMIT = 3;

type Store = Pick<Storage, 'getItem' | 'setItem'> | null | undefined;

const key = (owner: string, name: string) => `practice:${owner}:${name}`;

function read(storage: Store, name: string): string | null {
  try { return storage?.getItem(name) ?? null; } catch { return null; }
}
function write(storage: Store, name: string, value: string) {
  try { storage?.setItem(name, value); } catch { /* Keeps the in-memory default. */ }
}

export function today(now = new Date()): string {
  return `${now.getFullYear()}-${String(now.getMonth() + 1).padStart(2, '0')}-${String(now.getDate()).padStart(2, '0')}`;
}

/** Margin offers are off unless the reader turned them on. */
export function marginEnabled(storage: Store, owner: string): boolean {
  return read(storage, key(owner, 'margin')) === 'on';
}
export function setMarginEnabled(storage: Store, owner: string, enabled: boolean) {
  write(storage, key(owner, 'margin'), enabled ? 'on' : 'off');
}

export function recordMarginIgnored(storage: Store, owner: string, day: string) {
  const name = key(owner, `margin-ignored:${day}`);
  write(storage, name, String(Number(read(storage, name) ?? '0') + 1));
}
export function marginQuietToday(storage: Store, owner: string, day: string): boolean {
  return Number(read(storage, key(owner, `margin-ignored:${day}`)) ?? '0') >= MARGIN_DAILY_IGNORES;
}

/** One warm-up offer per workspace and day. Returns true when it should be shown now. */
export function claimWarmupOffer(storage: Store, owner: string, workspace: string, day: string): boolean {
  const name = key(owner, `warmup:${workspace}`);
  if (read(storage, name) === day) return false;
  write(storage, name, day);
  return true;
}

/**
 * Tracks one chapter visit. The clock runs only while the page is visible and the
 * reader stays in the same chapter; each visit yields at most one offer.
 */
export class MarginTracker {
  private chapter: string | null = null;
  private readMs = 0;
  private lastTick: number | null = null;
  private offered = false;

  observe(chapter: string | null, visible: boolean, nowMs: number): boolean {
    if (chapter !== this.chapter) {
      this.chapter = chapter; this.readMs = 0; this.offered = false; this.lastTick = null;
    }
    if (!chapter || !visible) { this.lastTick = null; return false; }
    if (this.lastTick !== null) this.readMs += Math.max(0, nowMs - this.lastTick);
    this.lastTick = nowMs;
    if (this.offered || this.readMs < MARGIN_READING_MS) return false;
    this.offered = true;
    return true;
  }
}

/** Problems in today's exam-sprint practice: more in the final days. */
export function examDailyCount(remainingDays: number): number {
  return remainingDays <= 3 ? 5 : WARMUP_LIMIT;
}
