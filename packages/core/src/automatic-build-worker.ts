import { closeSync, existsSync, mkdirSync, openSync, readFileSync, readdirSync, renameSync, unlinkSync, writeFileSync } from "node:fs";
import path from "node:path";
import type { AutomaticBuildStage } from "./build-orchestrator";
import { resolveAutomaticBuildExecutorRegistryRoot } from "./automatic-build-executor-session";

export type AutomaticBuildWorkerTerminalReason = "context_rotation" | "stage_complete" | "needs_user" | "complete" | "failed" | "child_exited";
export interface AutomaticBuildWorkerRecord {
  version: "automatic_build_worker.v1";
  root_handoff_ref: string;
  invocation_ref: string;
  stage: AutomaticBuildStage;
  current_handoff_ref: string;
  dispatch_slot_ref: string;
  status: "active" | "waiting" | "done";
  terminal_reason?: AutomaticBuildWorkerTerminalReason;
  assignments: string[];
  completed_assignments: string[];
  delivered_input_bytes: number;
  submitted_output_bytes: number;
  wait_response_bytes?: number;
  wait_responses?: number;
  deliveries: Record<string, number>;
  submissions: Record<string, number>;
  context_byte_limit: number;
}

export type AutomaticBuildWorkerClaim =
  | { kind: "ASSIGNMENT"; opaque_handoff_ref: string }
  | { kind: "WAIT"; retry_after_ms: number }
  | { kind: "DONE"; reason: AutomaticBuildWorkerTerminalReason };

// A conservative transport-byte budget, independent of dispatch size. One byte
// per token and a reserve for tool instructions/reasoning avoid relying on an
// unpublished Codex model context size. Large first assignments retain the
// existing input contract, then rotate at their committed boundary.
export const AUTOMATIC_BUILD_WORKER_CONTEXT_BYTE_LIMIT = 196_608;
export const AUTOMATIC_BUILD_WORKER_NEXT_ASSIGNMENT_RESERVE = 65_536;
export const AUTOMATIC_BUILD_WORKER_WAIT_WINDOW_MS = 20_000;
export const AUTOMATIC_BUILD_WORKER_REOPEN_DELAY_MS = 1_000;
const WORKER_PROGRESS_POLL_MS = 1_000;

function registryFile(directory: string, ref: string): string {
  if (!/^(?:abhandoff1|abinv1)_[a-f0-9]{64}$/u.test(ref)) throw new Error("invalid worker identity");
  return path.join(resolveAutomaticBuildExecutorRegistryRoot(), directory, `${ref}.json`);
}
function read<T>(file: string): T | undefined {
  try { return JSON.parse(readFileSync(file, "utf8")) as T; }
  catch (error) { if ((error as NodeJS.ErrnoException).code === "ENOENT") return undefined; throw error; }
}
function save(file: string, value: unknown): void {
  mkdirSync(path.dirname(file), { recursive: true });
  const temporary = `${file}.${process.pid}.tmp`;
  writeFileSync(temporary, JSON.stringify(value));
  renameSync(temporary, file);
}

export function readAutomaticBuildWorker(rootRef: string): AutomaticBuildWorkerRecord | undefined {
  return read<AutomaticBuildWorkerRecord>(registryFile("workers", rootRef));
}
export function readAutomaticBuildWorkerForAssignment(rawRef: string): AutomaticBuildWorkerRecord | undefined {
  const index = read<{ root_handoff_ref: string }>(registryFile("worker-assignments", rawRef));
  return index ? readAutomaticBuildWorker(index.root_handoff_ref) : undefined;
}
export function listAutomaticBuildWorkers(invocationRef: string): AutomaticBuildWorkerRecord[] {
  const directory = path.dirname(registryFile(path.join("invocation-workers", invocationRef), invocationRef));
  if (!existsSync(directory)) return [];
  return readdirSync(directory).filter(file => file.endsWith(".json"))
    .map(file => readAutomaticBuildWorker(file.slice(0, -5))).filter((record): record is AutomaticBuildWorkerRecord => Boolean(record));
}

