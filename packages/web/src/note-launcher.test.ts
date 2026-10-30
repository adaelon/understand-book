import { expect, it } from 'vitest';
import { placeNoteLauncher } from './note-launcher';

const desktop = { x: 0, y: 0, width: 1440, height: 900 };
const size = { width: 104, height: 44 };
it('stays above an expanded composer while retaining the right edge', () => {
  expect(placeNoteLauncher(desktop, size, [{ x: 1060, y: 650, width: 380, height: 250 }]))
    .toEqual({ x: 1320, y: 594 });
});
it('does not move a custom position that is clear of the input', () => {
  expect(placeNoteLauncher(desktop, size, [{ x: 1060, y: 650, width: 380, height: 250 }], { x: 100, y: 700 }))
    .toEqual({ x: 100, y: 700 });
});
it('temporarily fits a saved position above the phone keyboard and composer without changing it', () => {
  const preferred = { x: 270, y: 720 };
  const moved = placeNoteLauncher({ x: 0, y: 40, width: 390, height: 460 }, size,
    [{ x: 0, y: 300, width: 390, height: 200 }], preferred);
  expect(moved).toEqual({ x: 270, y: 244 });
  expect(preferred).toEqual({ x: 270, y: 720 });
  expect(placeNoteLauncher({ x: 0, y: 0, width: 390, height: 844 }, size, [], preferred)).toEqual(preferred);
});
it('moves beside an input if its top fills the available vertical space', () => {
  const result = placeNoteLauncher({ x: 0, y: 0, width: 1024, height: 440 }, size,
    [{ x: 700, y: 0, width: 324, height: 440 }]);
  expect(result.x + size.width).toBeLessThanOrEqual(688);
});
