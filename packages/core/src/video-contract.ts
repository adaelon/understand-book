// Runtime validation mirrors the authoritative Rust media contracts in base-schema/src/video.rs.
import { z } from "zod";
import type { ReadOnlyBase } from "./generated/ReadOnlyBase";
import type { VideoSourceManifest } from "./generated/VideoSourceManifest";
import type { VideoSourceAsset } from "./generated/VideoSourceAsset";
import type { VideoCoverage } from "./generated/VideoCoverage";
import type { MediaLocation } from "./generated/MediaLocation";
import type { FrameLocation } from "./generated/FrameLocation";
import type { FrameRegion } from "./generated/FrameRegion";
import type { TranscriptMap } from "./generated/TranscriptMap";
import type { TranscriptTranslation } from "./generated/TranscriptTranslation";
import type { TranscriptTextRange } from "./generated/TranscriptTextRange";
import type { VideoQuestionContext } from "./generated/VideoQuestionContext";
import type { EvidenceTarget } from "./generated/EvidenceTarget";
import type { MediaObservation } from "./generated/MediaObservation";
import type { PublishedBookRef } from "./generated/PublishedBookRef";

const id = z.string().refine(v => v.trim().length > 0);
const time = z.number().finite().nonnegative();
const uint = z.number().int().nonnegative().max(0xffffffff);
const relativePath = id.refine(v => !/^[/\\]/.test(v) && !v.includes(":") && !v.split(/[/\\]/).includes(".."));
const span = z.object({ start: uint, end: uint });
export const PublishedBookRefZ = z.object({ book_id: id, publication_id: id });
export const VideoSourceRefZ = z.object({ media_id: id, media_revision: id });
export const TranscriptRefZ = z.object({ transcript_id: id, transcript_revision: id, video: VideoSourceRefZ });
export const VideoMaterialZ = z.object({ book_id: id, video: VideoSourceRefZ, transcript: TranscriptRefZ });
export const MediaLocationZ = z.object({ video: VideoSourceRefZ, start_ms: time, end_ms: time });
export const FrameLocationZ = z.object({ video: VideoSourceRefZ, actual_pts_ms: time, width: uint.positive(), height: uint.positive() });
export const FrameReadZ = z.object({ requested_ms: time, frame: FrameLocationZ });
export const FrameRegionZ = z.object({ frame: FrameLocationZ, rect_normalized: z.tuple([time, time, time, time]) });
export const VideoSourceAssetZ = z.object({ video: VideoSourceRefZ, imported_path: relativePath, original_path: id, duration_ms: time.positive(), width: uint.positive(), height: uint.positive() });
export const TranscriptTextRangeZ = z.object({ lid: id, range: span });
export const TranscriptSegmentZ = z.object({ segment_id: id, text: id, location: MediaLocationZ, text_range: TranscriptTextRangeZ,
  words: z.array(z.object({ range: span, start_ms: time, end_ms: time })) });
export const TranscriptMapZ: z.ZodType<TranscriptMap> = z.object({
  transcript: TranscriptRefZ, raw_result_path: relativePath,
  machine_transcription: z.object({ engine: id, model: id, language: id }),
  segments: z.array(TranscriptSegmentZ),
});
export const TranscriptTranslationZ: z.ZodType<TranscriptTranslation> = z.object({ transcript: TranscriptRefZ, segment_id: id, zh_text: id });
export const VideoCoverageZ = z.object({ capability: z.enum(["transcript", "translation", "visual_index", "graph", "discourse", "audio_review"]),
  accepted: z.array(MediaLocationZ), pending: z.array(MediaLocationZ), failed: z.array(MediaLocationZ) });
export const VideoSourceManifestZ: z.ZodType<VideoSourceManifest> = z.object({
  version: z.literal("video_source_manifest.v1"), material: VideoMaterialZ, source: VideoSourceAssetZ,
  canonical_text_path: z.literal("source.txt"), transcript_map_path: relativePath,
  coverage: z.array(VideoCoverageZ), previous_book_id: id.nullable(),
});
export const VideoQuestionContextZ: z.ZodType<VideoQuestionContext> = z.object({ draft_id: id, publication: PublishedBookRefZ,
  material: VideoMaterialZ, frame: FrameLocationZ, region: FrameRegionZ.nullable(), range: MediaLocationZ.nullable() });
