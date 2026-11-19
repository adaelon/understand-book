import * as fs from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { afterEach, describe, expect, it, vi } from "vitest";
import { claimAutomaticBuildTask } from "../src/automatic-build-lease";
import { failAutomaticBuildTask } from "../src/automatic-build-mailbox";
import { readAutomaticBuildAttemptRecord, readAutomaticBuildExecutionIdentity, nextAutomaticBuildExecutionIdentity } from "../src/automatic-build-task-store";
import { resolveAutomaticBuildTarget } from "../src/build-orchestrator";
import { createAutomaticBuildFailureDiagnostic } from "../src/extractor-contract";
import { profileSidecarPolicyScopeFixture } from "./fixtures/profile-sidecar-contract-drift/policy-scope";

vi.mock("node:fs", async importOriginal => {
  const actual = await importOriginal<typeof import("node:fs")>();
  return { ...actual, writeFileSync: vi.fn(actual.writeFileSync) };
});
afterEach(() => vi.restoreAllMocks());

function fixture() {
  const root = fs.mkdtempSync(path.join(tmpdir(), "ub-terminal-publication-"));
  const source = path.join(root, "guide.md");
  fs.writeFileSync(source, "# Guide\n\nA compact paragraph.\n");
  const target = resolveAutomaticBuildTarget(source, root);
  const policy = profileSidecarPolicyScopeFixture(target);
  const claim = claimAutomaticBuildTask(target, policy.descriptor.stage, policy.descriptor.work_unit_id, {
    owner: "failure-publisher", now: "2026-10-22T00:00:00.000Z", descriptor: policy.descriptor,
    binding: policy.scope_a.task_binding, policy_generation: "v3_only", max_semantic_attempts: 3,
  });
  if (claim.status !== "leased") throw new Error("expected leased fixture");
  return { root, target, policy, claim };
}

describe("terminal failure publication recovery", () => {
  it.each([false, true])("reconciles a published receipt before its event, publisher interrupted=%s", async interrupted => {
    const { root, target, policy, claim } = fixture();
    const actual = await vi.importActual<typeof import("node:fs")>("node:fs");
    const resultPath = path.join(path.dirname(claim.lease_ref), "result.json");
    let observedFailures: number | undefined, eventBeforePublisher: Buffer | undefined;
    const stop = new Error("publisher stopped after durable failure receipt");
    let entered = false;
    vi.mocked(fs.writeFileSync).mockImplementation((file, data, options) => {
      if (String(file) !== resultPath || entered) return actual.writeFileSync(file, data, options);
      entered = true;
      if (interrupted) throw stop;
      expect(readAutomaticBuildExecutionIdentity(target, policy.descriptor.stage, policy.descriptor.work_unit_id))
        .toMatchObject({ semantic_attempt: 1, lease_epoch: 1 });
      observedFailures = readAutomaticBuildAttemptRecord(target, policy.descriptor.stage, policy.descriptor.work_unit_id)?.failures;
      eventBeforePublisher = fs.readFileSync(resultPath);
      return actual.writeFileSync(file, data, options);
    });
    const fail = () => failAutomaticBuildTask(target, claim.lease_ref, claim.lease.token, {
      failure_diagnostic: createAutomaticBuildFailureDiagnostic({ category: "provider", code: "provider_timeout" }),
      now: "2026-10-22T00:00:01.000Z",
    });
    try {
      if (interrupted) expect(fail).toThrow(stop);
      else expect(fail().state).toBe("retryable_failure");
      const receiptPath = path.join(path.dirname(claim.lease_ref), "failure.json");
      const receiptBeforeRead = fs.readFileSync(receiptPath);
      expect(readAutomaticBuildAttemptRecord(target, policy.descriptor.stage, policy.descriptor.work_unit_id))
        .toMatchObject({ failures: 1, semantic_attempt: 1, lease_epoch: 1 });
      expect(nextAutomaticBuildExecutionIdentity(target, policy.descriptor.stage, policy.descriptor.work_unit_id, {
        max_semantic_attempts: 3, max_lease_epochs: 3, attempt_scope: policy.scope_a,
      })).toMatchObject({ status: "ready", execution_identity: { semantic_attempt: 2, lease_epoch: 1 } });
      expect(fs.readFileSync(receiptPath)).toEqual(receiptBeforeRead);
      const event = fs.readFileSync(resultPath);
      if (!interrupted) { expect(observedFailures).toBe(1); expect(event).toEqual(eventBeforePublisher); }
      expect(readAutomaticBuildAttemptRecord(target, policy.descriptor.stage, policy.descriptor.work_unit_id)?.failures).toBe(1);
      expect(fs.readFileSync(resultPath)).toEqual(event);
    } finally { fs.rmSync(root, { recursive: true, force: true }); }
  });
});
