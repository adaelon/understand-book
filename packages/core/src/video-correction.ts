import { existsSync, mkdirSync, readFileSync, renameSync, writeFileSync } from 'node:fs';
import { randomUUID } from 'node:crypto';
import { isDeepStrictEqual } from 'node:util';
import path from 'node:path';
import { z } from 'zod';
import type { VideoMaterial } from './generated/VideoMaterial';
import type { TranscriptSegment } from './generated/TranscriptSegment';
import type { PublishedBookRef } from './generated/PublishedBookRef';
import type { EvidenceTarget } from './generated/EvidenceTarget';
import type { ExecutorMediaInputV1 } from './build-executor-media';
import { prepareVideoGraph } from './video-graph';
import { MediaLocationZ, PublishedBookRefZ, TranscriptSegmentZ, validateTranscriptMap, validateVideoSourceManifest } from './video-contract';

const nonempty = z.string().trim().min(1);
const text = z.string().refine(v => !!v.trim(), 'Text must not be empty');
const evidence = z.array(z.object({ source_id: nonempty, role: nonempty }).strict()).min(1).max(16);
export const VideoCorrectionCandidateZ = z.object({ input_ref: nonempty,
  changes: z.array(z.object({ id: nonempty, segment_id: nonempty, start: z.number().int().nonnegative(),
    before: text, after: z.string(), reason: nonempty, basis: z.enum(['audio_review', 'transcript_context']), evidence,
  }).strict()).max(32),
  notes: z.array(z.object({ kind: z.enum(['uncertain', 'visual_explanation', 'background']),
    text: nonempty, evidence,
  }).strict()).max(32),
}).strict();
export const CorrectionAudioReviewZ = z.object({
  review_result_ref: nonempty, publication: PublishedBookRefZ, location: MediaLocationZ,
  question: nonempty, reviewed_text: z.string(), uncertainties: z.array(z.string()),
  reviewed_words: z.array(z.object({ text: z.string(), start_ms: z.number(), end_ms: z.number(), probability: z.number() })),
  provider: z.record(z.unknown()), transcript: z.array(TranscriptSegmentZ),
}).passthrough();
export type CorrectionAudioReview = z.infer<typeof CorrectionAudioReviewZ>;
type Candidate = z.infer<typeof VideoCorrectionCandidateZ>;
export type CorrectionChange = Candidate['changes'][number] & {
  status: 'pending' | 'accepted' | 'rejected'; applied_revision?: number;
};
export interface CorrectionSegment {
  segment_id: string; text: string; edit_revision: number; location: TranscriptSegment['location'];
}
export interface CorrectionSource {
  source_id: string; kind: 'transcript' | 'frame' | 'audio_review'; target: EvidenceTarget;
  segment_id?: string; transcript?: TranscriptSegment; review?: CorrectionAudioReview;
}
export interface VideoCorrectionInput {
  version: 'video_correction_input.v1'; input_ref: string; draft_id: string;
  material: VideoMaterial; publication: PublishedBookRef; question: string;
  segments: CorrectionSegment[]; sources: CorrectionSource[]; media: ExecutorMediaInputV1;
}
export interface VideoCorrectionDraft {
  draft_id: string; input_ref: string; publication: PublishedBookRef; producer: { harness: string; model: string | null };
  segments: CorrectionSegment[]; sources: CorrectionSource[]; changes: CorrectionChange[]; notes: Candidate['notes'];
}
export interface VideoCorrections {
  version: 'video_corrections.v1'; material: VideoMaterial; revision: number;
  segments: CorrectionSegment[]; drafts: VideoCorrectionDraft[];
  history: Array<{ revision: number; segment_id: string; before: string; after: string; reason: string;
    origin: 'manual' | 'draft'; draft_id?: string; change_ids?: string[] }>;
}
const STATE = 'transcript_corrections.json';
const read = <T>(file: string): T => JSON.parse(readFileSync(file, 'utf8'));
export function saveCorrectionJson(file: string, value: unknown) {
  mkdirSync(path.dirname(file), { recursive: true });
  writeFileSync(file + '.tmp', JSON.stringify(value, null, 2) + '\n'); renameSync(file + '.tmp', file);
}
export function correctionRoot(workspace: string, draft?: string) {
  if (draft !== undefined && !/^[a-zA-Z0-9_-]+$/.test(draft)) throw new Error('Invalid correction draft ID');
  return path.join(workspace, '.build/video-corrections', ...(draft ? [draft] : []));
}
export function readVideoCorrections(workspace: string): VideoCorrections {
  const manifest = validateVideoSourceManifest(read(path.join(workspace, 'video_source_manifest.json')));
  const file = path.join(workspace, STATE);
  if (existsSync(file)) {
    const state = read<VideoCorrections>(file);
    if (state.version !== 'video_corrections.v1' || !isDeepStrictEqual(state.material, manifest.material))
      throw new Error('Correction material changed; use the matching source workspace');
    return state;
  }
  const map = validateTranscriptMap(read(path.join(workspace, manifest.transcript_map_path)), manifest,
    read(path.join(workspace, 'base.json')), readFileSync(path.join(workspace, manifest.canonical_text_path), 'utf8'));
  return { version: 'video_corrections.v1', material: manifest.material, revision: 0,
    segments: map.segments.map(s => ({ segment_id: s.segment_id, text: s.text, location: s.location, edit_revision: 0 })),
    drafts: [], history: [] };
}
function save(workspace: string, state: VideoCorrections) { saveCorrectionJson(path.join(workspace, STATE), state); }
function segment(state: { segments: CorrectionSegment[] }, id: string) {
  const found = state.segments.find(s => s.segment_id === id);
  if (!found) throw new Error('Unknown correction segment: ' + id);
  return found;
}
export function editVideoTranscript(workspace: string, value: unknown) {
  const edit = z.object({ segment_id: nonempty, expected_revision: z.number().int().nonnegative(),
    before: text, after: text, reason: nonempty }).strict().parse(value);
  const state = readVideoCorrections(workspace), current = segment(state, edit.segment_id);
  if (current.edit_revision !== edit.expected_revision || current.text !== edit.before)
    throw new Error('Transcript edit conflict; reread the current segment');
  if (edit.before === edit.after) return state;
  current.text = edit.after; current.edit_revision++; state.revision++;
  state.history.push({ revision: state.revision, segment_id: edit.segment_id, before: edit.before, after: edit.after,
    reason: edit.reason, origin: 'manual' });
  save(workspace, state); return state;
}

