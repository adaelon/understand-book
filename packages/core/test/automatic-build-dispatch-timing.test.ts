import { mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { afterEach, describe, expect, it } from "vitest";
import { createAutomaticBuildRefillTimingObserver, recordAutomaticBuildGenerationTiming } from "../src/automatic-build-dispatch-timing";
import { automaticBuildRefill } from "../../../skills/build/automatic-build-refill";
import type { AutomaticBuildStepRequestV1, AutomaticBuildStepResponseV1 } from "../../../skills/build/automatic-build-driver";

const directories: string[] = [];
afterEach(() => { for (const directory of directories.splice(0)) rmSync(directory, { recursive: true, force: true }); });
function fixture() {
  const root = mkdtempSync(path.join(tmpdir(), "ub-dispatch-observation-"));
  directories.push(root);
  let now = "2026-10-23T00:00:00.000Z";
  const clock = () => now;
  const engine = (action: AutomaticBuildStepResponseV1["action"]) => ({
    validateStep: (value: unknown) => value as AutomaticBuildStepRequestV1,
    maxParallel: () => 3,
    timing: createAutomaticBuildRefillTimingObserver({ registry_root: root, now: clock }),
    step: () => ({ version: "automatic_build_step.v1" as const, action }),
  });
  const request = { version: "automatic_build_refill_request.v1", invocation_ref: "invocation", capacity_limit: 3,
    live_by_slot: {}, completed_refs: [] as string[], terminal_children: [] as string[] };
  const handoff = (ref: string) => JSON.parse(readFileSync(path.join(root, "dispatch-timing", "handoffs", `${ref}.json`), "utf8"));
  return { root, engine, request, handoff, setNow: (value: string) => { now = value; } };
}
const launch = (ref: string) => ({ opaque_handoff_ref: ref, dispatch_slot_ref: `slot-${ref}` });
const spawn = (...refs: string[]): AutomaticBuildStepResponseV1["action"] => ({ kind: "SPAWN_EXECUTORS", executors: refs.map(launch) });
const wait: AutomaticBuildStepResponseV1["action"] = { kind: "WAIT", reason: "backoff", retry_after_ms: 10 };

describe("dispatch timing observations", () => {
  it("keeps two released vacancies over WAIT and restart, matches each once, and preserves replay timestamps", () => {
    const value = fixture();
    const initial = automaticBuildRefill(value.request, value.engine(spawn("a", "b", "c")));
    expect(value.handoff("a").vacancy.source).toBe("initial_capacity");
    const live = Object.fromEntries(initial.ready_executors.map(item => [item.dispatch_slot_ref!, {
      child: `child-${item.opaque_handoff_ref}`, opaque_handoff_ref: item.opaque_handoff_ref }]));
    value.setNow("2026-10-23T00:00:10.000Z");
    const waiting = automaticBuildRefill({ ...value.request, live_by_slot: live,
      terminal_children: ["child-b", "child-c"] }, value.engine(wait));
    expect(waiting.ready_executors).toHaveLength(0);
    value.setNow("2026-10-23T00:00:20.000Z");
    const refill = automaticBuildRefill({ ...value.request, live_by_slot: waiting.live_by_slot,
      completed_refs: waiting.completed_refs }, value.engine(spawn("d", "e")));
    expect(refill.ready_executors).toHaveLength(2);
    expect([value.handoff("d").vacancy.previous_handoff_ref, value.handoff("e").vacancy.previous_handoff_ref]).toEqual(["b", "c"]);
    expect(value.handoff("d").vacancy.observed_at).toBe("2026-10-23T00:00:10.000Z");
    expect(value.handoff("d").refill_started_at).toBe("2026-10-23T00:00:20.000Z");
    const original = value.handoff("d");
    value.setNow("2026-10-23T00:00:30.000Z");
    automaticBuildRefill({ ...value.request, live_by_slot: waiting.live_by_slot,
      completed_refs: waiting.completed_refs }, value.engine(spawn("d", "e")));
    expect(value.handoff("d")).toEqual(original);
  });

  it("does not call a submitted but still live child a vacant slot", () => {
    const value = fixture();
    automaticBuildRefill({ ...value.request, capacity_limit: 1, live_by_slot: {
      "slot-old": { child: "old-child", opaque_handoff_ref: "old" },
    } }, value.engine(wait));
    value.setNow("2026-10-23T00:00:40.000Z");
    automaticBuildRefill({ ...value.request, capacity_limit: 1, live_by_slot: {
      "slot-old": { child: "old-child", opaque_handoff_ref: "old" },
    }, terminal_children: ["old-child"] }, value.engine(spawn("new")));
    expect(value.handoff("new").vacancy).toMatchObject({ source: "terminal_refill", previous_handoff_ref: "old",
      observed_at: "2026-10-23T00:00:40.000Z" });
  });

  it("does not invent a vacancy for historical completions or reuse a consumed release", () => {
    const value = fixture();
    automaticBuildRefill({ ...value.request, capacity_limit: 0 }, value.engine(wait));
    automaticBuildRefill({ ...value.request, capacity_limit: 1, completed_refs: ["old"] }, value.engine(spawn("a")));
    expect(value.handoff("a").vacancy).toBeUndefined();
    const input = { ...value.request, capacity_limit: 1,
      live_by_slot: { old: { child: "old-child", opaque_handoff_ref: "old" } }, terminal_children: ["old-child"] };
    automaticBuildRefill(input, value.engine(spawn("b")));
    automaticBuildRefill(input, value.engine(spawn("c")));
    expect(value.handoff("b").vacancy.source).toBe("terminal_refill");
    expect(value.handoff("c").vacancy).toBeUndefined();
  });

  it("leaves dispatch behavior intact when timing storage is unavailable", () => {
    const value = fixture();
    const invalidRoot = path.join(value.root, "file");
    writeFileSync(invalidRoot, "not a directory");
    const engine = { ...value.engine(spawn("a")), timing: createAutomaticBuildRefillTimingObserver({ registry_root: invalidRoot }) };
    expect(automaticBuildRefill(value.request, engine).ready_executors).toEqual([launch("a")]);
    expect(() => recordAutomaticBuildGenerationTiming(invalidRoot, path.join(invalidRoot, "lease.json"), {
      opaque_handoff_ref: "a", dispatch_id: "dispatch-a", dispatch_run_id: "run-a", work_unit_id: "task-a",
      physical_attempt: 1, semantic_attempt: 1, lease_epoch: 1,
      opened_at: "2026-10-23T00:00:00.000Z", generation_started_at: "2026-10-23T00:00:10.000Z",
    })).not.toThrow();
  });

  it("assigns an observed vacancy only to the first generation in a multi-unit handoff", () => {
    const value = fixture();
    automaticBuildRefill({ ...value.request, capacity_limit: 1 }, value.engine(spawn("a")));
    const input = { opaque_handoff_ref: "a", dispatch_id: "dispatch-a", dispatch_run_id: "run-a",
      work_unit_id: "task-first", physical_attempt: 1, semantic_attempt: 1, lease_epoch: 1,
      opened_at: "2026-10-23T00:00:01.000Z", generation_started_at: "2026-10-23T00:00:02.000Z" };
    const firstLease = path.join(value.root, "first-attempt", "lease.json");
    const secondLease = path.join(value.root, "second-attempt", "lease.json");
    recordAutomaticBuildGenerationTiming(value.root, firstLease, input);
    recordAutomaticBuildGenerationTiming(value.root, secondLease, { ...input, work_unit_id: "task-second",
      generation_started_at: "2026-10-23T00:01:02.000Z" });
    const firstFile = path.join(path.dirname(firstLease), "generation-timing.json");
    const first = readFileSync(firstFile, "utf8");
    expect(JSON.parse(first)).toMatchObject({ first_generation_for_handoff: true,
      handoff: { vacancy: { source: "initial_capacity" } } });
    expect(JSON.parse(readFileSync(path.join(path.dirname(secondLease), "generation-timing.json"), "utf8")))
      .toMatchObject({ first_generation_for_handoff: false });
    expect(JSON.parse(readFileSync(path.join(path.dirname(secondLease), "generation-timing.json"), "utf8")).handoff)
      .toBeUndefined();
    recordAutomaticBuildGenerationTiming(value.root, firstLease, { ...input, generation_started_at: "2026-10-23T00:02:00.000Z" });
    expect(readFileSync(firstFile, "utf8")).toBe(first);
  });
});
