import { existsSync, readFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { execFile } from 'node:child_process';
import { promisify } from 'node:util';
import { prepareVideoRevision, readVideoRevision, saveVideoRevision, previousRevisionRoot,
  reuseRevisionTranslations, revisionVisualAnalyses, prepareRevisionVisual, revisionSemanticBatches, prepareRevisionSemantic } from '../../packages/core/src/video-revision';
import { importVideo } from '../../packages/core/src/video-import';
import { buildVideoTranscript } from '../../packages/core/src/video-transcript';
import { saveCorrectionJson as save } from '../../packages/core/src/video-correction';
import { prepareVideoVisualUnits, type VisualUnit, type VideoVisualPlan } from '../../packages/core/src/video-visual-units';
import { prepareVideoGraph } from '../../packages/core/src/video-graph';
import { prepareVideoDiscourse } from '../../packages/core/src/video-discourse';
import { runVideoTranslation } from './video-translation';
import { runVideoVisualUnits, publishVideoVisualSnapshot } from './video-visual-units';
import { runVideoGraph } from './video-graph';
import { runVideoDiscourse } from './video-discourse';
import type { TranscriptMap } from '../../packages/core/src/generated/TranscriptMap';
import type { VideoSourceManifest } from '../../packages/core/src/generated/VideoSourceManifest';
import type { VideoEnrichments } from '../../packages/core/src/video-enrichment';
const read = <T>(file: string): T => JSON.parse(readFileSync(file, 'utf8'));

/** New media has no old timeline correspondence. All times come from its new ASR/scan. */
export async function buildReplacementTranscript(workspace: string, options: Parameters<typeof buildVideoTranscript>[1] = {}) {
  const state = readVideoRevision(workspace);
  if (state.mode !== 'replacement') throw new Error('Expected a media replacement revision');
  await importVideo(state.replacement_path, workspace);
  const result = await buildVideoTranscript(workspace, options);
  if (result.stage !== 'ready') throw new Error(result.failure || 'Replacement transcript incomplete');
  const file = path.join(workspace, 'video_source_manifest.json'), manifest = read<VideoSourceManifest>(file);
  manifest.previous_book_id = state.previous_material.book_id;
  save(file, manifest);
  state.book_id = manifest.material.book_id; state.transcript_revision = manifest.material.transcript.transcript_revision;
  saveVideoRevision(workspace, state); return manifest;
}
function replacementSelections(workspace: string) {
  const map = read<TranscriptMap>(path.join(workspace, 'transcript_map.json'));
  const catalog = existsSync(path.join(workspace, 'video_enrichments.json'))
    ? read<VideoEnrichments>(path.join(workspace, 'video_enrichments.json')) : { batches: [] };
  const units = catalog.batches.flatMap(b => b.files.filter(f => f.endsWith('/visual_units.json'))
    .flatMap(f => read<{units: VisualUnit[]}>(path.join(workspace, f)).units));
  const selections: Array<{segment_ids: string[]; vu_ids: string[]}> = [];
  let group: TranscriptMap['segments'] = [];
  for (const s of map.segments) {
    if (group.length && s.location.end_ms - group[0].location.start_ms > 40000) {
      selections.push({segment_ids: group.map(s => s.segment_id), vu_ids: []}); group = [];
    }
    group.push(s);
  }
  if (group.length) selections.push({segment_ids: group.map(s => s.segment_id), vu_ids: []});
  for (const u of units) {
    // Pure visual units also get work even where ASR detected silence.
    const ids = map.segments.filter(s => s.location.start_ms < u.location.end_ms && s.location.end_ms > u.location.start_ms);
    const begin = Math.min(u.location.start_ms, ...ids.map(s => s.location.start_ms));
    const end = Math.max(u.location.end_ms, ...ids.map(s => s.location.end_ms));
    if (end - begin > 60000) throw new Error('Replacement visual unit exceeds semantic task bound; split its visual analysis before continuing');
    selections.push({segment_ids: ids.map(s => s.segment_id), vu_ids: [u.vu_id]});
  }
  return selections;
}
export async function publishVideoRevision(workspace: string, publisher: string, service: string) {
  // The first revision publication may have only English; later calls use the
  // normal cumulative catalog and durable publisher link for recovery.
  if (existsSync(path.join(workspace, 'video_enrichments.json'))) return publishVideoVisualSnapshot(workspace, publisher, service);
  const linkFile = path.join(workspace, 'video_publication.json');
  if (existsSync(linkFile)) {
    const link = read<any>(linkFile);
    if (existsSync(path.join(service, 'library/published', link.reference.book_id, link.reference.publication_id)))
      return { reused: true, reference: link.reference, elapsed_ms: 0 };
  }
  const started = Date.now();
  const {stdout} = await promisify(execFile)(publisher, [path.resolve(service), 'import', path.resolve(workspace)], {windowsHide:true,maxBuffer:8*1024*1024});
  return {reused:false, reference:JSON.parse(stdout).reference, elapsed_ms:Date.now()-started};
}
export async function runVideoRevision(workspace: string, executable: string, publisher: string, service: string) {
  workspace = path.resolve(workspace);
  let state = readVideoRevision(workspace);
  if (state.stage === 'ready') return state;
  state.stage = 'deriving'; state.failure = null; saveVideoRevision(workspace, state);
  const publish = () => publishVideoRevision(workspace, publisher, service);
  try {
    if (state.mode === 'replacement') await buildReplacementTranscript(workspace, {
      python: process.env.VIDEO_WHISPER_PYTHON, modelDir: process.env.VIDEO_WHISPER_MODEL_DIR });
    const priorCatalogFile = path.join(previousRevisionRoot(workspace), 'video_enrichments.json');
    const priorCatalog = existsSync(priorCatalogFile) ? read<VideoEnrichments>(priorCatalogFile) : {batches:[]};
    if (existsSync(path.join(previousRevisionRoot(workspace), '.build/video-translation/plan.json'))
      || priorCatalog.batches.some(b => b.coverage.some(c => c.capability === 'translation'))) {
      reuseRevisionTranslations(workspace);
      await runVideoTranslation(workspace, executable);
    }
    for (const analysis of revisionVisualAnalyses(workspace)) {
      if (state.mode === 'transcript') prepareRevisionVisual(workspace, analysis);
      else {
        const old = read<VideoVisualPlan>(path.join(previousRevisionRoot(workspace), `.build/video-visual/${analysis}/plan.json`));
        const manifest = read<VideoSourceManifest>(path.join(workspace, 'video_source_manifest.json'));
        const ranges = [];
        for (let start_ms = 0; start_ms < manifest.source.duration_ms; start_ms += 30000)
          ranges.push({video:manifest.material.video,start_ms,end_ms:Math.min(start_ms+30000,manifest.source.duration_ms)});
        await prepareVideoVisualUnits(workspace, analysis, old.options, ranges);
      }
      await runVideoVisualUnits(workspace, analysis, executable);
    }
    await publish(); // Semantic evidence needs a real, readable new-material publication.
    for (const stage of ['graph', 'discourse'] as const) {
      const batches = revisionSemanticBatches(workspace, stage);
      if (state.mode === 'transcript') for (const batch of batches) {
        prepareRevisionSemantic(workspace, stage, batch);
        if (stage === 'graph') await runVideoGraph(workspace, batch, executable);
        else await runVideoDiscourse(workspace, batch, executable);
        // currentGraph reads accepted workspace contributions; defer the full
        // media copy until all tasks in this stage are admitted.
      }
      else if (batches.length) for (const [i, selection] of replacementSelections(workspace).entries()) {
        const batch = `replacement-${stage}-${i+1}`;
        if (stage === 'graph') { prepareVideoGraph(workspace, batch, selection); await runVideoGraph(workspace, batch, executable); }
        else { prepareVideoDiscourse(workspace, batch, selection); await runVideoDiscourse(workspace, batch, executable); }
      }
      if (batches.length) await publish();
    }
    const publication = await publish();
    state = readVideoRevision(workspace); state.stage = 'ready'; state.failure = null; state.publication = publication.reference;
    saveVideoRevision(workspace, state); return state;
  } catch (error) {
    state = readVideoRevision(workspace); state.stage = 'failed'; state.failure = (error as Error).message;
    saveVideoRevision(workspace, state); throw error;
  }
}
export async function runVideoRevisionCli(argv: string[]) {
  const [mode, first, second, third, fourth] = argv;
  let result: unknown;
  if (mode === 'prepare' && first && second) result = prepareVideoRevision(first, second, {corrections: third || first});
  else if (mode === 'replace' && first && second && third) result = prepareVideoRevision(first, second, {replacement:third});
  else if (mode === 'status' && first) result = readVideoRevision(first);
  else if (mode === 'run' && first && second && third && fourth)
    result = await runVideoRevision(first, path.resolve(second), path.resolve(third), path.resolve(fourth));
  else throw new Error('Expected prepare OLD NEW [CORRECTIONS] | replace OLD NEW MP4 | run NEW CODEX_EXE PUBLISHER SERVICE_ROOT | status NEW');
  console.log(JSON.stringify(result, null, 2));
}
if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) await runVideoRevisionCli(process.argv.slice(2));
