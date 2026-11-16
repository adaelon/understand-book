import { expect, it } from 'vitest';
import type { MusicScoreObject } from '../../core/src/music-contract';
import { scorePartTarget } from './music-score-navigation';

const entries = [
  { part_id: 'flute', measure: '65', source_id: 'flute-65', rest: false },
  { part_id: 'bass', measure: '65', source_id: 'bass-65', rest: false },
  { part_id: 'bass', measure: '69', source_id: 'bass-rest-69', rest: true },
  { part_id: 'bass', measure: '70', source_id: 'bass-70', rest: false },
] as MusicScoreObject[];

it('stays at the current measure even when the selected instrument is resting', () => {
  expect(scorePartTarget(entries, 'bass', '69')?.source_id).toBe('bass-rest-69');
});

it('uses the same page when its instrument has no object in the requested measure', () => {
  expect(scorePartTarget(entries, 'flute', '69')?.source_id).toBe('flute-65');
});

it('does not jump to another instrument when the requested one is absent', () => {
  expect(scorePartTarget(entries, 'tuba', '69')).toBeUndefined();
});
