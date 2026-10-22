import { PassThrough, Writable } from "node:stream";
import path from "node:path";
import { describe, expect, it } from "vitest";
import { createBuildExecutorMcpSession, serveBuildExecutorMcpSession } from "../../../skills/build/build-executor-mcp";
import { BUILD_EXECUTOR_BOOTSTRAP_CONTRACT_V3 } from "../src/build-executor-connection-capability";

describe("executor MCP transport lifecycle", () => {
  it.each(["committed", "retryable_failure", "interrupted"] as const)(
    "flushes the complete %s response and closes a terminal connection even while the host keeps stdin open", async status => {
      const session = createBuildExecutorMcpSession({
        bootstrap_version: BUILD_EXECUTOR_BOOTSTRAP_CONTRACT_V3.version,
        protocol_generation: BUILD_EXECUTOR_BOOTSTRAP_CONTRACT_V3.session_protocol,
        session_private_root: path.resolve("unused-lifecycle-fixture"),
        execute_request: () => ({ version: "automatic_build_executor_session.v3", action: { kind: "DONE", status } }),
      });
      const input = new PassThrough();
      let bytes = "";
      let flush: (() => void) | undefined;
      const output = new Writable({ write(chunk, _encoding, callback) { bytes += chunk.toString(); flush = callback; } });
      serveBuildExecutorMcpSession(session, input, output);
      input.write(`${JSON.stringify({ jsonrpc: "2.0", id: 7, method: "tools/call", params: {
        name: "executor.open", arguments: { version: "automatic_build_executor_open_request.v3", opaque_handoff_ref: `abhandoff1_${"1".repeat(64)}` },
      } })}\n`);
      expect(input.destroyed).toBe(true);
      expect(output.writableFinished).toBe(false);
      const finished = new Promise<void>(resolve => output.once("finish", resolve));
      flush!();
      await finished;
      const response = JSON.parse(bytes);
      expect(response.id).toBe(7);
      expect(JSON.parse(response.result.content[0].text).action).toEqual({ kind: "DONE", status });
    });

  it("keeps a WAIT connection open for a later call", () => {
    const session = createBuildExecutorMcpSession({
      bootstrap_version: BUILD_EXECUTOR_BOOTSTRAP_CONTRACT_V3.version,
      protocol_generation: BUILD_EXECUTOR_BOOTSTRAP_CONTRACT_V3.session_protocol,
      session_private_root: path.resolve("unused-lifecycle-fixture"),
      execute_request: () => ({ version: "automatic_build_executor_session.v3", action: { kind: "WAIT", retry_after_ms: 1 } }),
    });
    const input = new PassThrough();
    const output = new PassThrough();
    serveBuildExecutorMcpSession(session, input, output);
    input.write(`${JSON.stringify({ jsonrpc: "2.0", id: 1, method: "tools/call", params: {
      name: "executor.open", arguments: { version: "automatic_build_executor_open_request.v3", opaque_handoff_ref: `abhandoff1_${"2".repeat(64)}` },
    } })}\n`);
    expect(input.destroyed).toBe(false);
    expect(output.writableEnded).toBe(false);
    input.destroy(); output.destroy();
  });

  it("closes after a terminal MCP error without losing its diagnostic", () => {
    const session = createBuildExecutorMcpSession({
      bootstrap_version: BUILD_EXECUTOR_BOOTSTRAP_CONTRACT_V3.version,
      protocol_generation: BUILD_EXECUTOR_BOOTSTRAP_CONTRACT_V3.session_protocol,
      session_private_root: path.resolve("unused-lifecycle-fixture"),
      execute_request: () => { throw new Error("synthetic backend failure"); },
    });
    const input = new PassThrough();
    const output = new PassThrough();
    serveBuildExecutorMcpSession(session, input, output);
    input.write(`${JSON.stringify({ jsonrpc: "2.0", id: 1, method: "tools/call", params: {
      name: "executor.open", arguments: { version: "automatic_build_executor_open_request.v3", opaque_handoff_ref: `abhandoff1_${"2".repeat(64)}` },
    } })}\n`);
    expect(input.destroyed).toBe(true);
    expect(output.writableEnded).toBe(true);
    const response = JSON.parse(output.read().toString());
    expect(JSON.parse(response.result.content[0].text)).toMatchObject({
      status: "interrupted", diagnostic_code: "executor_internal", phase: "open",
    });
  });

  it("allows the supported unbound ref correction before closing on DONE", () => {
    const session = createBuildExecutorMcpSession({
      bootstrap_version: BUILD_EXECUTOR_BOOTSTRAP_CONTRACT_V3.version,
      protocol_generation: BUILD_EXECUTOR_BOOTSTRAP_CONTRACT_V3.session_protocol,
      session_private_root: path.resolve("unused-lifecycle-fixture"),
      execute_request: () => ({ version: "automatic_build_executor_session.v3", action: { kind: "DONE", status: "committed" } }),
    });
    const input = new PassThrough();
    const output = new PassThrough();
    serveBuildExecutorMcpSession(session, input, output);
    const send = (ref: string) => input.write(`${JSON.stringify({ jsonrpc: "2.0", id: 1, method: "tools/call", params: {
      name: "executor.open", arguments: { version: "automatic_build_executor_open_request.v3", opaque_handoff_ref: ref },
    } })}\n`);
    send("abhandoff1_mistyped");
    expect(input.destroyed).toBe(false);
    expect(output.writableEnded).toBe(false);
    expect(JSON.parse(JSON.parse(output.read().toString()).result.content[0].text)).toMatchObject({
      diagnostic_code: "invalid_arguments", field: "opaque_handoff_ref",
    });
    send(`abhandoff1_${"3".repeat(64)}`);
    expect(input.destroyed).toBe(true);
    expect(output.writableEnded).toBe(true);
    expect(JSON.parse(JSON.parse(output.read().toString()).result.content[0].text).action.status).toBe("committed");
  });
});
