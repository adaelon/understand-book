import type { MusicEvent, MusicSourceManifest, MusicVoiceFocus } from './music-contract';

/** Curated score events whose windows exactly name whole played measures follow later calibration. */
export function bindMusicEventOccurrences(manifest: Pick<MusicSourceManifest, 'events' | 'occurrences'>): MusicEvent[] {
  return manifest.events.map(event => {
    if (event.occurrence_ids?.length || event.evidence.some(e => e.audio !== null) || !event.evidence.some(e => e.score_object_ids.length)) return event;
    const selected = manifest.occurrences.filter(o => o.start_ms < event.end_ms && o.end_ms > event.start_ms);
    if (!selected.length || Math.abs(selected[0].start_ms - event.start_ms) > 0.001 || Math.abs(selected.at(-1)!.end_ms - event.end_ms) > 0.001) return event;
    return { ...event, occurrence_ids: selected.map(o => o.occurrence_id) };
  });
}

export function musicEventsAt(manifest: MusicSourceManifest, timeMs: number, partIds: string[] = [], voices: MusicVoiceFocus[] = []): MusicEvent[] {
  return manifest.events.filter(event => event.start_ms <= timeMs && event.end_ms > timeMs
    && (!partIds.length && !voices.length || event.roles.some(role => partIds.includes(role.part_id)
      || voices.some(focus => focus.part_id === role.part_id && (!role.voice_ids?.length || role.voice_ids.includes(focus.voice))))));
}

/** A comparison returns both original evidence sets, not an unsupported similarity label. */
export function musicEventComparison(manifest: MusicSourceManifest, eventId: string) {
  const event = manifest.events.find(e => e.event_id === eventId);
  if (!event) throw new Error(`Unknown music event: ${eventId}`);
  return { event, comparisons: event.relations.map(relation => {
    const target = manifest.events.find(e => e.event_id === relation.target_event_id);
    if (!target) throw new Error(`Music event ${eventId} refers to missing event ${relation.target_event_id}`);
    return { relation, target };
  }) };
}
