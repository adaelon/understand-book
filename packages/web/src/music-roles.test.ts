import { describe, expect, it } from 'vitest';
import type { MusicEvent, MusicScoreObject } from '../../core/src/music-contract';
import { musicRoleColors, musicRoleGroups } from './music-roles';
const event = (id: string, part: string, role: string, theme?: string): MusicEvent => ({ event_id: id, title: id, explanation: '', listen_for: '', start_ms: 0, end_ms: 1000, roles: [{ part_id: part, role, theme_id: theme }], evidence: [{ provenance_id: 'score', score_object_ids: [id], audio: null }], relations: [], concept_tags: [] });
describe('curated musical role colors', () => {
  it('keeps a theme color through instrument and role-label changes without joining another theme', () => {
    const first = event('a', 'cello', '主题线条', 'joy'), second = event('b', 'viola', '接过主题', 'joy'), other = event('c', 'violin', '主题线条', 'other');
    const all = [first, second, other];
    expect(musicRoleGroups([first], all)[0].color).toBe(musicRoleGroups([second], all)[0].color);
    expect(musicRoleGroups([first], all)[0].color).not.toBe(musicRoleGroups([other], all)[0].color);
    expect(musicRoleGroups([second], all)[0].roles[0].part_id).toBe('viola');
  });
  it('colors only evidenced objects in the curated voice, and does not infer theme identity from prose', () => {
    const first = event('a', 'piano', '主题线条'); first.roles[0].voice_ids = ['1']; first.evidence[0].score_object_ids = ['a', 'b'];
    const objects = new Map(['a', 'b', 'c'].map((id, i) => [id, { source_id: id, part_id: 'piano', voice: i === 1 ? '2' : '1' } as MusicScoreObject]));
    expect(musicRoleGroups([first], [first])[0].key).toBe('role:主题线条');
    expect([...musicRoleColors([first], [first], objects).keys()]).toEqual(['a']);
  });
});
