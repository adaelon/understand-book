import { describe, expect, it, onTestFinished } from "vitest";
import { mkdirSync, mkdtempSync, writeFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { resolveAutomaticBuildTarget, buildAutomaticBuildSnapshot, nextAutomaticBuildAction } from "../src/build-orchestrator";
import { buildBookStructureUnitSources, createBookStructureExecutionContractsV2, BOOK_STRUCTURE_EXECUTION_PROMPTS_V2, evaluateBookStructureExecution } from "../src/book-structure";
import { structureOrganizationContracts, renderStructureOrganizationInput, routeStructureOrganization, type StructureOrganizationInput } from "../src/book-structure-organization";
import { structureSourceOutline, structureDiscoverySources } from "../src/book-structure-discovery";
import { markdownToBlocks } from "../src/md-adapter";
import { segment } from "../src/segment";
import { resolveContentProfile } from "../src/content-profile";
import { CODEX_BUILD_EXECUTION_PROFILE_V1 } from "../src/build-execution-profile";
import { createCandidateTransportContract, measureExecutorCandidateRequest } from "../src/executor-transport";
import { estimateTokens } from "../src/window";
import { collectStructureCandidates } from "../src/book-structure-candidates";
import { freezeBookStructureGenerationTask, writeBookStructureGenerationCandidate } from "../src/book-structure-generation";

describe("whole-book outline capacity", () => {
  it("routes an oversized whole-book outline into executable chapter batches", () => {
    const root = mkdtempSync(path.join(tmpdir(), "outline-input-batches-"));
    onTestFinished(() => rmSync(root, { recursive: true, force: true }));
    const source = path.join(root, "book.md");
    const text = Array.from({ length: 48 }, (_, index) =>
      `# 第${index + 1}章\n\n${"解释执行条件以及结果之间的关系。".repeat(35)}\n\n${"保留来源和恢复步骤。".repeat(60)}\n`).join("\n");
    writeFileSync(source, text);
    const target = resolveAutomaticBuildTarget(source, root);
    const nodes = segment(markdownToBlocks(text));
    const sources = buildBookStructureUnitSources({ source: text, lidNodes: nodes });
    const outline = structureSourceOutline(structureDiscoverySources(sources, nodes, text), nodes, text);
    const result = routeStructureOrganization({ target, execution_profile: CODEX_BUILD_EXECUTION_PROFILE_V1,
      contracts: structureOrganizationContracts(createBookStructureExecutionContractsV2({
        profile: resolveContentProfile("technical_learning"), quality_profile: "full", prompts: BOOK_STRUCTURE_EXECUTION_PROMPTS_V2,
      }).stitch_fragment), outline, context: { catalog: collectStructureCandidates([]), titles: {}, excerpts: [] }, stop_after_outline: true });
    expect(result.blocked).toBeUndefined();
    expect(result.pending).toHaveLength(12);
    const delivered = result.tasks.flatMap(task => {
      if (!("phase" in task.input) || task.input.phase !== "outline_chapters") throw new Error("expected chapter batch");
      expect(task.descriptor.execution_budget_proof.estimated_rendered_tokens).toBeLessThanOrEqual(20000);
      return task.input.context.chapters;
    });
    expect(delivered).toEqual(outline.chapters);
  });
  it("assembles all 184 source units from independently bounded, resumable outline submissions", async () => {
    const root = mkdtempSync(path.join(tmpdir(), "outline-output-budget-"));
    onTestFinished(() => rmSync(root, { recursive: true, force: true }));
    const source = path.join(root, "book.md");
    const text = Array.from({ length: 184 }, (_, index) => `# 第${index + 1}单元\n\n说明输入、执行条件以及结果之间的关系。\n`).join("\n");
    writeFileSync(source, text);
    const target = resolveAutomaticBuildTarget(source, root);
    mkdirSync(target.workspace_dir, { recursive: true });
    writeFileSync(path.join(target.workspace_dir, "base.json"), JSON.stringify({ graph_nodes: [], graph_edges: [] }));
    const header = { book_id: target.book_id, profile_id: target.profile_id };
    for (const name of ["discourse_index.json", "formula_semantics.json"]) writeFileSync(path.join(target.workspace_dir, name), JSON.stringify({ header, items: [] }));
    const nodes = segment(markdownToBlocks(text)), sources = buildBookStructureUnitSources({ source: text, lidNodes: nodes });
    const outline = structureSourceOutline(structureDiscoverySources(sources, nodes, text), nodes, text);
    const chapters = outline.chapters;
    expect(chapters).toHaveLength(184);
    const answer = (chapter: typeof chapters[number]) => ({ unit_lid: chapter.unit_lid,
      question: { text: "本单元解释执行条件怎样影响结果。", evidence_lids: chapter.overview.evidence_lids },
      progression: { text: "在前面介绍的基础上展开具体执行步骤。", evidence_lids: chapter.overview.evidence_lids } });
    const transport = CODEX_BUILD_EXECUTION_PROFILE_V1.transport_profile;
    const measure = (candidate: unknown) => measureExecutorCandidateRequest({ version: "automatic_build_executor_candidate_submit.v3",
      opaque_session_ref: `absession1_${"0".repeat(64)}`, candidate_sink_ref: `absink1_${"0".repeat(64)}`, candidate }, transport);
    expect(measure({ chapters: chapters.map(answer), themes: [] }).blocking_reasons).toContain("token_cap_exceeded");
    const route = () => routeStructureOrganization({ target, execution_profile: CODEX_BUILD_EXECUTION_PROFILE_V1,
      contracts: structureOrganizationContracts(createBookStructureExecutionContractsV2({
        profile: resolveContentProfile("technical_learning"), quality_profile: "full", prompts: BOOK_STRUCTURE_EXECUTION_PROMPTS_V2,
      }).stitch_fragment), outline, context: { catalog: collectStructureCandidates([]), titles: {}, excerpts: [] }, stop_after_outline: true });
    const first = route();
    expect(first.pending.length).toBeGreaterThan(1);
    expect(first.tasks.every(task => task.descriptor.kind === "structure_outline_chapters")).toBe(true);
    const write = (task: typeof first.tasks[number], candidate: unknown) => {
      expect(measure(candidate).blocking_reasons).toEqual([]);
      freezeBookStructureGenerationTask(target, task);
      writeBookStructureGenerationCandidate({ target, task, candidate,
        provenance: { executor: "bounded-outline-test", attempt: 1, generated_at: "2026-10-29T00:00:00.000Z" } });
    };
    for (const [index, task] of first.tasks.entries()) {
      if (!("phase" in task.input) || task.input.phase !== "outline_chapters") throw new Error("expected outline chapter batch");
      const candidate = { chapters: task.input.context.chapters.map(answer) };
      if (index === 0) {
        freezeBookStructureGenerationTask(target, task);
        expect(() => writeBookStructureGenerationCandidate({ target, task, candidate: { chapters: candidate.chapters.slice(1) },
          provenance: { executor: "bounded-outline-test", attempt: 1, generated_at: "2026-10-29T00:00:00.000Z" } })).toThrow(/order/);
      }
      write(task, candidate);
      await new Promise(resolve => setImmediate(resolve));
      if (index === 0) {
        const resumed = route();
        expect(resumed.pending).not.toContain(task.descriptor.work_unit_id);
        expect(resumed.pending).toEqual(first.pending.slice(1));
        expect(resumed.outline_result).toBeUndefined();
      }
    }
    const members = chapters.slice(0, 3), memberLids = members.map(chapter => chapter.unit_lid);
    const memberEvidence = members.map(chapter => chapter.overview.evidence_lids[0]);
    for (const candidate of [
      { kind: "append", themes: [{ question: { text: "不同条件如何影响执行？", evidence_lids: memberEvidence.slice(0, 2) }, unit_lids: memberLids.slice(0, 2) }] },
      { kind: "extend", theme_index: 0, unit_lids: memberLids.slice(2), evidence_lids: memberEvidence.slice(2) },
      { kind: "finish" },
    ]) {
      const current = route();
      expect(current.pending).toHaveLength(1);
      expect(current.outline_result).toBeUndefined();
      write(current.tasks.find(task => current.pending.includes(task.descriptor.work_unit_id))!, candidate);
      await new Promise(resolve => setImmediate(resolve));
    }
    const completed = route();
    expect(completed.pending).toEqual([]);
    expect(completed.outline_result?.chapters).toEqual(chapters.map(answer));
    expect(completed.outline_result?.themes).toEqual([{ question: { text: "不同条件如何影响执行？", evidence_lids: memberEvidence }, unit_lids: memberLids }]);
  }, 120000);
  it.each(["chapter", "theme", "reconcile"] as const)("removes only formatting overhead from oversized %s inputs", phase => {
    // Serialization does not consume the phase-specific acceptance context.
    const packet = { version: "book_structure_organization_input.v1", phase, context: {}, body: { candidates: Array.from({ length: 700 }, (_, i) => ({
      ref: `candidate:${i}`, meaning: "a grounded mechanism", evidence_lids: ["1.1", "1.2"],
    })) }, reference_scope: { unit_lids: ["1"], dependency_target_lids: [], evidence_by_unit: {} } } as unknown as StructureOrganizationInput;
    const delivery = { phase, input: packet.body, reference_scope: packet.reference_scope };
    const pretty = JSON.stringify(delivery, null, 2) + "\n";
    expect(estimateTokens(pretty)).toBeGreaterThan(20000);
    const rendered = renderStructureOrganizationInput(packet);
    expect(estimateTokens(rendered)).toBeLessThanOrEqual(20000);
    expect(JSON.parse(rendered)).toEqual(delivery);
    const small = { ...packet, body: { candidates: [] } } as StructureOrganizationInput;
    expect(renderStructureOrganizationInput(small)).toBe(JSON.stringify({ phase, input: small.body,
      reference_scope: small.reference_scope }, null, 2) + "\n");
    const oversized = { ...packet, body: { text: "真实内容超出单次容量。".repeat(4000) } } as StructureOrganizationInput;
    expect(estimateTokens(renderStructureOrganizationInput(oversized))).toBeGreaterThan(20000);
  });
  it("routes a dense section directory with each chapter's own sections and evidence", () => {
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
    expect(stage.pending_tasks.length).toBeGreaterThan(1);
    expect(stage.structure_budget_blocked).toBeUndefined();
    expect(nextAutomaticBuildAction(snapshot)).toMatchObject({ kind: "extract", stage: "book_structure" });
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
