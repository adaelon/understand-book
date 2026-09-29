import { mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { describe, expect, it } from "vitest";
import { findOwnedExecutorChildRollout, readOwnedExecutorOpenDiagnostic } from "../src/build-executor-call-diagnostics";
import { validateAutomaticBuildOpenCallCorrection } from "../src/automatic-build-executor-session";

const parent = "01a09996-7874-7582-b6ce-a7cd03d2cf9a";
const child = "01a09a62-70ee-7603-b00f-73482425eada";
const issued = `abhandoff1_${"a".repeat(64)}`;
const error = { version: "automatic_build_executor_mcp_error.v2", status: "interrupted",
  category: "bootstrap", diagnostic_code: "protocol_incompatible", phase: "open" };
function event(tool: string, args: object, response: object) {
  return { type: "event_msg", payload: { type: "item_completed", thread_id: child, item: {
    type: "McpToolCall", server: "understand_book_build_executor", tool, arguments: args,
    result: { content: [{ type: "text", text: JSON.stringify(response) }] },
  } } };
}
function rollout(events: unknown[]) {
  const file = path.join(mkdtempSync(path.join(tmpdir(), "ub-call-diagnostic-")), "rollout.jsonl");
  writeFileSync(file, events.map(x => JSON.stringify(x)).join("\n"));
  return file;
}
const meta = { type: "session_meta", payload: { id: child, parent_thread_id: parent, base_instructions: "PRIVATE_INSTRUCTIONS" } };

describe("bounded executor call diagnostics", () => {
  it("resolves the collaboration child name through the owning parent's activity without exposing its history", async () => {
    const root = mkdtempSync(path.join(tmpdir(), "ub-child-name-"));
    const childFile = path.join(root, `rollout-${child}.jsonl`);
    writeFileSync(childFile, [meta, event("executor.open", {
      version: "automatic_build_executor_open_request.v3", opaque_handoff_ref: issued.slice(0, -2),
    }, error)].map(x => JSON.stringify(x)).join("\n"));
    const parentFile = path.join(root, `rollout-${parent}.jsonl`);
    writeFileSync(parentFile, [
      { type: "session_meta", payload: { id: parent } },
      { type: "response_item", payload: { content: "PRIVATE_PARENT_HISTORY" } },
      { type: "event_msg", payload: { type: "item_completed", thread_id: parent,
        item: { type: "SubAgentActivity", kind: "started", agent_thread_id: child, agent_path: "/root/book_exec_9" } } },
    ].map(x => JSON.stringify(x)).join("\n"));
    for (const name of [child, "/root/book_exec_9", "book_exec_9"]) {
      const found = await findOwnedExecutorChildRollout(root, parent, name);
      expect(found).toEqual({ file: childFile, child_id: child });
      const result = await readOwnedExecutorOpenDiagnostic(found!.file, parent, found!.child_id, issued);
      expect(result).toMatchObject({ status: "observed", child_id: child,
        open_call_correction: { cause: "invalid_ref" } });
      expect(JSON.stringify(result)).not.toContain("PRIVATE_");
    }
    expect(await findOwnedExecutorChildRollout(root, parent, "book_exec_99")).toBeUndefined();
  });

  it("does not select an arbitrary child when a name was reused, or accept another parent's activity", async () => {
    const root = mkdtempSync(path.join(tmpdir(), "ub-child-ambiguous-"));
    const file = path.join(root, `rollout-${parent}.jsonl`);
    const activity = (id: string) => ({ type: "event_msg", payload: {
      type: "item_completed", thread_id: parent, item: { type: "SubAgentActivity", kind: "started",
        agent_thread_id: id, agent_path: "/root/book_exec_9" },
    } });
    writeFileSync(file, [{ type: "session_meta", payload: { id: parent } }, activity(child), activity(parent)]
      .map(x => JSON.stringify(x)).join("\n"));
    await expect(findOwnedExecutorChildRollout(root, parent, "book_exec_9")).rejects.toThrow(/ambiguous/);
    writeFileSync(file, [{ type: "session_meta", payload: { id: child } }, activity(child)]
      .map(x => JSON.stringify(x)).join("\n"));
    await expect(findOwnedExecutorChildRollout(root, parent, "book_exec_9")).rejects.toThrow(/ownership/);
  });

  it("extracts actual nested MCP control facts, preserves the real error and excludes semantic bodies", async () => {
    const file = rollout([meta,
      { type: "response_item", payload: { type: "reasoning", content: "PRIVATE_REASONING" } },
      event("executor.open", { version: "automatic_build_executor_open_request.v3", opaque_handoff_ref: issued.slice(0, -2), extra: "PRIVATE_ARGUMENT" }, { ...error, message: "PRIVATE_ERROR" }),
      event("executor.submit_candidate", { candidate: "PRIVATE_CANDIDATE" }, { action: { kind: "GENERATE", semantic_input: "PRIVATE_INPUT" } }),
    ]);
    const result = await readOwnedExecutorOpenDiagnostic(file, parent, child, issued);
    expect(result).toMatchObject({ status: "observed", connection_state: "unbound",
      open_call_correction: { cause: "invalid_ref", issued_handoff_ref: issued, reported_diagnostic: error } });
    expect(result.recent_operations).toEqual(["executor.open", "executor.submit_candidate"]);
    expect(JSON.stringify(result)).not.toContain("PRIVATE_");
    await expect(readOwnedExecutorOpenDiagnostic(file, child, child, issued)).rejects.toThrow(/ownership/);
  });

  it("reports missing evidence and never invents a correction for a genuine version mismatch", async () => {
    expect(await readOwnedExecutorOpenDiagnostic(rollout([meta]), parent, child, issued))
      .toMatchObject({ status: "evidence_missing" });
    const result = await readOwnedExecutorOpenDiagnostic(rollout([meta,
      event("executor.open", { version: "automatic_build_executor_open_request.v999", opaque_handoff_ref: issued.slice(0, -2) }, error),
      event("executor.open", {}, { version: "automatic_build_executor_session.v3", action: { kind: "DONE" } }),
    ]), parent, child, issued);
    expect(result).toMatchObject({ status: "observed", connection_state: "terminal" });
    expect(result).not.toHaveProperty("open_call_correction");
  });

  it("rejects insufficient, unchanged, mismatched-cause, extra-field and unknown-version observations", () => {
    const valid = { version: "automatic_build_open_call_correction.v1", issued_handoff_ref: issued,
      attempted_handoff_ref: issued.slice(0, -2), request_version: "automatic_build_executor_open_request.v3",
      field: "opaque_handoff_ref", phase: "open", cause: "invalid_ref", reported_diagnostic: error };
    expect(validateAutomaticBuildOpenCallCorrection(valid)).toEqual(valid);
    for (const change of [{ attempted_handoff_ref: issued }, { cause: "different_ref" },
      { reported_diagnostic: undefined }, { request_version: "unknown" }, { phase: "candidate_submit" }, { candidate: {} }]) {
      expect(() => validateAutomaticBuildOpenCallCorrection({ ...valid, ...change })).toThrow();
    }
    expect(validateAutomaticBuildOpenCallCorrection({ ...valid, cause: "different_ref",
      attempted_handoff_ref: `abhandoff1_${"b".repeat(64)}` }).cause).toBe("different_ref");
  });

  it.each(["handoff_ref_mismatch", "executor_internal"])("does not call a format-valid failed open %s an unbound reusable connection", async (code) => {
    const result = await readOwnedExecutorOpenDiagnostic(rollout([meta,
      event("executor.open", { version: "automatic_build_executor_open_request.v3",
        opaque_handoff_ref: `abhandoff1_${"b".repeat(64)}` },
      { ...error, category: code === "executor_internal" ? "internal" : "session", diagnostic_code: code }),
    ]), parent, child, issued);
    expect(result.connection_state).toBe("bound");
    expect(result).toMatchObject({ open_call_correction: { cause: "different_ref" } });
  });
});
