import { copyFileSync, existsSync, mkdirSync, readFileSync, readdirSync } from 'node:fs';
import { randomUUID } from 'node:crypto';
import { isDeepStrictEqual } from 'node:util';
import path from 'node:path';
import { segment } from './segment';
import { exportVideoCorrectionInput, saveCorrectionJson as save } from './video-correction';
import { validateTranscriptMap, validateVideoSourceManifest } from './video-contract';
import { publishAutomaticBuildArtifactSet } from './automatic-build-publication';
import { prepareVideoTranslation, videoTranslationInput, acceptVideoTranslation, publishVideoTranslationGroup,
  type VideoTranslationPlan } from './video-translation';
import { acceptVideoVisualCandidate, publishVideoVisualUnits, readVideoVisualPlan, type VideoVisualPlan, type VisualUnit } from './video-visual-units';
import { prepareVideoGraph, acceptVideoGraphCandidate, type VideoGraphInput } from './video-graph';
import { prepareVideoDiscourse, acceptVideoDiscourseCandidate, type VideoDiscourseInput } from './video-discourse';
import type { VideoSourceManifest } from './generated/VideoSourceManifest';
import type { TranscriptMap } from './generated/TranscriptMap';
import type { ReadOnlyBase } from './generated/ReadOnlyBase';
import type { PublishedBookRef } from './generated/PublishedBookRef';
import type { VideoEnrichments } from './video-enrichment';

