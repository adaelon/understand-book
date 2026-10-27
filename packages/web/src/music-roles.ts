import { t as uiText } from './i18n';
import type { MusicEvent, MusicEventRole, MusicScoreObject } from '../../core/src/music-contract';
import { MUSIC_PART_COLORS } from './music-playback';

export interface MusicRoleGroup { key: string; label: string; color: string; roles: MusicEventRole[] }
export function musicRoleKey(role: MusicEventRole): string { return role.theme_id ? `theme:${role.theme_id}` : `role:${role.role}`; }
/** Theme identity comes from curation, never from instrument names or role prose. */
export function musicRoleGroups(events: readonly MusicEvent[], allEvents: readonly MusicEvent[]): MusicRoleGroup[] {
  const keys = [...new Set(allEvents.flatMap(event => event.roles.map(musicRoleKey)))];
  const themes = keys.filter(key => key.startsWith('theme:'));
  const groups = new Map<string, MusicRoleGroup>();
  for (const role of events.flatMap(event => event.roles)) {
    const key = musicRoleKey(role);
    let group = groups.get(key);
    if (!group) {
      const themeIndex = themes.indexOf(key);
      group = { key, label: role.theme_id ? themes.length > 1 ? uiText('主题 {0}', [themeIndex + 1]) : uiText('主题') : role.role,
        color: role.theme_id ? ['#ae761a', '#9664a6', '#bc5367'][themeIndex % 3] : MUSIC_PART_COLORS[keys.indexOf(key) % MUSIC_PART_COLORS.length], roles: [] };
      groups.set(key, group);
    }
    group.roles.push(role);
  }
  return [...groups.values()];
}
export function musicRoleMatches(role: MusicEventRole, object: MusicScoreObject): boolean {
  return role.part_id === object.part_id && (!role.voice_ids?.length || role.voice_ids.includes(object.voice));
}
export function musicRoleColors(events: readonly MusicEvent[], allEvents: readonly MusicEvent[], objects: ReadonlyMap<string, MusicScoreObject>): Map<string, string> {
  const colors = new Map<string, string>();
  const palette = new Map(musicRoleGroups(events, allEvents).map(group => [group.key, group.color]));
  for (const event of events) for (const id of event.evidence.flatMap(evidence => evidence.score_object_ids)) {
    const object = objects.get(id); if (!object) continue;
    const roles = event.roles.filter(role => musicRoleMatches(role, object));
    const role = roles.find(role => role.theme_id) ?? roles[0];
    if (role) colors.set(id, palette.get(musicRoleKey(role))!);
  }
  return colors;
}