export const EvidenceTargetZ: z.ZodType<EvidenceTarget> = z.lazy(() => z.discriminatedUnion("kind", [
  z.object({ kind: z.literal("text"), publication: PublishedBookRefZ, transcript: TranscriptRefZ.nullable(), text_range: TranscriptTextRangeZ }),
  z.object({ kind: z.literal("visual"), publication: PublishedBookRefZ, frames: z.array(FrameLocationZ), regions: z.array(FrameRegionZ) }),
  z.object({ kind: z.literal("audio"), publication: PublishedBookRefZ, location: MediaLocationZ, review_result_ref: id }),
  z.object({ kind: z.literal("joint"), members: z.array(z.object({ target: EvidenceTargetZ, support_role: id })).min(2) }),
]));
export const MediaObservationZ: z.ZodType<MediaObservation> = z.discriminatedUnion("kind", [
  z.object({ kind: z.literal("visual"), observation_id: id, run_id: id, publication: PublishedBookRefZ, model_request_id: id,
    frames: z.array(FrameReadZ), regions: z.array(FrameRegionZ) }),
  z.object({ kind: z.literal("audio"), observation_id: id, run_id: id, publication: PublishedBookRefZ, tool_call_id: id,
    location: MediaLocationZ, review_result_ref: id }),
]);