const read = <T>(file: string): T => JSON.parse(readFileSync(file, 'utf8'));
const json = (value: unknown) => JSON.stringify(value, null, 2) + '\n';
const STATE = 'video_revision.json';
export const previousRevisionRoot = (workspace: string) => path.join(workspace, '.build/video-revision/previous');
type Decision = { stage: string; task: string; action: 'reused' | 'recompute'; reason: string; previous_task?: string };
export interface VideoRevision {
  version: 'video_revision.v1'; mode: 'transcript' | 'replacement'; previous_material: VideoSourceManifest['material'];
  previous_publication: PublishedBookRef; book_id: string; transcript_revision: string;
  replacement_path?: string; correction_revision?: number;
  stage: 'preparing' | 'deriving' | 'ready' | 'failed'; failure: string | null;
  initialized: boolean; decisions: Decision[]; publication?: PublishedBookRef;
}
export function readVideoRevision(workspace: string) { return read<VideoRevision>(path.join(workspace, STATE)); }
export function saveVideoRevision(workspace: string, state: VideoRevision) { save(path.join(workspace, STATE), state); }
function record(workspace: string, decision: Decision) {
  const state = readVideoRevision(workspace);
  const previous = state.decisions.find(d => d.stage === decision.stage && d.task === decision.task);
  if (previous) Object.assign(previous, decision); else state.decisions.push(decision);
  saveVideoRevision(workspace, state);
}
function copy(from: string, to: string) {
  mkdirSync(path.dirname(to), { recursive: true }); copyFileSync(from, to);
}
function directories(root: string) {
  return existsSync(root) ? readdirSync(root, { withFileTypes: true }).filter(e => e.isDirectory()).map(e => e.name) : [];
}
/** Save only reusable inputs/results, not executor leases, runs or publication links. */
function freezePrevious(source: string, workspace: string) {
  const destination = previousRevisionRoot(workspace);
  for (const name of ['video_source_manifest.json', 'transcript_map.json', 'base.json']) copy(path.join(source, name), path.join(destination, name));
  if (existsSync(path.join(source, 'video_enrichments.json'))) copy(path.join(source, 'video_enrichments.json'), path.join(destination, 'video_enrichments.json'));
  for (const stage of ['video-translation', 'video-visual', 'video-graph', 'video-discourse']) {
    const root = path.join(source, '.build', stage);
    const units = stage === 'video-translation' ? [''] : directories(root);
    for (const unit of units) {
      const relative = path.join('.build', stage, unit), folder = path.join(source, relative);
      for (const name of ['plan.json', 'input.json', 'accepted.json']) if (existsSync(path.join(folder, name)))
        copy(path.join(folder, name), path.join(destination, relative, name));
      if (existsSync(path.join(folder, 'accepted'))) for (const name of readdirSync(path.join(folder, 'accepted')))
        copy(path.join(folder, 'accepted', name), path.join(destination, relative, 'accepted', name));
      if (stage === 'video-visual' && existsSync(path.join(folder, 'plan.json'))) {
        for (const frame of read<VideoVisualPlan>(path.join(folder, 'plan.json')).frames)
          copy(path.join(source, frame.path), path.join(destination, frame.path));
      }
    }
  }
}
export function prepareVideoRevision(source: string, workspace: string,
  request: { corrections: string } | { replacement: string }) {
  source = path.resolve(source); workspace = path.resolve(workspace);
  if (source === workspace || workspace.startsWith(source + path.sep)) throw new Error('Use an independent new revision workspace');
  const manifest = validateVideoSourceManifest(read(path.join(source, 'video_source_manifest.json')));
  const link = read<{ reference: PublishedBookRef }>(path.join(source, 'video_publication.json'));
  if (link.reference.book_id !== manifest.material.book_id) throw new Error('Previous publication material mismatch');
  const correction = 'corrections' in request ? exportVideoCorrectionInput(path.resolve(request.corrections)).input : undefined;
  if (correction && !isDeepStrictEqual(correction.source_material, manifest.material)) throw new Error('Corrections belong to another material');
  if (correction && correction.canonical_text === readFileSync(path.join(source, manifest.canonical_text_path), 'utf8'))
    throw new Error('No adopted text change to publish');
  const file = path.join(workspace, STATE), mode = correction ? 'transcript' : 'replacement';
  let state: VideoRevision;
  if (existsSync(file)) {
    state = readVideoRevision(workspace);
    if (state.mode !== mode || !isDeepStrictEqual(state.previous_material, manifest.material)
      || correction && !isDeepStrictEqual(read(path.join(workspace, '.build/video-revision/accepted-input.json')), correction)
      || 'replacement' in request && state.replacement_path !== path.resolve(request.replacement))
      throw new Error('Revision input changed; use a new revision workspace');
    if (state.initialized) return state;
  } else {
    if (existsSync(workspace) && readdirSync(workspace).length) throw new Error('Revision destination must be empty');
    state = { version: 'video_revision.v1', mode, previous_material: manifest.material, previous_publication: link.reference,
      book_id: randomUUID(), transcript_revision: randomUUID(), initialized: false, stage: 'preparing', failure: null, decisions: [],
      ...(correction ? { correction_revision: correction.correction_revision } : { replacement_path: path.resolve((request as { replacement: string }).replacement) }) };
    // Save the frozen request before its resumable state becomes visible.
    if (correction) save(path.join(workspace, '.build/video-revision/accepted-input.json'), correction);
    saveVideoRevision(workspace, state);
  }
  freezePrevious(source, workspace);
  if (correction) {
    const before = read<TranscriptMap>(path.join(source, manifest.transcript_map_path));
    if (correction.segments.length !== before.segments.length || correction.segments.some((s, i) => s.segment_id !== before.segments[i].segment_id
      || !isDeepStrictEqual(s.location, before.segments[i].location))) throw new Error('Text correction must preserve original segment locations');
    let text = '';
    const blocks = correction.segments.map(s => { const start = text.length; text += s.text + '\n\n';
      return { kind: 'leaf' as const, text: s.text, span: { start, end: text.length } }; });
    const base: ReadOnlyBase = { book_id: state.book_id, lid_nodes: segment(blocks), graph_nodes: [], graph_edges: [] };
    const revised = validateVideoSourceManifest({ ...manifest, previous_book_id: manifest.material.book_id,
      material: { ...manifest.material, book_id: state.book_id, transcript: { ...manifest.material.transcript, transcript_revision: state.transcript_revision } },
      coverage: manifest.coverage.map(c => c.capability === 'transcript' ? c : { ...c, accepted: [], failed: [],
        pending: [{ video: manifest.material.video, start_ms: 0, end_ms: manifest.source.duration_ms }] }) });
    const map: TranscriptMap = { ...before, transcript: revised.material.transcript,
      segments: correction.segments.map((s, i) => ({ ...s, text_range: { lid: base.lid_nodes[i].lid, range: { start: 0, end: s.text.length } }, words: [] })) };
    validateTranscriptMap(map, revised, base, text);
    copy(path.join(source, manifest.source.imported_path), path.join(workspace, manifest.source.imported_path));
    copy(path.join(source, before.raw_result_path), path.join(workspace, before.raw_result_path));
    if (existsSync(path.join(source, 'video_import.json'))) copy(path.join(source, 'video_import.json'), path.join(workspace, 'video_import.json'));
    publishAutomaticBuildArtifactSet({ workspace_dir: workspace, stage: 'video_transcript', artifacts: {
      [manifest.canonical_text_path]: text, 'base.json': json(base), 'video_source_manifest.json': json(revised),
      [manifest.transcript_map_path]: json(map), 'transcript_revision.json': json({ version: 'video_transcript_revision.v1',
        previous_material: manifest.material, material: revised.material, correction_revision: correction.correction_revision,
        segments: before.segments.map((s, i) => ({ segment_id: s.segment_id, previous: s.text_range, current: map.segments[i].text_range,
          status: s.text === map.segments[i].text ? 'equivalent' : 'revised' })) }) } });
  }
  state.initialized = true; state.stage = 'deriving'; saveVideoRevision(workspace, state); return state;
}

