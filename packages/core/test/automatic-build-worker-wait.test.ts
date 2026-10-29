import { existsSync, mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import {
  AUTOMATIC_BUILD_WORKER_WAIT_WINDOW_MS,
  automaticBuildWorkerNeedsContextRotation,
  readAutomaticBuildWorker,
  recordAutomaticBuildWorkerUsage,
  registerAutomaticBuildWorker,
  saveAutomaticBuildWorker,
  waitForAutomaticBuildWorkerAssignment,
  withAutomaticBuildWorkerSchedulerLock,
  type AutomaticBuildWorkerClaim,
} from "../src/automatic-build-worker";

const invocationRef = `abinv1_${"a".repeat(64)}`;
const rootRef = `abhandoff1_${"b".repeat(64)}`;
const peerRef = `abhandoff1_${"c".repeat(64)}`;
const nextRef = `abhandoff1_${"d".repeat(64)}`;
let directory: string;

beforeEach(() => {
  directory = mkdtempSync(path.join(tmpdir(), "worker-wait-"));
  vi.stubEnv("UNDERSTAND_BOOK_AUTOMATIC_BUILD_DRIVER_ROOT", directory);
  vi.useFakeTimers();
  for (const ref of [rootRef, peerRef]) registerAutomaticBuildWorker({
    invocation_ref: invocationRef, opaque_handoff_ref: ref,
    dispatch_slot_ref: `abdispatchslot1_${ref.slice(-64)}`, stage: "book_structure",
  });
});

afterEach(() => {
  vi.useRealTimers();
  vi.unstubAllEnvs();
  rmSync(directory, { recursive: true, force: true });
});

describe("worker wait inside the executor tool", () => {
  it("returns ready work immediately without scheduling a timer", async () => {
    const claim = vi.fn(async (): Promise<AutomaticBuildWorkerClaim> => ({ kind: "ASSIGNMENT", opaque_handoff_ref: nextRef }));
    const started = Date.now();
    expect(await waitForAutomaticBuildWorkerAssignment(rootRef, claim)).toEqual({ kind: "ASSIGNMENT", opaque_handoff_ref: nextRef });
    expect(Date.now()).toBe(started);
    expect(vi.getTimerCount()).toBe(0);
    expect(claim).toHaveBeenCalledTimes(1);
  });

  it("wakes on a peer submission before its dispatch completes and leaves waiting outside the scheduler lock", async () => {
    const claim = vi.fn(() => withAutomaticBuildWorkerSchedulerLock(invocationRef, async (): Promise<AutomaticBuildWorkerClaim> =>
      readAutomaticBuildWorker(peerRef)!.submitted_output_bytes > 0
        ? { kind: "ASSIGNMENT", opaque_handoff_ref: nextRef }
        : { kind: "WAIT", retry_after_ms: 1_000 }));
    const waiting = waitForAutomaticBuildWorkerAssignment(rootRef, claim);
    await vi.advanceTimersByTimeAsync(0);
    expect(claim).toHaveBeenCalledTimes(1);
    expect(existsSync(path.join(directory, "worker-scheduler", `${invocationRef}.json.lock`))).toBe(false);
    let siblingEntered = false;
    await withAutomaticBuildWorkerSchedulerLock(invocationRef, () => { siblingEntered = true; });
    expect(siblingEntered).toBe(true);
    await vi.advanceTimersByTimeAsync(999);
    expect(claim).toHaveBeenCalledTimes(1);
    recordAutomaticBuildWorkerUsage(peerRef, { submission_ref: "accepted-step", output_bytes: 128 });
    await vi.advanceTimersByTimeAsync(1);
    expect(await waiting).toEqual({ kind: "ASSIGNMENT", opaque_handoff_ref: nextRef });
    expect(claim).toHaveBeenCalledTimes(2);
    expect(readAutomaticBuildWorker(rootRef)!.wait_responses).toBe(0);
  });

  it("waits at most twenty seconds and rechecks the route at the boundary without routing every progress poll", async () => {
    const claim = vi.fn(async (): Promise<AutomaticBuildWorkerClaim> => ({ kind: "WAIT", retry_after_ms: 1_000 }));
    const waiting = waitForAutomaticBuildWorkerAssignment(rootRef, claim);
    await vi.advanceTimersByTimeAsync(AUTOMATIC_BUILD_WORKER_WAIT_WINDOW_MS - 1);
    expect(claim).toHaveBeenCalledTimes(1);
    await vi.advanceTimersByTimeAsync(1);
    expect(await waiting).toEqual({ kind: "WAIT", retry_after_ms: 1_000 });
    expect(claim).toHaveBeenCalledTimes(2);
    expect(vi.getTimerCount()).toBe(0);
    // No WAIT has been emitted to a model by this internal scheduling helper.
    expect(readAutomaticBuildWorker(rootRef)!.wait_response_bytes).toBe(0);
  });

  it("retries a settled-dispatch race after fifty milliseconds without requiring peer progress", async () => {
    const claim = vi.fn<() => Promise<AutomaticBuildWorkerClaim>>()
      .mockResolvedValueOnce({ kind: "WAIT", retry_after_ms: 50 })
      .mockResolvedValueOnce({ kind: "ASSIGNMENT", opaque_handoff_ref: nextRef });
    const waiting = waitForAutomaticBuildWorkerAssignment(rootRef, claim);
    await vi.advanceTimersByTimeAsync(49);
    expect(claim).toHaveBeenCalledTimes(1);
    await vi.advanceTimersByTimeAsync(1);
    expect(await waiting).toEqual({ kind: "ASSIGNMENT", opaque_handoff_ref: nextRef });
    expect(claim).toHaveBeenCalledTimes(2);
  });

  it("observes a terminal worker without claiming further work", async () => {
    const claim = vi.fn(async (): Promise<AutomaticBuildWorkerClaim> => ({ kind: "WAIT", retry_after_ms: 1_000 }));
    const waiting = waitForAutomaticBuildWorkerAssignment(rootRef, claim);
    await vi.advanceTimersByTimeAsync(0);
    const worker = readAutomaticBuildWorker(rootRef)!;
    worker.status = "done";
    worker.terminal_reason = "needs_user";
    saveAutomaticBuildWorker(worker);
    await vi.advanceTimersByTimeAsync(1_000);
    expect(await waiting).toEqual({ kind: "DONE", reason: "needs_user" });
    expect(claim).toHaveBeenCalledTimes(1);
  });

  it("counts each emitted wait control turn in the same context budget", () => {
    expect(automaticBuildWorkerNeedsContextRotation(readAutomaticBuildWorker(rootRef)!)).toBe(false);
    for (let turn = 0; turn < 61; turn++) recordAutomaticBuildWorkerUsage(rootRef, { wait_response_bytes: 128 });
    const worker = readAutomaticBuildWorker(rootRef)!;
    expect(worker.wait_responses).toBe(61);
    expect(worker.wait_response_bytes).toBe(61 * 128);
    expect(worker.delivered_input_bytes).toBe(0);
    expect(worker.submitted_output_bytes).toBe(0);
    expect(automaticBuildWorkerNeedsContextRotation(worker)).toBe(true);
  });
});
