import { describe, expect, it, vi } from "vitest";
import { automaticBuildRefill } from "../../../skills/build/automatic-build-refill";
import type { AutomaticBuildStepRequestV1, AutomaticBuildStepResponseV1 } from "../../../skills/build/automatic-build-driver";

const owned = (child: string) => ({ child, opaque_handoff_ref: `ref-${child}` });
const launch = (child: string, slot = child) => ({ dispatch_slot_ref: slot, opaque_handoff_ref: `ref-${child}` });
const request = () => ({ version: "automatic_build_refill_request.v1", invocation_ref: "invocation",
  capacity_limit: 3, live_by_slot: { A: owned("A"), B: owned("B"), C: owned("C") },
  completed_refs: [] as string[], terminal_children: ["C"] });
function engine(action: AutomaticBuildStepResponseV1["action"], maxParallel = 3) {
  return { validateStep: (value: unknown) => value as AutomaticBuildStepRequestV1,
    maxParallel: vi.fn(() => maxParallel),
    step: vi.fn((_request: AutomaticBuildStepRequestV1): AutomaticBuildStepResponseV1 => ({ version: "automatic_build_step.v1", action })) };
}

describe("single-operation refill", () => {
  it("consumes C and requests one slot in its only step, then starts D preserving A and B", () => {
    const input = request();
    const runtime = engine({ kind: "SPAWN_EXECUTORS", executors: [launch("A"), launch("B"), launch("D", "C"), launch("D", "C")] });
    const result = automaticBuildRefill(input, runtime);
    expect(runtime.step).toHaveBeenCalledExactlyOnceWith({ version: "automatic_build_step_request.v1",
      invocation_ref: "invocation", available_agent_slots: 1 }, ["ref-A", "ref-B"], ["ref-C"]);
    expect(result.live_by_slot).toEqual({ A: owned("A"), B: owned("B") });
    expect(result.completed_refs).toEqual(["ref-C"]);
    expect(result.ready_executors).toEqual([launch("D", "C")]);
    expect(input.live_by_slot.C).toEqual(owned("C"));
    // The host starts a fresh child from the returned reference.
    result.live_by_slot.C = owned("D");
    const replay = automaticBuildRefill({ ...input, live_by_slot: result.live_by_slot,
      completed_refs: result.completed_refs }, runtime);
    expect(replay.available_agent_slots).toBe(0);
    expect(replay.live_by_slot).toEqual({ A: owned("A"), B: owned("B"), C: owned("D") });
    expect(replay.ready_executors).toEqual([]);
  });

  it("uses the next delivered terminal batch before launching and preserves unstarted engine references", () => {
    const input = request();
    const runtime = engine({ kind: "SPAWN_EXECUTORS", executors: [launch("D", "C"), launch("E", "B")] });
    const first = automaticBuildRefill(input, runtime);
    const next = automaticBuildRefill({ ...input, live_by_slot: first.live_by_slot,
      completed_refs: first.completed_refs, terminal_children: ["B"] }, runtime);
    expect(next.available_agent_slots).toBe(2);
    expect(next.live_by_slot).toEqual({ A: owned("A") });
    expect(next.ready_executors).toEqual([launch("D", "C"), launch("E", "B")]);
  });

  it("caps launches by host capacity and confirmed invocation concurrency", () => {
    const runtime = engine({ kind: "SPAWN_EXECUTORS", executors: [launch("D"), launch("E")] }, 1);
    const result = automaticBuildRefill({ ...request(), live_by_slot: {} }, runtime);
    expect(result.available_agent_slots).toBe(1);
    expect(result.ready_executors).toHaveLength(1);
    const full = automaticBuildRefill({ ...request(), capacity_limit: 2 }, runtime);
    expect(full.available_agent_slots).toBe(0);
    expect(full.ready_executors).toEqual([]);
  });

  it("forwards a queued observation unchanged and retains engine WAIT with released ownership", () => {
    const runtime = engine({ kind: "WAIT", reason: "backoff", retry_after_ms: 50 });
    const observation = { opaque_handoff_ref: "ref-C" };
    const result = automaticBuildRefill({ ...request(), bootstrap_failure: observation }, runtime);
    expect(runtime.step.mock.calls[0]?.[0]).toMatchObject({ bootstrap_failure: observation, available_agent_slots: 1 });
    expect(result.step.action.kind).toBe("WAIT");
    expect(result.live_by_slot).not.toHaveProperty("C");
    expect(result.ready_executors).toEqual([]);
  });
});
