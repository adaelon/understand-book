// @vitest-environment happy-dom
import { afterEach, expect, it, vi } from 'vitest';
import { MusicScoreMarkers } from './music-score';
import type { MusicNoteAlignment, MusicScoreObject } from '../../core/src/music-contract';

afterEach(() => vi.unstubAllGlobals());
it('keeps unused animation layers hidden and restores them for an active aligned note', () => {
  const svg = document.createElementNS('http://www.w3.org/2000/svg', 'svg');
  svg.innerHTML = '<g id="n"><rect class="notehead" /></g>';
  const element = svg.querySelector<SVGGraphicsElement>('#n')!;
  const head = element.querySelector<SVGGraphicsElement>('rect')!;
  const matrix = { inverse() { return this; } } as unknown as DOMMatrix;
  svg.getBoundingClientRect = () => ({ width: 100, height: 100 }) as DOMRect;
  svg.getScreenCTM = () => matrix;
  head.getScreenCTM = () => matrix;
  head.getBBox = () => ({ x: 10, y: 10, width: 8, height: 6 }) as DOMRect;
  vi.stubGlobal('DOMPoint', class {
    constructor(public x: number, public y: number) {}
    matrixTransform() { return this; }
  });
  const object = { source_id: 'n', svg_element_id: 'n', rest: false, part_id: 'violin', voice: '1' } as MusicScoreObject;
  const marks = new MusicScoreMarkers(svg, [object], () => '#398574');
  expect(marks.ready).toBe(true);
  marks.attach();
  const overlay = svg.querySelector<SVGGElement>('.music-note-overlay')!;
  expect(overlay.style.display).toBe('none');
  marks.colors(new Map([['n', 'blue']]));
  expect(element.style.fill).toBe('blue');
  marks.render([], 200, true);
  expect(overlay.style.display).toBe('none');
  const note = { score_object_id: 'n', start_ms: 100, end_ms: 500, attack: true, unobserved: false } as MusicNoteAlignment;
  marks.render([note], 200, true);
  expect(overlay.style.display).toBe('');
  expect(overlay.querySelector('circle')!.style.opacity).not.toBe('0');
  marks.render([], 1000, true);
  expect(overlay.style.display).toBe('none');
  expect(element.style.fill).toBe('blue');
  marks.colors(null);
  marks.render([note], 200, true);
  expect(element.style.fill).toBe('#398574');
  marks.render([], 1000, true);
  expect(element.style.fill).toBe('');
  marks.dispose();
  expect(svg.querySelector('.music-note-overlay')).toBeNull();
});

it('defers hidden SVG geometry even when the browser supplies an identity matrix', () => {
  const svg = document.createElementNS('http://www.w3.org/2000/svg', 'svg');
  svg.getBoundingClientRect = () => ({ width: 0, height: 0 }) as DOMRect;
  const matrix = { inverse() { return this; } } as unknown as DOMMatrix;
  const measure = vi.fn(() => matrix);
  svg.getScreenCTM = measure;
  const hidden = new MusicScoreMarkers(svg, [], () => 'blue');
  expect(hidden.ready).toBe(false);
  expect(measure).not.toHaveBeenCalled();
  hidden.attach(); hidden.dispose();
  svg.getBoundingClientRect = () => ({ width: 100, height: 100 }) as DOMRect;
  const visible = new MusicScoreMarkers(svg, [], () => 'blue');
  expect(visible.ready).toBe(true);
  expect(measure).toHaveBeenCalledOnce();
  visible.dispose();
});
