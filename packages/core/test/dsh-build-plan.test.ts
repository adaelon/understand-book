import { mkdirSync, mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { describe, expect, it } from "vitest";
import { runAutomaticBuildDriverCommand, createAutomaticBuildInvocation, automaticBuildStep } from "../../../skills/build/automatic-build-driver";
import { buildSourceManifestV2 } from "../src/source-manifest";
import { emptyReconciliationSummary, sha256Text } from "../src/source-reconciliation";
import type { BuildPlanV1 } from "../src/build-intent";

function writeJson(file: string, value: unknown) { mkdirSync(path.dirname(file), { recursive: true }); writeFileSync(file, JSON.stringify(value)); }

// Same trusted post-reconciliation boundary as automatic-build-cli.test.ts.
function paperWorkspace(root: string) {
  const dir = path.join(root, ".understand-book", "paper-dsh");
  const source = "# Abstract\n\nThis paper studies transaction boundaries.\n";
  mkdirSync(dir, { recursive: true });
  for (const name of ["source.txt", "paper.md"]) writeFileSync(path.join(dir, name), source);
  writeFileSync(path.join(dir, "paper.pdf"), "synthetic-pdf");
  const fingerprint = { paper_md_sha256: sha256Text(source), paper_pdf_sha256: "sha-pdf", config_hash: "cfg-dsh" };
  writeJson(path.join(dir, "base.json"), { book_id: "paper-dsh", lid_nodes: [], graph_nodes: [], graph_edges: [] });
  writeJson(path.join(dir, "source_manifest.json"), buildSourceManifestV2({ book_id: "paper-dsh", source_sha256: sha256Text(source),
    original_pdf_path: "paper.pdf", original_pdf_sha256: "sha-pdf", pdf_source_map_path: "pdf_source_map.json",
    pdf_selection_map_manifest_path: "pdf_selection_map/manifest.json", alignment_report_path: "alignment_report.json", config_hash: "cfg-dsh" }));
  writeJson(path.join(dir, ".build", "source-reconciliation", "report.json"), { version: "source_reconciliation_report.v1", book_id: "paper-dsh",
    input_fingerprint: fingerprint, summary: { ...emptyReconciliationSummary(), verified: 1 }, unresolved: [] });
  writeJson(path.join(dir, ".build", "input", "manifest.json"), { version: "workbench_input_manifest.v1", book_id: "paper-dsh", profile_id: "paper", fingerprint,
    inputs: { paper_md: { path: "paper.md", original_path: null, sha256: fingerprint.paper_md_sha256 },
      paper_pdf: { path: "paper.pdf", original_path: null, sha256: fingerprint.paper_pdf_sha256 } } });
  return dir;
}

function create(root: string, target: string, plan: BuildPlanV1, planPath: string) {
  return createAutomaticBuildInvocation({ version: "automatic_build_invocation_create.v2", target_input: target, root_dir: root,
    build_plan_path: planPath, quality_profile: "full", max_parallel: 1, created_at: new Date().toISOString(),
    execution_profile: { profile_id: "dsh_native_v4", profile_revision: 1 },
    model_runtime: { version: "build_executor_model_runtime.v1", provider: "test", model: "test", reasoning_effort: null,
      max_output_tokens: 4096, context_window_tokens: 131072, safety_margin_tokens: 4096, context_source: "provider_model_metadata" },
    confirmation: { version: "dsh_build_confirmation.v1", plan_id: plan.plan_id, plan_revision: plan.revision, plan_digest: plan.plan_digest,
      root_session_id: "root", question_id: "plan", selected: "批准", answered_at: new Date().toISOString() } });
}

describe("DSH standard plan boundaries", () => {
  for (const kind of ["technical_learning", "paper"] as const) it(`${kind} preserves full closure and blocks zero budget before dispatch`, () => {
    const root = mkdtempSync(path.join(tmpdir(), "dsh-plan-"));
    const target = kind === "paper" ? paperWorkspace(root) : path.join(root, "book.md");
    if (kind !== "paper") writeFileSync(target, "# Transactions\n\nAtomic operations.\n");
    const prepared = runAutomaticBuildDriverCommand({ version: "dsh_build_prepare.v1", target_input: target, root_dir: root,
      pass2: "enabled", budget: { on_exceed: "needs_user", max_total_tokens: 0 } }) as { plan: BuildPlanV1; build_plan_path: string; review_markdown: string };
    expect(prepared.plan.content_profile.id).toBe(kind);
    expect(prepared.plan.public_stage_closure).toContain("pass2");
    if (kind === "paper") expect(prepared.plan.public_stage_closure).toEqual(expect.arrayContaining(["paper_metadata", "paper_lexicon", "paper_reading_guide"]));
    expect(prepared.review_markdown).not.toMatch(/transaction boundaries|Atomic operations/);
    const invocation = create(root, target, prepared.plan, prepared.build_plan_path);
    expect(automaticBuildStep({ version: "automatic_build_step_request.v1", invocation_ref: invocation.invocation_ref, available_agent_slots: 1 }).action)
      .toMatchObject({ kind: "NEEDS_USER", reason: "budget_exceeded" });
  }, 30000);

  it("rejects a valid private plan before starting DSH", () => {
    const root = mkdtempSync(path.join(tmpdir(), "dsh-private-"));
    const golden = JSON.parse(readFileSync(new URL("./fixtures/build-intent.v1.golden.json", import.meta.url), "utf8"));
    const plan = golden.plan as BuildPlanV1;
    expect(plan.private_artifacts.length).toBeGreaterThan(0);
    const planPath = path.join(root, "plan.json"); writeJson(planPath, plan);
    expect(() => create(root, "unused.md", plan, planPath)).toThrow(/DSH private plans/);
  });
});
