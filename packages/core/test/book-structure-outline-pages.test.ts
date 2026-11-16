import { describe, expect, it, onTestFinished } from "vitest";
import { mkdirSync, mkdtempSync, writeFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { resolveAutomaticBuildTarget } from "../src/build-orchestrator";
import { buildBookStructureUnitSources, createBookStructureExecutionContractsV2, BOOK_STRUCTURE_EXECUTION_PROMPTS_V2 } from "../src/book-structure";
import { structureSourceOutline, structureDiscoverySources } from "../src/book-structure-discovery";
import { routeStructureOrganization, structureOrganizationContracts, fitStructureOrganizationExecution } from "../src/book-structure-organization";
import { structureOutlineThemePageInput, applyStructureOutlineThemePageAction,
  type StructureOutlineThemePageContext, type StructureOutlineThemePageWork } from "../src/book-structure-outline-pages";
import { markdownToBlocks } from "../src/md-adapter";
import { segment } from "../src/segment";
import { resolveContentProfile } from "../src/content-profile";
import { CODEX_BUILD_EXECUTION_PROFILE_V1 } from "../src/build-execution-profile";
import { createCandidateTransportContract } from "../src/executor-transport";
import { freezeBookStructureGenerationTask, writeBookStructureGenerationCandidate } from "../src/book-structure-generation";

const contracts = () => structureOrganizationContracts(createBookStructureExecutionContractsV2({
  profile: resolveContentProfile("technical_learning"), quality_profile: "full", prompts: BOOK_STRUCTURE_EXECUTION_PROMPTS_V2,
}).stitch_fragment);

function pageFixture(count = 48) {
  const source = { version: "book_structure_outline_input.v1" as const, preface: [],
    chapters: Array.from({ length: count }, (_, index) => ({
      unit_lid: String(index + 1), title: `第${index + 1}章`,
      overview: { text: "解释条件、过程和结果。".repeat(50), evidence_lids: [`${index + 1}.1`] },
    })) };
  const context: StructureOutlineThemePageContext = { source,
    chapters: source.chapters.map(chapter => ({ unit_lid: chapter.unit_lid,
      question: { text: "条件怎样影响执行结果？", evidence_lids: chapter.overview.evidence_lids },
      progression: { text: "在前文基础上介绍新的执行条件。", evidence_lids: chapter.overview.evidence_lids },
    })), chapter_page_size: 16, theme_page_size: 16 };
  const work: StructureOutlineThemePageWork = {
    themes: [], complete: false, chapter_offset: 0, theme_offset: 0, covered_unit_lids: [], reading: [],
  };
  return { context, work };
}

describe("paged whole-book outline themes", () => {
  it("requires current delivered evidence, supports distant members and covers every chapter before finish", () => {
    const { context, work } = pageFixture();
    const apply = (state: StructureOutlineThemePageWork, action: unknown) => applyStructureOutlineThemePageAction(state, action, context);
    expect(() => apply(work, { kind: "finish" })).toThrow(/every chapter/);
    expect(() => apply(work, { kind: "browse", offset: 32 })).toThrow(/next_offset/);
    expect(() => apply(work, { kind: "append", themes: [{
      question: { text: "执行条件怎样发展？", evidence_lids: ["48.1"] }, unit_lids: ["48"],
    }] })).toThrow();
    let current = apply(work, { kind: "append", themes: [{
      question: { text: "执行条件怎样发展？", evidence_lids: ["1.1"] }, unit_lids: ["1"],
    }] });
    current = apply(current, { kind: "browse", offset: 16 });
    current = apply(current, { kind: "extend", theme_index: 0, unit_lids: ["17"], evidence_lids: ["17.1"] });
    expect(() => apply(current, { kind: "extend", theme_index: 0, unit_lids: ["48"], evidence_lids: ["48.1"] })).toThrow();
    current = apply(current, { kind: "inspect", unit_lids: ["1", "48"] });
    current = apply(current, { kind: "extend", theme_index: 0, unit_lids: ["48"], evidence_lids: ["48.1"] });
    expect(current.themes[0]).toEqual({
      question: { text: "执行条件怎样发展？", evidence_lids: ["1.1", "17.1", "48.1"] }, unit_lids: ["1", "17", "48"],
    });
    expect(() => apply(current, { kind: "finish" })).toThrow(/every chapter/);
    current = apply(current, { kind: "browse", offset: 32 });
    const finished = apply(current, { kind: "finish" });
    expect(finished.covered_unit_lids).toHaveLength(48);
    expect(finished.complete).toBe(true);
    expect(work.covered_unit_lids).toEqual([]);
  });

  it("fits growing chapter and theme pages and binds coverage to the fitted delivery", () => {
    const { context, work } = pageFixture();
    for (const chapter of context.chapters) {
      chapter.question.text = "条".repeat(600);
      chapter.progression.text = "件".repeat(600);
    }
    work.themes = context.chapters.slice(0, 24).map(chapter => ({
      question: { text: "题".repeat(600), evidence_lids: chapter.question.evidence_lids }, unit_lids: [chapter.unit_lid],
    }));
    const transport = CODEX_BUILD_EXECUTION_PROFILE_V1.transport_profile;
    const fitted = fitStructureOrganizationExecution({
      contract: contracts().structure_outline_theme_page, transport_profile: transport,
      budget: { stage_body_limit_tokens: 20000, executor_context_floor_tokens: 32768, output_reserve_tokens: 5000,
        max_candidate_tokens: Math.min(5000, createCandidateTransportContract(transport).candidate_value_max_estimated_tokens), safety_margin_tokens: 512 },
      packet: { version: "book_structure_organization_input.v1", phase: "outline_theme_page", context, work,
        body: structureOutlineThemePageInput(work, context), reference_scope: { unit_lids: context.chapters.map(c => c.unit_lid), dependency_target_lids: [], evidence_by_unit: {} } },
    });
    expect(fitted.evaluation.status).toBe("within_limit");
    if (fitted.packet.phase !== "outline_theme_page") throw new Error("expected theme page");
    const fittedContext = fitted.packet.context;
    const delivered = structureOutlineThemePageInput(work, fittedContext);
    expect(delivered.chapters.items.length).toBeLessThan(16);
    const next = applyStructureOutlineThemePageAction(work, { kind: "browse", offset: delivered.chapters.next_offset }, fittedContext);
    expect(next.covered_unit_lids).toEqual(delivered.chapters.items.map(chapter => chapter.unit_lid));
    expect(() => applyStructureOutlineThemePageAction(work, {
      kind: "extend", theme_index: 23, unit_lids: ["1"], evidence_lids: ["1.1"],
    }, fittedContext)).toThrow(/directory entry/);
    const moved = applyStructureOutlineThemePageAction(work, { kind: "browse_themes", offset: 23 }, fittedContext);
    expect(structureOutlineThemePageInput(moved, fittedContext).themes.items[0].theme_index).toBe(23);
  });

  it("resumes accepted chapter batches and paged theme actions through the production writer", async () => {
    const root = mkdtempSync(path.join(tmpdir(), "outline-page-recovery-"));
    onTestFinished(() => rmSync(root, { recursive: true, force: true }));
    const source = path.join(root, "book.md");
    const text = Array.from({ length: 48 }, (_, index) =>
      `# 第${index + 1}章\n\n${"解释执行条件以及结果之间的关系。".repeat(35)}\n\n${"保留来源和恢复步骤。".repeat(60)}\n`).join("\n");
    writeFileSync(source, text);
    const target = resolveAutomaticBuildTarget(source, root);
    mkdirSync(target.workspace_dir, { recursive: true });
    writeFileSync(path.join(target.workspace_dir, "base.json"), JSON.stringify({ graph_nodes: [], graph_edges: [] }));
    const header = { book_id: target.book_id, profile_id: target.profile_id };
    for (const name of ["discourse_index.json", "formula_semantics.json"]) {
      writeFileSync(path.join(target.workspace_dir, name), JSON.stringify({ header, items: [] }));
    }
    const nodes = segment(markdownToBlocks(text));
    const units = buildBookStructureUnitSources({ source: text, lidNodes: nodes });
    const outline = structureSourceOutline(structureDiscoverySources(units, nodes, text), nodes, text);
    const route = () => routeStructureOrganization({ target, execution_profile: CODEX_BUILD_EXECUTION_PROFILE_V1,
      contracts: contracts(), outline, context: { catalog: { version: "book_structure_candidates.v1", candidates: [] }, titles: {}, excerpts: [] }, stop_after_outline: true });
    const first = route();
    const submit = async (task: typeof first.tasks[number], candidate: unknown) => {
      freezeBookStructureGenerationTask(target, task);
      writeBookStructureGenerationCandidate({ target, task, candidate,
        provenance: { executor: "outline-page-test", attempt: 1, generated_at: "2026-11-16T00:00:00.000Z" } });
      await new Promise(resolve => setImmediate(resolve));
    };
    for (const task of first.tasks) {
      if (!("phase" in task.input) || task.input.phase !== "outline_chapters") throw new Error("expected chapter batch");
      await submit(task, { chapters: task.input.context.chapters.map(chapter => ({ unit_lid: chapter.unit_lid,
        question: { text: "条件怎样影响执行结果？", evidence_lids: chapter.overview.evidence_lids },
        progression: { text: "在前文基础上介绍新的执行条件。", evidence_lids: chapter.overview.evidence_lids },
      })) });
    }
    const currentTask = () => {
      const current = route();
      expect(current.blocked).toBeUndefined();
      expect(current.pending).toHaveLength(1);
      const task = current.tasks.find(task => current.pending.includes(task.descriptor.work_unit_id))!;
      expect(task.descriptor.kind).toBe("structure_outline_theme_page");
      expect(task.descriptor.execution_budget_proof.estimated_rendered_tokens).toBeLessThanOrEqual(20000);
      return task;
    };
    const member = outline.chapters[0];
    const append = currentTask();
    await submit(append, { kind: "append", themes: [{
      question: { text: "执行条件怎样发展？", evidence_lids: member.overview.evidence_lids }, unit_lids: [member.unit_lid],
    }] });
    const resumed = currentTask();
    expect(resumed.descriptor.work_unit_id).not.toBe(append.descriptor.work_unit_id);
    expect(currentTask().descriptor.work_unit_id).toBe(resumed.descriptor.work_unit_id);
    if (!("phase" in resumed.input) || resumed.input.phase !== "outline_theme_page") throw new Error("expected theme page");
    expect(resumed.input.work.themes).toHaveLength(1);
    await submit(resumed, { kind: "browse", offset: 16 });
    const second = outline.chapters[16];
    await submit(currentTask(), { kind: "extend", theme_index: 0,
      unit_lids: [second.unit_lid], evidence_lids: second.overview.evidence_lids });
    await submit(currentTask(), { kind: "browse", offset: 32 });
    await submit(currentTask(), { kind: "finish" });
    const completed = route();
    expect(completed.pending).toEqual([]);
    expect(completed.outline_result?.chapters.map(chapter => chapter.unit_lid)).toEqual(outline.chapters.map(chapter => chapter.unit_lid));
    expect(completed.outline_result?.themes[0].unit_lids).toEqual([member.unit_lid, second.unit_lid]);
    expect(route().outline_result).toEqual(completed.outline_result);
  }, 120000);
});
