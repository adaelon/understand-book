import { mkdtempSync, writeFileSync } from "node:fs";
import path from "node:path";
import { tmpdir } from "node:os";
import { setImmediate as yieldToRunner } from "node:timers/promises";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import {
  automaticBuildWorkerNeedsContextRotation, listAutomaticBuildWorkers, readAutomaticBuildWorker,
  readAutomaticBuildWorkerForAssignment, recordAutomaticBuildWorkerUsage, registerAutomaticBuildWorker,
  withAutomaticBuildWorkerSchedulerLock,
} from "../src/automatic-build-worker";
import { automaticBuildWorkerStep, claimNextAutomaticBuildWorkerAssignment, createAutomaticBuildInvocation,
  runAutomaticBuildDriverCommandWithPreparation } from "../../../skills/build/automatic-build-driver";
import { nextAutomaticBuildExecutorInput, openAutomaticBuildExecutorSessionV3, startAutomaticBuildExecutorGeneration,
  submitAutomaticBuildExecutorCandidateV3 } from "../src/automatic-build-executor-session";
import { confirmedStandardBuildPlan } from "./helpers/confirmed-build-plan";
import type { AutomaticBuildRefillResponseV1 } from "../../../skills/build/automatic-build-refill";
import * as automaticBuildEngine from "../../../skills/build/automatic-build";
import { AutomaticBuildDispatchSettledError } from "../src/automatic-build-dispatch-runtime";

beforeEach(async () => {
  await yieldToRunner();
  vi.stubEnv("UNDERSTAND_BOOK_AUTOMATIC_BUILD_DRIVER_ROOT", mkdtempSync(path.join(tmpdir(), "persistent-workers-registry-")));
});
afterEach(() => { vi.restoreAllMocks(); vi.unstubAllEnvs(); });

function fixture(paragraphs = 900, maxParallel: 2 | 8 = 8) {
  const root = mkdtempSync(path.join(tmpdir(), "persistent-workers-book-")), source = path.join(root, "book.md");
  writeFileSync(source, `# Scheduling\n\n${Array.from({ length: paragraphs }, (_, index) =>
    `Paragraph ${index + 1} describes a stable observation and an independently accepted result.`).join("\n\n")}`);
  const planFile = path.join(root, "plan.json");
  writeFileSync(planFile, JSON.stringify(confirmedStandardBuildPlan(source, root)));
  const invocation = createAutomaticBuildInvocation({ version: "automatic_build_invocation_create.v1", target_input: source,
    root_dir: root, build_plan_path: planFile, quality_profile: "full", max_parallel: maxParallel, created_at: new Date().toISOString() });
  return { invocation_ref: invocation.invocation_ref, request: { version: "automatic_build_step_request.v1" as const,
    invocation_ref: invocation.invocation_ref, available_agent_slots: maxParallel } };
}
function commitRawAssignment(ref: string) {
  let response = openAutomaticBuildExecutorSessionV3(ref);
  let committed = 0;
  while (response.action.kind === "DELIVER_INPUT") {
    let delivery = nextAutomaticBuildExecutorInput(response.action.next_request);
    while (delivery.action.kind === "INPUT_BATCH" && !delivery.action.batch.final_for_generation) {
      delivery = nextAutomaticBuildExecutorInput({ version: "automatic_build_executor_input_next_request.v4",
        opaque_session_ref: delivery.action.batch.opaque_session_ref,
        generation_input_ref: delivery.action.batch.generation_input_ref,
        ack_through_ordinal: delivery.action.batch.last_ordinal });
    }
    if (delivery.action.kind !== "INPUT_BATCH") throw new Error(`input ${delivery.action.kind}`);
    const started = startAutomaticBuildExecutorGeneration({ version: "automatic_build_executor_generation_start_request.v3",
      opaque_session_ref: delivery.action.batch.opaque_session_ref, generation_input_ref: delivery.action.batch.generation_input_ref,
      confirmed_through_ordinal: delivery.action.batch.last_ordinal });
    if (started.action.kind !== "GENERATE") throw new Error(`start ${started.action.kind}`);
    response = submitAutomaticBuildExecutorCandidateV3({ version: "automatic_build_executor_candidate_submit.v3",
      opaque_session_ref: started.action.opaque_session_ref, candidate_sink_ref: started.action.candidate_sink_ref,
      candidate: { nodes: [], edges: [] } });
    committed++;
  }
  expect(response.action).toMatchObject({ kind: "DONE", status: "committed" });
  return committed;
}

