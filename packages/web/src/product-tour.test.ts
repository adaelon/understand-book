import { describe, expect, it } from 'vitest';
import { placeTour, tourStorageKey, type TourRect } from './product-tour';

const overlaps = (a: TourRect, b: TourRect) => Math.max(a.x, b.x) < Math.min(a.x + a.width, b.x + b.width)
  && Math.max(a.y, b.y) < Math.min(a.y + a.height, b.y + b.height);

describe('spotlight positioning on the real page', () => {
  it.each([
    { viewport: { width: 1440, height: 900 }, target: { x: 142, y: 10, width: 112, height: 40 }, size: { width: 352, height: 320 } },
    { viewport: { width: 390, height: 844 }, target: { x: 12, y: 680, width: 366, height: 150 }, size: { width: 352, height: 390 } },
    { viewport: { width: 320, height: 568 }, target: { x: 0, y: 160, width: 320, height: 400 }, size: { width: 296, height: 310 } },
    { viewport: { width: 844, height: 390 }, target: { x: 0, y: 160, width: 844, height: 230 }, size: { width: 352, height: 278 } },
  ])('keeps instructions visible and clear of the spotlight at $viewport.width × $viewport.height', ({ viewport, target, size }) => {
    const result = placeTour(target, viewport, size);
    const card = { ...result.card, ...size };
    expect(card.x).toBeGreaterThanOrEqual(0); expect(card.y).toBeGreaterThanOrEqual(0);
    expect(card.x + card.width).toBeLessThanOrEqual(viewport.width);
    expect(card.y + card.height).toBeLessThanOrEqual(viewport.height);
    expect(result.hole.height).toBeGreaterThan(0);
    expect(overlaps(card, result.hole)).toBe(false);
  });
  it('remembers each account and page independently', () => {
    expect(tourStorageKey('first', 'reader')).not.toBe(tourStorageKey('second', 'reader'));
    expect(tourStorageKey('first', 'reader')).not.toBe(tourStorageKey('first', 'board'));
  });
});
