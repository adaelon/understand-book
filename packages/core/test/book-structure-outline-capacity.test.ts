import { describe, expect, it, onTestFinished } from "vitest";
import { mkdirSync, mkdtempSync, writeFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { resolveAutomaticBuildTarget, buildAutomaticBuildSnapshot, nextAutomaticBuildAction } from "../src/build-orchestrator";
import { buildBookStructureUnitSources, createBookStructureExecutionContractsV2, BOOK_STRUCTURE_EXECUTION_PROMPTS_V2, evaluateBookStructureExecution } from "../src/book-structure";
import { structureOrganizationContracts, renderStructureOrganizationInput } from "../src/book-structure-organization";
import { structureSourceOutline, structureDiscoverySources } from "../src/book-structure-discovery";
import { markdownToBlocks } from "../src/md-adapter";
import { segment } from "../src/segment";
import { resolveContentProfile } from "../src/content-profile";
import { CODEX_BUILD_EXECUTION_PROFILE_V1 } from "../src/build-execution-profile";
import { createCandidateTransportContract } from "../src/executor-transport";

describe("whole-book outline capacity", () => {
  it("reports actual outline budget failure before creating semantic tasks", () => {
    const root = mkdtempSync(path.join(tmpdir(), "outline-budget-"));
    onTestFinished(() => rmSync(root, { recursive: true, force: true }));
    const source = path.join(root, "large.md");
    writeFileSync(source, Array.from({ length: 35 }, (_, i) => `# 第${i + 1}章 构建机制\n\n` +
      Array.from({ length: 60 }, (_, j) => `## ${i + 1}.${j + 1} 本节详细解释任务执行与恢复机制\n\n正文说明执行条件。\n`).join("\n")).join("\n"));
    const target = resolveAutomaticBuildTarget(source, root);
    mkdirSync(target.workspace_dir, { recursive: true });
    writeFileSync(path.join(target.workspace_dir, "base.json"), JSON.stringify({ graph_nodes: [], graph_edges: [] }));
    const header = { book_id: target.book_id, profile_id: target.profile_id };
    writeFileSync(path.join(target.workspace_dir, "discourse_index.json"), JSON.stringify({ header, items: [] }));
    writeFileSync(path.join(target.workspace_dir, "formula_semantics.json"), JSON.stringify({ header, items: [] }));
    const snapshot = buildAutomaticBuildSnapshot(target, { stage: "book_structure", quality_profile: "full" });
    const stage = snapshot.stages.find(s => s.stage === "book_structure")!;
    expect(stage.pending_tasks).toEqual([]);
    expect(stage.structure_budget_blocked?.reasons).toContain("stage_limit");
    expect(nextAutomaticBuildAction(snapshot)).toMatchObject({ kind: "needs_user", reason: "structure_execution_budget_exceeded",
      stage: "book_structure", violations: expect.arrayContaining([{ code: "execution_body_limit",
        actual: stage.structure_budget_blocked!.estimated_rendered_tokens, limit: stage.structure_budget_blocked!.effective_body_limit_tokens }]) });
  });
  it("fits a 35-chapter outline without losing sections or source excerpts", () => {
    const source = Array.from({ length: 35 }, (_, i) => `# 第${i + 1}章 构建流程与状态管理\n\n${"构建输入保留章节结构和原文依据。".repeat(12)}\n\n` +
      Array.from({ length: 15 }, (_, j) => `## ${i + 1}.${j + 1} 任务执行与恢复机制\n\n正文说明执行条件及恢复步骤。\n`).join("\n")).join("\n");
    const lidNodes = segment(markdownToBlocks(source));
    const units = buildBookStructureUnitSources({ source, lidNodes });
    const outline = structureSourceOutline(structureDiscoverySources(units, lidNodes, source), lidNodes, source);
    const packet = { version: "book_structure_organization_input.v1" as const, phase: "outline" as const,
      body: outline, context: outline, reference_scope: { unit_lids: units.map(u => u.unit_lid), dependency_target_lids: [], evidence_by_unit: {} } };
    const delivery = { phase: packet.phase, input: packet.body, reference_scope: packet.reference_scope };
    const transport = CODEX_BUILD_EXECUTION_PROFILE_V1.transport_profile;
    const contract = structureOrganizationContracts(createBookStructureExecutionContractsV2({
      profile: resolveContentProfile("technical_learning"), quality_profile: "full", prompts: BOOK_STRUCTURE_EXECUTION_PROMPTS_V2,
    }).stitch_fragment).structure_outline;
    const evaluate = (rendered_input: string) => evaluateBookStructureExecution({ contract, rendered_input, transport_profile: transport,
      budget: { stage_body_limit_tokens: 20000, executor_context_floor_tokens: 32768, output_reserve_tokens: 5000,
        max_candidate_tokens: Math.min(5000, createCandidateTransportContract(transport).candidate_value_max_estimated_tokens), safety_margin_tokens: 512 } });
    expect(evaluate(JSON.stringify(delivery, null, 2) + "\n").status).toBe("blocked");
    const rendered = renderStructureOrganizationInput(packet);
    expect(JSON.parse(rendered)).toEqual(delivery);
    expect(outline.chapters).toHaveLength(35);
    expect(evaluate(rendered).status).toBe("within_limit");
    const small = structuredClone(packet);
    small.body.chapters = small.body.chapters.slice(0, 1);
    expect(renderStructureOrganizationInput(small)).toBe(JSON.stringify({ phase: small.phase, input: small.body,
      reference_scope: small.reference_scope }, null, 2) + "\n");
    const oversized = structuredClone(packet);
    oversized.body.chapters[0].overview.text += "真实内容超出单次容量。".repeat(4000);
    expect(evaluate(renderStructureOrganizationInput(oversized)).status).toBe("blocked");
  });
});
