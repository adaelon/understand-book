/** Source-only Engine executable for L1 integration tests. */
import { readFileSync, existsSync, writeFileSync, appendFileSync } from "node:fs";
import { runBuildExecutorMcpServer } from "../../../skills/build/build-executor-mcp.ts";
import { runAutomaticBuildDriverCommandWithPreparation } from "../../../skills/build/automatic-build-driver.ts";
const argv = process.argv.slice(2);
let dropCommitMarker: string | undefined;
let trace: string | undefined;
while (argv[0]?.startsWith("--dh5-")) {
  const option = argv.shift(); const value = argv.shift()!;
  if (option === "--dh5-drop-commit") dropCommitMarker = value;
  else if (option === "--dh5-trace") trace = value;
  else throw new Error("unknown test fault");
}
const [operation, ...args] = argv;
if (trace && operation === "executor.mcp") appendFileSync(trace, JSON.stringify({ pid: process.pid }) + "\n");
if (operation === "executor.mcp" && dropCommitMarker && !existsSync(dropCommitMarker)) {
  const original = process.stdout.write.bind(process.stdout);
  process.stdout.write = ((data: any, ...rest: any[]) => {
    try {
      const rpc = JSON.parse(String(data));
      const response = JSON.parse(rpc.result?.content?.[0]?.text ?? "null");
      if (response?.action?.kind === "DONE" && response.action.status === "committed") {
        writeFileSync(dropCommitMarker, "durable commit before response");
        process.exit(0);
      }
    } catch { /* Initialization and other responses are unchanged. */ }
    return (original as any)(data, ...rest);
  }) as typeof process.stdout.write;
}
if (operation === "executor.mcp") runBuildExecutorMcpServer(args);
else if (operation === "build.step") {
  const result = await runAutomaticBuildDriverCommandWithPreparation(JSON.parse(readFileSync(0, "utf8"))) as any;
  if (trace && result.action) appendFileSync(trace, JSON.stringify({ kind: result.action.kind, request_id: result.action.request_id }) + "\n");
  process.stdout.write(JSON.stringify(result));
}
else throw new Error("unsupported source test operation");
