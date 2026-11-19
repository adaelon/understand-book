import { expect, it } from 'vitest';
import { claimWarmupOffer, examDailyCount, MARGIN_READING_MS, MarginTracker, marginEnabled, marginQuietToday, recordMarginIgnored, setMarginEnabled } from './practice-pacing';

function memory(): Storage {
  const values = new Map<string, string>();
  return { getItem: (k: string) => values.get(k) ?? null, setItem: (k: string, v: string) => { values.set(k, v); } } as Storage;
}

it('keeps margin offers off until the reader turns them on, per owner', () => {
  const storage = memory();
  expect(marginEnabled(storage, 'local')).toBe(false);
  setMarginEnabled(storage, 'local', true);
  expect(marginEnabled(storage, 'local')).toBe(true);
  expect(marginEnabled(storage, 'other')).toBe(false);
  const broken = { getItem: () => { throw new Error('blocked'); }, setItem: () => { throw new Error('blocked'); } } as unknown as Storage;
  expect(marginEnabled(broken, 'local')).toBe(false);
  expect(() => setMarginEnabled(broken, 'local', true)).not.toThrow();
});

it('offers once per chapter visit after continuous visible reading', () => {
  const tracker = new MarginTracker();
  const minute = 60_000;
  expect(tracker.observe('3', true, 0)).toBe(false);
  expect(tracker.observe('3', true, 2 * minute)).toBe(false);
  // Hidden time does not count.
  expect(tracker.observe('3', false, 10 * minute)).toBe(false);
  expect(tracker.observe('3', true, 11 * minute)).toBe(false);
  expect(tracker.observe('3', true, 11 * minute + MARGIN_READING_MS - 2 * minute)).toBe(true);
  expect(tracker.observe('3', true, 30 * minute)).toBe(false);
  // A new chapter visit starts over.
  expect(tracker.observe('4', true, 31 * minute)).toBe(false);
  expect(tracker.observe('4', true, 31 * minute + MARGIN_READING_MS)).toBe(true);
});

it('goes quiet for the day after two ignored offers', () => {
  const storage = memory();
  recordMarginIgnored(storage, 'local', '2026-10-30');
  expect(marginQuietToday(storage, 'local', '2026-10-30')).toBe(false);
  recordMarginIgnored(storage, 'local', '2026-10-30');
  expect(marginQuietToday(storage, 'local', '2026-10-30')).toBe(true);
  expect(marginQuietToday(storage, 'local', '2026-10-31')).toBe(false);
});

it('offers a warm-up once per workspace and day', () => {
  const storage = memory();
  expect(claimWarmupOffer(storage, 'local', 'book-a', '2026-10-30')).toBe(true);
  expect(claimWarmupOffer(storage, 'local', 'book-a', '2026-10-30')).toBe(false);
  expect(claimWarmupOffer(storage, 'local', 'book-b', '2026-10-30')).toBe(true);
  expect(claimWarmupOffer(storage, 'local', 'book-a', '2026-10-31')).toBe(true);
});

it('gives more exam practice in the final days', () => {
  expect(examDailyCount(10)).toBe(3);
  expect(examDailyCount(3)).toBe(5);
  expect(examDailyCount(0)).toBe(5);
});
