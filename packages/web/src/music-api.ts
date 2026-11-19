import type { MusicSourceManifest, MusicQuestionContext, MusicRecordingRef, MusicSourceTarget } from '../../core/src/music-contract';
import type { PublishedBookRef } from './reader-context';
export type { MusicQuestionContext, MusicSourceTarget };
export interface MusicSourceSnapshot { target: MusicSourceTarget; title: string; label: string; preview: string; material: unknown }
export interface MusicContext {
  publication: PublishedBookRef;
  manifest: MusicSourceManifest;
  audio_url: string;
  score_pages: { page_id: string; url: string }[];
  demonstrations: { demonstration_id: string; url: string }[];
}
export type MusicAction = { kind: 'play'; start_ms?: number | null; end_ms?: number | null; cue_event_id?: string | null }
  | { kind: 'pause' | 'clear_loop' } | { kind: 'seek'; time_ms: number }
  | { kind: 'loop'; start_ms: number; end_ms: number } | { kind: 'focus'; part_ids: string[]; voices?: { part_id: string; voice: string }[] }
  | { kind: 'rate'; rate: number } | { kind: 'demonstration'; demonstration_id: string; cue_event_id?: string | null };
export interface MusicCommand {
  command_id: string; run_turn_id: string; session_id: string;
  request: { operation: 'observe' | 'act'; publication: PublishedBookRef; recording: MusicRecordingRef; based_on?: string; action?: MusicAction };
}
export interface MusicPlaybackReceipt {
  receipt_id: string; command_id: string; publication: PublishedBookRef; recording: MusicRecordingRef;
  status: 'observed' | 'applied' | 'failed'; current_ms: number; paused: boolean; playback_rate: number;
  loop: { start_ms: number; end_ms: number } | null; focused_part_ids: string[];
  focused_voices?: { part_id: string; voice: string }[];
  occurrence_id: string | null; demonstration_id: string | null; error: string | null;
  demonstration_state?: { current_ms: number; paused: boolean };
  displayed_cue?: { event_id: string; text: string };
}