/** Compare complete translation requests, not merely the translated paragraphs. */
export function reuseRevisionTranslations(workspace: string) {
  const state = readVideoRevision(workspace), previous = previousRevisionRoot(workspace);
  const oldFile = path.join(previous, '.build/video-translation/plan.json');
  const old = state.mode === 'transcript' && existsSync(oldFile) ? read<VideoTranslationPlan>(oldFile) : undefined;
  const planFile = path.join(workspace, '.build/video-translation/plan.json');
  if (!existsSync(planFile) && old?.version === 'video_translation_plan.v1') {
    const manifest = validateVideoSourceManifest(read(path.join(workspace, 'video_source_manifest.json')));
    const map = read<TranscriptMap>(path.join(workspace, manifest.transcript_map_path));
    // Preserve previously admitted smaller task cuts (e.g. transport-budget
    // splits) when they still cover the new input and fit its request budget.
    if (isDeepStrictEqual(old.tasks.flatMap(t => t.segment_ids), map.segments.map(s => s.segment_id))
      && old.tasks.every(t => t.segment_ids.length <= 32 && map.segments.filter(s => t.segment_ids.includes(s.segment_id)).reduce((n,s) => n+s.text.length,0) <= 3200)) {
      save(planFile, { ...old, material: manifest.material, segments: map.segments,
        tasks: old.tasks.map(t => ({...t,status:'pending',attempts:0,failure:null})) });
    }
  }
  const plan = prepareVideoTranslation(workspace);
  for (const task of plan.tasks.filter(t => t.status !== 'accepted')) {
    const target = videoTranslationInput(plan, task);
    const prior = old?.tasks.find(t => t.status === 'accepted' && isDeepStrictEqual(
      { ...videoTranslationInput(old, t), transcript: plan.material.transcript }, target));
    const file = prior && path.join(previous, `.build/video-translation/accepted/${prior.id}.json`);
    if (file && existsSync(file)) {
      acceptVideoTranslation(workspace, task.id, { transcript: plan.material.transcript, translations: read<Array<{segment_id: string; zh_text: string}>>(file)
        .map(t => ({ segment_id: t.segment_id, zh_text: t.zh_text })) });
      record(workspace, { stage: 'translation', task: task.id, action: 'reused', previous_task: prior!.id, reason: 'required text and both context windows equivalent' });
    } else record(workspace, { stage: 'translation', task: task.id, action: 'recompute', reason: 'full input changed or completed result unavailable' });
  }
  const current = prepareVideoTranslation(workspace);
  for (const group of new Set(current.tasks.map(t => t.group))) if (current.tasks.filter(t => t.group === group).every(t => t.status === 'accepted'))
    publishVideoTranslationGroup(workspace, group);
  return current;
}
export function revisionVisualAnalyses(workspace: string) { return directories(path.join(previousRevisionRoot(workspace), '.build/video-visual')); }
function nearby(plan: VideoVisualPlan, task: VideoVisualPlan['tasks'][number]) {
  return plan.transcript.segments.filter(s => s.location.end_ms > task.location.start_ms - 5000 && s.location.start_ms < task.location.end_ms + 5000);
}
export function prepareRevisionVisual(workspace: string, analysis: string) {
  const previous = previousRevisionRoot(workspace), relative = `.build/video-visual/${analysis}/plan.json`;
  const before = read<VideoVisualPlan>(path.join(previous, relative));
  if (readVideoRevision(workspace).mode !== 'transcript') throw new Error('Replacement media requires a new scan');
  if (!existsSync(path.join(workspace, relative))) {
    const manifest = validateVideoSourceManifest(read(path.join(workspace, 'video_source_manifest.json')));
    const transcript = read<TranscriptMap>(path.join(workspace, manifest.transcript_map_path));
    const plan: VideoVisualPlan = { ...before, input_ref: randomUUID(), manifest, transcript,
      tasks: before.tasks.map(t => ({ ...t, status: 'pending', attempts: 0, failure: null })) };
    for (const frame of plan.frames) copy(path.join(previous, frame.path), path.join(workspace, frame.path));
    save(path.join(workspace, relative), plan);
  }
  const plan = readVideoVisualPlan(workspace, analysis);
  for (const task of plan.tasks.filter(t => t.status !== 'accepted')) {
    const old = before.tasks.find(t => t.id === task.id)!;
    const accepted = path.join(previous, `.build/video-visual/${analysis}/accepted/${task.id}.json`);
    // Same imported media, scan/options, task boundaries, frame bytes and ordering
    // were copied above; nearby transcript is the remaining model dependency.
    if (old.status === 'accepted' && existsSync(accepted) && isDeepStrictEqual(nearby(before, old), nearby(plan, task))) {
      const units = read<VisualUnit[]>(accepted);
      acceptVideoVisualCandidate(workspace, analysis, task.id, { input_ref: plan.input_ref, task_id: task.id,
        events: units.map(u => ({ start_ms: u.location.start_ms, end_ms: u.location.end_ms,
          frame_ids: u.representative_frames.map(f => f.id), screen_text: u.screen_text, description: u.description, change: u.change })) });
      record(workspace, { stage: 'visual', task: analysis + '/' + task.id, action: 'reused', previous_task: task.id, reason: 'frames, scan settings and full nearby transcript equivalent' });
    } else record(workspace, { stage: 'visual', task: analysis + '/' + task.id, action: 'recompute', reason: 'nearby transcript changed or completed result unavailable' });
  }
  publishVideoVisualUnits(workspace, analysis); return readVideoVisualPlan(workspace, analysis);
}

