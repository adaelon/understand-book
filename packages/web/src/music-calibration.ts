import { t as uiText } from './i18n';
import type { MusicAlignmentRevision, MusicAlignmentRevisionReceipt } from '../../core/src/music-alignment';
import type { MusicNoteAlignment, MusicOccurrence, MusicScorePage, MusicSourceManifest } from '../../core/src/music-contract';
export type { MusicAlignmentRevision };

export interface MusicCalibrationContext {
  manifest: MusicSourceManifest;
  audio_url: string;
  score_pages: Array<MusicScorePage & { url: string }>;
  history: Array<{ revision: MusicAlignmentRevision; receipt: MusicAlignmentRevisionReceipt }>;
}
export interface CalibrationNoteDraft {
  occurrence_id: string; score_object_id: string;
  start_ms: number | null; end_ms: number | null; attack: boolean; unobserved?: boolean;
}
export interface CalibrationDraft {
  start_ms: number; end_ms: number;
  occurrences: MusicOccurrence[]; notes: CalibrationNoteDraft[];
  reason: string; evidence: string; reviewed: boolean;
  acceptance: 'pending' | 'measure' | 'note';
}

export function createCalibrationDraft(manifest: MusicSourceManifest, first: number, last: number): CalibrationDraft {
  const selected = manifest.occurrences.slice(first, last + 1);
  if (!selected.length || last < first) throw new Error(uiText('请选择连续的小节出现。'));
  const objects = new Map(manifest.score_objects.map(object => [object.source_id, object]));
  const notes = new Map(manifest.note_alignments.map(note => [`${note.occurrence_id}/${note.score_object_id}`, note]));
  return {
    start_ms: selected[0].start_ms, end_ms: selected.at(-1)!.end_ms,
    occurrences: selected.map(occurrence => ({ ...occurrence, score_object_ids: [...occurrence.score_object_ids] })),
    notes: selected.flatMap(occurrence => occurrence.score_object_ids.flatMap(id => {
      const object = objects.get(id);
      if (!object || object.rest) return [];
      const previous = notes.get(`${occurrence.occurrence_id}/${id}`);
      return [{ occurrence_id: occurrence.occurrence_id, score_object_id: id, start_ms: previous?.start_ms ?? null, end_ms: previous?.end_ms ?? null,
        attack: previous?.attack ?? (object.tie !== 'continue' && object.tie !== 'stop'),
        ...(previous?.unobserved ? { unobserved: true } : {}) }];
    })),
    reason: '', evidence: '', reviewed: false, acceptance: 'pending',
  };
}

/** Every saved window has an explicit precision decision; missing notes stay absent. */
export function calibrationRevision(manifest: MusicSourceManifest, draft: CalibrationDraft, provenanceId: string): MusicAlignmentRevision {
  if (!draft.reason.trim()) throw new Error(uiText('请写下本次修改原因。'));
  if (!draft.evidence.trim()) throw new Error(uiText('请说明时间判断的依据。'));
  if (draft.acceptance !== 'pending' && !draft.reviewed) throw new Error(uiText('接纳前请复听并确认本窗口。'));
  const finite = (value: number | null): value is number => value !== null && Number.isFinite(value);
  for (const [index, occurrence] of draft.occurrences.entries()) {
    if (!finite(occurrence.start_ms) || !finite(occurrence.end_ms) || occurrence.start_ms < draft.start_ms || occurrence.end_ms > draft.end_ms || occurrence.start_ms >= occurrence.end_ms)
      throw new Error(uiText('第 {0} 小节的起止必须位于当前窗口内；外移边界请同时选择相邻小节。', [occurrence.measure]));
    if (index && occurrence.start_ms < draft.occurrences[index - 1].end_ms) throw new Error(uiText('相邻小节的时间不能重叠。'));
  }
  const oldNotes = new Map(manifest.note_alignments.map(note => [`${note.occurrence_id}/${note.score_object_id}`, note]));
  const notes: MusicNoteAlignment[] = [];
  for (const note of draft.notes) {
    if (note.start_ms === null && note.end_ms === null) continue;
    if (!finite(note.start_ms) || !finite(note.end_ms) || note.start_ms < draft.start_ms || note.start_ms >= draft.end_ms || note.end_ms <= note.start_ms || note.end_ms > manifest.recording.duration_ms)
      throw new Error(uiText('每个已填写音符都需要有效的起止时间；长音结束可以跨小节。'));
    const occurrence = draft.occurrences.find(item => item.occurrence_id === note.occurrence_id)!;
    if (note.start_ms < occurrence.start_ms || note.start_ms >= occurrence.end_ms) throw new Error(uiText('音符起点必须位于所属小节内。'));
    const previous = oldNotes.get(`${note.occurrence_id}/${note.score_object_id}`);
    // Explicitly confirming a new attack supersedes an unobserved-tone decision.
    const unobserved = note.unobserved && !note.attack;
    const unchanged = previous && previous.start_ms === note.start_ms && previous.end_ms === note.end_ms && previous.attack === note.attack
      && !!previous.unobserved === !!unobserved;
    if (!unchanged && !draft.reviewed) throw new Error(uiText('新增或修改音符时间需要复听依据；请先核对录音并勾选复听确认，仍可保存为待核候选。'));
    const object = manifest.score_objects.find(item => item.source_id === note.score_object_id)!;
    if (note.attack && (object.tie === 'continue' || object.tie === 'stop')) throw new Error(uiText('延音线接续音不能标记为重新起音。'));
    notes.push({ ...note, unobserved, start_ms: note.start_ms, end_ms: note.end_ms, provenance_id: unchanged ? previous.provenance_id : provenanceId });
  }
  if (draft.acceptance === 'note' && notes.length !== draft.notes.length) throw new Error(uiText('还有 {0} 个音符没有时间，不能接纳逐音符定位。', [draft.notes.length - notes.length]));
  if (draft.acceptance !== 'pending' && (draft.occurrences[0].start_ms !== draft.start_ms || draft.occurrences.at(-1)!.end_ms !== draft.end_ms || draft.occurrences.some((item, index) => index > 0 && item.start_ms !== draft.occurrences[index - 1].end_ms))) throw new Error(uiText('接纳的窗口需要被连续小节完整覆盖，不能留下时间空隙。'));
  const base = { start_ms: draft.start_ms, end_ms: draft.end_ms, method: 'manual' as const, provenance_id: provenanceId, detail: draft.evidence.trim() };
  return {
    recording: { ...manifest.recording.source }, score: { ...manifest.score.source }, start_ms: draft.start_ms, end_ms: draft.end_ms, reason: draft.reason.trim(),
    occurrences: draft.occurrences.map(occurrence => ({ ...occurrence, score_object_ids: [...occurrence.score_object_ids] })), note_alignments: notes,
    coverage: [{ ...base, granularity: 'measure', status: draft.acceptance === 'pending' ? 'pending' : 'accepted' },
      ...(notes.length || draft.acceptance === 'note' ? [{ ...base, granularity: 'note' as const, status: draft.acceptance === 'note' ? 'accepted' as const : 'pending' as const }] : [])],
    provenance: [{ provenance_id: provenanceId, kind: draft.reviewed ? 'human_listening' : 'interpretation', description: draft.evidence.trim(), source_url: null, processor: null }],
  };
}

export function calibrationTime(ms: number) {
  const seconds = ms / 1000;
  return `${Math.floor(seconds / 60)}:${(seconds % 60).toFixed(3).padStart(6, '0')}`;
}
