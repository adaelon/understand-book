import { afterEach, describe, expect, it } from "vitest";
import { execFileSync } from "node:child_process";
import { existsSync, mkdtempSync, readFileSync, renameSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { importVideo, readVideoImport } from "../src/video-import";
const roots: string[] = [];
afterEach(() => { for (const root of roots.splice(0)) rmSync(root, { recursive: true, force: true }); });
function fixture() {
  const root = mkdtempSync(path.join(tmpdir(), "vi2-")); roots.push(root);
  const source = path.join(root, "教学 sample.mp4"), workspace = path.join(root, "workspace");
  execFileSync(process.env.FFMPEG || "ffmpeg", ["-v", "error", "-f", "lavfi", "-i", "color=s=160x90:r=30:d=1",
    "-f", "lavfi", "-i", "sine=frequency=440:duration=1", "-c:v", "libx264", "-pix_fmt", "yuv420p", "-c:a", "aac", "-shortest", source], { windowsHide: true });
  return { root, source, workspace };
}
describe("VI2 imported snapshot", () => {
  it("imports actual H264/AAC, survives lost provenance and a moved workspace with the same identity", async () => {
    const { root, source, workspace } = fixture();
    const original = readFileSync(source);
    const state = await importVideo(source, workspace);
    expect(state.stage).toBe("awaiting_transcript");
    expect(state.source).toMatchObject({ imported_path: "media/original.mp4", width: 160, height: 90 });
    expect(readFileSync(path.join(workspace, state.source!.imported_path))).toEqual(original);
    expect(existsSync(path.join(workspace, "base.json"))).toBe(false);
    renameSync(source, path.join(root, "moved-away.mp4"));
    const moved = path.join(root, "moved-workspace"); renameSync(workspace, moved);
    expect(await importVideo(undefined, moved)).toEqual(state);
    expect(readVideoImport(moved).video).toEqual(state.video);
  });
  it("persists missing-tool failure and resumes probing the snapshot without the external file", async () => {
    const { root, source, workspace } = fixture();
    const previous = process.env.FFPROBE;
    try {
      process.env.FFPROBE = path.join(root, "missing-ffprobe");
      await expect(importVideo(source, workspace)).rejects.toThrow();
    } finally { if (previous === undefined) delete process.env.FFPROBE; else process.env.FFPROBE = previous; }
    const failed = readVideoImport(workspace);
    expect(failed.stage).toBe("failed"); expect(failed.failure).toContain("missing-ffprobe");
    renameSync(source, path.join(root, "elsewhere.mp4"));
    const ready = await importVideo(undefined, workspace);
    expect(ready.video).toEqual(failed.video); expect(ready.stage).toBe("awaiting_transcript");
  });
  it("reports corrupt media, refuses replacing workspace identity and detects a lost snapshot", async () => {
    const { source, workspace } = fixture();
    const state = await importVideo(source, workspace);
    await expect(importVideo(source + ".other", workspace)).rejects.toThrow("another video");
    renameSync(path.join(workspace, state.source!.imported_path), path.join(workspace, "removed.mp4"));
    await expect(importVideo(undefined, workspace)).rejects.toThrow();
    expect(readVideoImport(workspace).stage).toBe("failed");
    const bad = path.join(path.dirname(source), "bad.mp4"); writeFileSync(bad, "not a video");
    await expect(importVideo(bad, workspace + "-bad")).rejects.toThrow();
    expect(readVideoImport(workspace + "-bad").stage).toBe("failed");
  });
  it("rejects media outside the supported video/audio combination with a recoverable reason", async () => {
    const { source, workspace, root } = fixture();
    const mute = path.join(root, "no-audio.mp4");
    execFileSync(process.env.FFMPEG || "ffmpeg", ["-v", "error", "-i", source, "-c:v", "copy", "-an", mute], { windowsHide: true });
    await expect(importVideo(mute, workspace)).rejects.toThrow("H.264 video and AAC audio");
    expect(readVideoImport(workspace).failure).toContain("Currently supported");
  });
});