export function revisionSemanticBatches(workspace: string, stage: 'graph' | 'discourse') {
  const root = path.join(previousRevisionRoot(workspace), '.build/video-' + stage);
  return directories(root).filter(id => existsSync(path.join(root, id, 'input.json'))
    && (stage !== 'graph' || !id.startsWith('correction-')
      && !existsSync(path.join(previousRevisionRoot(workspace), '.build/video-discourse', id, 'input.json'))))
    .sort((a, b) => {
      const x = read<VideoGraphInput>(path.join(root, a, 'input.json')), y = read<VideoGraphInput>(path.join(root, b, 'input.json'));
      return x.existing_nodes.length - y.existing_nodes.length || a.localeCompare(b);
    });
}
/** Normalize only explicit source bindings; text/meanings/support and PNG bytes remain compared. */
export function revisionSemanticInput(input: VideoGraphInput | VideoDiscourseInput, stage: 'graph' | 'discourse') {
  function normalize(value: unknown): unknown {
    if (Array.isArray(value)) return value.map(normalize);
    if (!value || typeof value !== 'object') return value;
    const v = value as Record<string, unknown>;
    if ('book_id' in v && 'publication_id' in v) return { book_id: 'current', publication_id: 'same-material' };
    if ('transcript_id' in v && 'transcript_revision' in v) return { ...v, transcript_revision: 'current' };
    return Object.fromEntries(Object.entries(v).map(([k, v]) => [k, normalize(v)]));
  }
  return normalize({ vu_ids: input.vu_ids, transcript: input.media.transcript, images: input.media.images,
    ...(stage === 'graph' ? { existing_nodes: input.existing_nodes } : { visual_units: (input as VideoDiscourseInput).visual_units }) });
}
export function prepareRevisionSemantic(workspace: string, stage: 'graph' | 'discourse', batch: string) {
  if (readVideoRevision(workspace).mode !== 'transcript') throw new Error('Replacement semantic work must select new media locations');
  const root = path.join(previousRevisionRoot(workspace), '.build/video-' + stage, batch);
  const old = read<VideoDiscourseInput>(path.join(root, 'input.json'));
  const link = read<{directory: string}>(path.join(workspace, 'video_publication.json')), published = path.resolve(workspace, link.directory);
  const catalog = existsSync(path.join(published, 'video_enrichments.json')) ? read<VideoEnrichments>(path.join(published, 'video_enrichments.json')) : { batches: [] };
  const units = catalog.batches.flatMap(b => b.files.filter(f => f.endsWith('/visual_units.json'))
    .flatMap(f => read<{ units: VisualUnit[] }>(path.join(published, f)).units));
  // VUs may split after the context changes. Select by the old actual media ranges,
  // then compare the newly delivered frame set; an ID alone never proves equality.
  const oldFrames = old.media.images.map(i => i.frame);
  const vu_ids = units.filter(u => old.vu_ids.includes(u.vu_id) || u.representative_frames.some(f => oldFrames.some(o => isDeepStrictEqual(o, f.frame))))
    .map(u => u.vu_id);
  const selection = { vu_ids, segment_ids: old.media.transcript.map(s => s.segment_id) };
  const fresh = stage === 'graph' ? prepareVideoGraph(workspace, batch, selection) : prepareVideoDiscourse(workspace, batch, selection);
  if (!existsSync(path.join(workspace, `video-enrichments/${batch}/${stage}.json`))) {
    if (existsSync(path.join(root, 'accepted.json')) && isDeepStrictEqual(revisionSemanticInput(old, stage), revisionSemanticInput(fresh, stage))) {
      const saved = read<any>(path.join(root, 'accepted.json'));
      const candidate = { ...(stage === 'graph' ? saved.candidate : saved), input_ref: fresh.input_ref };
      if (stage === 'graph') acceptVideoGraphCandidate(workspace, batch, candidate); else acceptVideoDiscourseCandidate(workspace, batch, candidate);
      record(workspace, { stage, task: batch, action: 'reused', previous_task: batch, reason: 'complete delivered transcript, frames and semantic context equivalent after source rebinding' });
    } else record(workspace, { stage, task: batch, action: 'recompute', reason: 'complete model input changed' });
  }
  return fresh;
}