export interface CorrectionSelection { segment_ids: string[]; vu_ids: string[]; question: string; audio_reviews?: CorrectionAudioReview[] }
export function prepareVideoCorrection(workspace: string, draft: string, selection: CorrectionSelection) {
  correctionRoot(workspace, draft);
  if (!selection.question.trim() || !selection.segment_ids.length || new Set(selection.segment_ids).size !== selection.segment_ids.length)
    throw new Error('Select unique transcript segments and a correction question');
  const file = path.join(correctionRoot(workspace, draft), 'input.json');
  // VI13 already freezes the VI4 native frame bytes and original transcript sources.
  const material = prepareVideoGraph(workspace, 'correction-' + draft, selection);
  const state = readVideoCorrections(workspace);
  if (!isDeepStrictEqual(material.manifest.material, state.material)) throw new Error('Correction publication material mismatch');
  const reviews = (selection.audio_reviews ?? []).map(value => CorrectionAudioReviewZ.parse(value));
  if (new Set(reviews.map(r => r.review_result_ref)).size !== reviews.length) throw new Error('Duplicate audio review');
  const sources: CorrectionSource[] = material.media.transcript.map(s => ({ source_id: 'text:' + s.segment_id,
    kind: 'transcript', segment_id: s.segment_id, transcript: s, target: { kind: 'text', publication: material.publication,
      transcript: material.manifest.material.transcript, text_range: s.text_range } }));
  sources.push(...material.media.images.map(i => ({ source_id: i.image_id, kind: 'frame' as const,
    target: { kind: 'visual' as const, publication: material.publication, frames: [i.frame], regions: [] } })));
  for (const review of reviews) {
    const r = review.location;
    if (!isDeepStrictEqual(review.publication, material.publication) || !isDeepStrictEqual(r.video, material.manifest.material.video)
      || r.start_ms < 0 || r.end_ms > material.manifest.source.duration_ms || r.end_ms <= r.start_ms || r.end_ms - r.start_ms > 30000
      || !material.media.transcript.some(s => s.location.start_ms < r.end_ms && r.start_ms < s.location.end_ms)
      || !isDeepStrictEqual(review.transcript, material.transcript.segments.filter(s => s.location.start_ms < r.end_ms && r.start_ms < s.location.end_ms)))
      throw new Error('Audio review does not match the input publication/transcript range');
    sources.push({ source_id: 'audio:' + review.review_result_ref, kind: 'audio_review', review,
      target: { kind: 'audio', publication: review.publication, location: r, review_result_ref: review.review_result_ref } });
  }
  if (existsSync(file)) {
    const prior = readVideoCorrectionInput(workspace, draft);
    if (prior.question !== selection.question || !isDeepStrictEqual(prior.sources, sources))
      throw new Error('Correction request changed; use a new draft ID');
    return prior; // A manual edit while a model runs does not rewrite its frozen input.
  }
  const input: VideoCorrectionInput = { version: 'video_correction_input.v1', input_ref: randomUUID(), draft_id: draft,
    material: state.material, publication: material.publication, question: selection.question,
    segments: selection.segment_ids.map(id => structuredClone(segment(state, id))), sources,
    media: { ...material.media, stage: 'video_correction' } };
  saveCorrectionJson(file, input); return input;
}
export function readVideoCorrectionInput(workspace: string, draft: string) {
  const input = read<VideoCorrectionInput>(path.join(correctionRoot(workspace, draft), 'input.json'));
  if (input.version !== 'video_correction_input.v1' || input.draft_id !== draft
    || !isDeepStrictEqual(input.material, readVideoCorrections(workspace).material)) throw new Error('Correction input material mismatch');
  return input;
}
function apply(text: string, changes: Candidate['changes']) {
  for (const c of [...changes].sort((a,b) => b.start - a.start)) text = text.slice(0,c.start) + c.after + text.slice(c.start + c.before.length);
  return text;
}
export class VideoCorrectionCandidateError extends Error {
  constructor(message: string, readonly json_pointer = '/') { super(message); }
}
export function admitVideoCorrection(input: VideoCorrectionInput, value: unknown): Candidate {
  const parsed = VideoCorrectionCandidateZ.safeParse(value);
  if (!parsed.success) {
    const issue = parsed.error.issues[0];
    throw new VideoCorrectionCandidateError(issue.message, '/' + issue.path.join('/'));
  }
  const c = parsed.data;
  if (c.input_ref !== input.input_ref) throw new VideoCorrectionCandidateError('Correction candidate input mismatch','/input_ref');
  if (new Set(c.changes.map(x => x.id)).size !== c.changes.length) throw new VideoCorrectionCandidateError('Duplicate correction ID','/changes');
  const resolve = (refs: z.infer<typeof evidence>) => refs.map(e => {
    const source = input.sources.find(s => s.source_id === e.source_id);
    if (!source) throw new VideoCorrectionCandidateError('Correction source was not delivered: ' + e.source_id);
    return source;
  });
  for (const [index, change] of c.changes.entries()) {
    const s = input.segments.find(s => s.segment_id === change.segment_id), refs = resolve(change.evidence);
    if (!s) throw new VideoCorrectionCandidateError('Unknown correction segment','/changes/'+index+'/segment_id');
    if (s.text.slice(change.start, change.start + change.before.length) !== change.before || change.before === change.after)
      throw new VideoCorrectionCandidateError('Correction before text/range mismatch or unchanged proposal. Use the exact before substring and its zero-based UTF-16 offset in segments[].text.', '/changes/'+index);
    if (change.basis === 'audio_review' ? !refs.some(r => r.review && r.review.location.start_ms < s.location.end_ms && s.location.start_ms < r.review.location.end_ms)
      : !refs.some(r => r.kind === 'transcript' && r.segment_id === change.segment_id))
      throw new VideoCorrectionCandidateError('Correction requires matching speech/text support; a frame alone is not spoken wording','/changes/'+index+'/evidence');
  }
  for (const s of input.segments) {
    const edits = c.changes.filter(c => c.segment_id === s.segment_id).sort((a,b) => a.start-b.start);
    if (edits.some((c,i) => i > 0 && c.start < edits[i-1].start + edits[i-1].before.length)) throw new VideoCorrectionCandidateError('Overlapping correction changes','/changes');
    if (!apply(s.text, edits).trim()) throw new VideoCorrectionCandidateError('Correction cannot erase an entire segment','/changes');
  }
  c.notes.forEach(n => resolve(n.evidence)); return c;
}
/** Candidate admission commits only a reviewable draft. No proposed change is applied here. */
export function saveVideoCorrectionDraft(workspace: string, draft: string, value: unknown,
  producer: VideoCorrectionDraft['producer'] = { harness: 'codex', model: null }) {
  const input = readVideoCorrectionInput(workspace, draft), c = admitVideoCorrection(input, value), state = readVideoCorrections(workspace);
  const prior = state.drafts.find(d => d.draft_id === draft);
  if (prior) {
    const changes = prior.changes.map(({ status: _, applied_revision: __, ...change }) => change);
    if (!isDeepStrictEqual({ input_ref: prior.input_ref, changes, notes: prior.notes }, c)) throw new Error('Correction draft cannot be replaced');
    return prior;
  }
  const record: VideoCorrectionDraft = { draft_id: draft, input_ref: input.input_ref, publication: input.publication, producer,
    segments: input.segments, sources: input.sources, changes: c.changes.map(x => ({ ...x, status: 'pending' })), notes: c.notes };
  state.drafts.push(record); save(workspace, state); return record;
}
function expectedSegment(draft: VideoCorrectionDraft, id: string) {
  const frozen = segment(draft, id), accepted = draft.changes.filter(c => c.segment_id === id && c.status === 'accepted');
  return { text: apply(frozen.text, accepted), edit_revision: Math.max(frozen.edit_revision, ...accepted.map(c => c.applied_revision!)) };
}
export function inspectVideoCorrection(workspace: string, id: string) {
  const state = readVideoCorrections(workspace), draft = state.drafts.find(d => d.draft_id === id);
  if (!draft) throw new Error('Correction draft is not ready');
  return { ...draft, material: state.material, changes: draft.changes.map(c => {
    const current = segment(state,c.segment_id), expected = expectedSegment(draft,c.segment_id);
    const conflict = c.status === 'pending' && (current.text !== expected.text || current.edit_revision !== expected.edit_revision);
    return { ...c, current_text: current.text, conflict,
      conflict_reason: conflict ? '该语段在草稿之后已有其他编辑；旧建议保持待处理，请以当前文字生成新草稿。' : null };
  }) };
}
export function decideVideoCorrection(workspace: string, id: string, action: 'accept' | 'reject', ids: string[] | 'all') {
  if (action !== 'accept' && action !== 'reject') throw new Error('Expected accept or reject');
  const state = readVideoCorrections(workspace), draft = state.drafts.find(d => d.draft_id === id);
  if (!draft) throw new Error('Correction draft is not ready');
  const selected = ids === 'all' ? draft.changes.filter(c => c.status === 'pending') : [...new Set(ids)].map(id => {
    const c = draft.changes.find(c => c.id === id); if (!c) throw new Error('Unknown correction ID: ' + id); return c;
  });
  const pending = selected.filter(c => c.status === 'pending'), conflicts: string[] = [], applied: string[] = [];
  if (action === 'reject') pending.forEach(c => { c.status = 'rejected'; });
  else for (const id of new Set(pending.map(c => c.segment_id))) {
    const current = segment(state,id), expected = expectedSegment(draft,id), edits = pending.filter(c => c.segment_id === id);
    if (current.edit_revision !== expected.edit_revision || current.text !== expected.text) { conflicts.push(...edits.map(c => c.id)); continue; }
    const before = current.text;
    current.text = apply(segment(draft,id).text, draft.changes.filter(c => c.segment_id === id && (c.status === 'accepted' || edits.includes(c))));
    current.edit_revision++; state.revision++;
    edits.forEach(c => { c.status = 'accepted'; c.applied_revision = current.edit_revision; applied.push(c.id); });
    state.history.push({ revision: state.revision, segment_id: id, before, after: current.text,
      reason: edits.map(c => c.reason).join('\n'), origin: 'draft', draft_id: draft.draft_id, change_ids: edits.map(c => c.id) });
  }
  if (applied.length || action === 'reject' && pending.length) save(workspace,state);
  return { applied, rejected: action === 'reject' ? pending.map(c => c.id) : [], conflicts, draft: inspectVideoCorrection(workspace,id) };
}
export function exportVideoCorrectionInput(workspace: string) {
  const state = readVideoCorrections(workspace);
  const output = { version: 'video_transcript_revision_input.v1', source_material: state.material, correction_revision: state.revision,
    canonical_text: state.segments.map(s => s.text).join('\n\n') + '\n\n',
    segments: state.segments.map(({ edit_revision: _, ...s }) => s), history: state.history,
    adopted_drafts: state.drafts.filter(d => d.changes.some(c => c.status === 'accepted')).map(d => ({ ...d, changes: d.changes.filter(c => c.status === 'accepted') })) };
  const file = path.join(correctionRoot(workspace), 'accepted-input.json'); saveCorrectionJson(file,output);
  return { file, input: output };
}
