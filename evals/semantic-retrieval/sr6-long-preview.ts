import { mkdirSync, writeFileSync } from "node:fs";
import path from "node:path";
import { prepareExplicitLegacyBuildPlan, automaticBuildPlan } from "../../skills/build/automatic-build";
import { runAutomaticBuildDriverCommand } from "../../skills/build/automatic-build-driver";
import type { BuildPlanV1 } from "../../packages/core/src/build-intent";

// Development acceptance uses the current source Engine to author the same BuildPlan
// and exact retrieval confirmation projection as the installed entry point.
const [workspace, root, output, configFile, pass2] = process.argv.slice(2);
if (!workspace || !root || !output || !configFile || !["enabled", "disabled"].includes(pass2))
  throw new Error("Usage: sr6-long-preview.ts WORKSPACE ROOT OUTPUT LOCAL_CONFIG enabled|disabled");
const initial = prepareExplicitLegacyBuildPlan(path.resolve(workspace), path.resolve(root), {
  pass2: pass2 as "enabled" | "disabled", budget: { max_total_tokens: 5_000_000, max_wall_clock_minutes: 240, on_exceed: "needs_user" } });
const configured = runAutomaticBuildDriverCommand({ version: "build_retrieval_configure.v1", build_plan_path: initial.build_plan_path,
  retrieval_mode: "semantic_required", budget: { max_documents: 20_000, max_queries: 5_000, max_calls: 10_000 },
  config_file: path.resolve(configFile) }) as { build_plan_path: string; plan: BuildPlanV1; review_markdown: string };
const snapshot = automaticBuildPlan(path.resolve(workspace), path.resolve(root), { build_plan: configured.plan });
const result = { version: "sr6-long-preview.v1", source: path.resolve(workspace), build_plan_path: configured.build_plan_path,
  plan: configured.plan, retrieval_review: configured.review_markdown,
  stages: snapshot.snapshot.stages.map(s => ({ stage: s.stage, closed: s.closed, pending: s.pending_tasks.length,
    preparation_required: s.preparation_required })),
  next_action: snapshot.next_action, preflight: snapshot.preflight };
mkdirSync(path.dirname(path.resolve(output)), { recursive: true }); writeFileSync(output, JSON.stringify(result, null, 2) + "\n");
console.log(JSON.stringify({ output: path.resolve(output), ...result, plan: { status: configured.plan.status,
  plan_id: configured.plan.plan_id, plan_digest: configured.plan.plan_digest, public_stage_closure: configured.plan.public_stage_closure,
  create: configured.plan.create, reuse: configured.plan.reuse, excluded: configured.plan.excluded,
  estimate: configured.plan.estimate, budget: configured.plan.budget } }, null, 2));
