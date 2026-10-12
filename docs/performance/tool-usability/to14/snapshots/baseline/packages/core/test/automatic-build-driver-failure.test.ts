import { existsSync, mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import { tmpdir } from "node:os";
import * as os from "node:os";
import path from "node:path";
import { afterEach, describe, expect, it, vi } from "vitest";
import { AutomaticBuildDispatchSettledError } from "../src/automatic-build-dispatch-runtime";
import { automaticBuildDriverFailureResponse, runAutomaticBuildDriverCommand } from "../../../skills/build/automatic-build-driver";
import { resolveAutomaticBuildExecutorRegistryRoot } from "../src/automatic-build-executor-session";

vi.mock("node:os", async importOriginal => {
  const actual = await importOriginal<typeof import("node:os")>();
  return { ...actual, homedir: vi.fn(actual.homedir) };
});

afterEach(() => { vi.unstubAllEnvs(); vi.restoreAllMocks(); });

describe("build driver failure boundary", () => {
  it("accepts refill stdin larger than 64 KiB with the full completed-ref history", () => {
    const registry = mkdtempSync(path.join(tmpdir(), "build-driver-large-refill-"));
    const repoRoot = fileURLToPath(new URL("../../../", import.meta.url));
    const request = JSON.stringify({ version: "automatic_build_refill_request.v1",
      invocation_ref: `abinv1_${"b".repeat(64)}`, capacity_limit: 3, live_by_slot: {},
      completed_refs: Array.from({ length: 900 }, (_, index) => `abhandoff1_${index.toString(16).padStart(64, "0")}`),
      terminal_children: [] });
    expect(Buffer.byteLength(request, "utf8")).toBeGreaterThan(65_536);
    const result = spawnSync(process.execPath, [path.join(repoRoot, "node_modules", "tsx", "dist", "cli.mjs"),
      path.join(repoRoot, "skills", "build", "automatic-build-driver.ts")], {
      cwd: repoRoot, input: request, encoding: "utf8", timeout: 30_000,
      env: { ...process.env, UNDERSTAND_BOOK_AUTOMATIC_BUILD_DRIVER_ROOT: registry },
    });
    expect(result.status, result.stderr).toBe(0);
    // A deliberately absent invocation proves stdin reached normal refill handling.
    expect(JSON.parse(result.stdout)).toMatchObject({ version: "automatic_build_step.v1", action: {
      kind: "NEEDS_USER", projection: { code: "invocation_record_missing" },
    } });
  }, 40_000);

  it("shares durable control storage with the executor when the OS temp directory changes", () => {
    const directory = mkdtempSync(path.join(tmpdir(), "build-driver-persistent-"));
    const userDirectory = path.join(directory, "user");
    vi.mocked(os.homedir).mockReturnValue(userDirectory);
    vi.stubEnv("UNDERSTAND_BOOK_AUTOMATIC_BUILD_DRIVER_ROOT", undefined);
    vi.stubEnv("TEMP", path.join(directory, "temp-before"));
    vi.stubEnv("TMP", path.join(directory, "temp-before"));
    vi.stubEnv("TMPDIR", path.join(directory, "temp-before"));
    const response = automaticBuildDriverFailureResponse(new Error("control storage test"));
    if (response.version !== "automatic_build_step.v1" || response.action.kind !== "NEEDS_USER") {
      throw new Error("expected bounded diagnostic");
    }
    vi.stubEnv("TEMP", path.join(directory, "temp-after"));
    vi.stubEnv("TMP", path.join(directory, "temp-after"));
    vi.stubEnv("TMPDIR", path.join(directory, "temp-after"));
    const registry = resolveAutomaticBuildExecutorRegistryRoot();
    expect(registry).toBe(path.join(userDirectory, ".understand-book", "automatic-build-driver-v1"));
    expect(existsSync(path.join(registry, "diagnostics", `${response.action.request_id}.json`))).toBe(true);
  });

  it.each(["automatic_build_step_request.v1", "automatic_build_refill_request.v1"])(
    "identifies a missing invocation for %s without exposing workspace input", version => {
      const registry = mkdtempSync(path.join(tmpdir(), "build-driver-missing-invocation-"));
      vi.stubEnv("UNDERSTAND_BOOK_AUTOMATIC_BUILD_DRIVER_ROOT", registry);
      let response: unknown;
      try {
        runAutomaticBuildDriverCommand({ version, invocation_ref: `abinv1_${"b".repeat(64)}`,
          ...(version === "automatic_build_step_request.v1" ? { available_agent_slots: 3 }
            : { capacity_limit: 3, live_by_slot: {}, completed_refs: [], terminal_children: [] }) });
      } catch (error) { response = automaticBuildDriverFailureResponse(error); }
      expect(response).toMatchObject({ version: "automatic_build_step.v1", action: {
        kind: "NEEDS_USER", reason: "build_engine_failed", choices: [],
        projection: { category: "internal", code: "invocation_record_missing" },
      } });
      expect(JSON.stringify(response)).not.toContain(registry);
      expect(JSON.stringify(response)).toContain("confirmed plan");
    });

  it.each([{ capacity_limit: "3" }, { open_call_correction: "double-serialized" }])(
    "rejects malformed refill control before reading invocation state: %j", (invalid) => {
      const directory = mkdtempSync(path.join(tmpdir(), "build-refill-request-"));
      const registry = path.join(directory, "untouched-registry");
      vi.stubEnv("UNDERSTAND_BOOK_AUTOMATIC_BUILD_DRIVER_ROOT", registry);
      let response: unknown;
      try {
        runAutomaticBuildDriverCommand({ version: "automatic_build_refill_request.v1",
          invocation_ref: `abinv1_${"b".repeat(64)}`, capacity_limit: 3,
          live_by_slot: {}, completed_refs: [], terminal_children: [], ...invalid });
      } catch (error) { response = automaticBuildDriverFailureResponse(error); }
      expect(response).toMatchObject({ version: "automatic_build_request_error.v1", code: "invalid_refill_request" });
      expect(existsSync(registry)).toBe(false);
    });
  const correction = {
    version: "automatic_build_open_call_correction.v1",
    issued_handoff_ref: `abhandoff1_${"a".repeat(64)}`,
    attempted_handoff_ref: "PRIVATE_REJECTED_CALL",
    request_version: "automatic_build_executor_open_request.v3",
    field: "opaque_handoff_ref", phase: "open", cause: "invalid_ref",
    reported_diagnostic: { version: "automatic_build_executor_mcp_error.v2", status: "interrupted",
      category: "session", diagnostic_code: "invalid_arguments", phase: "open", field: "opaque_handoff_ref" },
  };

  it.each([
    ["double-serialized correction", { open_call_correction: JSON.stringify(correction) }],
    ["correction in the wrong field", { executor_open_failure: correction }],
  ])("returns a caller-correctable rejection for %s before touching build state", (_label, observation) => {
    const directory = mkdtempSync(path.join(tmpdir(), "build-driver-request-"));
    const registry = path.join(directory, "untouched-registry");
    vi.stubEnv("UNDERSTAND_BOOK_AUTOMATIC_BUILD_DRIVER_ROOT", registry);
    let response: unknown;
    try {
      runAutomaticBuildDriverCommand({ version: "automatic_build_step_request.v1",
        invocation_ref: `abinv1_${"b".repeat(64)}`, available_agent_slots: 3, ...observation });
    } catch (error) {
      response = automaticBuildDriverFailureResponse(error);
    }
    expect(response).toMatchObject({ version: "automatic_build_request_error.v1",
      code: "invalid_step_request", request_version: "automatic_build_step_request.v1" });
    expect(response).not.toHaveProperty("action");
    expect(JSON.stringify(response)).not.toContain("PRIVATE_REJECTED_CALL");
    expect(existsSync(registry)).toBe(false);
  });

  it("rereads state when a dispatch finishes during publication instead of reporting an engine failure", () => {
    expect(automaticBuildDriverFailureResponse(new AutomaticBuildDispatchSettledError("dispatch has no current work unit")))
      .toEqual({ version: "automatic_build_step.v1", action: { kind: "WAIT", reason: "backoff", retry_after_ms: 50 } });
  });
  it("keeps bounded diagnostic details local and returns no invented recovery choice", () => {
    const root = mkdtempSync(path.join(tmpdir(), "build-driver-diagnostic-"));
    vi.stubEnv("UNDERSTAND_BOOK_AUTOMATIC_BUILD_DRIVER_ROOT", root);
    const error = new Error(`PRIVATE_CANDIDATE_TEXT ${"x".repeat(20_000)}`);
    const response = automaticBuildDriverFailureResponse(error);
    if (response.version !== "automatic_build_step.v1") throw new Error("expected engine boundary");
    expect(response.action).toMatchObject({ kind: "NEEDS_USER", reason: "build_engine_failed",
      choices: [], projection: { category: "internal", code: "build_step_failed" } });
    expect(JSON.stringify(response)).not.toContain("PRIVATE_CANDIDATE_TEXT");
    if (response.action.kind !== "NEEDS_USER") throw new Error("expected boundary");
    const diagnostic = JSON.parse(readFileSync(path.join(root, "diagnostics", `${response.action.request_id}.json`), "utf8"));
    expect(diagnostic.error.message).toContain("PRIVATE_CANDIDATE_TEXT");
    expect(diagnostic.error.message.length).toBeLessThanOrEqual(2048);
    expect(diagnostic.error.stack.length).toBeLessThanOrEqual(8192);
  });

  it("reports when the diagnostic cannot be saved without leaking the original error", () => {
    const root = mkdtempSync(path.join(tmpdir(), "build-driver-diagnostic-unavailable-"));
    const file = path.join(root, "file-not-directory");
    writeFileSync(file, "occupied");
    vi.stubEnv("UNDERSTAND_BOOK_AUTOMATIC_BUILD_DRIVER_ROOT", file);
    const response = automaticBuildDriverFailureResponse(new Error("PRIVATE_INPUT"));
    if (response.version !== "automatic_build_step.v1") throw new Error("expected engine boundary");
    expect(response.action).toMatchObject({ kind: "NEEDS_USER", choices: [],
      projection: { code: "build_step_failed_diagnostic_unavailable" } });
    expect(JSON.stringify(response)).not.toContain("PRIVATE_INPUT");
  });
});
