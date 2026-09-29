import path from "node:path";
import { homedir } from "node:os";
import { findOwnedExecutorChildRollout, readOwnedExecutorOpenDiagnostic } from "../../packages/core/src/build-executor-call-diagnostics";

export async function runBuildChildDiagnostic(args: string[]): Promise<void> {
  if (args.length !== 3) throw new Error("usage: build.diagnose-child <parent-id> <child-id-or-name> <issued-handoff-ref>");
  const [parent, child, issued] = args as [string, string, string];
  const root = path.join(process.env.CODEX_HOME ?? path.join(homedir(), ".codex"), "sessions");
  try {
    const found = await findOwnedExecutorChildRollout(root, parent, child);
    if (!found) throw new Error("missing rollout");
    process.stdout.write(JSON.stringify(await readOwnedExecutorOpenDiagnostic(found.file, parent, found.child_id, issued)) + "\n");
  } catch (error) {
    process.stdout.write(JSON.stringify({ version: "automatic_build_child_open_diagnostic.v1",
      status: "evidence_missing", missing: error instanceof Error && error.message.startsWith("child diagnostic name is ambiguous")
        ? "unique child identity; name is ambiguous, use the child UUID"
        : "readable child rollout with matching parent/child ownership and valid issued ref" }) + "\n");
  }
}
