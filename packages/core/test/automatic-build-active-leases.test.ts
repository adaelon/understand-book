import * as fs from "node:fs";
import path from "node:path";
import { tmpdir } from "node:os";
import { describe, expect, it, vi } from "vitest";
import { buildAutomaticBuildSnapshot, resolveAutomaticBuildTarget } from "../src/build-orchestrator";
import { auditAutomaticBuildLegacy } from "../src/automatic-build-legacy";
import { claimAutomaticBuildTask, listActiveAutomaticBuildTaskLeases, startAutomaticBuildLease, heartbeatAutomaticBuildLease } from "../src/automatic-build-lease";
import { listAutomaticBuildStoredAttempts } from "../src/automatic-build-task-store";
import { buildAutomaticBuildStageMetricsSummary, readAutomaticBuildStageUsage } from "../src/automatic-build-metrics";

const reads = vi.hoisted(() => ({ files: undefined as string[] | undefined }));
vi.mock("node:fs", async (original) => {
  const actual = await original<typeof fs>();
  return { ...actual, readFileSync: (...args: Parameters<typeof fs.readFileSync>) => {
    reads.files?.push(String(args[0]));
    return actual.readFileSync(...args);
  } };
});

describe("active lease reads for migration routing", () => {
  it("computes the same exact budget usage without loading lifecycle history", () => {
    const root = fs.mkdtempSync(path.join(tmpdir(), "ub-budget-usage-"));
    const source = path.join(root, "source.md");
    fs.writeFileSync(source, "# Test\n\nBudget usage.\n");
    const target = resolveAutomaticBuildTarget(source, root);
    for (const [id, usage] of [["known", { input_tokens: 10, output_tokens: 5 }],
      ["partial", { cached_input_tokens: 3 }], ["unknown", {}]] as const) {
      const claim = claimAutomaticBuildTask(target, "pass1", id, { owner: "test", now: "2026-09-27T00:00:00.000Z" });
      if (claim.status !== "leased") throw new Error("expected lease");
      fs.writeFileSync(path.join(path.dirname(claim.lease_ref), "usage.json"), JSON.stringify({
        version: "automatic_build_usage_receipt.v1", source: id === "unknown" ? "unavailable" : "native", ...usage,
      }));
    }
    const expected = buildAutomaticBuildStageMetricsSummary(target, "pass1").usage;
    reads.files = [];
    let actual;
    try { actual = readAutomaticBuildStageUsage(target, "pass1"); }
    finally {
      const observed = reads.files;
      reads.files = undefined;
      expect(observed.filter(file => /[\\/](execution|lease|start|heartbeat|input|result|submission)\.json$/.test(file))).toEqual([]);
    }
    expect(actual).toEqual(expected);
    expect(actual).toMatchObject({ fully_known_attempts: 1, partially_known_attempts: 1, unavailable_attempts: 1,
      input_tokens: 10, cached_input_tokens: 3, output_tokens: 5 });
  });
  it("reuses the current routing snapshot for the legacy audit without reading the source again", () => {
    const root = fs.mkdtempSync(path.join(tmpdir(), "ub-audit-snapshot-"));
    const source = path.join(root, "source.md");
    fs.writeFileSync(source, "# Test\n\nAudit snapshot.\n");
    const target = resolveAutomaticBuildTarget(source, root);
    const snapshot = buildAutomaticBuildSnapshot(target);
    const expected = auditAutomaticBuildLegacy(target, "pass1");
    reads.files = [];
    let actual;
    try { actual = auditAutomaticBuildLegacy(target, "pass1", { snapshot }); }
    finally {
      const observed = reads.files;
      reads.files = undefined;
      expect(observed).not.toContain(source);
    }
    expect(actual).toEqual(expected);
  });
  it("reads only the requested task when dispatch recovery looks up one receipt", () => {
    const root = fs.mkdtempSync(path.join(tmpdir(), "ub-scoped-attempts-"));
    const source = path.join(root, "source.md");
    fs.writeFileSync(source, "# Test\n\nDispatch recovery.\n");
    const target = resolveAutomaticBuildTarget(source, root);
    for (const id of ["wanted", "unrelated"]) claimAutomaticBuildTask(target, "pass1", id, {
      owner: "test", now: "2026-09-27T00:00:00.000Z",
    });
    const expected = listAutomaticBuildStoredAttempts(target, "pass1").filter(a => a.work_unit_id === "wanted");
    reads.files = [];
    let actual;
    try { actual = listAutomaticBuildStoredAttempts(target, "pass1", "wanted"); }
    finally {
      const observed = reads.files;
      reads.files = undefined;
      expect(observed.filter(file => /[\\/]unrelated[\\/]/.test(file))).toEqual([]);
    }
    expect(actual).toEqual(expected);
    expect(listAutomaticBuildStoredAttempts(target, "pass1", "absent")).toEqual([]);
  });
  it("skips execution history for terminal and expired tasks while retaining live reservations and heartbeats", () => {
    const root = fs.mkdtempSync(path.join(tmpdir(), "ub-active-leases-"));
    const source = path.join(root, "source.md");
    fs.writeFileSync(source, "# Test\n\nLease routing.\n");
    const target = resolveAutomaticBuildTarget(source, root);
    const now = "2026-09-27T00:00:00.000Z";
    for (const id of ["terminal", "expired", "reserved", "running"]) {
      const claim = claimAutomaticBuildTask(target, "pass1", id, {
        owner: "test", now, reserve_ttl_ms: id === "expired" ? 1_000 : 600_000,
      });
      if (claim.status !== "leased") throw new Error("expected lease");
      if (id === "terminal") fs.writeFileSync(path.join(path.dirname(claim.lease_ref), "result.json"), "{}");
      if (id === "running") {
        startAutomaticBuildLease(target, claim.lease_ref, claim.lease.token, { now, run_ttl_ms: 1_000 });
        heartbeatAutomaticBuildLease(target, claim.lease_ref, claim.lease.token, { now, ttl_ms: 120_000 });
      }
    }
    reads.files = [];
    let leases;
    try { leases = listActiveAutomaticBuildTaskLeases(target, "pass1", "2026-09-27T00:01:00.000Z"); }
    finally {
      const observed = reads.files;
      reads.files = undefined;
      expect(observed.filter(file => /[\\/](terminal|expired)[\\/].*[\\/]execution\.json$/.test(file))).toEqual([]);
    }
    expect(leases.map(item => item.lease.work_unit_id).sort()).toEqual(["reserved", "running"]);
    expect(listActiveAutomaticBuildTaskLeases(target, "pass1", "2026-09-27T00:11:00.000Z")).toEqual([]);
  });
});
