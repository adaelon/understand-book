import { createHash } from "node:crypto";
import { existsSync, mkdirSync, mkdtempSync, readFileSync, readdirSync, writeFileSync } from "node:fs";
import * as fs from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { describe, expect, it, vi } from "vitest";
import { issueAutomaticBuildOpaqueHandoff } from "../src/automatic-build-executor-session";
import { automaticBuildDriverFailureResponse } from "../../../skills/build/automatic-build-driver";
import {
  advanceAutomaticBuildDispatch,
  finishAutomaticBuildDispatch,
  finishAutomaticBuildDispatchIfTerminal,
  inspectAutomaticBuildDispatchRecoveryGeneration,
  inspectAutomaticBuildDispatchRecoveryGenerations,
  recordAutomaticBuildDispatchBootstrapFailure,
  recordAutomaticBuildDispatchTerminalSession,
  inspectAutomaticBuildDispatch,
  persistAutomaticBuildDispatch as persistAutomaticBuildDispatchRuntime,
  prepareAutomaticBuildDispatch,
  persistAutomaticBuildDispatchPlan,
  selectAutomaticBuildDispatchHandoff,
} from "../src/automatic-build-dispatch-runtime";
import { planAutomaticBuildExecutorDispatches } from "../src/automatic-build-dispatch";
import { claimAutomaticBuildTask, startAutomaticBuildLease } from "../src/automatic-build-lease";
import { failAutomaticBuildTask, submitAutomaticBuildCandidate } from "../src/automatic-build-mailbox";
import {
  createBookStructureExecutionContractsV2,
  routeBookStructureUnitWorkUnitsV2,
} from "../src/book-structure";
import {
  listAutomaticBuildStoredAttempts,
  readAutomaticBuildAttemptSnapshot,
} from "../src/automatic-build-task-store";
import { resolveAutomaticBuildTarget } from "../src/build-orchestrator";
import { resolveContentProfile } from "../src/content-profile";
import { CODEX_EXECUTOR_TRANSPORT_PROFILE_V2 } from "../src/executor-transport";
import { automaticBuildExtractionPolicy } from "../src/semantic-artifact";
import {
  buildWorkUnitCost,
  createWorkUnitDescriptor,
  taskPolicyBindingForWorkUnit,
} from "../src/stage-work-unit";

vi.mock("node:fs", async importOriginal => {
  const actual = await importOriginal<typeof import("node:fs")>();
  return { ...actual, openSync: vi.fn(actual.openSync), writeFileSync: vi.fn(actual.writeFileSync) };
});

function fixture(count = 8) {
  const root = mkdtempSync(path.join(tmpdir(), "understand-book-dispatch-runtime-"));
  const source = path.join(root, "guide.md");
  writeFileSync(source, "# Guide\n\nA deterministic source.\n", "utf8");
  const target = resolveAutomaticBuildTarget(source, root);
  const policy = automaticBuildExtractionPolicy(
    "profile_sidecar",
    resolveContentProfile("technical_learning"),
    "full",
  );
  const descriptors = Array.from({ length: count }, (_, index) => createWorkUnitDescriptor({
    target: target.target_ref,
    stage: "profile_sidecar",
    work_unit_id: `formula-${index}`,
    kind: "profile_sidecar_formula",
    input_hash: createHash("sha256").update(`input-${index}`).digest("hex"),
    policy_fingerprint: policy,
    evidence_lids: [`p${index + 1}`],
    cost: buildWorkUnitCost({
      estimated_input_tokens: 181,
      visible_lids: 1,
      formula_lids: 1,
      expected_output_items: 1,
    }),
  }));
  const plan = planAutomaticBuildExecutorDispatches({
    target_ref: target.target_ref,
    stage: "profile_sidecar",
    work_units: descriptors,
    pending_ids: descriptors.map((descriptor) => descriptor.work_unit_id),
    available_agent_slots: 1,
  });
  expect(plan.dispatches).toHaveLength(Math.ceil(count / 8));
  const bindings = Object.fromEntries(descriptors.map((descriptor) => [descriptor.work_unit_id, {
    input_hash: descriptor.input_hash,
    policy_fingerprint: descriptor.policy_fingerprint,
  }]));
  return { root, target, descriptors, plan, manifest: plan.dispatches[0], bindings };
}

function persistAutomaticBuildDispatch(
  target: Parameters<typeof persistAutomaticBuildDispatchRuntime>[0],
  manifest: Parameters<typeof persistAutomaticBuildDispatchRuntime>[1],
  options: Omit<Parameters<typeof persistAutomaticBuildDispatchRuntime>[2], "executor_handoff">,
): ReturnType<typeof persistAutomaticBuildDispatchRuntime> {
  const publication = prepareAutomaticBuildDispatch(target, manifest, options);
  const prompt = "# Runtime fixture executor prompt\nautomatic_build_dispatch_executor.v1\n";
  const handoff = {
    version: "automatic_build_dispatch_executor_handoff.v1",
    prompt_sha256: createHash("sha256").update(prompt, "utf8").digest("hex"),
    prompt,
    envelope: {
      version: "automatic_build_dispatch_executor.v1",
      manifest,
      dispatch_run_id: publication.prepared.dispatch_run_id,
      manifest_path: publication.manifest_path,
    },
  };
  const bytes = Buffer.from(`${JSON.stringify(handoff)}\n`, "utf8");
  const handoffPath = path.join(path.dirname(publication.manifest_path), "executor-handoff.json");
  mkdirSync(path.dirname(handoffPath), { recursive: true });
  writeFileSync(handoffPath, bytes, { flag: "wx" });
  return persistAutomaticBuildDispatchRuntime(target, manifest, {
    ...options,
    executor_handoff: {
      version: "automatic_build_dispatch_executor_handoff_ref.v1",
      path: handoffPath,
      sha256: createHash("sha256").update(bytes).digest("hex"),
      byte_length: bytes.byteLength,
    },
  });
}

function commit(
  target: ReturnType<typeof resolveAutomaticBuildTarget>,
  advance: Extract<ReturnType<typeof advanceAutomaticBuildDispatch>, { status: "leased" }>,
  ordinal: number,
) {
  const candidate = JSON.stringify({ ordinal, marker: `PRIVATE_FORMULA_${ordinal}` });
  const candidatePath = path.join(path.dirname(advance.claim.lease_ref), "candidate.json");
  writeFileSync(candidatePath, candidate, "utf8");
  const artifactPath = path.join(target.workspace_dir, ".build", "profile-sidecar", `${advance.descriptor.work_unit_id}.json`);
  const receipt = submitAutomaticBuildCandidate(
    target,
    advance.claim.lease_ref,
    advance.claim.lease.token,
    candidatePath,
    (sourcePath) => {
      mkdirSync(path.dirname(artifactPath), { recursive: true });
      writeFileSync(artifactPath, readFileSync(sourcePath));
      return { artifact_path: artifactPath, output_counts: { formula_semantics: 1 } };
    },
    { now: advance.claim.lease.issued_at, completed_at: advance.claim.lease.issued_at },
  );
  expect(receipt.candidate_sha256).toBe(createHash("sha256").update(candidate).digest("hex"));
  return receipt;
}

function seedSemanticFailure(
  target: ReturnType<typeof resolveAutomaticBuildTarget>,
  workUnitId: string,
  owner: string,
  claimedAt: string,
  failedAt: string,
) {
  const claim = claimAutomaticBuildTask(target, "profile_sidecar", workUnitId, {
    owner,
    now: claimedAt,
    reserve_ttl_ms: 60_000,
    max_semantic_attempts: 3,
  });
  if (claim.status !== "leased") throw new Error(`expected seed lease for ${workUnitId}`);
  return failAutomaticBuildTask(target, claim.lease_ref, claim.lease.token, {
    diagnostic_code: "semantic_invalid",
    now: failedAt,
  });
}