/** Called while the invocation scheduler lock is held. */
export function saveAutomaticBuildWorker(worker: AutomaticBuildWorkerRecord): void {
  save(registryFile("workers", worker.root_handoff_ref), worker);
}
export function registerAutomaticBuildWorker(input: {
  invocation_ref: string; stage: AutomaticBuildStage; opaque_handoff_ref: string; dispatch_slot_ref: string;
}): AutomaticBuildWorkerRecord {
  const existing = readAutomaticBuildWorkerForAssignment(input.opaque_handoff_ref);
  if (existing) return existing;
  const worker: AutomaticBuildWorkerRecord = {
    version: "automatic_build_worker.v1", root_handoff_ref: input.opaque_handoff_ref,
    invocation_ref: input.invocation_ref, stage: input.stage, current_handoff_ref: input.opaque_handoff_ref,
    dispatch_slot_ref: input.dispatch_slot_ref, status: "active", assignments: [input.opaque_handoff_ref],
    completed_assignments: [], delivered_input_bytes: 0, submitted_output_bytes: 0,
    wait_response_bytes: 0, wait_responses: 0,
    deliveries: {}, submissions: {}, context_byte_limit: AUTOMATIC_BUILD_WORKER_CONTEXT_BYTE_LIMIT,
  };
  saveAutomaticBuildWorker(worker);
  save(registryFile(path.join("invocation-workers", input.invocation_ref), worker.root_handoff_ref), {});
  save(registryFile("worker-assignments", input.opaque_handoff_ref), { root_handoff_ref: worker.root_handoff_ref });
  return worker;
}
export function assignAutomaticBuildWorker(worker: AutomaticBuildWorkerRecord, rawRef: string): AutomaticBuildWorkerRecord {
  const owner = readAutomaticBuildWorkerForAssignment(rawRef);
  if (owner && owner.root_handoff_ref !== worker.root_handoff_ref) throw new Error("assignment already belongs to another worker");
  worker.current_handoff_ref = rawRef;
  worker.status = "active";
  if (!worker.assignments.includes(rawRef)) worker.assignments.push(rawRef);
  saveAutomaticBuildWorker(worker);
  save(registryFile("worker-assignments", rawRef), { root_handoff_ref: worker.root_handoff_ref });
  return worker;
}
export function finishAutomaticBuildWorker(worker: AutomaticBuildWorkerRecord, reason: AutomaticBuildWorkerTerminalReason): AutomaticBuildWorkerClaim {
  worker.status = "done";
  worker.terminal_reason = reason;
  saveAutomaticBuildWorker(worker);
  return { kind: "DONE", reason };
}
export function recordAutomaticBuildWorkerUsage(rootRef: string, usage: {
  delivery_ref?: string; input_bytes?: number; submission_ref?: string; output_bytes?: number;
  wait_response_bytes?: number;
  wait_response_count?: number;
}): void {
  const worker = readAutomaticBuildWorker(rootRef);
  if (!worker) return;
  if (usage.delivery_ref && !Object.hasOwn(worker.deliveries, usage.delivery_ref)) {
    const bytes = usage.input_bytes ?? 0;
    if (!Number.isSafeInteger(bytes) || bytes < 0) throw new Error("invalid worker input usage");
    worker.deliveries[usage.delivery_ref] = bytes;
    worker.delivered_input_bytes += bytes;
  }
  if (usage.submission_ref && !Object.hasOwn(worker.submissions, usage.submission_ref)) {
    const bytes = usage.output_bytes ?? 0;
    if (!Number.isSafeInteger(bytes) || bytes < 0) throw new Error("invalid worker output usage");
    worker.submissions[usage.submission_ref] = bytes;
    worker.submitted_output_bytes += bytes;
  }
  // Each emitted WAIT is another model-visible control turn, even when it
  // repeats the same open request. Internal progress checks never call this.
  if (usage.wait_response_bytes !== undefined) {
    const bytes = usage.wait_response_bytes;
    const count = usage.wait_response_count ?? 1;
    if (!Number.isSafeInteger(bytes) || bytes < 0 || !Number.isSafeInteger(count) || count < 1) {
      throw new Error("invalid worker wait usage");
    }
    worker.wait_response_bytes = (worker.wait_response_bytes ?? 0) + bytes;
    worker.wait_responses = (worker.wait_responses ?? 0) + count;
  }
  saveAutomaticBuildWorker(worker);
}
export function automaticBuildWorkerNeedsContextRotation(worker: AutomaticBuildWorkerRecord,
  nextAssignmentBytes = AUTOMATIC_BUILD_WORKER_NEXT_ASSIGNMENT_RESERVE): boolean {
  const protocolAndReasoningReserve = (Object.keys(worker.submissions).length + (worker.wait_responses ?? 0)) * 2_048;
  return worker.delivered_input_bytes + worker.submitted_output_bytes + (worker.wait_response_bytes ?? 0) + protocolAndReasoningReserve
    + Math.max(AUTOMATIC_BUILD_WORKER_NEXT_ASSIGNMENT_RESERVE, nextAssignmentBytes) >= worker.context_byte_limit;
}

