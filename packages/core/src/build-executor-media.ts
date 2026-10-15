import { randomUUID } from "node:crypto";
import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import path from "node:path";
import type { AutomaticBuildTarget } from "./build-orchestrator";
import type { WorkUnitDescriptor } from "./stage-work-unit";
import type { VideoSourceManifest } from "./generated/VideoSourceManifest";
import type { TranscriptSegment } from "./generated/TranscriptSegment";
import type { FrameLocation } from "./generated/FrameLocation";
import type { FrameRegion } from "./generated/FrameRegion";
import { validateFrameLocation, validateFrameRegion, validateVideoSourceManifest, TranscriptMapZ } from "./video-contract";
import type { TranscriptMap } from "./generated/TranscriptMap";
import { canonicalAutomaticBuildJson } from "./automatic-build-protocol";
import { estimateTokens } from "./window";

/** A tested carrier tier, not a claim about a provider's hard limit. */
export const CODEX_EXECUTOR_MEDIA_CAPABILITY_V1 = Object.freeze({
  version: "executor_media_input.v1" as const,
  native_images: true,
  mime_type: "image/png" as const,
  max_images: 8,
  max_png_bytes: 4_194_304,
  max_pixels: 16_588_800,
  max_mcp_result_bytes: 8_388_608,
  max_jsonrpc_line_bytes: 8_389_632,
  image_token_reserve: { base: 256, tile_size: 512, per_tile: 256 },
});

export const DSH_EXECUTOR_MEDIA_CAPABILITY_V1 = Object.freeze({
  version: "executor_media_input.v1" as const,
  native_images: false,
  reason: "native_image_carrier_not_verified" as const,
});

export interface ExecutorMediaImageV1 {
  image_id: string;
  requested_ms: number;
  frame: FrameLocation;
  region: FrameRegion | null;
  width: number;
  height: number;
  png_bytes: number;
  data: string;
}

export interface ExecutorMediaInputV1 {
  version: "executor_media_input.v1";
  media_input_ref: string;
  work_unit_id: string;
  stage: WorkUnitDescriptor["stage"] | "video_visual" | "video_graph" | "video_correction";
  input_hash: string;
  material: VideoSourceManifest["material"];
  transcript: TranscriptSegment[];
  images: ExecutorMediaImageV1[];
}

export type ExecutorMediaManifestV1 = Omit<ExecutorMediaInputV1, "images" | "input_hash" | "transcript"> & {
  images: Omit<ExecutorMediaImageV1, "data">[];
  reserved_image_tokens: number;
};

function mediaFile(target: AutomaticBuildTarget, descriptor: Pick<WorkUnitDescriptor, "stage" | "work_unit_id" | "input_hash">): string {
  return path.join(target.workspace_dir, ".build", "executor-media", descriptor.stage,
    encodeURIComponent(descriptor.work_unit_id), `${descriptor.input_hash}.json`);
}

export function executorMediaManifest(media: ExecutorMediaInputV1): ExecutorMediaManifestV1 {
  const { input_hash: _hash, transcript: _transcript, images, ...identity } = media;
  return { ...identity, images: images.map(({ data: _data, ...image }) => image),
    reserved_image_tokens: images.reduce((n, image) => n + reservedImageTokens(image), 0) };
}

function reservedImageTokens(image: Pick<ExecutorMediaImageV1, "width" | "height">): number {
  const reserve = CODEX_EXECUTOR_MEDIA_CAPABILITY_V1.image_token_reserve;
  return reserve.base + Math.ceil(image.width / reserve.tile_size) * Math.ceil(image.height / reserve.tile_size) * reserve.per_tile;
}

/** Engine-only preparation. PNGs come from the media decoder, never model arguments.
 * Store complete bytes once so relocation/reconnect does not reread mutable frame files.
 */