function requireValid(ok: boolean, field: string): asserts ok {
  if (!ok) throw new Error(`invalid video contract: ${field}`);
}
function sameVideo(a: VideoSourceAsset["video"], b: VideoSourceAsset["video"]): boolean {
  return a.media_id === b.media_id && a.media_revision === b.media_revision;
}
function sameTranscript(a: TranscriptMap["transcript"], b: TranscriptMap["transcript"]): boolean {
  return a.transcript_id === b.transcript_id && a.transcript_revision === b.transcript_revision && sameVideo(a.video, b.video);
}
function sameFrame(a: FrameLocation, b: FrameLocation): boolean {
  return sameVideo(a.video, b.video) && a.actual_pts_ms === b.actual_pts_ms && a.width === b.width && a.height === b.height;
}
function publication(p: PublishedBookRef, manifest: VideoSourceManifest): void {
  PublishedBookRefZ.parse(p);
  requireValid(p.book_id === manifest.material.book_id, "publication");
}
export function validateMediaLocation(value: MediaLocation, source: VideoSourceAsset): void {
  MediaLocationZ.parse(value);
  requireValid(sameVideo(value.video, source.video) && value.start_ms < value.end_ms && value.end_ms <= source.duration_ms, "location");
}
export function validateFrameLocation(value: FrameLocation, source: VideoSourceAsset): void {
  FrameLocationZ.parse(value);
  requireValid(sameVideo(value.video, source.video) && value.actual_pts_ms < source.duration_ms
    && value.width === source.width && value.height === source.height, "frame");
}
export function validateFrameRegion(value: FrameRegion, source: VideoSourceAsset): void {
  FrameRegionZ.parse(value);
  validateFrameLocation(value.frame, source);
  const [x, y, w, h] = value.rect_normalized;
  requireValid(w > 0 && h > 0 && x + w <= 1 && y + h <= 1, "region");
}
export function validateVideoCoverage(value: VideoCoverage, source: VideoSourceAsset): void {
  VideoCoverageZ.parse(value);
  const intervals = [...value.accepted, ...value.pending, ...value.failed].sort((a,b) => a.start_ms-b.start_ms);
  intervals.forEach(p => validateMediaLocation(p, source));
  requireValid(intervals.every((p,i) => i === 0 || intervals[i-1].end_ms <= p.start_ms), "coverage overlap");
}
export function videoCoverageGaps(value: VideoCoverage, source: VideoSourceAsset): MediaLocation[] {
  validateVideoCoverage(value, source);
  let cursor = 0;
  const gaps: MediaLocation[] = [];
  for (const p of [...value.accepted].sort((a,b) => a.start_ms-b.start_ms)) {
    if (cursor < p.start_ms) gaps.push({ video: source.video, start_ms: cursor, end_ms: p.start_ms });
    cursor = p.end_ms;
  }
  if (cursor < source.duration_ms) gaps.push({ video: source.video, start_ms: cursor, end_ms: source.duration_ms });
  return gaps;
}
export function validateVideoSourceManifest(input: unknown): VideoSourceManifest {
  const m = VideoSourceManifestZ.parse(input);
  requireValid(sameVideo(m.material.video, m.source.video) && sameVideo(m.material.transcript.video, m.source.video), "material");
  requireValid(m.previous_book_id !== m.material.book_id, "previous_book_id");
  requireValid(new Set(m.coverage.map(c => c.capability)).size === m.coverage.length, "duplicate capability");
  m.coverage.forEach(c => validateVideoCoverage(c, m.source));
  return m;
}
export function validateVideoMaterialSuccessor(input: unknown, previous: VideoSourceManifest): VideoSourceManifest {
  const next = validateVideoSourceManifest(input);
  validateVideoSourceManifest(previous);
  const sameSource = sameVideo(next.material.video, previous.material.video)
    && sameTranscript(next.material.transcript, previous.material.transcript);
  requireValid(sameSource
    ? next.material.book_id === previous.material.book_id && next.previous_book_id === previous.previous_book_id
    : next.material.book_id !== previous.material.book_id && next.previous_book_id === previous.material.book_id,
  "material successor");
  return next;
}
function textAt(r: TranscriptTextRange, base: ReadOnlyBase, source: string): { text: string; start: number; end: number } {
  const node = base.lid_nodes.find(n => n.lid === r.lid);
  requireValid(!!node && node.children.length === 0 && node.span.start >= 0 && node.span.start <= node.span.end && node.span.end <= source.length, "LID span");
  requireValid(r.range.start < r.range.end && r.range.end <= node.span.end-node.span.start, "text range");
  const start = node.span.start+r.range.start, end = node.span.start+r.range.end;
  return { text: source.slice(start,end), start, end };
}
export function validateTranscriptMap(input: unknown, manifest: VideoSourceManifest, base: ReadOnlyBase, source: string): TranscriptMap {
  validateVideoSourceManifest(manifest);
  const map = TranscriptMapZ.parse(input);
  requireValid(sameTranscript(map.transcript, manifest.material.transcript) && base.book_id === manifest.material.book_id, "transcript binding");
  const seen = new Set<string>();
  let previousEnd = 0, previousTime = 0;
  for (const s of map.segments) {
    requireValid(!seen.has(s.segment_id), "segment identity"); seen.add(s.segment_id);
    validateMediaLocation(s.location, manifest.source);
    const mapped = textAt(s.text_range, base, source);
    requireValid(mapped.text === s.text, "mapped text");
    requireValid(mapped.start >= previousEnd && s.location.start_ms >= previousTime, "segment order");
    previousEnd = mapped.end; previousTime = s.location.end_ms;
    let wordEnd = 0, timeEnd = s.location.start_ms;
    for (const w of s.words) {
      requireValid(w.range.start >= wordEnd && w.range.start < w.range.end && w.range.end <= s.text.length
        && w.start_ms <= w.end_ms && w.start_ms >= timeEnd && w.end_ms <= s.location.end_ms, "word mapping");
      wordEnd = w.range.end; timeEnd = w.end_ms;
    }
  }
  return map;
}
export function validateTranscriptTranslation(input: unknown, map: TranscriptMap): TranscriptTranslation {
  const t = TranscriptTranslationZ.parse(input);
  requireValid(sameTranscript(t.transcript, map.transcript) && map.segments.some(s => s.segment_id === t.segment_id), "translation");
  return t;
}
export function validateVideoQuestionContext(input: unknown, manifest: VideoSourceManifest): VideoQuestionContext {
  validateVideoSourceManifest(manifest);
  const q = VideoQuestionContextZ.parse(input);
  publication(q.publication, manifest);
  requireValid(q.material.book_id === manifest.material.book_id && sameVideo(q.material.video, manifest.material.video)
    && sameTranscript(q.material.transcript, manifest.material.transcript), "question material");
  validateFrameLocation(q.frame, manifest.source);
  if (q.region) { validateFrameRegion(q.region, manifest.source); requireValid(sameFrame(q.frame, q.region.frame), "question region frame"); }
  if (q.range) validateMediaLocation(q.range, manifest.source);
  return q;
}
export function validateEvidenceTarget(input: unknown, manifest: VideoSourceManifest, base: ReadOnlyBase, source: string): EvidenceTarget {
  validateVideoSourceManifest(manifest);
  requireValid(base.book_id === manifest.material.book_id, "evidence base");
  const e = EvidenceTargetZ.parse(input);
  if (e.kind === "joint") {
    let first: string | undefined;
    for (const m of e.members) {
      requireValid(m.target.kind !== "joint", "nested joint");
      const p = m.target.publication.publication_id;
      requireValid(first === undefined || first === p, "joint publication"); first = p;
      validateEvidenceTarget(m.target, manifest, base, source);
    }
  } else {
    publication(e.publication, manifest);
    if (e.kind === "text") {
      requireValid(e.transcript !== null && sameTranscript(e.transcript, manifest.material.transcript), "evidence transcript");
      textAt(e.text_range, base, source);
    } else if (e.kind === "visual") {
      requireValid(e.frames.length+e.regions.length > 0, "empty visual evidence");
      e.frames.forEach(f => validateFrameLocation(f, manifest.source));
      e.regions.forEach(r => validateFrameRegion(r, manifest.source));
    } else validateMediaLocation(e.location, manifest.source);
  }
  return e;
}
export function validateMediaObservation(input: unknown, manifest: VideoSourceManifest): MediaObservation {
  validateVideoSourceManifest(manifest);
  const o = MediaObservationZ.parse(input);
  publication(o.publication, manifest);
  if (o.kind === "visual") {
    requireValid(o.frames.length+o.regions.length > 0, "visual receipt");
    for (const f of o.frames) {
      requireValid(f.requested_ms < manifest.source.duration_ms, "requested time");
      validateFrameLocation(f.frame, manifest.source);
    }
    o.regions.forEach(r => validateFrameRegion(r, manifest.source));
  } else validateMediaLocation(o.location, manifest.source);
  return o;
}