function workerProgress(worker: AutomaticBuildWorkerRecord) {
  return {
    root_handoff_ref: worker.root_handoff_ref,
    current_handoff_ref: worker.current_handoff_ref,
    completed_assignments: worker.completed_assignments.length,
    submitted_output_bytes: worker.submitted_output_bytes,
    status: worker.status,
  };
}

/** Poll cheap peer records outside the scheduler lock; route again only on progress. */
export async function waitForAutomaticBuildWorkerAssignment(
  rootRef: string,
  claim: () => Promise<AutomaticBuildWorkerClaim>,
): Promise<AutomaticBuildWorkerClaim> {
  const worker = readAutomaticBuildWorker(rootRef);
  if (!worker) throw new Error("automatic build worker is not registered");
  const deadline = Date.now() + AUTOMATIC_BUILD_WORKER_WAIT_WINDOW_MS;
  const snapshot = () => listAutomaticBuildWorkers(worker.invocation_ref)
    .filter(peer => peer.root_handoff_ref !== rootRef && peer.status !== "done").map(workerProgress);
  let observed = snapshot();
  let result = await claim();
  while (result.kind === "WAIT" && Date.now() < deadline) {
    const retryAfterMs = result.retry_after_ms;
    const settledRetry = retryAfterMs <= 50;
    await new Promise(resolve => setTimeout(resolve, Math.min(
      settledRetry ? Math.max(1, retryAfterMs) : WORKER_PROGRESS_POLL_MS,
      deadline - Date.now(),
    )));
    const currentWorker = readAutomaticBuildWorker(rootRef);
    if (currentWorker?.status === "done") return { kind: "DONE", reason: currentWorker.terminal_reason! };
    const changed = observed.some(previous => {
      const peer = readAutomaticBuildWorker(previous.root_handoff_ref);
      return !peer || peer.current_handoff_ref !== previous.current_handoff_ref
        || peer.completed_assignments.length !== previous.completed_assignments
        || peer.submitted_output_bytes !== previous.submitted_output_bytes || peer.status !== previous.status;
    });
    // A final route also discovers newly registered peers or external changes
    // that are not represented in this call's initial peer set.
    if (settledRetry || changed || Date.now() >= deadline) {
      observed = snapshot();
      result = await claim();
    }
  }
  return result;
}

/** Serialize only route/selection/ownership publication; generation never holds it. */
export async function withAutomaticBuildWorkerSchedulerLock<T>(invocationRef: string,
  run: () => T | Promise<T>, signal?: AbortSignal, onContention?: () => T): Promise<T> {
  const lock = `${registryFile("worker-scheduler", invocationRef)}.lock`;
  mkdirSync(path.dirname(lock), { recursive: true });
  for (;;) {
    signal?.throwIfAborted();
    try {
      const descriptor = openSync(lock, "wx");
      try { writeFileSync(descriptor, String(process.pid)); }
      catch (error) { closeSync(descriptor); unlinkSync(lock); throw error; }
      closeSync(descriptor);
      break;
    } catch (error) {
      if ((error as NodeJS.ErrnoException).code !== "EEXIST") throw error;
      try {
        const pid = Number(readFileSync(lock, "utf8"));
        if (pid > 0) {
          try { process.kill(pid, 0); }
          catch (error) { if ((error as NodeJS.ErrnoException).code === "ESRCH") { unlinkSync(lock); continue; } }
        }
      } catch (error) {
        // The owner can release its lock before this PID read opens it. Windows
        // may report a delete-pending file as EPERM/EACCES instead of ENOENT.
        // Keep waiting; only a successful exclusive create grants ownership.
        const code = (error as NodeJS.ErrnoException).code;
        if (code !== "ENOENT" && code !== "EPERM" && code !== "EACCES") throw error;
      }
      if (onContention) return onContention();
      await new Promise(resolve => setTimeout(resolve, 25));
    }
  }
  try { return await run(); }
  finally { unlinkSync(lock); }
}
