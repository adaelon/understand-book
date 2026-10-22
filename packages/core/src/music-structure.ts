import type { MusicSourceManifest } from './music-contract';

export function musicNavigation(manifest: MusicSourceManifest) {
  return manifest.structure.map(section => ({ ...section,
    events: section.event_ids.map(id => manifest.events.find(event => event.event_id === id)).filter(event => event !== undefined),
    children: manifest.structure.filter(child => child.parent_id === section.section_id).map(child => child.section_id),
  }));
}
