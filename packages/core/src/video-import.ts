import { createReadStream, createWriteStream, existsSync, mkdirSync, readFileSync, renameSync, statSync, writeFileSync } from "node:fs";
import { pipeline } from "node:stream/promises";
import { Transform } from "node:stream";
import { execFile } from "node:child_process";
import { promisify } from "node:util";
import { randomUUID } from "node:crypto";
import path from "node:path";
import { videoTool } from './video-dependencies';
import { z } from "zod";
import { VideoSourceAssetZ, VideoSourceRefZ } from "./video-contract";
import type { VideoImport } from "./generated/VideoImport";

export const VIDEO_IMPORT_FILE = "video_import.json";
const importedPath = "media/original.mp4";
const run = promisify(execFile);
const schema = z.object({
  version: z.literal("video_import.v1"), video: VideoSourceRefZ, title: z.string().min(1), original_path: z.string().min(1),
  stage: z.enum(["copying", "probing", "awaiting_transcript", "failed"]),
  copied_bytes: z.number().finite().nonnegative(), total_bytes: z.number().finite().positive(),
  source: VideoSourceAssetZ.nullable(), failure: z.string().nullable(),
});
export function readVideoImport(workspace: string): VideoImport {
  const state = schema.parse(JSON.parse(readFileSync(path.join(workspace, VIDEO_IMPORT_FILE), "utf8")));
  if (state.copied_bytes > state.total_bytes || (state.source && (state.source.imported_path !== importedPath
    || state.source.video.media_id !== state.video.media_id || state.source.video.media_revision !== state.video.media_revision))
    || (state.stage === "awaiting_transcript" && (!state.source || state.copied_bytes !== state.total_bytes))
    || (state.stage === "failed" && !state.failure?.trim())) throw new Error("Invalid video import state");
  return state;
}
function save(workspace: string, state: VideoImport): void {
  const file = path.join(workspace, VIDEO_IMPORT_FILE);
  writeFileSync(`${file}.tmp`, JSON.stringify(state, null, 2) + "\n");
  renameSync(`${file}.tmp`, file);
}

/** The workspace owns a byte-for-byte snapshot. Resume never needs the external path after copying. */
export async function importVideo(input: string | undefined, workspace: string): Promise<VideoImport> {
  workspace = path.resolve(workspace);
  const file = path.join(workspace, VIDEO_IMPORT_FILE);
  let state: VideoImport;
  if (existsSync(file)) {
    state = readVideoImport(workspace);
    if (input && path.resolve(input) !== state.original_path) throw new Error("This workspace already owns another video; use a new workspace");
  } else {
    if (!input) throw new Error("A local MP4 path is required for a new import");
    input = path.resolve(input);
    const info = statSync(input);
    if (!info.isFile() || info.size === 0) throw new Error("Video input must be a nonempty file");
    if (existsSync(path.join(workspace, "base.json"))) throw new Error("Use a new video workspace, not an existing Book");
    mkdirSync(workspace, { recursive: true });
    state = { version: "video_import.v1", video: { media_id: randomUUID(), media_revision: "1" },
      title: path.basename(input, path.extname(input)), original_path: input, stage: "copying",
      copied_bytes: 0, total_bytes: info.size, source: null, failure: null };
    save(workspace, state);
  }
  const snapshot = path.join(workspace, importedPath);
  try {
    if (state.stage === "awaiting_transcript") {
      if (statSync(snapshot).size !== state.total_bytes) throw new Error("Imported video is missing or incomplete");
      return state;
    }
    // Only the completed rename establishes a usable snapshot; .part is restarted after interruption.
    if (!existsSync(snapshot)) {
      state.stage = "copying"; state.failure = null; state.copied_bytes = 0;
      save(workspace, state);
      mkdirSync(path.dirname(snapshot), { recursive: true });
      let lastSaved = Date.now();
      const progress = new Transform({ transform(chunk, _encoding, callback) {
        state.copied_bytes += chunk.length;
        if (Date.now() - lastSaved >= 250) { save(workspace, state); lastSaved = Date.now(); }
        callback(null, chunk);
      } });
      await pipeline(createReadStream(state.original_path), progress, createWriteStream(`${snapshot}.part`));
      if (state.copied_bytes !== state.total_bytes) throw new Error("Input size changed during import; retry with a new workspace");
      renameSync(`${snapshot}.part`, snapshot);
    }
    if (statSync(snapshot).size !== state.total_bytes) throw new Error("Imported video is incomplete");
    state.copied_bytes = state.total_bytes; state.stage = "probing"; state.failure = null;
    save(workspace, state);
    const result = await run(videoTool('ffprobe'), ["-v", "error", "-show_format", "-show_streams", "-of", "json", snapshot], { windowsHide: true });
    const info = JSON.parse(result.stdout);
    const video = info.streams.find((s: any) => s.codec_type === "video" && !s.disposition?.attached_pic);
    const audio = info.streams.find((s: any) => s.codec_type === "audio");
    if (!info.format.format_name.split(",").includes("mp4") || video?.codec_name !== "h264" || audio?.codec_name !== "aac")
      throw new Error("Currently supported: MP4 with H.264 video and AAC audio");
    // Probe and decode the imported bytes, never the provenance path. Ignore attached cover pictures.
    await run(videoTool('ffmpeg'), ["-v", "error", "-xerror", "-i", snapshot, "-map", `0:${video.index}`, "-frames:v", "1", "-f", "null", "-"], { windowsHide: true });
    state.source = VideoSourceAssetZ.parse({ video: state.video, imported_path: importedPath,
      original_path: state.original_path, duration_ms: Number(info.format.duration) * 1000, width: video.width, height: video.height });
    writeFileSync(path.join(workspace, "media/probe.json"), result.stdout);
    state.stage = "awaiting_transcript"; state.failure = null;
    save(workspace, state);
    return state;
  } catch (error) {
    state.stage = "failed"; state.failure = error instanceof Error ? error.message : String(error);
    save(workspace, state);
    throw error;
  }
}
