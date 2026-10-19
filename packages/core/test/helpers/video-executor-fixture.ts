import { mkdirSync, writeFileSync } from "node:fs";
import path from "node:path";
import { confirmedStandardBuildPlan } from "./confirmed-build-plan";
import { automaticBuildPlan, automaticBuildNext } from "../../../../skills/build/automatic-build";
import { resolveAutomaticBuildTarget } from "../../src/build-orchestrator";
import { readPass1ShadowTask } from "../../src/pass1-reduction";
import type { WorkUnitDescriptor } from "../../src/stage-work-unit";
import type { VideoSourceManifest } from "../../src/generated/VideoSourceManifest";
import type { TranscriptMap } from "../../src/generated/TranscriptMap";

export function videoExecutorFixture(root: string, body = "Bfloat16 retains the dynamic range of float32 with lower resolution.") {
  mkdirSync(root, { recursive: true });
  process.env.UNDERSTAND_BOOK_AUTOMATIC_BUILD_DRIVER_ROOT = path.join(root, "registry");
  const source = path.join(root, "transcript.md");
  writeFileSync(source, `# Tensor precision\n\n${body}\n`);
  const buildPlan = confirmedStandardBuildPlan(source, root);
  const plan = automaticBuildPlan(source, root, { requested_workers: 1, available_agent_slots: 1, build_plan: buildPlan });
  const next = automaticBuildNext(source, root, 1, { accepted_plan_digest: plan.preflight!.descriptor_plan_digest,
    available_agent_slots: 1, executor_dispatches: true, build_plan: buildPlan });
  if (!("dispatches" in next.action) || !next.action.dispatches?.length) throw new Error("missing fixture dispatch");
  const envelope = next.action.dispatches[0];
  const target = resolveAutomaticBuildTarget(source, root);
  const workId = envelope.manifest.ordered_work_unit_ids[0];
  const binding = envelope.manifest.task_bindings![workId];
  if (!("policy_generation_id" in binding)) throw new Error("missing generation binding");
  const descriptor: WorkUnitDescriptor = readPass1ShadowTask(target, binding.policy_generation_id, workId).descriptor;
  const video = { media_id: "vi4-video", media_revision: "1" };
  const transcriptRef = { transcript_id: "vi4-transcript", transcript_revision: "1", video };
  const manifest: VideoSourceManifest = { version: "video_source_manifest.v1",
    material: { book_id: target.book_id, video, transcript: transcriptRef },
    source: { video, imported_path: "media/original.mp4", original_path: "original.mp4", duration_ms: 10000, width: 1, height: 1 },
    canonical_text_path: "source.txt", transcript_map_path: "transcript_map.json", coverage: [], previous_book_id: null };
  const transcript: TranscriptMap = { transcript: transcriptRef, raw_result_path: "raw.json",
    machine_transcription: { engine: "test", model: "fixture", language: "en" },
    segments: [{ segment_id: "s1", text: body, location: { video, start_ms: 0, end_ms: 10000 },
      text_range: { lid: descriptor.evidence_lids.at(-1)!, range: { start: 0, end: body.length } }, words: [] }] };
  return { root, source, target, envelope, descriptor, manifest, transcript, next };
}
