import { mkdtempSync, readdirSync, readFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { afterEach, describe, expect, it, vi } from "vitest";

const host = vi.hoisted(() => ({
  agents: new Map<string, any>(), launches: [] as string[], observations: [] as string[],
  terminal: new Map<string, () => void>(), step: 0,
}));
vi.mock("../src/capability-check.ts", () => ({ checkHarnessCapabilities: () => ({ status: "available", missing: [] }) }));
vi.mock("../src/executor-runtime.ts", () => ({ resolveExecutorRuntime: vi.fn() }));
vi.mock("../src/executor-bridge.ts", () => ({ prepareExecutorBridge: (_binding: unknown, _config: unknown, observation: any) => {
  observation.done = "committed"; observation.calls = 1;
} }));
vi.mock("../src/engine-client.ts", () => ({ engineCommand: async (_config: unknown, _args: unknown, request: any) => {
  if (request.version === "dsh_build_invocation_read.v2") return {
    version: "dsh_build_invocation.v2", max_parallel: 2,
    model_runtime: { version: "build_executor_model_runtime.v1", provider: "fixture", model: "scripted",
      reasoning_effort: null, max_output_tokens: 1000, context_window_tokens: 1000000,
      safety_margin_tokens: 100, context_source: "provider_model_metadata" },
  };
  if (request.action === "acquire") return { acquired: true };
  if (request.action === "release") return { released: true };
  if (request.action === "observe") { host.observations.push(request.observation.opaque_handoff_ref); return { recorded: true }; }
  if (request.action !== "step") throw new Error("unexpected fixture command");
  if (request.available_agent_slots === 0) return { version: "automatic_build_step.v1",
    action: { kind: "DONE", summary: { status: "complete", completed_stages: 1 } } };
  const refs = host.step++ === 0 ? ["a", "b"] : host.step === 2 ? ["a", "c"] : ["a", "d"];
  return { version: "automatic_build_step.v1", action: { kind: "SPAWN_EXECUTORS",
    executors: refs.map(ref => ({ opaque_handoff_ref: ref, dispatch_slot_ref: `slot-${ref}` })) } };
} }));
vi.mock("../src/executor-host.ts", () => ({ createExecutorHost: (_ctx: unknown, _preset: unknown, bind: (value: any) => void) => ({
  launch: async (launch: any) => {
    const ref = launch.opaque_handoff_ref;
    host.launches.push(ref);
    const agent = { id: `child-${ref}`, options: { subagentDepth: 1 } };
    host.agents.set(agent.id, agent);
    bind({ launch, agent, costObservation: { model_calls: 0, usages: [] } });
    const terminal = new Promise<{ stop_reason: string }>(resolve => {
      host.terminal.set(ref, () => resolve({ stop_reason: "completed" }));
    });
    // A is intentionally kept alive while B and then C complete and trigger refills.
    if (ref !== "a") queueMicrotask(() => {
      host.terminal.get(ref)!();
      if (ref === "d") host.terminal.get("a")!();
    });
    return { terminal, dispose: async () => { host.agents.delete(agent.id); } };
  },
  dispose: async () => { for (const finish of host.terminal.values()) finish(); },
}) }));

import { createBuildControl } from "../src/build-control.ts";

let root: string | undefined;
afterEach(() => {
  if (root) rmSync(root, { recursive: true, force: true });
  root = undefined;
  host.agents.clear(); host.launches = []; host.observations = []; host.terminal.clear(); host.step = 0;
});

describe("DSH dispatch timing through the real drive loop", () => {
  it("observes initial capacity and terminal refills without consuming still-live replayed handoffs", async () => {
    root = mkdtempSync(path.join(tmpdir(), "ub-dsh-timing-loop-"));
    const parent = { id: "parent", options: {} };
    host.agents.set(parent.id, parent);
    const context = { agents: { get: (id: string) => host.agents.get(id), list: () => [...host.agents.values()] } };
    const control = createBuildControl(context as never, { executable: process.execPath,
      driverRoot: root, maxOutputTokens: 1000, safetyMarginTokens: 100, handoffsPerCall: 4 }, "executor");
    try {
      await control.run(parent as never, new AbortController().signal, "invocation", false);
      expect(host.launches).toEqual(["a", "b", "c", "d"]);
      expect(host.observations.slice(0, 2)).toEqual(["b", "c"]);
      const timingDirectory = path.join(root, "dispatch-timing", "handoffs");
      const timings = readdirSync(timingDirectory).map(file => JSON.parse(readFileSync(path.join(timingDirectory, file), "utf8")));
      expect(timings).toHaveLength(4);
      const byRef = Object.fromEntries(timings.map(item => [item.opaque_handoff_ref, item]));
      expect(byRef.a.vacancy.source).toBe("initial_capacity");
      expect(byRef.b.vacancy.source).toBe("initial_capacity");
      expect(byRef.c.vacancy).toMatchObject({ source: "terminal_refill", previous_handoff_ref: "b" });
      expect(byRef.d.vacancy).toMatchObject({ source: "terminal_refill", previous_handoff_ref: "c" });
      expect(timings.every(item => Date.parse(item.ready_at) >= Date.parse(item.vacancy.observed_at))).toBe(true);
      expect([...host.agents.values()]).toEqual([parent]);
    } finally { await control.dispose(); }
  });
});
