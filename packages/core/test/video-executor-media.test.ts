import { mkdtempSync, renameSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { afterEach, describe, expect, it } from "vitest";
import { videoExecutorFixture } from "./helpers/video-executor-fixture";
import { readDshExecutorObservation } from "../src/dsh-executor-observation";
import { freezeBuildExecutorMediaInput, readBuildExecutorMediaInput, assertExecutorMediaBudget } from "../src/build-executor-media";
import { createBuildExecutorMcpSession } from "../../../skills/build/build-executor-mcp";
import { BUILD_EXECUTOR_BOOTSTRAP_CONTRACT_V3 as bootstrap } from "../src/build-executor-connection-capability";
import { startAutomaticBuildExecutorGeneration, openAutomaticBuildExecutorSessionV3, nextAutomaticBuildExecutorInput,
  automaticBuildExecutorMcpErrorFromSessionError } from "../src/automatic-build-executor-session";

const priorRoot = process.env.UNDERSTAND_BOOK_AUTOMATIC_BUILD_DRIVER_ROOT;
afterEach(() => { if (priorRoot === undefined) delete process.env.UNDERSTAND_BOOK_AUTOMATIC_BUILD_DRIVER_ROOT;
  else process.env.UNDERSTAND_BOOK_AUTOMATIC_BUILD_DRIVER_ROOT = priorRoot; });
const png = Buffer.from("iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+jVJkAAAAASUVORK5CYII=", "base64");
function prepare() {
  const fixture = videoExecutorFixture(mkdtempSync(path.join(tmpdir(), "vi4-executor-")));
  const frame = { video: fixture.manifest.source.video, actual_pts_ms: 1000, width: 1, height: 1 };
  const input = { target: fixture.target, descriptor: fixture.descriptor, source: fixture.manifest, transcript: fixture.transcript,
    images: [{ requested_ms: 1000, frame, png }, { requested_ms: 2000, frame: { ...frame, actual_pts_ms: 2000 }, png }] };
  return { ...fixture, input };
}
function session(root: string) {
  return createBuildExecutorMcpSession({ bootstrap_version: bootstrap.version, protocol_generation: bootstrap.session_protocol,
    session_private_root: path.join(root, "registry") });
}
function call(server: ReturnType<typeof session>, name: string, args: unknown) {
  const rpc = server.handle_message({ jsonrpc: "2.0", id: 1, method: "tools/call", params: { name, arguments: args } }) as any;
  if (rpc.error || rpc.result.isError) throw new Error(JSON.stringify(rpc));
  return { result: rpc.result, response: JSON.parse(rpc.result.content[0].text) };
}

describe("VI4 native media executor", () => {
  it("keeps crop coordinates and frozen media after workspace relocation", () => {
    const f = prepare();
    const frame = { ...f.input.images[0].frame, width: 2 };
    const region = { frame, rect_normalized: [0.5, 0, 0.5, 1] as [number, number, number, number] };
    freezeBuildExecutorMediaInput({ ...f.input, source: { ...f.manifest, source: { ...f.manifest.source, width: 2 } },
      images: [{ requested_ms: 1000, frame, region, png }] });
    const moved = f.target.workspace_dir + "-moved";
    renameSync(f.target.workspace_dir, moved);
    const media = readBuildExecutorMediaInput({ ...f.target, workspace_dir: moved }, f.descriptor)!;
    expect(media.images[0].region).toEqual(region);
    expect(Buffer.from(media.images[0].data, "base64")).toEqual(png);
  }, 30000);

  it("refuses a frozen session whose media disappeared and reports actionable media limits", () => {
    const f = prepare();
    freezeBuildExecutorMediaInput(f.input);
    const opened = openAutomaticBuildExecutorSessionV3(f.envelope.opaque_handoff_ref);
    if (opened.action.kind !== "DELIVER_INPUT") throw new Error("expected delivery");
    const nextRequest = opened.action.next_request;
    const file = path.join(f.target.workspace_dir, ".build/executor-media", f.descriptor.stage,
      encodeURIComponent(f.descriptor.work_unit_id), `${f.descriptor.input_hash}.json`);
    rmSync(file);
    expect(() => nextAutomaticBuildExecutorInput(nextRequest)).toThrow(/identity drifted/);
    for (const code of ["executor_media_context_limit", "executor_media_capability_unavailable"] as const) {
      const engine_error = automaticBuildExecutorMcpErrorFromSessionError(new Error(code), "open");
      expect(engine_error).toMatchObject({ diagnostic_code: code, category: "transport" });
      expect(readDshExecutorObservation({ version: "dsh_executor_observation.v1", launch_id: "vi4",
        opaque_handoff_ref: f.envelope.opaque_handoff_ref, dispatch_slot_ref: `abdispatchslot1_${"1".repeat(64)}`,
        outcome: "open", calls: 1, last_operation: "executor.open", engine_error }).engine_error).toEqual(engine_error);
    }
  }, 30000);

  it("freezes bytes/positions once and rejects changed images, wrong sources and budgets", () => {
    const f = prepare();
    const manifest = freezeBuildExecutorMediaInput(f.input);
    expect(freezeBuildExecutorMediaInput(f.input)).toEqual(manifest);
    const media = readBuildExecutorMediaInput(f.target, f.descriptor)!;
    expect(media.images.map(i => Buffer.from(i.data, "base64"))).toEqual([png, png]);
    expect(() => freezeBuildExecutorMediaInput({ ...f.input, images: f.input.images.slice(0, 1) })).toThrow(/already_frozen/);
    expect(() => freezeBuildExecutorMediaInput({ ...f.input, images: Array(9).fill(f.input.images[0]) })).toThrow(/image_limit/);
    expect(() => freezeBuildExecutorMediaInput({ ...f.input, images: [{ ...f.input.images[0], frame: { ...f.input.images[0].frame, width: 2 } }] })).toThrow(/frame/);
    expect(() => assertExecutorMediaBudget(media, f.descriptor, "deepseek_harness")).toThrow(/capability_unavailable/);
    const descriptor = { ...f.descriptor, execution_budget_proof: { ...(f.descriptor as any).execution_budget_proof,
      estimated_rendered_tokens: 1, effective_body_limit_tokens: 100 }, version: "automatic_build_work_unit.v4" } as any;
    expect(() => assertExecutorMediaBudget(media, descriptor, "codex")).toThrow(/context_limit/);
  }, 30000);

  it("delivers all native images, replays after reconnect, requires final ACK, then accepts candidate privately", () => {
    const f = prepare();
    const media = freezeBuildExecutorMediaInput(f.input);
    let server = session(f.root);
    const opened = call(server, "executor.open", { version: "automatic_build_executor_open_request.v3", opaque_handoff_ref: f.envelope.opaque_handoff_ref }).response;
    expect(opened.action.input_manifest.media_input).toEqual(media);
    expect(JSON.stringify(f.next)).not.toContain("data:image");
    expect(JSON.stringify(f.next)).not.toContain("media_input_ref");
    const start = { version: "automatic_build_executor_generation_start_request.v3" as const,
      opaque_session_ref: opened.action.input_manifest.opaque_session_ref, generation_input_ref: opened.action.input_manifest.generation_input_ref,
      confirmed_through_ordinal: opened.action.input_manifest.total_chunk_count - 1 };
    expect(() => startAutomaticBuildExecutorGeneration(start)).toThrow(/delivery|batch/);
    let request = opened.action.next_request;
    let images = 0;
    for (;;) {
      const got = call(server, "executor.input.next", request);
      const batch = got.response.action.batch;
      if (batch.final_for_generation) {
        const native = got.result.content.filter((item: any) => item.type === "image");
        images += native.length;
        expect(native.map((item: any) => Buffer.from(item.data, "base64"))).toEqual([png, png]);
        expect(got.result.content[0].text).not.toContain(png.toString("base64"));
        expect(batch.chunks.at(-1).media.images.map((i: any) => i.frame.actual_pts_ms)).toEqual([1000, 2000]);
        // Lose the connection before final acknowledgement; a fresh connection must replay it.
        server = session(f.root);
        const reopened = call(server, "executor.open", { version: "automatic_build_executor_open_request.v3", opaque_handoff_ref: f.envelope.opaque_handoff_ref }).response;
        expect(reopened.action.input_manifest).toEqual(opened.action.input_manifest);
        expect(call(server, "executor.input.next", reopened.action.next_request).result).toEqual(got.result);
        break;
      }
      request = { ...request, ack_through_ordinal: batch.last_ordinal };
    }
    expect(images).toBe(2);
    const generated = call(server, "executor.generation.start", start).response.action;
    expect(generated.kind).toBe("GENERATE");
    const done = call(server, "executor.submit_candidate", { version: "automatic_build_executor_candidate_submit.v3",
      opaque_session_ref: generated.opaque_session_ref, candidate_sink_ref: generated.candidate_sink_ref,
      candidate: { nodes: [], edges: [] } }).response;
    expect(["DONE", "DELIVER_INPUT"]).toContain(done.action.kind);
    if (done.action.kind === "DONE") expect(done.action.status).toBe("committed");
  }, 30000);
});
