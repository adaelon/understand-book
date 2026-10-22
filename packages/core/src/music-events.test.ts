import { readFileSync } from 'node:fs';
import { describe, expect, it } from 'vitest';
import { musicEventComparison, musicEventsAt } from './music-events';
import { musicNavigation } from './music-structure';
import type { MusicSourceManifest } from './music-contract';

const fixture = (): MusicSourceManifest => JSON.parse(readFileSync(new URL('../../../crates/base-schema/tests/fixtures/music.json', import.meta.url), 'utf8'));

describe('music teaching material queries', () => {
  it('returns overlapping events and filters actual voices inside the same instrument', () => {
    const manifest = fixture();
    manifest.events[0].roles[0].voice_ids = ['1'];
    manifest.events.push({ ...manifest.events[0], event_id: 'support', roles: [{ part_id: 'piano', role: 'support', voice_ids: ['2'] }] });
    expect(musicEventsAt(manifest, 1000)).toHaveLength(2);
    expect(musicEventsAt(manifest, 1000, [], [{ part_id: 'piano', voice: '2' }]).map(event => event.event_id)).toEqual(['support']);
    expect(musicEventsAt(manifest, 1000, ['piano'], [{ part_id: 'piano', voice: '2' }])).toHaveLength(2);
    expect(musicEventsAt(manifest, 2000).map(event => event.event_id)).toEqual(['return']);
  });
  it('combines a complete instrument selection with a voice in a different instrument', () => {
    const manifest = fixture(), opening = manifest.events[0];
    manifest.parts.push({ part_id: 'violin', name: 'Violin', instrument: 'Violin', group: 'Strings', voices: ['1'] }, { part_id: 'cello', name: 'Cello', instrument: 'Cello', group: 'Strings', voices: ['1', '2'] });
    manifest.events = [
      { ...opening, event_id: 'violin', roles: [{ part_id: 'violin', role: 'theme', voice_ids: ['1'] }] },
      { ...opening, event_id: 'cello-1', roles: [{ part_id: 'cello', role: 'support', voice_ids: ['1'] }] },
      { ...opening, event_id: 'cello-2', roles: [{ part_id: 'cello', role: 'bass', voice_ids: ['2'] }] },
    ];
    expect(musicEventsAt(manifest, 1000, ['violin'], [{ part_id: 'cello', voice: '1' }]).map(event => event.event_id)).toEqual(['violin', 'cello-1']);
  });
  it('retains both ends of a comparison and distinguishes navigation from detailed sections', () => {
    const manifest = fixture(), comparison = musicEventComparison(manifest, 'opening');
    expect(comparison.event.evidence[0].score_object_ids).toHaveLength(2);
    expect(comparison.comparisons[0].target.evidence[0].audio?.start_ms).toBe(2000);
    manifest.structure.push({ section_id: 'work', parent_id: null, title: 'Full work', description: 'Work navigation only', start_ms: null, end_ms: null, event_ids: [], coverage: 'navigation_only' });
    expect(musicNavigation(manifest).map(section => [section.coverage, section.events.length])).toEqual([['detailed', 2], ['navigation_only', 0]]);
  });
});