function workspaceFileSnapshot(root: string): Record<string, string> {
  const snapshot: Record<string, string> = {};
  const visit = (directory: string) => {
    for (const entry of readdirSync(directory, { withFileTypes: true })) {
      const file = path.join(directory, entry.name);
      if (entry.isDirectory()) {
        visit(file);
      } else if (entry.isFile()) {
        snapshot[path.relative(root, file)] = readFileSync(file).toString("base64");
      }
    }
  };
  visit(root);
  return snapshot;
}

describe("automatic build executor dispatch runtime", () => {
  it("skips work leased by an older accepted plan when refilling a new plan", () => {
    const { target, descriptors, plan, manifest, bindings } = fixture(16);
    const original = persistAutomaticBuildDispatch(target, manifest, {
      owner: "original-plan", created_at: "2026-09-02T01:00:00.000Z",
      reserve_ttl_ms: 60_000, run_ttl_ms: 1_800_000,
    }).persisted;
    expect(advanceAutomaticBuildDispatch(target, "profile_sidecar", manifest.dispatch_id, {
      now: "2026-09-02T01:00:00.100Z", dispatch_run_id: original.dispatch_run_id,
      descriptors, task_bindings: bindings,
    }).status).toBe("leased");
    const refill = selectAutomaticBuildDispatchHandoff(target, {
      accepted_plan_digest: "new-plan-with-additional-ready-work", current_dispatch_plan: plan,
      max_parallel: 3,
      available_new_executor_slots: 1, created_at: "2026-09-02T01:00:00.200Z",
    });
    expect(refill.selected_manifests).toEqual([plan.dispatches[1]]);
    // Once the original lease expires, that work becomes dispatchable again.
    const recovered = selectAutomaticBuildDispatchHandoff(target, {
      accepted_plan_digest: "new-plan-with-additional-ready-work", current_dispatch_plan: plan,
      max_parallel: 3,
      available_new_executor_slots: 1, created_at: "2026-09-02T02:00:00.000Z",
    });
    expect(recovered.selected_manifests).toEqual([plan.dispatches[0]]);
  });

  it("keeps a live executor's delivered batch occupied before generation.start claims its task", () => {
    const { target, plan, manifest } = fixture(16);
    persistAutomaticBuildDispatch(target, manifest, {
      owner: "receiving-input", created_at: "2026-09-02T01:00:00.000Z",
      reserve_ttl_ms: 60_000, run_ttl_ms: 1_800_000,
    });
    const held = selectAutomaticBuildDispatchHandoff(target, {
      accepted_plan_digest: "changed-ready-work-during-delivery", current_dispatch_plan: plan,
      max_parallel: 3,
      available_new_executor_slots: 1, created_at: "2026-09-02T01:00:00.200Z",
      held_work_unit_ids: manifest.ordered_work_unit_ids,
    });
    expect(held.selected_manifests).toEqual([plan.dispatches[1]]);
    // The same plan remains usable when the caller releases that executor.
    const released = selectAutomaticBuildDispatchHandoff(target, {
      accepted_plan_digest: "changed-ready-work-during-delivery", current_dispatch_plan: plan,
      max_parallel: 3,
      available_new_executor_slots: 1, created_at: "2026-09-02T01:01:00.000Z",
    });
    expect(released.selected_manifests).toEqual([plan.dispatches[0]]);
  });

  it.each(["held", "leased"] as const)("reports waiting when all work belongs to another live run (%s)", (ownership) => {
    const { target, descriptors, plan, manifest, bindings } = fixture(1);
    const original = persistAutomaticBuildDispatch(target, manifest, {
      owner: "original-plan", created_at: "2026-09-02T01:00:00.000Z",
      reserve_ttl_ms: 60_000, run_ttl_ms: 1_800_000,
    }).persisted;
    if (ownership === "leased") {
      expect(advanceAutomaticBuildDispatch(target, "profile_sidecar", manifest.dispatch_id, {
        now: "2026-09-02T01:00:00.100Z", dispatch_run_id: original.dispatch_run_id,
        descriptors, task_bindings: bindings,
      }).status).toBe("leased");
    }
    const refill = selectAutomaticBuildDispatchHandoff(target, {
      accepted_plan_digest: "changed-ready-work", current_dispatch_plan: plan,
      max_parallel: 3,
      available_new_executor_slots: 2, created_at: "2026-09-02T01:00:00.200Z",
      ...(ownership === "held" ? { held_work_unit_ids: manifest.ordered_work_unit_ids } : {}),
    });
    expect(refill.selected_manifests).toEqual([]);
    expect(refill.active_dispatch_ids).toEqual([]);
    expect(refill.waiting_reason).toBe(ownership === "held" ? "active_executors" : "active_leases");
    const released = selectAutomaticBuildDispatchHandoff(target, {
      accepted_plan_digest: "changed-ready-work", current_dispatch_plan: plan,
      max_parallel: 3,
      available_new_executor_slots: 2, created_at: "2026-09-02T02:00:00.000Z",
    });
    expect(released.selected_manifests).toEqual([manifest]);
    expect(released.waiting_reason).toBeUndefined();
  });

  it("R2 preserves an active lease when a stale unopened correction is reported", () => {
    const { target, descriptors, manifest, bindings } = fixture();
    const persisted = persistAutomaticBuildDispatch(target, manifest, {
      owner: `r2-lease:${manifest.dispatch_id}`, created_at: "2026-09-02T01:00:00.000Z",
      reserve_ttl_ms: 60_000, run_ttl_ms: 1_800_000,
    }).persisted;
    const options = { now: "2026-09-02T01:00:00.200Z", dispatch_run_id: persisted.dispatch_run_id };
    const initial = inspectAutomaticBuildDispatchRecoveryGeneration(target, "profile_sidecar", manifest.dispatch_id, options);
    expect(advanceAutomaticBuildDispatch(target, "profile_sidecar", manifest.dispatch_id, {
      ...options, descriptors, task_bindings: bindings,
    }).status).toBe("leased");
    const before = workspaceFileSnapshot(target.workspace_dir);
    recordAutomaticBuildDispatchBootstrapFailure(target, "profile_sidecar", initial.recovery_identity,
      `abhandoff1_${"a".repeat(64)}`, options.now);
    expect(workspaceFileSnapshot(target.workspace_dir)).toEqual(before);
    expect(inspectAutomaticBuildDispatchRecoveryGeneration(target, "profile_sidecar", manifest.dispatch_id, options))
      .toEqual(initial);
  });

  it.each([false, true])("skips unpublished obsolete batches and retains pending work (partial=%s)", (partial) => {
    const { target, descriptors, plan } = fixture(16);
    const accepted = "same-accepted-descriptor-plan";
    persistAutomaticBuildDispatchPlan(target, accepted, plan, "2026-09-12T13:34:43.325Z");
    // The current snapshot has already validated the completed work. The old batch was never published.
    const pending = descriptors.slice(partial ? 1 : 8);
    const current = planAutomaticBuildExecutorDispatches({
      target_ref: target.target_ref, stage: "profile_sidecar", work_units: descriptors,
      pending_ids: pending.map(unit => unit.work_unit_id), available_agent_slots: 1,
    });
    const selected = selectAutomaticBuildDispatchHandoff(target, {
      accepted_plan_digest: accepted, current_dispatch_plan: current,
      max_parallel: 3,
      available_new_executor_slots: 1, created_at: "2026-09-12T13:38:08.000Z",
    });
    expect(selected.selected_manifests).not.toContainEqual(plan.dispatches[0]);
    expect(selected.selected_manifests.flatMap(item => item.ordered_work_unit_ids))
      .not.toContain(descriptors[0].work_unit_id);
    expect(selected.completed_dispatch_ids).toContain(plan.dispatches[0].dispatch_id);
    expect(selected.selected_manifests).toContainEqual(plan.dispatches[1]);
    const remaining = partial ? descriptors.slice(1, 8) : [];
    const nextPlan = planAutomaticBuildExecutorDispatches({
      target_ref: target.target_ref, stage: "profile_sidecar", work_units: descriptors,
      pending_ids: remaining.map(unit => unit.work_unit_id), available_agent_slots: 1,
    });
    const next = selectAutomaticBuildDispatchHandoff(target, {
      accepted_plan_digest: accepted, current_dispatch_plan: nextPlan,
      max_parallel: 3,
      available_new_executor_slots: 1, created_at: "2026-09-12T13:39:08.000Z",
    });
    expect(next.selected_manifests.flatMap(item => item.ordered_work_unit_ids))
      .toEqual(remaining.map(unit => unit.work_unit_id));
  });

  it("projects initial, active, and expired recovery generations without writing workspace state", () => {
    const { target, descriptors, manifest, bindings } = fixture();
    const persisted = persistAutomaticBuildDispatch(target, manifest, {
      owner: `dispatch-recovery-projection:${manifest.dispatch_id}`,
      created_at: "2026-09-02T01:00:00.000Z",
      reserve_ttl_ms: 1_000,
      run_ttl_ms: 1_800_000,
    }).persisted;
    const project = (now: string) => {
      const before = workspaceFileSnapshot(target.workspace_dir);
      const projection = inspectAutomaticBuildDispatchRecoveryGeneration(
        target,
        "profile_sidecar",
        manifest.dispatch_id,
        { now, dispatch_run_id: persisted.dispatch_run_id },
      );
      expect(workspaceFileSnapshot(target.workspace_dir)).toEqual(before);
      return projection;
    };

    const initial = project("2026-09-02T01:00:00.100Z");
    expect(initial.recovery_identity).toMatchObject({
      version: "automatic_build_recovery_generation_identity.v1",
      dispatch_id: manifest.dispatch_id,
      dispatch_run_id: persisted.dispatch_run_id,
      current_work_unit_id: manifest.ordered_work_unit_ids[0],
      semantic_attempt: 1,
      lease_epoch: 1,
    });

    const claim = advanceAutomaticBuildDispatch(target, "profile_sidecar", manifest.dispatch_id, {
      descriptors,
      task_bindings: bindings,
      dispatch_run_id: persisted.dispatch_run_id,
      now: "2026-09-02T01:00:00.200Z",
    });
    if (claim.status !== "leased") throw new Error("expected active recovery projection lease");
    expect(project("2026-09-02T01:00:00.500Z").recovery_identity).toMatchObject({
      semantic_attempt: 1,
      lease_epoch: 1,
    });
    expect(project("2026-09-02T01:00:01.200Z").recovery_identity).toMatchObject({
      semantic_attempt: 1,
      lease_epoch: 2,
    });
  });

  it("snapshots three ordered recovery identities and removes committed work without writing state", () => {
    const { target, descriptors, manifest, bindings } = fixture(3);
    const persisted = persistAutomaticBuildDispatch(target, manifest, {
      owner: "bounded-recovery-snapshot", created_at: "2026-10-28T01:00:00.000Z",
      reserve_ttl_ms: 60_000, run_ttl_ms: 1_800_000,
    }).persisted;
    const options = { now: "2026-10-28T01:00:00.100Z", dispatch_run_id: persisted.dispatch_run_id };
    const project = () => {
      const before = workspaceFileSnapshot(target.workspace_dir);
      const projection = inspectAutomaticBuildDispatchRecoveryGenerations(target, "profile_sidecar", manifest.dispatch_id, options);
      expect(workspaceFileSnapshot(target.workspace_dir)).toEqual(before);
      return projection;
    };
    const initial = project();
    expect(initial.identities).toEqual(manifest.ordered_work_unit_ids.map(current_work_unit_id => ({
      version: "automatic_build_recovery_generation_identity.v1", dispatch_id: manifest.dispatch_id,
      dispatch_run_id: persisted.dispatch_run_id, current_work_unit_id, semantic_attempt: 1, lease_epoch: 1,
    })));
    expect(inspectAutomaticBuildDispatchRecoveryGeneration(target, "profile_sidecar", manifest.dispatch_id, options))
      .toMatchObject({ dispatch_slot_ref: initial.dispatch_slot_ref, recovery_identity: initial.identities[0] });
    expect(listAutomaticBuildStoredAttempts(target, "profile_sidecar")).toEqual([]);

    const claimed = advanceAutomaticBuildDispatch(target, "profile_sidecar", manifest.dispatch_id, {
      ...options, descriptors, task_bindings: bindings,
    });
    if (claimed.status !== "leased") throw new Error("expected first snapshot lease");
    commit(target, claimed, 0);
    const remaining = project();
    expect(remaining.dispatch_slot_ref).toBe(initial.dispatch_slot_ref);
    expect(remaining.identities).toEqual(initial.identities.slice(1));
    expect(initial.identities).toHaveLength(3);
  });

  it("snapshots bootstrap, running, expired and semantic retry identities without claiming the suffix", () => {
    const { target, descriptors, manifest, bindings } = fixture(3);
    seedSemanticFailure(target, manifest.ordered_work_unit_ids[1], "future-semantic-retry",
      "2026-10-28T01:10:00.000Z", "2026-10-28T01:10:00.100Z");
    const persisted = persistAutomaticBuildDispatch(target, manifest, {
      owner: "bounded-recovery-epochs", created_at: "2026-10-28T01:10:01.000Z",
      reserve_ttl_ms: 1_000, run_ttl_ms: 1_000,
    }).persisted;
    const project = (now: string) => {
      const before = workspaceFileSnapshot(target.workspace_dir);
      const projection = inspectAutomaticBuildDispatchRecoveryGenerations(target, "profile_sidecar", manifest.dispatch_id,
        { now, dispatch_run_id: persisted.dispatch_run_id });
      expect(workspaceFileSnapshot(target.workspace_dir)).toEqual(before);
      expect(projection.identities[0]).toEqual(inspectAutomaticBuildDispatchRecoveryGeneration(
        target, "profile_sidecar", manifest.dispatch_id, { now, dispatch_run_id: persisted.dispatch_run_id },
      ).recovery_identity);
      return projection;
    };
    const initial = project("2026-10-28T01:10:01.100Z");
    expect(initial.identities[1]).toMatchObject({ semantic_attempt: 2, lease_epoch: 1 });
    recordAutomaticBuildDispatchBootstrapFailure(target, "profile_sidecar", initial.identities[0],
      "bounded-startup-failure", "2026-10-28T01:10:01.100Z");
    const released = project("2026-10-28T01:10:01.200Z");
    expect(released.identities[0]).toMatchObject({
      version: "automatic_build_recovery_generation_identity.v2", bootstrap_epoch: 1,
      semantic_attempt: 1, lease_epoch: 1,
    });
    expect(released.identities.slice(1)).toEqual(initial.identities.slice(1));
    const claimed = advanceAutomaticBuildDispatch(target, "profile_sidecar", manifest.dispatch_id, {
      descriptors, task_bindings: bindings, dispatch_run_id: persisted.dispatch_run_id,
      now: "2026-10-28T01:10:01.300Z",
    });
    if (claimed.status !== "leased") throw new Error("expected running snapshot lease");
    startAutomaticBuildLease(target, claimed.claim.lease_ref, claimed.claim.lease.token, {
      now: "2026-10-28T01:10:01.400Z", run_ttl_ms: 1_000,
    });
    expect(project("2026-10-28T01:10:01.500Z").identities).toEqual(released.identities);
    const recovered = project("2026-10-28T01:10:02.500Z");
    expect(recovered.identities[0]).toEqual({ ...initial.identities[0], lease_epoch: 2 });
    expect(recovered.identities.slice(1)).toEqual(initial.identities.slice(1));
    expect(listAutomaticBuildStoredAttempts(target, "profile_sidecar", manifest.ordered_work_unit_ids[2])).toEqual([]);
  });

  it("bounds the snapshot before an exhausted future task and retains the current-task error", () => {
    const { target, descriptors, manifest, bindings } = fixture(3);
    for (let attempt = 0; attempt < 3; attempt++) {
      seedSemanticFailure(target, manifest.ordered_work_unit_ids[1], `exhausted-future-${attempt}`,
        `2026-10-28T01:20:0${attempt}.000Z`, `2026-10-28T01:20:0${attempt}.100Z`);
    }
    const persisted = persistAutomaticBuildDispatch(target, manifest, {
      owner: "bounded-recovery-blocked-suffix", created_at: "2026-10-28T01:20:03.000Z",
      reserve_ttl_ms: 60_000, run_ttl_ms: 1_800_000,
    }).persisted;
    const options = { now: "2026-10-28T01:20:03.100Z", dispatch_run_id: persisted.dispatch_run_id };
    const before = workspaceFileSnapshot(target.workspace_dir);
    const projection = inspectAutomaticBuildDispatchRecoveryGenerations(target, "profile_sidecar", manifest.dispatch_id, options);
    expect(projection.identities).toEqual([
      inspectAutomaticBuildDispatchRecoveryGeneration(target, "profile_sidecar", manifest.dispatch_id, options).recovery_identity,
    ]);
    expect(workspaceFileSnapshot(target.workspace_dir)).toEqual(before);

    const claimed = advanceAutomaticBuildDispatch(target, "profile_sidecar", manifest.dispatch_id, {
      ...options, descriptors, task_bindings: bindings,
    });
    if (claimed.status !== "leased") throw new Error("expected claim before blocked suffix");
    commit(target, claimed, 0);
    const completed = workspaceFileSnapshot(target.workspace_dir);
    expect(() => inspectAutomaticBuildDispatchRecoveryGenerations(target, "profile_sidecar", manifest.dispatch_id, options))
      .toThrow("dispatch recovery generation is not launchable: retry_exhausted");
    expect(workspaceFileSnapshot(target.workspace_dir)).toEqual(completed);
  });

  it.each(["terminal", "bootstrap"] as const)("reconciles %s-first startup release observations once per recovery generation", (firstKind) => {
    const { target, manifest } = fixture();
    const now = "2026-10-21T02:00:00.000Z";
    const persisted = persistAutomaticBuildDispatch(target, manifest, {
      owner: `startup-release:${manifest.dispatch_id}`, created_at: now,
      reserve_ttl_ms: 60_000, run_ttl_ms: 1_800_000,
    }).persisted;
    const project = () => inspectAutomaticBuildDispatchRecoveryGeneration(target, "profile_sidecar",
      manifest.dispatch_id, { now, dispatch_run_id: persisted.dispatch_run_id });
    const initial = project();
    for (let epoch = 0; epoch < 2; epoch++) {
      const identity = project().recovery_identity;
      const ref = `startup-release-${epoch}`;
      const terminal = () => recordAutomaticBuildDispatchTerminalSession(target, "profile_sidecar", identity, ref, now);
      const bootstrap = () => recordAutomaticBuildDispatchBootstrapFailure(target, "profile_sidecar", identity, ref, now);
      if (firstKind === "terminal") expect(terminal()).toBe(true);
      else bootstrap();
      const released = workspaceFileSnapshot(target.workspace_dir);
      bootstrap();
      expect(terminal()).toBe(false);
      bootstrap();
      expect(workspaceFileSnapshot(target.workspace_dir)).toEqual(released);
      expect(project()).toEqual({ ...initial, recovery_identity: {
        ...initial.recovery_identity,
        version: "automatic_build_recovery_generation_identity.v2", bootstrap_epoch: epoch + 1,
      } });
      expect(() => recordAutomaticBuildDispatchBootstrapFailure(target, "profile_sidecar", identity,
        "different-handoff", now)).toThrow("bootstrap failure conflicts");
    }
    const replacement = project();
    recordAutomaticBuildDispatchBootstrapFailure(target, "profile_sidecar", initial.recovery_identity,
      "startup-release-0", now);
    expect(recordAutomaticBuildDispatchTerminalSession(target, "profile_sidecar", initial.recovery_identity,
      "startup-release-0", now)).toBe(false);
    expect(project()).toEqual(replacement);
    expect(listAutomaticBuildStoredAttempts(target, "profile_sidecar")).toEqual([]);
  });

  it("projects a semantic retry as attempt two and lease epoch one without claiming it", () => {
    const { target, manifest } = fixture();
    seedSemanticFailure(
      target,
      manifest.ordered_work_unit_ids[0],
      "semantic-retry-seed",
      "2026-09-02T01:10:00.000Z",
      "2026-09-02T01:10:00.100Z",
    );
    const persisted = persistAutomaticBuildDispatch(target, manifest, {
      owner: `dispatch-semantic-retry-projection:${manifest.dispatch_id}`,
      created_at: "2026-09-02T01:10:01.000Z",
      reserve_ttl_ms: 60_000,
      run_ttl_ms: 1_800_000,
    }).persisted;
    const before = workspaceFileSnapshot(target.workspace_dir);

    const projection = inspectAutomaticBuildDispatchRecoveryGeneration(
      target,
      "profile_sidecar",
      manifest.dispatch_id,
      { now: "2026-09-02T01:10:01.100Z", dispatch_run_id: persisted.dispatch_run_id },
    );

    expect(projection.recovery_identity).toMatchObject({
      current_work_unit_id: manifest.ordered_work_unit_ids[0],
      semantic_attempt: 2,
      lease_epoch: 1,
    });
    expect(workspaceFileSnapshot(target.workspace_dir)).toEqual(before);
  });

  it("changes recovery identity after a committed work unit while keeping the dispatch slot stable", () => {
    const { target, descriptors, manifest, bindings } = fixture();
    const persisted = persistAutomaticBuildDispatch(target, manifest, {
      owner: `dispatch-work-unit-projection:${manifest.dispatch_id}`,
      created_at: "2026-09-02T01:20:00.000Z",
      reserve_ttl_ms: 60_000,
      run_ttl_ms: 1_800_000,
    }).persisted;
    const first = inspectAutomaticBuildDispatchRecoveryGeneration(
      target,
      "profile_sidecar",
      manifest.dispatch_id,
      { now: "2026-09-02T01:20:00.100Z", dispatch_run_id: persisted.dispatch_run_id },
    );
    const claimed = advanceAutomaticBuildDispatch(target, "profile_sidecar", manifest.dispatch_id, {
      descriptors,
      task_bindings: bindings,
      dispatch_run_id: persisted.dispatch_run_id,
      now: "2026-09-02T01:20:00.200Z",
    });
    if (claimed.status !== "leased") throw new Error("expected first work-unit projection lease");
    commit(target, claimed, 0);
    const before = workspaceFileSnapshot(target.workspace_dir);

    const second = inspectAutomaticBuildDispatchRecoveryGeneration(
      target,
      "profile_sidecar",
      manifest.dispatch_id,
      { now: "2026-09-02T01:20:00.300Z", dispatch_run_id: persisted.dispatch_run_id },
    );

    expect(second.dispatch_slot_ref).toBe(first.dispatch_slot_ref);
    expect(second.recovery_identity.current_work_unit_id).toBe(manifest.ordered_work_unit_ids[1]);
    expect(second.recovery_identity).not.toEqual(first.recovery_identity);
    expect(workspaceFileSnapshot(target.workspace_dir)).toEqual(before);
  });

  it("waits and reissues after a work unit commits between recovery inspection and handoff publication", () => {
    const { root, target, descriptors, manifest, bindings } = fixture(2);
    vi.stubEnv("UNDERSTAND_BOOK_AUTOMATIC_BUILD_DRIVER_ROOT", path.join(root, "driver-registry"));
    try {
      const persisted = persistAutomaticBuildDispatch(target, manifest, {
        owner: "handoff-publication-progress", created_at: "2026-09-02T01:20:00.000Z",
        reserve_ttl_ms: 60_000, run_ttl_ms: 1_800_000,
      }).persisted;
      const inspect = () => inspectAutomaticBuildDispatchRecoveryGeneration(
        target, "profile_sidecar", manifest.dispatch_id,
        { now: "2026-09-02T01:20:00.300Z", dispatch_run_id: persisted.dispatch_run_id },
      );
      const publish = (recovery_identity: ReturnType<typeof inspect>["recovery_identity"]) =>
        issueAutomaticBuildOpaqueHandoff({
          target, kind: "public_dispatch",
          owner_identity: { version: "automatic_build_dispatch_owner_identity.v1", stage: "profile_sidecar",
            dispatch_id: manifest.dispatch_id, dispatch_run_id: persisted.dispatch_run_id },
          recovery_identity, executor_handoff: persisted.executor_handoff,
          issued_at: "2026-09-02T01:20:00.300Z",
        });
      const first = inspect();
      const original = publish(first.recovery_identity);
      const claimed = advanceAutomaticBuildDispatch(target, "profile_sidecar", manifest.dispatch_id, {
        descriptors, task_bindings: bindings, dispatch_run_id: persisted.dispatch_run_id,
        now: "2026-09-02T01:20:00.200Z",
      });
      if (claimed.status !== "leased") throw new Error("expected first work-unit lease");
      commit(target, claimed, 0);
      const committed = workspaceFileSnapshot(target.workspace_dir);

      let response: unknown;
      try { publish(first.recovery_identity); }
      catch (error) { response = automaticBuildDriverFailureResponse(error); }
      expect(response).toEqual({ version: "automatic_build_step.v1",
        action: { kind: "WAIT", reason: "backoff", retry_after_ms: 50 } });
      const current = inspect();
      expect(current.dispatch_slot_ref).toBe(first.dispatch_slot_ref);
      expect(current.recovery_identity.current_work_unit_id).toBe(manifest.ordered_work_unit_ids[1]);
      const replacement = publish(current.recovery_identity);
      expect(replacement.opaque_handoff_ref).not.toBe(original.opaque_handoff_ref);
      expect(publish(current.recovery_identity)).toEqual(replacement);
      expect(workspaceFileSnapshot(target.workspace_dir)).toEqual(committed);
    } finally { vi.unstubAllEnvs(); }
  });

  it("reads a legacy dispatch manifest binding without inventing a V3 attempt scope", () => {
    const { target, manifest, bindings } = fixture();
    const legacyManifest = { ...manifest, task_bindings: bindings } as unknown as typeof manifest;
    persistAutomaticBuildDispatch(target, legacyManifest, {
      owner: `legacy-dispatch-runtime:${legacyManifest.dispatch_id}`,
      created_at: "2026-08-27T01:00:00.000Z",
      reserve_ttl_ms: 60_000,
      run_ttl_ms: 1_800_000,
    });

    expect(inspectAutomaticBuildDispatch(
      target,
      "profile_sidecar",
      legacyManifest.dispatch_id,
      "2026-08-27T01:00:01.000Z",
    )).toMatchObject({
      state: "active",
      next_work_unit_id: legacyManifest.ordered_work_unit_ids[0],
      task_receipts: [],
    });
    expect(listAutomaticBuildStoredAttempts(target, "profile_sidecar")).toEqual([]);
  });

  it("advances a proof-bound V4 BookStructure dispatch through the scoped binding path", () => {
    const root = mkdtempSync(path.join(tmpdir(), "understand-book-dispatch-runtime-v4-"));
    const sourcePath = path.join(root, "guide.md");
    writeFileSync(sourcePath, "# Guide\n\nA bounded BookStructure unit.\n", "utf8");
    const target = resolveAutomaticBuildTarget(sourcePath, root);
    const profile = resolveContentProfile("technical_learning");
    const routed = routeBookStructureUnitWorkUnitsV2({
      target: target.target_ref,
      source: {
        job_id: "unit:1",
        unit_lid: "1",
        unit_kind: "chapter",
        title_path: [],
        leaf_lids: ["1.1"],
        excerpts: [{ lid: "1.1", text: "A bounded BookStructure unit." }],
        graph_nodes: [],
        graph_edges: [],
        discourse_items: [],
        formula_semantics: [],
        pass2_edges: [],
      },
      lid_nodes: [{
        lid: "1.1",
        path: [1, 1],
        kind: "paragraph",
        span: { start: 0, end: 29 },
        children: [],
      }],
      source_fingerprint: target.target_ref.input_fingerprint,
      contracts: createBookStructureExecutionContractsV2({
        profile,
        quality_profile: "full",
        prompts: {
          whole: "BOOK_STRUCTURE_WHOLE_V2\nReturn one strict JSON unit card.\n",
          fragment: "BOOK_STRUCTURE_FRAGMENT_V1\nReturn one grounded local observation.\n",
          reduce: "BOOK_STRUCTURE_REDUCE_V1\nReduce proof-bound child observations.\n",
          stitch: "BOOK_STRUCTURE_STITCH_V2\nReturn the public BookStructure candidate.\n",
          stitch_fragment: "BOOK_STRUCTURE_STITCH_FRAGMENT_V1\nReturn one local stitch observation.\n",
          stitch_reduce: "BOOK_STRUCTURE_STITCH_REDUCE_V1\nReduce proof-bound stitch observations.\n",
        },
      }),
      transport_profile: CODEX_EXECUTOR_TRANSPORT_PROFILE_V2,
    });
    expect(routed.status).toBe("ready");
    if (routed.status !== "ready") throw new Error("expected one routable V4 BookStructure unit");
    expect(routed.work_units).toHaveLength(1);
    const descriptor = routed.work_units[0].descriptor;
    expect(descriptor.version).toBe("automatic_build_work_unit.v4");
    const binding = taskPolicyBindingForWorkUnit(descriptor, "a".repeat(64));
    const plan = planAutomaticBuildExecutorDispatches({
      target_ref: target.target_ref,
      stage: "book_structure",
      work_units: [descriptor],
      task_bindings: { [descriptor.work_unit_id]: binding },
      pending_ids: [descriptor.work_unit_id],
      available_agent_slots: 1,
    });
    expect(plan.dispatches).toHaveLength(1);
    const manifest = plan.dispatches[0];
    persistAutomaticBuildDispatch(target, manifest, {
      owner: `dispatch-v4:${manifest.dispatch_id}`,
      created_at: "2026-08-27T02:00:00.000Z",
      reserve_ttl_ms: 60_000,
      run_ttl_ms: 1_800_000,
    });
    expect(listAutomaticBuildStoredAttempts(target, "book_structure")).toEqual([]);

    const advanced = advanceAutomaticBuildDispatch(target, "book_structure", manifest.dispatch_id, {
      descriptors: [descriptor],
      task_bindings: { [descriptor.work_unit_id]: binding },
      now: "2026-08-27T02:00:01.000Z",
    });

    expect(advanced.status).toBe("leased");
    expect(listAutomaticBuildStoredAttempts(target, "book_structure")).toHaveLength(1);
  });

  it("runs an eight-task formula dispatch with one lease at a time and eight canonical receipts", () => {
    const { target, descriptors, manifest, bindings } = fixture();
    persistAutomaticBuildDispatch(target, manifest, {
      owner: `dispatch-runtime:${manifest.dispatch_id}`,
      created_at: "2026-07-25T04:00:00.000Z",
      reserve_ttl_ms: 60_000,
      run_ttl_ms: 1_800_000,
    });
    for (let index = 0; index < 8; index += 1) {
      const advanced = advanceAutomaticBuildDispatch(target, "profile_sidecar", manifest.dispatch_id, {
        descriptors,
        task_bindings: bindings,
        now: `2026-07-25T04:00:${String(index + 1).padStart(2, "0")}.000Z`,
      });
      expect(advanced.status).toBe("leased");
      if (advanced.status !== "leased") throw new Error("expected dispatch lease");
      expect(advanced.descriptor.work_unit_id).toBe(manifest.ordered_work_unit_ids[index]);
      expect(advanced.descriptor.input_hash).toBe(bindings[advanced.descriptor.work_unit_id].input_hash);
      commit(target, advanced, index);
      expect(Object.keys(readAutomaticBuildAttemptSnapshot(target).stages.profile_sidecar ?? {})).toHaveLength(index + 1);
    }
    const complete = advanceAutomaticBuildDispatch(target, "profile_sidecar", manifest.dispatch_id, {
      descriptors,
      task_bindings: bindings,
      now: "2026-07-25T04:00:20.000Z",
    });
    expect(complete.status).toBe("ready_to_finish");
    const terminalOptions = { now: "2026-07-25T04:00:21.000Z", dispatch_run_id: "run-20260725T040000000Z" };
    expect(finishAutomaticBuildDispatchIfTerminal(target, "profile_sidecar", manifest.dispatch_id, terminalOptions)).toBe(true);
    expect(finishAutomaticBuildDispatchIfTerminal(target, "profile_sidecar", manifest.dispatch_id, terminalOptions)).toBe(true);
    const receipt = finishAutomaticBuildDispatch(target, "profile_sidecar", manifest.dispatch_id, {
      now: "2026-07-25T04:00:21.000Z",
    });
    expect(receipt).toMatchObject({ terminal_reason: "complete", unclaimed_work_unit_ids: [] });
    expect(receipt.task_receipts).toHaveLength(8);
    expect(new Set(receipt.task_receipts.map((item) => item.work_unit_id)).size).toBe(8);
    expect(Buffer.byteLength(JSON.stringify(receipt))).toBeLessThanOrEqual(16_384);
    expect(JSON.stringify(receipt)).not.toContain("PRIVATE_FORMULA_");
    // A second plan existed before those commits, but publishes this dispatch only afterwards.
    const late = persistAutomaticBuildDispatch(target, manifest, {
      owner: "late-publication", created_at: "2026-07-25T04:00:00.500Z", reserve_ttl_ms: 60_000, run_ttl_ms: 1_800_000,
    });
    expect(finishAutomaticBuildDispatchIfTerminal(target, "profile_sidecar", manifest.dispatch_id, {
      now: "2026-07-25T04:00:22.000Z", dispatch_run_id: late.persisted.dispatch_run_id,
    })).toBe(true);
    expect(listAutomaticBuildStoredAttempts(target, "profile_sidecar")).toHaveLength(8);

  }, 15_000);

  it.each([0, 893])("removes a progress write interrupted after %i bytes and recovers from the canonical receipt", async partialBytes => {
    const actualFs = await vi.importActual<typeof import("node:fs")>("node:fs");
    const { target, descriptors, manifest, bindings } = fixture(2);
    const publication = persistAutomaticBuildDispatch(target, manifest, {
      owner: "progress-write-recovery", created_at: "2026-10-22T00:00:00.000Z",
      reserve_ttl_ms: 60_000, run_ttl_ms: 1_800_000,
    });
    const options = { descriptors, task_bindings: bindings, now: "2026-10-22T00:00:01.000Z" };
    const first = advanceAutomaticBuildDispatch(target, "profile_sidecar", manifest.dispatch_id, options);
    if (first.status !== "leased") throw new Error("expected first lease");
    const receipt = commit(target, first, 0);
    const file = path.join(path.dirname(publication.manifest_path), "progress", "0000.json");
    const diskFull = Object.assign(new Error("disk full"), { code: "ENOSPC", syscall: "write" });
    let progressDescriptor: number | undefined;
    vi.mocked(fs.openSync).mockImplementation((...args) => {
      const descriptor = actualFs.openSync(...args);
      if (args[0] === file) progressDescriptor = descriptor;
      return descriptor;
    });
    vi.mocked(fs.writeFileSync).mockImplementation((destination, data, writeOptions) => {
      if (destination === file || destination === progressDescriptor) {
        actualFs.writeFileSync(destination, Buffer.alloc(partialBytes), writeOptions);
        throw diskFull;
      }
      actualFs.writeFileSync(destination, data, writeOptions);
    });
    try {
      expect(() => advanceAutomaticBuildDispatch(target, "profile_sidecar", manifest.dispatch_id, options)).toThrow(diskFull);
      expect(existsSync(file)).toBe(false);
    } finally {
      vi.mocked(fs.openSync).mockRestore();
      vi.mocked(fs.writeFileSync).mockRestore();
    }
    const next = advanceAutomaticBuildDispatch(target, "profile_sidecar", manifest.dispatch_id, options);
    expect(next.status).toBe("leased");
    if (next.status !== "leased") throw new Error("expected next lease");
    expect(next.descriptor.work_unit_id).toBe(manifest.ordered_work_unit_ids[1]);
    expect(JSON.parse(readFileSync(file, "utf8")).task_receipt).toMatchObject({
      task_ref: receipt.task_ref, state: "committed", attempt: 1,
    });
    expect(listAutomaticBuildStoredAttempts(target, "profile_sidecar", manifest.ordered_work_unit_ids[0])).toHaveLength(1);
  });

  it("preserves three commits, recovers the fourth lease, and never claims the remaining suffix", () => {
    const { target, descriptors, manifest, bindings } = fixture();
    persistAutomaticBuildDispatch(target, manifest, {
      owner: `dispatch-interrupt:${manifest.dispatch_id}`,
      created_at: "2026-07-25T05:00:00.000Z",
      reserve_ttl_ms: 1_000,
      run_ttl_ms: 1_800_000,
    });
    for (let index = 0; index < 3; index += 1) {
      const advanced = advanceAutomaticBuildDispatch(target, "profile_sidecar", manifest.dispatch_id, {
        descriptors,
        task_bindings: bindings,
        now: `2026-07-25T05:00:0${index + 1}.000Z`,
      });
      if (advanced.status !== "leased") throw new Error("expected dispatch lease");
      commit(target, advanced, index);
    }
    const fourth = advanceAutomaticBuildDispatch(target, "profile_sidecar", manifest.dispatch_id, {
      descriptors,
      task_bindings: bindings,
      now: "2026-07-25T05:00:04.000Z",
    });
    expect(fourth.status).toBe("leased");
    const interrupted = finishAutomaticBuildDispatch(target, "profile_sidecar", manifest.dispatch_id, {
      terminal_reason: "executor_interrupted",
      now: "2026-07-25T05:00:04.500Z",
      interruption: {
        diagnostic_code: "executor_lost",
        reporter: "executor",
        last_command_role: "dispatch_next",
      },
    });
    expect(interrupted.task_receipts).toHaveLength(3);
    expect(interrupted.unclaimed_work_unit_ids).toEqual(manifest.ordered_work_unit_ids.slice(4));
    expect(interrupted.interruption).toMatchObject({
      phase: "task_reserved",
      last_completed_ordinal: 2,
      active_work_unit_id: manifest.ordered_work_unit_ids[3],
    });
    expect(Object.keys(readAutomaticBuildAttemptSnapshot(target).stages.profile_sidecar ?? {})).toEqual(
      manifest.ordered_work_unit_ids.slice(0, 4),
    );

    const recoveryPlan = planAutomaticBuildExecutorDispatches({
      target_ref: target.target_ref,
      stage: "profile_sidecar",
      work_units: descriptors,
      pending_ids: manifest.ordered_work_unit_ids.slice(3),
      available_agent_slots: 1,
    });
    const recovery = recoveryPlan.dispatches[0];
    persistAutomaticBuildDispatch(target, recovery, {
      owner: `dispatch-recovery:${recovery.dispatch_id}`,
      created_at: "2026-07-25T05:00:06.000Z",
      reserve_ttl_ms: 60_000,
      run_ttl_ms: 1_800_000,
    });
    const reclaimed = advanceAutomaticBuildDispatch(target, "profile_sidecar", recovery.dispatch_id, {
      descriptors,
      task_bindings: bindings,
      now: "2026-07-25T05:00:06.000Z",
    });
    expect(reclaimed.status).toBe("leased");
    if (reclaimed.status !== "leased") throw new Error("expected recovered lease");
    expect(reclaimed.descriptor.work_unit_id).toBe(manifest.ordered_work_unit_ids[3]);
    expect(reclaimed.claim.execution_identity.lease_epoch).toBe(2);
    expect(listAutomaticBuildStoredAttempts(target, "profile_sidecar")
      .filter((attempt) => attempt.work_unit_id === manifest.ordered_work_unit_ids[3])).toHaveLength(2);
    expect(manifest.ordered_work_unit_ids.slice(4).some((workUnitId) => (
      readAutomaticBuildAttemptSnapshot(target).stages.profile_sidecar?.[workUnitId]
    ))).toBe(false);
  });

  it("records a bounded structured interruption before the first task claim", () => {
    const { target, manifest } = fixture();
    persistAutomaticBuildDispatch(target, manifest, {
      owner: `dispatch-before-claim:${manifest.dispatch_id}`,
      created_at: "2026-08-02T02:00:00.000Z",
      reserve_ttl_ms: 60_000,
      run_ttl_ms: 1_800_000,
    });
    const receipt = finishAutomaticBuildDispatch(target, "profile_sidecar", manifest.dispatch_id, {
      terminal_reason: "executor_interrupted",
      now: "2026-08-02T02:00:01.000Z",
      interruption: {
        diagnostic_code: "harness_cancelled",
        reporter: "root_supervisor",
        last_command_role: "dispatch_next",
      },
    });
    expect(receipt).toMatchObject({
      terminal_reason: "executor_interrupted",
      task_receipts: [],
      interruption: {
        version: "automatic_build_executor_interruption.v1",
        diagnostic_code: "harness_cancelled",
        phase: "before_first_claim",
        reporter: "root_supervisor",
        last_command_role: "dispatch_next",
        last_completed_ordinal: -1,
        observed_at: "2026-08-02T02:00:01.000Z",
      },
    });
    expect(Buffer.byteLength(JSON.stringify(receipt))).toBeLessThanOrEqual(16_384);
  });

  it("derives a running interruption from the persisted task start", () => {
    const { target, descriptors, manifest, bindings } = fixture();
    persistAutomaticBuildDispatch(target, manifest, {
      owner: `dispatch-running:${manifest.dispatch_id}`,
      created_at: "2026-08-02T02:10:00.000Z",
      reserve_ttl_ms: 60_000,
      run_ttl_ms: 1_000,
    });
    const claimed = advanceAutomaticBuildDispatch(target, "profile_sidecar", manifest.dispatch_id, {
      descriptors,
      task_bindings: bindings,
      now: "2026-08-02T02:10:01.000Z",
    });
    if (claimed.status !== "leased") throw new Error("expected running interruption lease");
    startAutomaticBuildLease(target, claimed.claim.lease_ref, claimed.claim.lease.token, {
      now: "2026-08-02T02:10:01.100Z",
      run_ttl_ms: 1_000,
    });
    const receipt = finishAutomaticBuildDispatch(target, "profile_sidecar", manifest.dispatch_id, {
      terminal_reason: "executor_interrupted",
      now: "2026-08-02T02:10:01.500Z",
      interruption: {
        diagnostic_code: "command_nonzero_exit",
        reporter: "executor",
        last_command_role: "task_input",
      },
    });
    expect(receipt.interruption).toMatchObject({
      phase: "task_running",
      last_completed_ordinal: -1,
      active_work_unit_id: manifest.ordered_work_unit_ids[0],
    });
    expect(claimed.claim.execution_identity).toMatchObject({
      semantic_attempt: 1,
      lease_epoch: 1,
    });
  });

  it("derives a between-tasks interruption after a committed prefix", () => {
    const { target, descriptors, manifest, bindings } = fixture();
    persistAutomaticBuildDispatch(target, manifest, {
      owner: `dispatch-between:${manifest.dispatch_id}`,
      created_at: "2026-08-02T02:20:00.000Z",
      reserve_ttl_ms: 60_000,
      run_ttl_ms: 1_800_000,
    });
    const first = advanceAutomaticBuildDispatch(target, "profile_sidecar", manifest.dispatch_id, {
      descriptors,
      task_bindings: bindings,
      now: "2026-08-02T02:20:01.000Z",
    });
    if (first.status !== "leased") throw new Error("expected first between-tasks lease");
    commit(target, first, 0);
    const receipt = finishAutomaticBuildDispatch(target, "profile_sidecar", manifest.dispatch_id, {
      terminal_reason: "executor_interrupted",
      now: "2026-08-02T02:20:02.000Z",
      interruption: {
        diagnostic_code: "harness_cancelled",
        reporter: "root_supervisor",
        last_command_role: "dispatch_next",
      },
    });
    expect(receipt.interruption).toMatchObject({
      phase: "between_tasks",
      last_completed_ordinal: 0,
    });
    expect(receipt.interruption).not.toHaveProperty("active_work_unit_id");
    expect(receipt.unclaimed_work_unit_ids).toEqual(manifest.ordered_work_unit_ids.slice(1));
  });

  it("rejects missing, unbounded, or non-allowlisted interruption input", () => {
    const { target, manifest } = fixture();
    persistAutomaticBuildDispatch(target, manifest, {
      owner: `dispatch-invalid-interruption:${manifest.dispatch_id}`,
      created_at: "2026-08-02T02:30:00.000Z",
      reserve_ttl_ms: 60_000,
      run_ttl_ms: 1_800_000,
    });
    expect(() => finishAutomaticBuildDispatch(target, "profile_sidecar", manifest.dispatch_id, {
      terminal_reason: "executor_interrupted",
      now: "2026-08-02T02:30:01.000Z",
    })).toThrow("requires interruption diagnostics");
    expect(() => finishAutomaticBuildDispatch(target, "profile_sidecar", manifest.dispatch_id, {
      terminal_reason: "executor_interrupted",
      now: "2026-08-02T02:30:01.000Z",
      interruption: {
        diagnostic_code: "command_nonzero_exit",
        reporter: "executor",
        last_command_role: "dispatch_next",
        stderr: "PRIVATE RAW STDERR",
      } as NonNullable<Parameters<typeof finishAutomaticBuildDispatch>[3]>["interruption"] & { stderr: string },
    })).toThrow("unsupported fields");
    expect(() => finishAutomaticBuildDispatch(target, "profile_sidecar", manifest.dispatch_id, {
      terminal_reason: "executor_interrupted",
      now: "2026-08-02T02:30:01.000Z",
      interruption: {
        diagnostic_code: "private_exception",
        reporter: "executor",
        last_command_role: "dispatch_next",
      } as unknown as NonNullable<Parameters<typeof finishAutomaticBuildDispatch>[3]>["interruption"],
    })).toThrow("not allowlisted");
  });

  it("keeps a historical interrupted v1 receipt without diagnostics readable", () => {
    const { target, manifest } = fixture();
    const publication = persistAutomaticBuildDispatch(target, manifest, {
      owner: `dispatch-legacy-receipt:${manifest.dispatch_id}`,
      created_at: "2026-08-02T02:40:00.000Z",
      reserve_ttl_ms: 60_000,
      run_ttl_ms: 1_800_000,
    });
    const historical = {
      version: "automatic_build_executor_dispatch_receipt.v1",
      dispatch_id: manifest.dispatch_id,
      dispatch_run_id: publication.persisted.dispatch_run_id,
      target_ref: target.target_ref,
      stage: "profile_sidecar",
      task_receipts: [],
      unclaimed_work_unit_ids: [...manifest.ordered_work_unit_ids],
      terminal_reason: "executor_interrupted",
      finished_at: "2026-08-02T02:40:01.000Z",
    };
    writeFileSync(
      path.join(path.dirname(publication.manifest_path), "receipt.json"),
      `${JSON.stringify(historical, null, 2)}\n`,
      "utf8",
    );
    const replay = finishAutomaticBuildDispatch(target, "profile_sidecar", manifest.dispatch_id);
    expect(replay).toMatchObject(historical);
    expect(replay.interruption).toBeUndefined();
  });

  it("uses a new dispatch run when a failed one-task manifest keeps the same planner identity", () => {
    const { target, descriptors, bindings } = fixture();
    const one = planAutomaticBuildExecutorDispatches({
      target_ref: target.target_ref,
      stage: "profile_sidecar",
      work_units: descriptors,
      pending_ids: [descriptors[0].work_unit_id],
      available_agent_slots: 1,
    }).dispatches[0];
    const firstRun = persistAutomaticBuildDispatch(target, one, {
      owner: `dispatch-failure:${one.dispatch_id}:1`,
      created_at: "2026-07-25T06:00:00.000Z",
      reserve_ttl_ms: 60_000,
      run_ttl_ms: 1_800_000,
    }).persisted;
    const first = advanceAutomaticBuildDispatch(target, "profile_sidecar", one.dispatch_id, {
      descriptors,
      task_bindings: bindings,
      dispatch_run_id: firstRun.dispatch_run_id,
      now: "2026-07-25T06:00:01.000Z",
    });
    if (first.status !== "leased") throw new Error("expected first run lease");
    failAutomaticBuildTask(target, first.claim.lease_ref, first.claim.lease.token, {
      diagnostic_code: "semantic_invalid",
      now: "2026-07-25T06:00:02.000Z",
    });
    expect(advanceAutomaticBuildDispatch(target, "profile_sidecar", one.dispatch_id, {
      descriptors,
      task_bindings: bindings,
      dispatch_run_id: firstRun.dispatch_run_id,
      now: "2026-07-25T06:00:03.000Z",
    }).status).toBe("ready_to_finish");
    expect(finishAutomaticBuildDispatch(target, "profile_sidecar", one.dispatch_id, {
      dispatch_run_id: firstRun.dispatch_run_id,
      now: "2026-07-25T06:00:04.000Z",
    }).terminal_reason).toBe("task_failure");

    const secondRun = persistAutomaticBuildDispatch(target, one, {
      owner: `dispatch-failure:${one.dispatch_id}:2`,
      created_at: "2026-07-25T06:01:00.000Z",
      reserve_ttl_ms: 60_000,
      run_ttl_ms: 1_800_000,
    }).persisted;
    expect(secondRun.dispatch_run_id).not.toBe(firstRun.dispatch_run_id);
    const retried = advanceAutomaticBuildDispatch(target, "profile_sidecar", one.dispatch_id, {
      descriptors,
      task_bindings: bindings,
      dispatch_run_id: secondRun.dispatch_run_id,
      now: "2026-07-25T06:01:01.000Z",
    });
    expect(retried.status).toBe("leased");
    if (retried.status !== "leased") throw new Error("expected retry dispatch lease");
    expect(retried.claim.execution_identity.semantic_attempt).toBe(2);
  });

  it("partially finishes task_failure only after a terminal exhausted task and preserves the exact suffix", () => {
    const { target, descriptors, manifest, bindings } = fixture();
    const firstId = manifest.ordered_work_unit_ids[0];
    seedSemanticFailure(target, firstId, "seed-failure-1", "2026-07-25T07:00:00.000Z", "2026-07-25T07:00:01.000Z");
    seedSemanticFailure(target, firstId, "seed-failure-2", "2026-07-25T07:01:00.000Z", "2026-07-25T07:01:01.000Z");
    persistAutomaticBuildDispatch(target, manifest, {
      owner: `dispatch-exhausted:${manifest.dispatch_id}`,
      created_at: "2026-07-25T07:02:00.000Z",
      reserve_ttl_ms: 60_000,
      run_ttl_ms: 1_800_000,
    });
    const third = advanceAutomaticBuildDispatch(target, "profile_sidecar", manifest.dispatch_id, {
      descriptors,
      task_bindings: bindings,
      now: "2026-07-25T07:02:01.000Z",
      max_semantic_attempts: 3,
    });
    if (third.status !== "leased") throw new Error("expected third semantic lease");
    failAutomaticBuildTask(target, third.claim.lease_ref, third.claim.lease.token, {
      diagnostic_code: "semantic_invalid",
      now: "2026-07-25T07:02:02.000Z",
    });
    expect(advanceAutomaticBuildDispatch(target, "profile_sidecar", manifest.dispatch_id, {
      descriptors,
      task_bindings: bindings,
      now: "2026-07-25T07:02:03.000Z",
      max_semantic_attempts: 3,
    })).toMatchObject({ status: "retry_exhausted", work_unit_id: firstId });
    const receipt = finishAutomaticBuildDispatch(target, "profile_sidecar", manifest.dispatch_id, {
      terminal_reason: "task_failure",
      now: "2026-07-25T07:02:04.000Z",
    });
    expect(receipt.task_receipts).toHaveLength(1);
    expect(receipt.unclaimed_work_unit_ids).toEqual(manifest.ordered_work_unit_ids.slice(1));
  });

  it("rejects task_failure without a canonical retryable failure receipt", () => {
    const { target, manifest } = fixture();
    persistAutomaticBuildDispatch(target, manifest, {
      owner: `dispatch-no-failure:${manifest.dispatch_id}`,
      created_at: "2026-07-25T08:00:00.000Z",
      reserve_ttl_ms: 60_000,
      run_ttl_ms: 1_800_000,
    });
    expect(() => finishAutomaticBuildDispatch(target, "profile_sidecar", manifest.dispatch_id, {
      terminal_reason: "task_failure",
      now: "2026-07-25T08:00:01.000Z",
    })).toThrow("canonical retryable failure");
  });

  it("rejects task_failure while a dispatch lease is active", () => {
    const { target, descriptors, manifest, bindings } = fixture();
    persistAutomaticBuildDispatch(target, manifest, {
      owner: `dispatch-active:${manifest.dispatch_id}`,
      created_at: "2026-07-25T09:00:00.000Z",
      reserve_ttl_ms: 60_000,
      run_ttl_ms: 1_800_000,
    });
    const first = advanceAutomaticBuildDispatch(target, "profile_sidecar", manifest.dispatch_id, {
      descriptors,
      task_bindings: bindings,
      now: "2026-07-25T09:00:01.000Z",
    });
    if (first.status !== "leased") throw new Error("expected first lease");
    failAutomaticBuildTask(target, first.claim.lease_ref, first.claim.lease.token, {
      diagnostic_code: "semantic_invalid",
      now: "2026-07-25T09:00:02.000Z",
    });
    const active = advanceAutomaticBuildDispatch(target, "profile_sidecar", manifest.dispatch_id, {
      descriptors,
      task_bindings: bindings,
      now: "2026-07-25T09:00:03.000Z",
    });
    if (active.status !== "leased") throw new Error("expected active suffix lease");
    expect(() => finishAutomaticBuildDispatch(target, "profile_sidecar", manifest.dispatch_id, {
      terminal_reason: "task_failure",
      now: "2026-07-25T09:00:04.000Z",
    })).toThrow("active lease");
  });

  it("rejects task_failure when the remaining work is not a strict unclaimed suffix", () => {
    const { target, manifest } = fixture();
    const owner = `dispatch-gap:${manifest.dispatch_id}`;
    persistAutomaticBuildDispatch(target, manifest, {
      owner,
      created_at: "2026-07-25T10:00:00.000Z",
      reserve_ttl_ms: 1_000,
      run_ttl_ms: 1_800_000,
    });
    seedSemanticFailure(
      target,
      manifest.ordered_work_unit_ids[0],
      owner,
      "2026-07-25T10:00:01.000Z",
      "2026-07-25T10:00:01.500Z",
    );
    const gap = claimAutomaticBuildTask(target, "profile_sidecar", manifest.ordered_work_unit_ids[2], {
      owner,
      now: "2026-07-25T10:00:02.000Z",
      reserve_ttl_ms: 1_000,
    });
    if (gap.status !== "leased") throw new Error("expected out-of-order gap lease");
    expect(() => finishAutomaticBuildDispatch(target, "profile_sidecar", manifest.dispatch_id, {
      terminal_reason: "task_failure",
      now: "2026-07-25T10:00:04.000Z",
    })).toThrow("strict unclaimed suffix");
  });
});
