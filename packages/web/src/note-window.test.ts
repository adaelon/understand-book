import { expect, it } from 'vitest';
import { changeNoteWindow, defaultNoteWindow, fitNoteWindow } from './note-window';

const desktop = { x: 0, y: 0, width: 1440, height: 900 };
it('keeps a desktop-sized saved window reachable on a phone', () => {
  expect(fitNoteWindow({ x: 1000, y: 500, width: 600, height: 500 }, { x: 0, y: 0, width: 320, height: 568 }))
    .toEqual({ x: 12, y: 56, width: 296, height: 500 });
});
it('uses a compact initial frame on a short phone viewport', () => {
  const frame = defaultNoteWindow({ x: 0, y: 0, width: 320, height: 568 });
  expect(frame.width).toBe(296);
  expect(frame.height).toBeLessThan(300);
  expect(frame.y).toBeGreaterThan(260);
});
it('stops a drag at the viewport edges without resizing the window', () => {
  expect(changeNoteWindow({ x: 700, y: 300, width: 400, height: 360 }, 'move', -2000, 2000, desktop))
    .toEqual({ x: 12, y: 528, width: 400, height: 360 });
});
it('holds the opposite corner when resizing and respects the usable minimum', () => {
  const frame = { x: 400, y: 300, width: 400, height: 360 };
  expect(changeNoteWindow(frame, 'nw', -100, -80, desktop)).toEqual({ x: 300, y: 220, width: 500, height: 440 });
  expect(changeNoteWindow(frame, 'nw', 1000, 1000, desktop)).toEqual({ x: 500, y: 380, width: 300, height: 280 });
});
it('fits above a panned keyboard without mutating the preferred frame', () => {
  const frame = { x: 12, y: 460, width: 366, height: 360 };
  expect(fitNoteWindow(frame, { x: 0, y: 90, width: 390, height: 340 }))
    .toEqual({ x: 12, y: 102, width: 366, height: 316 });
  expect(fitNoteWindow(frame, { x: 0, y: 0, width: 390, height: 844 })).toEqual(frame);
});
