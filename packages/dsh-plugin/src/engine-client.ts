import { spawn, type ChildProcessWithoutNullStreams } from "node:child_process";
import { DSH_BUILD_EXECUTOR_CONTRACT_V1 as contract } from "../../core/src/dsh-build-executor-contract.ts";

export interface EngineConfig {
  executable: string;
  /** Used by source tests; packaged deployments only set executable. */
  prefixArgs?: readonly string[];
  driverRoot: string;
}

function start(config: EngineConfig, args: string[]) {
  const env: NodeJS.ProcessEnv = {};
  for (const key of ["SystemRoot", "WINDIR", "TEMP", "TMP", "USERPROFILE", "HOME", "LOCALAPPDATA", "PATH", "PATHEXT", "UNDERSTAND_BOOK_EMBEDDING_CONFIG"]) {
    if (process.env[key] !== undefined) env[key] = process.env[key];
  }
  env.UNDERSTAND_BOOK_AUTOMATIC_BUILD_DRIVER_ROOT = config.driverRoot;
  return spawn(config.executable, [...(config.prefixArgs ?? []), ...args], { windowsHide: true, shell: false, env, stdio: "pipe" });
}

function closed(child: ChildProcessWithoutNullStreams) {
  return new Promise<void>(resolve => { child.once("close", () => resolve()); child.once("error", () => resolve()); });
}

/** No raw Engine stderr is exposed to the root Agent. */
export async function engineCommand(config: EngineConfig, args: string[], request: unknown, signal?: AbortSignal): Promise<unknown> {
  if (signal?.aborted) throw new Error("build_cancelled");
  const child = start(config, args);
  const exit = closed(child);
  let output = "";
  let exceeded = false;
  child.stdout.setEncoding("utf8");
  child.stdout.on("data", text => {
    output += text;
    if (Buffer.byteLength(output) > 262144) { exceeded = true; output = ""; child.kill(); }
  });
  child.stderr.resume();
  const abort = () => child.kill();
  signal?.addEventListener("abort", abort, { once: true });
  let spawnError = false; child.on("error", () => { spawnError = true; });
  child.stdin.on("error", () => undefined);
  child.stdin.end(JSON.stringify(request));
  try {
    await exit;
    if (signal?.aborted) throw new Error("build_cancelled");
    if (spawnError || exceeded || child.exitCode !== 0) throw new Error("build_engine_command_failed");
    try { return JSON.parse(output); } catch { throw new Error("build_engine_response_invalid"); }
  } finally { signal?.removeEventListener("abort", abort); }
}

export interface ExecutorConnection {
  call(name: string, args: unknown, signal?: AbortSignal): Promise<unknown>;
  dispose(): Promise<void>;
}

/** One process and one serial request stream per child. No reconnect or automatic retry. */
export async function connectExecutor(config: EngineConfig, signal: AbortSignal): Promise<ExecutorConnection> {
  if (signal.aborted) throw new Error("executor_cancelled");
  const child = start(config, ["executor.mcp", "--bootstrap-version", contract.bootstrap_version,
    "--protocol-generation", contract.session_protocol]);
  const exit = closed(child);
  let terminal = false;
  let pending: { id: number; resolve(value: any): void; reject(error: Error): void } | undefined;
  let serial = Promise.resolve();
  let id = 0;
  let buffer = "";
  function fail() {
    terminal = true;
    pending?.reject(new Error("executor_connection_closed")); pending = undefined;
  }
  child.on("error", fail); child.on("close", fail); child.stdin.on("error", fail); child.stderr.resume();
  child.stdout.setEncoding("utf8");
  child.stdout.on("data", text => {
    buffer += text;
    for (;;) {
      const end = buffer.indexOf("\n");
      if (end < 0) break;
      const line = buffer.slice(0, end); buffer = buffer.slice(end + 1);
      try {
        if (Buffer.byteLength(line) > 131072) throw new Error();
        const response = JSON.parse(line);
        if (!pending || response.jsonrpc !== "2.0" || response.id !== pending.id) throw new Error();
        const current = pending; pending = undefined;
        if (response.error) current.reject(new Error("executor_rpc_rejected")); else current.resolve(response.result);
      } catch { fail(); child.kill(); }
    }
    if (Buffer.byteLength(buffer) > 131072) { fail(); child.kill(); }
  });
  const abort = () => { fail(); child.kill(); };
  signal.addEventListener("abort", abort, { once: true });
  let disposal: Promise<void> | undefined;
  const dispose = () => disposal ??= (async () => { fail(); signal.removeEventListener("abort", abort); child.kill(); await exit; })();
  function rpc(method: string, params: unknown) {
    const work = serial.then(() => new Promise<any>((resolve, reject) => {
      if (terminal || signal.aborted) { reject(new Error("executor_connection_closed")); return; }
      const requestId = ++id;
      pending = { id: requestId, resolve, reject };
      child.stdin.write(`${JSON.stringify({ jsonrpc: "2.0", id: requestId, method, params })}\n`);
    }));
    serial = work.then(() => undefined, () => undefined);
    return work;
  }
  try {
    const initialized = await rpc("initialize", { protocolVersion: "2025-06-18", capabilities: {
      experimental: { understand_book_executor: contract } }, clientInfo: { name: "understand-book-dsh", version: "0.1.0" } });
    if (initialized?.protocolVersion !== "2025-06-18"
      || JSON.stringify(initialized?.capabilities?.experimental?.understand_book_executor) !== JSON.stringify(contract)) {
      throw new Error("executor_protocol_incompatible");
    }
    return { dispose, async call(name, args, callSignal) {
      if (callSignal?.aborted) throw new Error("executor_cancelled");
      const cancel = () => { void dispose(); };
      callSignal?.addEventListener("abort", cancel, { once: true });
      try {
        const result = await rpc("tools/call", { name, arguments: args });
        if (!Array.isArray(result?.content) || result.content.length !== 1 || result.content[0].type !== "text") throw new Error("executor_response_invalid");
        const response = JSON.parse(result.content[0].text);
        if (result.isError) {
          // The Engine's bounded structured error is safe inside the child; control reports only its code.
          return response;
        }
        if (response.version !== contract.session_protocol) throw new Error("executor_protocol_incompatible");
        return response;
      } finally { callSignal?.removeEventListener("abort", cancel); }
    } };
  } catch (error) { await dispose(); throw error; }
}