export function freezeBuildExecutorMediaInput(input: {
  target: AutomaticBuildTarget;
  descriptor: WorkUnitDescriptor;
  source: VideoSourceManifest;
  transcript: TranscriptMap;
  images: Array<{ requested_ms: number; frame: FrameLocation; region?: FrameRegion | null; png: Buffer }>;
}): ExecutorMediaManifestV1 {
  const source = validateVideoSourceManifest(input.source);
  const transcript = TranscriptMapZ.parse(input.transcript);
  if (source.material.book_id !== input.target.book_id || input.descriptor.target.book_id !== input.target.book_id
    || input.descriptor.target.input_fingerprint !== input.target.target_ref.input_fingerprint
    || canonicalAutomaticBuildJson(transcript.transcript) !== canonicalAutomaticBuildJson(source.material.transcript)) {
    throw new Error("executor_media_source_mismatch");
  }
  const cap = CODEX_EXECUTOR_MEDIA_CAPABILITY_V1;
  if (!input.images.length || input.images.length > cap.max_images) throw new Error("executor_media_image_limit");
  const images = input.images.map((image, index): ExecutorMediaImageV1 => {
    validateFrameLocation(image.frame, source.source);
    if (image.region) {
      validateFrameRegion(image.region, source.source);
      if (canonicalAutomaticBuildJson(image.region.frame) !== canonicalAutomaticBuildJson(image.frame)) throw new Error("executor_media_region_mismatch");
    }
    if (!Number.isFinite(image.requested_ms) || image.requested_ms < 0 || image.requested_ms >= source.source.duration_ms) throw new Error("executor_media_time_invalid");
    const png = image.png;
    if (png.length < 24 || !png.subarray(0, 8).equals(Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]))) throw new Error("executor_media_png_required");
    const width = png.readUInt32BE(16), height = png.readUInt32BE(20);
    const rect = image.region?.rect_normalized;
    const expectedWidth = rect ? Math.round(rect[2] * image.frame.width) : image.frame.width;
    const expectedHeight = rect ? Math.round(rect[3] * image.frame.height) : image.frame.height;
    if (width !== expectedWidth || height !== expectedHeight || !width || !height) throw new Error("executor_media_dimensions_mismatch");
    return { image_id: `image_${index + 1}`, requested_ms: image.requested_ms, frame: image.frame,
      region: image.region ?? null, width, height, png_bytes: png.length, data: png.toString("base64") };
  });
  if (images.reduce((n, i) => n + i.png_bytes, 0) > cap.max_png_bytes
    || images.reduce((n, i) => n + i.width * i.height, 0) > cap.max_pixels) throw new Error("executor_media_budget_exceeded");
  const media: ExecutorMediaInputV1 = {
    version: "executor_media_input.v1", media_input_ref: randomUUID(),
    work_unit_id: input.descriptor.work_unit_id, stage: input.descriptor.stage, input_hash: input.descriptor.input_hash,
    material: source.material,
    transcript: transcript.segments.filter(segment => input.descriptor.evidence_lids.includes(segment.text_range.lid)), images,
  };
  if (!media.transcript.length) throw new Error("executor_media_transcript_outside_work_unit");
  assertExecutorMediaBudget(media, input.descriptor, "codex");
  const file = mediaFile(input.target, input.descriptor);
  if (existsSync(file)) {
    const prior = readBuildExecutorMediaInput(input.target, input.descriptor)!;
    if (canonicalAutomaticBuildJson({ ...prior, media_input_ref: null }) !== canonicalAutomaticBuildJson({ ...media, media_input_ref: null })) {
      throw new Error("executor_media_input_already_frozen");
    }
    return executorMediaManifest(prior);
  }
  mkdirSync(path.dirname(file), { recursive: true });
  writeFileSync(file, canonicalAutomaticBuildJson(media), { flag: "wx" });
  return executorMediaManifest(media);
}

export function readBuildExecutorMediaInput(target: AutomaticBuildTarget, descriptor: WorkUnitDescriptor): ExecutorMediaInputV1 | undefined {
  const file = mediaFile(target, descriptor);
  if (!existsSync(file)) return undefined;
  const media = JSON.parse(readFileSync(file, "utf8")) as ExecutorMediaInputV1;
  if (media.version !== "executor_media_input.v1" || media.work_unit_id !== descriptor.work_unit_id
    || media.stage !== descriptor.stage || media.input_hash !== descriptor.input_hash || media.material.book_id !== target.book_id) {
    throw new Error("executor_media_input_binding_mismatch");
  }
  return media;
}

export function assertExecutorMediaBudget(media: ExecutorMediaInputV1, descriptor: WorkUnitDescriptor, harness: "codex" | "deepseek_harness"): void {
  if (harness !== "codex") throw new Error("executor_media_capability_unavailable");
  const proof = descriptor.version === "automatic_build_work_unit.v4" ? descriptor.execution_budget_proof
    : descriptor.version === "automatic_build_work_unit.v3" ? descriptor.input_budget_proof : undefined;
  // Older descriptors have no frozen context budget; do not silently attach images to them.
  if (!proof) throw new Error("executor_media_requires_input_budget");
  const manifest = executorMediaManifest(media);
  // open declares the images; the final input batch repeats their labels beside the pixels.
  const metadataTokens = estimateTokens(canonicalAutomaticBuildJson(manifest))
    + estimateTokens(canonicalAutomaticBuildJson({ manifest, transcript: media.transcript }));
  if (proof.estimated_rendered_tokens + metadataTokens + media.images.reduce((n, image) => n + reservedImageTokens(image), 0)
    > proof.effective_body_limit_tokens) throw new Error("executor_media_context_limit");
}