describe("persistent worker scheduling", () => {
  it("serializes eight simultaneous route/publication operations", async () => {
    const invocation = `abinv1_${"a".repeat(64)}`;
    let active = 0, maximum = 0, assignments = 0;
    await Promise.all(Array.from({ length: 8 }, () => withAutomaticBuildWorkerSchedulerLock(invocation, async () => {
      maximum = Math.max(maximum, ++active);
      const old = assignments;
      await new Promise(resolve => setTimeout(resolve, 2));
      assignments = old + 1;
      active--;
    })));
    expect(maximum).toBe(1);
    expect(assignments).toBe(8);
  });

  it("counts delivered input and generated output once and rotates by context rather than batch count", () => {
    const ref = `abhandoff1_${"b".repeat(64)}`;
    registerAutomaticBuildWorker({ invocation_ref: `abinv1_${"a".repeat(64)}`, opaque_handoff_ref: ref,
      dispatch_slot_ref: `abdispatchslot1_${"c".repeat(64)}`, stage: "book_structure" });
    for (let task = 0; task < 12; task++) recordAutomaticBuildWorkerUsage(ref, {
      delivery_ref: `input:${task}`, input_bytes: 1_000, submission_ref: `output:${task}`, output_bytes: 500 });
    recordAutomaticBuildWorkerUsage(ref, { delivery_ref: "input:0", input_bytes: 1_000, submission_ref: "output:0", output_bytes: 500 });
    const worker = readAutomaticBuildWorker(ref)!;
    expect(worker.delivered_input_bytes).toBe(12_000);
    expect(worker.submitted_output_bytes).toBe(6_000);
    expect(automaticBuildWorkerNeedsContextRotation(worker)).toBe(false);
    recordAutomaticBuildWorkerUsage(ref, { delivery_ref: "large-input", input_bytes: 150_000 });
    expect(automaticBuildWorkerNeedsContextRotation(readAutomaticBuildWorker(ref)!)).toBe(true);
    expect(readAutomaticBuildWorkerForAssignment(ref)?.root_handoff_ref).toBe(ref);
  });

  it("reserves at most eight workers when two root requests overlap and replays their stable refs", async () => {
    const f = fixture(3_000);
    const [first, second] = await Promise.all([automaticBuildWorkerStep(f.request), automaticBuildWorkerStep(f.request)]);
    expect(first.action.kind).toBe("SPAWN_EXECUTORS");
    expect(second.action.kind).toBe("SPAWN_EXECUTORS");
    if (first.action.kind !== "SPAWN_EXECUTORS" || second.action.kind !== "SPAWN_EXECUTORS") throw new Error("expected workers");
    expect(first.action.executors).toHaveLength(8);
    expect(new Set([...first.action.executors, ...second.action.executors].map(worker => worker.opaque_handoff_ref)).size).toBe(8);
    expect(listAutomaticBuildWorkers(f.invocation_ref)).toHaveLength(8);
  }, 120_000);

  it("keeps root ownership while a worker claims another dispatch, and old completion cannot allocate twice", async () => {
    const f = fixture(900, 2);
    const first = await automaticBuildWorkerStep(f.request);
    if (first.action.kind !== "SPAWN_EXECUTORS") throw new Error("expected workers");
    const root = first.action.executors[0]!.opaque_handoff_ref;
    expect(commitRawAssignment(root)).toBeGreaterThan(0);
    await yieldToRunner();
    const [a, replay] = await Promise.all([claimNextAutomaticBuildWorkerAssignment(root, root), claimNextAutomaticBuildWorkerAssignment(root, root)]);
    expect(a.kind).toBe("ASSIGNMENT");
    expect(replay).toEqual(a);
    if (a.kind !== "ASSIGNMENT") throw new Error("expected next assignment");
    expect(a.opaque_handoff_ref).not.toBe(root);
    expect(readAutomaticBuildWorker(root)?.assignments).toEqual([root, a.opaque_handoff_ref]);
    const sibling = first.action.executors[1]!.opaque_handoff_ref;
    commitRawAssignment(sibling);
    await yieldToRunner();
    expect(await claimNextAutomaticBuildWorkerAssignment(sibling, sibling)).toMatchObject({ kind: "WAIT" });
    expect(readAutomaticBuildWorker(sibling)?.status).toBe("waiting");
    const refill = await runAutomaticBuildDriverCommandWithPreparation({ version: "automatic_build_refill_request.v1",
      invocation_ref: f.invocation_ref, capacity_limit: 2, completed_refs: [], terminal_children: [],
      live_by_slot: Object.fromEntries(first.action.executors.map((executor, index) => [executor.dispatch_slot_ref!,
        { child: `worker-${index}`, opaque_handoff_ref: executor.opaque_handoff_ref }])) }) as AutomaticBuildRefillResponseV1;
    expect(refill.ready_executors).toEqual([]);
    expect(Object.values(refill.live_by_slot).map(worker => worker.opaque_handoff_ref)).toContain(root);
    expect(listAutomaticBuildWorkers(f.invocation_ref)).toHaveLength(2);
    commitRawAssignment(a.opaque_handoff_ref);
    await yieldToRunner();
    // Waiting consumes model context too, even without another semantic task.
    for (let turn = 0; turn < 61; turn++) recordAutomaticBuildWorkerUsage(root, { wait_response_bytes: 128 });
    expect(await claimNextAutomaticBuildWorkerAssignment(root, a.opaque_handoff_ref))
      .toEqual({ kind: "DONE", reason: "context_rotation" });
    expect(readAutomaticBuildWorker(root)?.assignments).toHaveLength(2);
    expect(await claimNextAutomaticBuildWorkerAssignment(sibling, sibling))
      .toEqual({ kind: "DONE", reason: "stage_complete" });
  }, 120_000);

  it("recovers a terminal child at its current assignment and preserves the live sibling", async () => {
    const f = fixture(900, 2);
    const first = await automaticBuildWorkerStep(f.request);
    if (first.action.kind !== "SPAWN_EXECUTORS") throw new Error("expected workers");
    const [a, b] = first.action.executors;
    commitRawAssignment(a!.opaque_handoff_ref);
    const next = await claimNextAutomaticBuildWorkerAssignment(a!.opaque_handoff_ref, a!.opaque_handoff_ref);
    if (next.kind !== "ASSIGNMENT") throw new Error("expected continuation");
    openAutomaticBuildExecutorSessionV3(next.opaque_handoff_ref);
    const refill = await runAutomaticBuildDriverCommandWithPreparation({ version: "automatic_build_refill_request.v1",
      invocation_ref: f.invocation_ref, capacity_limit: 2, completed_refs: [], terminal_children: ["worker-0"],
      live_by_slot: Object.fromEntries(first.action.executors.map((executor, index) => [executor.dispatch_slot_ref!,
        { child: `worker-${index}`, opaque_handoff_ref: executor.opaque_handoff_ref }])) }) as AutomaticBuildRefillResponseV1;
    expect(refill.ready_executors).toHaveLength(1);
    expect(refill.ready_executors[0]!.opaque_handoff_ref).not.toBe(a!.opaque_handoff_ref);
    expect(refill.live_by_slot[b!.dispatch_slot_ref!]?.opaque_handoff_ref).toBe(b!.opaque_handoff_ref);
    expect(readAutomaticBuildWorker(a!.opaque_handoff_ref)?.status).toBe("done");
    expect(openAutomaticBuildExecutorSessionV3(refill.ready_executors[0]!.opaque_handoff_ref).action.kind).toBe("DELIVER_INPUT");
  }, 120_000);

  it("keeps the worker waiting when a sibling settles during claim and resumes on the next poll", async () => {
    const f = fixture(900, 2);
    const first = await automaticBuildWorkerStep(f.request);
    if (first.action.kind !== "SPAWN_EXECUTORS") throw new Error("expected workers");
    const [worker, sibling] = first.action.executors;
    const root = worker!.opaque_handoff_ref;
    commitRawAssignment(root);
    const before = readAutomaticBuildWorker(root)!;
    const route = vi.spyOn(automaticBuildEngine, "automaticBuildNext").mockImplementationOnce(() => {
      // Reproduce the durable state change observed in the real-book baseline:
      // another dispatch completes while selection/publication is in progress.
      commitRawAssignment(sibling!.opaque_handoff_ref);
      throw new AutomaticBuildDispatchSettledError("dispatch has no current work unit");
    });

    expect(await claimNextAutomaticBuildWorkerAssignment(root, root))
      .toEqual({ kind: "WAIT", retry_after_ms: 50 });
    expect(route).toHaveBeenCalledTimes(1);
    expect(readAutomaticBuildWorker(root)).toMatchObject({
      root_handoff_ref: root, current_handoff_ref: before.current_handoff_ref,
      dispatch_slot_ref: before.dispatch_slot_ref, status: "waiting",
      assignments: [root], completed_assignments: [root],
    });
    expect(readAutomaticBuildWorker(root)?.terminal_reason).toBeUndefined();
    expect(listAutomaticBuildWorkers(f.invocation_ref)).toHaveLength(2);

    const next = await claimNextAutomaticBuildWorkerAssignment(root, root);
    expect(next.kind).toBe("ASSIGNMENT");
    if (next.kind !== "ASSIGNMENT") throw new Error("expected resumed assignment");
    expect(next.opaque_handoff_ref).not.toBe(root);
    expect(readAutomaticBuildWorker(root)).toMatchObject({ root_handoff_ref: root,
      current_handoff_ref: next.opaque_handoff_ref, dispatch_slot_ref: before.dispatch_slot_ref,
      status: "active", assignments: [root, next.opaque_handoff_ref], completed_assignments: [root] });
    expect(readAutomaticBuildWorkerForAssignment(next.opaque_handoff_ref)?.root_handoff_ref).toBe(root);
    expect(listAutomaticBuildWorkers(f.invocation_ref)).toHaveLength(2);
    expect(openAutomaticBuildExecutorSessionV3(next.opaque_handoff_ref).action.kind).toBe("DELIVER_INPUT");
  }, 120_000);
});
