import { expect, it } from "vitest";
import { applyStructureThemePlanPageAction, finishStructureThemePlanPages, structureThemePlanPageInput,
  type StructureThemePlanPageContext, type StructureThemePlanPageWork } from "../src/book-structure-theme-plan-pages";
import { structureThemePlanningInput } from "../src/book-structure-themes";
import { fitStructureOrganizationExecution, structureOrganizationContracts } from "../src/book-structure-organization";
import { BOOK_STRUCTURE_EXECUTION_PROMPTS_V2, createBookStructureExecutionContractsV2 } from "../src/book-structure";
import { resolveContentProfile } from "../src/content-profile";
import { CODEX_BUILD_EXECUTION_PROFILE_V1 } from "../src/build-execution-profile";
import { createCandidateTransportContract } from "../src/executor-transport";
import { estimateTokens } from "../src/window";

function fixture(count: number) {
  const chapters = Array.from({ length: count }, (_, i) => ({
    unit_lid: String(i + 1), title: `第 ${i + 1} 章`,
    question: "这一章解决什么问题？",
    overview: { text: "在明确前提下选择相应方法，判断其收益、成本和适用条件。".repeat(16), evidence_lids: [`${i + 1}.1`] },
  }));
  const candidates = chapters.flatMap(c => Array.from({ length: 5 }, (_, i) => ({
    ref: `unit:${c.unit_lid}:fragment:0001#candidate-${i}`, unit_lid: c.unit_lid, section_lid: c.unit_lid,
    contribution_ref: `unit:${c.unit_lid}:fragment:0001`, local_id: `candidate-${i}`, lid: `${c.unit_lid}.1`,
    type: "claim" as const, meaning: "依据前提比较成本和收益。", conditions: [], aliases: [],
    reason: c.overview, evidence_lids: c.overview.evidence_lids,
  })));
  const context: StructureThemePlanPageContext = { chapter_page_size: 96, seed_page_size: 400, theme: {
    catalog: { version: "book_structure_candidates.v1", candidates }, chapters, excerpts: [], mode: "lexical_only",
    planning_candidate_refs: candidates.map(c => c.ref),
  } };
  const work: StructureThemePlanPageWork = { chapter_offset: 0, seed_offset: 0, themes: [], complete: false };
  return { context, work };
}

it("advances through every chapter and seed of the fitted delivery without skips, including the short tail", () => {
  const { context, work } = fixture(147);
  const full = structureThemePlanningInput(context.theme);
  expect(estimateTokens(JSON.stringify(full))).toBeGreaterThan(20000);
  const contract = structureOrganizationContracts(createBookStructureExecutionContractsV2({
    profile: resolveContentProfile("technical_learning"), quality_profile: "full", prompts: BOOK_STRUCTURE_EXECUTION_PROMPTS_V2,
  }).stitch_fragment).structure_theme_plan_page;
  const transport = CODEX_BUILD_EXECUTION_PROFILE_V1.transport_profile;
  const chapters: string[] = [], seeds: string[] = [];
  let current = work;
  for (let n = 0; !current.complete; n++) {
    expect(n).toBeLessThan(16);
    const fit = fitStructureOrganizationExecution({ contract, transport_profile: transport,
      budget: { stage_body_limit_tokens: 20000, executor_context_floor_tokens: 32768, output_reserve_tokens: 6500,
        max_candidate_tokens: Math.min(6500, createCandidateTransportContract(transport).candidate_value_max_estimated_tokens), safety_margin_tokens: 512 },
      packet: { version: "book_structure_organization_input.v1", phase: "plan_page", context, work: current,
        body: structureThemePlanPageInput(current, context), reference_scope: { unit_lids: [], dependency_target_lids: [], evidence_by_unit: {} } } });
    expect(fit.evaluation.status).toBe("within_limit");
    if (fit.packet.phase !== "plan_page") throw new Error("expected a planning page");
    const page = structureThemePlanPageInput(current, fit.packet.context);
    chapters.push(...page.chapters.items.map(c => c.unit_lid));
    seeds.push(...page.seeds.items.map(c => c.ref));
    current = applyStructureThemePlanPageAction(current, { kind: "advance", themes: [] }, fit.packet.context);
  }
  expect(chapters).toEqual(full.chapters.map(c => c.unit_lid));
  expect(seeds).toEqual(full.candidate_index.map(c => c.ref));
});

it("carries a tentative question across pages and limits new references to what was delivered", () => {
  const { context, work } = fixture(3);
  context.chapter_page_size = 1;
  context.seed_page_size = 5;
  const [first, , , , , second] = context.theme.planning_candidate_refs!;
  const newTheme = { ref: "theme:cost", question: "How does the cost change?", distinction: "Compares the cost across conditions", unit_lids: ["1"] };
  expect(() => applyStructureThemePlanPageAction(work,
    { kind: "advance", themes: [{ ...newTheme, member_refs: [second] }] }, context)).toThrow(/seed must be delivered/);
  const firstPage = applyStructureThemePlanPageAction(work,
    { kind: "advance", themes: [{ ...newTheme, member_refs: [first] }] }, context);
  expect(() => finishStructureThemePlanPages(firstPage, context.theme)).toThrow(/undelivered/);
  expect(() => applyStructureThemePlanPageAction(firstPage,
    { kind: "advance", themes: [{ ref: newTheme.ref, unit_lids: ["3"] }] }, context)).toThrow(/current delivery/);
  const secondPage = applyStructureThemePlanPageAction(firstPage,
    { kind: "advance", themes: [{ ref: newTheme.ref, unit_lids: ["2"], member_refs: [first, second] }] }, context);
  const lastPage = applyStructureThemePlanPageAction(secondPage,
    { kind: "advance", themes: [{ ...newTheme, ref: "theme:local", unit_lids: ["3"] }] }, context);
  expect(finishStructureThemePlanPages(lastPage, context.theme).works.map(w => w.plan))
    .toEqual([{ ...newTheme, unit_lids: ["1", "2"], member_refs: [first, second] }]);
});
