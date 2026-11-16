// @vitest-environment happy-dom
import { expect, it } from 'vitest';
import { placeNoteLauncher, noteLauncherLayoutMutation } from './note-launcher';

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
  expect(result!.x + size.width).toBeLessThanOrEqual(688);
});
it('uses the nearby diagonal space between the composer and a running-answer control', () => {
  expect(placeNoteLauncher({ x: 0, y: 0, width: 1024, height: 700 }, size, [
    { x: 700, y: 500, width: 324, height: 200 },
    { x: 920, y: 440, width: 80, height: 40 },
  ])).toEqual({ x: 804, y: 444 });
});
it('yields when controls occupy the whole viewport, then restores the saved position when space returns', () => {
  const preferred = { x: 270, y: 400 };
  const viewport = { x: 0, y: 0, width: 390, height: 568 };
  expect(placeNoteLauncher(viewport, size, [{ ...viewport }], preferred)).toBeNull();
  expect(placeNoteLauncher(viewport, size, [], preferred)).toEqual(preferred);
});

it('ignores only geometric-neutral playback updates, retaining layout and control changes', () => {
  const host = document.createElement('div');
  host.innerHTML = `<div class="music-map"><span class="map-clock">01:00</span><i class="map-cursor"></i><div class="map-overview-track"><i></i><button>Section</button></div></div><div class="music-progress"><span>01:00</span><input type="range"></div><div class="music-score-page"><svg><g class="note"></g><g class="music-note-overlay"><circle /></g></svg></div>`;
  const mutation = (target: Node, type = 'attributes', attributeName: string | null = 'style', addedNodes: Node[] = []) =>
    ({target, type, attributeName, addedNodes, removedNodes: []} as unknown as MutationRecord);
  for (const selector of ['.map-cursor', '.map-overview-track > i', '.music-progress > input', '.music-score-page .note', '.music-note-overlay circle']) {
    expect(noteLauncherLayoutMutation(mutation(host.querySelector(selector)!))).toBe(false);
  }
  for (const selector of ['.map-clock', '.music-progress > span']) {
    const clock = host.querySelector(selector)!;
    expect(noteLauncherLayoutMutation(mutation(clock.firstChild!, 'characterData', null))).toBe(false);
    expect(noteLauncherLayoutMutation(mutation(clock, 'childList', null, [document.createTextNode('01:01')]))).toBe(false);
  }
  expect(noteLauncherLayoutMutation(mutation(host.querySelector('.music-map')!, 'attributes', 'open'))).toBe(true);
  expect(noteLauncherLayoutMutation(mutation(host.querySelector('.map-overview-track button')!))).toBe(true);
  expect(noteLauncherLayoutMutation(mutation(host.querySelector('.map-clock')!, 'childList', null, [document.createElement('button')]))).toBe(true);
  expect(noteLauncherLayoutMutation(mutation(host, 'childList', null, [document.createElement('button')]))).toBe(true);
});
