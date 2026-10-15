import { existsSync, mkdirSync, readFileSync, renameSync, writeFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { execFile } from "node:child_process";
import { promisify } from "node:util";
import { randomUUID } from "node:crypto";
import { readVideoImport } from "./video-import";
import { videoTool } from './video-dependencies';
import { segment } from "./segment";
import { validateTranscriptMap, validateVideoSourceManifest, videoCoverageGaps } from "./video-contract";
import { publishAutomaticBuildArtifactSet } from "./automatic-build-publication";
import type { ReadOnlyBase } from "./generated/ReadOnlyBase";
import type { TranscriptMap } from "./generated/TranscriptMap";
import type { VideoSourceRef } from "./generated/VideoSourceRef";
import type { VideoSourceAsset } from "./generated/VideoSourceAsset";

const run = promisify(execFile);
export const VIDEO_BUILD_FILE = "video_build.json";
const profile = { engine: "faster-whisper", model: "small.en", language: "en", device: "cpu", compute_type: "int8",
  cpu_threads: 8, beam_size: 5, temperature: 0, word_timestamps: true, condition_on_previous_text: false,
  vad_filter: true, min_silence_duration_ms: 500 };
export type VideoChunk = { id: string; start_ms: number; end_ms: number; input_start_ms: number; input_end_ms: number;
  status: "pending" | "running" | "completed" | "failed"; attempts: number; failure: string | null };
export type VideoBuild = { version: "video_build.v1"; video: VideoSourceRef; book_id: string; transcript_id: string;
  profile: typeof profile; stage: "transcribing" | "assembling" | "ready" | "failed"; chunks: VideoChunk[];
  failure: string | null; receipt?: string };
export type RawWhisper = { engine: string; model: string; language: string; inference_ms?: number; load_ms?: number;
  segments: Array<{ text: string; start: number; end: number; words: Array<{ word: string; start: number; end: number; probability: number }> }> };
type Word = { text: string; start: number; end: number; probability: number; chunk: string };
function read<T>(file: string): T { return JSON.parse(readFileSync(file, "utf8")); }
function save(file: string, value: unknown) {
  mkdirSync(path.dirname(file), { recursive: true });
  writeFileSync(`${file}.tmp`, JSON.stringify(value, null, 2) + "\n"); renameSync(`${file}.tmp`, file);
}
export function planVideoChunks(duration: number, chunkMs = 300_000, contextMs = 2000): VideoChunk[] {
  if (!Number.isFinite(duration) || duration <= 0 || !Number.isFinite(chunkMs) || chunkMs <= 0
    || !Number.isFinite(contextMs) || contextMs < 0) throw new Error("Invalid audio chunk range");
  const chunks: VideoChunk[] = [];
  for (let start = 0; start < duration; start += chunkMs) chunks.push({ id: `chunk-${String(chunks.length).padStart(3,"0")}`,
    start_ms: start, end_ms: Math.min(start + chunkMs, duration), input_start_ms: Math.max(0,start-contextMs),
    input_end_ms: Math.min(start+chunkMs+contextMs,duration), status: "pending", attempts: 0, failure: null });
  return chunks;
}
export function readVideoBuild(workspace: string): VideoBuild | null {
  const file = path.join(workspace, VIDEO_BUILD_FILE);
  return existsSync(file) ? read<VideoBuild>(file) : null;
}
function rawWords(raw: RawWhisper, chunk: VideoChunk, source: VideoSourceAsset): Word[] {
  if (raw.engine !== profile.engine || raw.model !== profile.model || raw.language !== "en" || !Array.isArray(raw.segments))
    throw new Error(`Invalid recognition response: ${chunk.id}`);
  return raw.segments.flatMap(s => {
    if (s.text.trim() && !s.words?.length) throw new Error(`Word timing missing: ${chunk.id}`);
    return (s.words || []).map(w => {
      const start = chunk.input_start_ms + w.start * 1000, end = chunk.input_start_ms + w.end * 1000;
      if (!w.word.trim() || !Number.isFinite(start) || !Number.isFinite(end) || start < chunk.input_start_ms || end < start
        || end > chunk.input_end_ms + 1 || !Number.isFinite(w.probability)) throw new Error(`Invalid word timing: ${chunk.id}`);
      // Audio sample/container rounding at the final millisecond does not extend the media timeline.
      return { text: w.word, start: Math.min(start,source.duration_ms), end: Math.min(end,source.duration_ms), probability: w.probability, chunk: chunk.id };
    });
  });
}
const normalized = (w: Word) => w.text.toLowerCase().replace(/[^a-z0-9']/g, "");
/** Join independently recognized overlap by shared spoken words, rather than timestamp ownership alone. */
export function stitchVideoWords(parts: Word[][], chunks: VideoChunk[]) {
  let words = parts[0] || [];
  const boundaries: Array<{ at_ms: number; kind: string; left: string; right: string; anchor?: string }> = [];
  for (let p = 1; p < parts.length; p++) {
    const right = parts[p], at = chunks[p].start_ms;
    let best: { i: number; j: number; n: number; distance: number } | undefined;
    for (let i = 0; i < words.length; i++) {
      if (words[i].start < chunks[p].input_start_ms || words[i].start > chunks[p-1].input_end_ms) continue;
      for (let j = 0; j < right.length && right[j].start <= chunks[p-1].input_end_ms; j++) {
        let n = 0;
        while (i+n < words.length && j+n < right.length && normalized(words[i+n])
          && normalized(words[i+n]) === normalized(right[j+n]) && Math.abs(words[i+n].start-right[j+n].start) < 1500) n++;
        const distance = Math.abs((words[i].start+words[i+n-1]?.end)/2-at);
        if (n >= 2 && (!best || n > best.n || n === best.n && distance < best.distance)) best = { i,j,n,distance };
      }
    }
    if (best) {
      const anchor = words.slice(best.i,best.i+best.n).map(w=>w.text).join("").trim();
      boundaries.push({ at_ms: at, kind: "shared_words", left: chunks[p-1].id, right: chunks[p].id, anchor });
      words = [...words.slice(0,best.i+best.n), ...right.slice(best.j+best.n)];
    } else {
      // Silence may yield no shared words; both responses must agree there is a speech-free join.
      const leftOwned = words.filter(w => (w.start+w.end)/2 < at);
      const rightOwned = right.filter(w => (w.start+w.end)/2 >= at);
      if (words.some(w => w.start < at+500 && w.end > at-500) || right.some(w => w.start < at+500 && w.end > at-500))
        throw new Error(`Unresolved transcript boundary at ${at} ms; inspect saved responses before publication`);
      boundaries.push({ at_ms: at, kind: "speech_free_join", left: chunks[p-1].id, right: chunks[p].id });
      words = [...leftOwned,...rightOwned];
    }
  }
  return { words, boundaries };
}
export function assembleVideoTranscript(workspace: string, build: VideoBuild, source: VideoSourceAsset) {
  if (build.chunks.some(c=>c.status !== "completed")) throw new Error("Full transcript coverage is required");
  const coverage = { capability: "transcript" as const, pending: [], failed: [],
    accepted: build.chunks.map(c=>({video:source.video,start_ms:c.start_ms,end_ms:c.end_ms})) };
  if (videoCoverageGaps(coverage,source).length) throw new Error("Transcript processing plan has gaps");
  const raw = build.chunks.map(c => read<RawWhisper>(path.join(workspace, `transcript/raw/${c.id}.json`)));
  const { words, boundaries } = stitchVideoWords(raw.map((r,i)=>rawWords(r,build.chunks[i],source)), build.chunks);
  if (!words.length) throw new Error("No recognized English text; processed silence is retained without creating an empty Book");
  const sentences: Word[][] = [];
  for (const word of words) {
    let last = sentences.at(-1);
    if (!last || /[.!?]["']?\s*$/.test(last.at(-1)!.text) || last.map(w=>w.text).join("").length >= 600) sentences.push(last=[]);
    last.push(word);
  }
  const groups: Word[][] = [], timingGroups: Array<{start_ms:number;end_ms:number;reason:string}> = [];
  const startOf = (g: Word[]) => Math.min(...g.map(w=>w.start));
  const endOf = (g: Word[]) => Math.max(...g.map(w=>w.end));
  for (const sentence of sentences) {
    const previous=groups.at(-1);
    // Whisper returns zero-length one-word sentences and slightly different overlap estimates.
    // Keep their text and raw estimates; use the enclosing paragraph's real interval instead of
    // inventing word durations or silently clipping either recognition result.
    if (previous && (endOf(sentence)===startOf(sentence) || endOf(previous)===startOf(previous)
      || startOf(sentence)<endOf(previous))) {
      timingGroups.push({start_ms:startOf(sentence),end_ms:endOf(sentence),reason:"merged_adjacent_sentence_estimates"});
      previous.push(...sentence);
    } else groups.push([...sentence]);
  }
  let text = "";
  const blocks = groups.map(group=> { const start=text.length; const value=group.map(w=>w.text).join("").trim();
    text += value+"\n\n"; return { kind: "leaf" as const, text: value, span: { start, end: text.length } }; });
  const base: ReadOnlyBase = { book_id: build.book_id, lid_nodes: segment(blocks), graph_nodes: [], graph_edges: [] };
  const manifest = validateVideoSourceManifest({ version: "video_source_manifest.v1",
    material: { book_id: build.book_id, video: source.video, transcript: { transcript_id: build.transcript_id, transcript_revision: "1", video: source.video } },
    source, canonical_text_path: "source.txt", transcript_map_path: "transcript_map.json", previous_book_id: null,
    coverage: ["transcript","translation","visual_index","graph","discourse","audio_review"].map(capability => ({ capability,
      accepted: capability === "transcript" ? coverage.accepted : [], failed: [],
      pending: capability === "transcript" ? [] : [{ video: source.video,start_ms:0,end_ms:source.duration_ms }] })) });
  const map: TranscriptMap = { transcript: manifest.material.transcript, raw_result_path: "transcript/raw_results.json",
    machine_transcription: { engine: profile.engine,model:profile.model,language:"en" }, segments: groups.map((group,i) => ({
      segment_id: `segment-${i+1}`, text: blocks[i].text,
      location: { video: source.video, start_ms: startOf(group), end_ms: endOf(group) },
      text_range: { lid: base.lid_nodes[i].lid, range: { start:0,end:blocks[i].text.length } }, words: [] })) };
  // Sentence timing is the public precision; raw word estimates and probabilities remain available for review.
  validateTranscriptMap(map,manifest,base,text);
  const json = (v: unknown) => JSON.stringify(v,null,2)+"\n";
  const receipt = publishAutomaticBuildArtifactSet({ workspace_dir: workspace, stage: "video_transcript", artifacts: {
    "source.txt": text, "base.json":json(base), "video_source_manifest.json":json(manifest), "transcript_map.json":json(map),
    "transcript/raw_results.json":json({ profile, chunks: build.chunks.map((chunk,i)=>({ chunk, response:raw[i] })) }),
    "transcript/joins.json":json({ boundaries, timing_groups:timingGroups, uncertainties: words.filter(w=>w.probability < .5) }),
  } });
  return receipt.transaction_id;
}
export async function buildVideoTranscript(workspace: string, options: { python?: string; modelDir?: string;
  recognize?: (audio: string, output: string, chunk: VideoChunk) => Promise<void>; chunkMs?: number; contextMs?: number } = {}) {
  workspace=path.resolve(workspace);
  const imported=readVideoImport(workspace), source=imported.source;
  if (imported.stage !== "awaiting_transcript" || !source) throw new Error("Complete video import first");
  const file=path.join(workspace,VIDEO_BUILD_FILE);
  let build=readVideoBuild(workspace);
  if (!build) {
    if (existsSync(path.join(workspace,"base.json"))) throw new Error("Workspace already contains a Book");
    build={ version:"video_build.v1",video:source.video,book_id:randomUUID(),transcript_id:randomUUID(),profile,
      stage:"transcribing",chunks:planVideoChunks(source.duration_ms,options.chunkMs,options.contextMs),failure:null };
  }
  if (JSON.stringify(build.video)!==JSON.stringify(source.video) || JSON.stringify(build.profile)!==JSON.stringify(profile))
    throw new Error("Video build input/configuration changed; use a new workspace");
  if (build.stage === "ready") return build;
  build.stage="transcribing"; build.failure=null; save(file,build);
  const dir=path.join(workspace,".build/video"); mkdirSync(dir,{recursive:true}); mkdirSync(path.join(workspace,"transcript/raw"),{recursive:true});
  for (const chunk of build.chunks) {
    const rawPath=path.join(workspace,`transcript/raw/${chunk.id}.json`);
    if (chunk.status === "completed") continue;
    try {
      // A completed service response survives an interruption before its Engine receipt was saved.
      if (!existsSync(rawPath)) {
        chunk.status="running"; chunk.failure=null; chunk.attempts++; save(file,build);
        const audio=path.join(dir,`${chunk.id}.wav`);
        await run(videoTool('ffmpeg'),["-v","error","-y","-i",path.join(workspace,source.imported_path),
          "-ss",String(chunk.input_start_ms/1000),"-t",String((chunk.input_end_ms-chunk.input_start_ms)/1000),
          "-map","0:a:0","-ac","1","-ar","16000","-c:a","pcm_s16le",audio],{windowsHide:true});
        if (options.recognize) await options.recognize(audio,rawPath,chunk);
        else {
          const modelDir = options.modelDir || process.env.VIDEO_WHISPER_MODEL_DIR;
          if (!modelDir) throw new Error("Provide --model-dir or VIDEO_WHISPER_MODEL_DIR for the local small.en model; run video-doctor, then repeat video-transcribe to resume");
          const request=path.join(dir,`${chunk.id}.request.json`);
          save(request,{audio,model_dir:path.resolve(modelDir)});
          let script = fileURLToPath(new URL("./video-whisper.py",import.meta.url));
          if (process.env.UNDERSTAND_BOOK_SIDECAR_SELF) {
            script = path.join(dir, 'whisper.py');
            const asset = await run(process.env.UNDERSTAND_BOOK_SIDECAR_SELF, ['video-asset', 'whisper'], { windowsHide: true });
            writeFileSync(script, asset.stdout);
          }
          await run(options.python || process.env.VIDEO_WHISPER_PYTHON || "python",[script,request,rawPath],{windowsHide:true});
        }
      }
      rawWords(read(rawPath),chunk,source); chunk.status="completed"; chunk.failure=null;
    } catch(error) { chunk.status="failed"; chunk.failure=error instanceof Error?error.message:String(error); }
    save(file,build);
  }
  try {
    if (build.chunks.some(c=>c.status !== "completed")) throw new Error("Recognition failed for one or more chunks; resume to retry missing work");
    build.stage="assembling"; save(file,build);
    build.receipt=assembleVideoTranscript(workspace,build,source); build.stage="ready";
  } catch(error) { build.stage="failed";build.failure=error instanceof Error?error.message:String(error); }
  save(file,build); return build;
}
