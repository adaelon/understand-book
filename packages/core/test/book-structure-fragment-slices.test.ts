import { expect, it } from "vitest";
import { BOOK_STRUCTURE_EXECUTION_PROMPTS_V2, createBookStructureExecutionContractsV2, routeBookStructureUnitWorkUnitsV2,
  type BookStructureUnitSource, type BookStructureFragmentInputV1 } from "../src/book-structure";
import { STRUCTURE_DISCOVERY_PROMPT } from "../src/book-structure-discovery";
import { resolveContentProfile } from "../src/content-profile";
import { CODEX_BUILD_EXECUTION_PROFILE_V1 } from "../src/build-execution-profile";
import { createCandidateTransportContract } from "../src/executor-transport";
import { validateModelExecutionBudgetProof } from "../src/model-input-budget";
import { bookStructureReferenceScope } from "../src/book-structure-evidence";
import { structureProductionFixture, structureProductionResponse } from "./helpers/book-structure-production";
import { prepareBookStructureInputs, readAutomaticBuildTaskStage } from "../src/build-orchestrator";
import type { LidNode } from "../src/generated/LidNode";
import { estimateTokens } from "../src/window";
import { runAutomaticBuildCloseStage } from "../../../skills/build/automatic-build";

const transport = CODEX_BUILD_EXECUTION_PROFILE_V1.transport_profile;
const budget = { stage_body_limit_tokens: 6000, executor_context_floor_tokens: 16384, output_reserve_tokens: 3500,
  safety_margin_tokens: 512, max_candidate_tokens: Math.min(3500, createCandidateTransportContract(transport).candidate_value_max_estimated_tokens) };
const contracts = createBookStructureExecutionContractsV2({ profile: resolveContentProfile("technical_learning"), quality_profile: "full",
  prompts: { ...BOOK_STRUCTURE_EXECUTION_PROMPTS_V2, fragment: STRUCTURE_DISCOVERY_PROMPT } });
const longText = (tokens: number) => "Conditions preserve ownership and transfer the resource. ".repeat(Math.ceil(tokens * 4 / 56));

function fixture(texts: string[]) {
  let canonical = "prefix\n";
  const nodes: LidNode[] = texts.map((text, i) => {
    const start = canonical.length; canonical += "  " + text + "  \n";
    return { lid: `1.${i + 1}`, path: [1, i + 1], kind: "paragraph", span: { start, end: canonical.length - 1 }, children: [] };
  });
  const source: BookStructureUnitSource = { job_id: "unit:1", unit_lid: "1", unit_kind: "chapter", title_path: [],
    leaf_lids: nodes.map(n => n.lid), excerpts: texts.map((text, i) => ({ lid: nodes[i].lid, text })),
    graph_nodes: [], graph_edges: [], discourse_items: [], formula_semantics: [], pass2_edges: [] };
  const route = (canonicalSource = canonical) => routeBookStructureUnitWorkUnitsV2({ source, canonical_source: canonicalSource, lid_nodes: nodes,
    target: { version: "build_target_ref.v2", workspace_dir: "C:/test", book_id: "test", profile_id: "technical_learning", input_fingerprint: "a".repeat(64) },
    source_fingerprint: "a".repeat(64), contracts, transport_profile: transport, budget,
    discovery: { outline: { chapters: [], themes: [] }, section_by_leaf: Object.fromEntries(nodes.map(n => [n.lid, { lid: "1", title: "Ownership" }])) } });
  return { canonical, source, route };
}

it("splits a leaf whose complete text fits but whose rendered packet exceeds the body limit", () => {
  const text = longText(5800), f = fixture([text, "A later paragraph."]);
  expect(estimateTokens(text)).toBeLessThan(6000);
  const result = f.route();
  if (result.status !== "ready") throw Error(JSON.stringify(result));
  const sliced = result.work_units.filter(w => w.descriptor.input_basis.kind === "source_slices");
  expect(sliced.length).toBeGreaterThan(1);
  const slices = sliced.map(w => (w.input as BookStructureFragmentInputV1).excerpt_slice!);
  expect(slices.map(s => f.canonical.slice(s.core_span_utf16.start, s.core_span_utf16.end)).join("")).toBe(text);
  for (const work of sliced) {
    const packet = work.input as BookStructureFragmentInputV1, slice = packet.excerpt_slice!;
    expect(packet.excerpts[0].text).toBe(f.canonical.slice(slice.context_span_utf16.start, slice.context_span_utf16.end));
    expect(packet.core_leaf_lids).toEqual(["1.1"]);
    expect(work.descriptor.cost.estimated_input_tokens).toBeLessThanOrEqual(6000);
    expect(() => validateModelExecutionBudgetProof(work.descriptor.execution_budget_proof, transport)).not.toThrow();
  }
  expect(result.coverage).toMatchObject({ expected_leaf_count: 2, covered_leaf_count: 2, gap_count: 0, core_overlap_count: 0 });
  expect(result.coverage.core_ranges[0].slice_work_unit_ids).toEqual(sliced.map(w => w.descriptor.work_unit_id));
  expect(result.coverage.source_slices?.[0]).toMatchObject({ slice_count: slices.length, expected_core_utf16: text.length,
    covered_core_utf16: text.length, gap_utf16: 0, core_overlap_utf16: 0 });
});

it("pages both ends of oversized relation evidence and grants only the visible page's citations", () => {
  const f = fixture([longText(4900), longText(4900)]);
  f.source.graph_nodes = f.source.leaf_lids.map((lid, i) => ({ id: `concept:${i}`, type: "concept", name: `Concept ${i}`, occurrences: [lid], source_lid: null }));
  f.source.graph_edges = [{ source: "concept:0", target: "concept:1", type: "related_to", direction: "directed", scope: "local", weight: 1 }];
  const result = f.route();
  if (result.status !== "ready") throw Error(JSON.stringify(result));
  const pages = result.work_units.filter(w => "shard_kind" in w.input && w.input.shard_kind === "graph_edge");
  expect(pages.length).toBeGreaterThanOrEqual(2);
  for (const work of pages) {
    const packet = work.input as BookStructureFragmentInputV1;
    expect(work.descriptor.input_basis.kind).toBe("semantic_projection");
    expect(packet.core_leaf_lids).toEqual([]);
    expect(packet.graph_edges).toEqual(f.source.graph_edges);
    expect(packet.graph_nodes.map(n => n.id)).toEqual(["concept:0", "concept:1"]);
    expect(bookStructureReferenceScope(packet).evidence_by_unit["1"]).toEqual(packet.excerpts.map(e => e.lid));
    expect(packet.excerpts).toHaveLength(1);
    expect(work.descriptor.cost.estimated_input_tokens).toBeLessThanOrEqual(6000);
    expect(() => validateModelExecutionBudgetProof(work.descriptor.execution_budget_proof, transport)).not.toThrow();
  }
  for (const excerpt of f.source.excerpts) {
    const slices = pages.map(w => (w.input as BookStructureFragmentInputV1).excerpt_slice!).filter(s => s.parent_lid === excerpt.lid);
    expect(slices.map(s => f.canonical.slice(s.core_span_utf16.start, s.core_span_utf16.end)).join("")).toBe(excerpt.text);
  }
  expect(result.coverage.source_slices).toBeUndefined();
});

it("retains the atomic block when the semantic record itself cannot fit", () => {
  const f = fixture(["A short source paragraph."]);
  f.source.graph_nodes = [{ id: "concept:huge", type: "concept", name: "x".repeat(400000), occurrences: ["1.1"], source_lid: null }];
  expect(f.route()).toMatchObject({ status: "blocked", recovery: { code: "budget/atomic_input_item_too_large", item_kind: "graph_node" } });
});

it("pages shared long evidence once for a fitting group of auxiliary records", () => {
  const f = fixture([longText(6200)]);
  f.source.graph_nodes = Array.from({ length: 24 }, (_, i) => ({ id: `concept:${i}`, type: "concept", name: `Concept ${i}`, occurrences: ["1.1"], source_lid: null }));
  const result = f.route();
  if (result.status !== "ready") throw Error(JSON.stringify(result));
  const pages = result.work_units.filter(w => "shard_kind" in w.input && w.input.shard_kind === "graph_node");
  expect(pages.length).toBeGreaterThan(1);
  expect(pages.length).toBeLessThan(f.source.graph_nodes.length);
  for (const work of pages) {
    const packet = work.input as BookStructureFragmentInputV1;
    expect(packet.graph_nodes.map(n => n.id).sort()).toEqual(f.source.graph_nodes.map(n => n.id).sort());
    expect(work.descriptor.cost.estimated_input_tokens).toBeLessThanOrEqual(6000);
  }
  const slices = pages.map(w => (w.input as BookStructureFragmentInputV1).excerpt_slice!);
  expect(slices.map(s => f.canonical.slice(s.core_span_utf16.start, s.core_span_utf16.end)).join("")).toBe(f.source.excerpts[0].text);
});

it("rejects an unread counterpart in a persisted relation page and accepts its visible evidence", () => {
  const f = structureProductionFixture(1, undefined, `# Chapter\n\n${longText(4900)}\n\n${longText(4900)}\n`);
  f.write("base.json", { graph_nodes: ["1.2", "1.3"].map((lid, i) => ({ id: `concept:${i}`, type: "concept", name: `Concept ${i}`, occurrences: [lid], source_lid: null })),
    graph_edges: [{ source: "concept:0", target: "concept:1", type: "related_to", direction: "directed", scope: "local", weight: 1 }] });
  const outline = Object.values(f.get().generation_tasks!).find(g => g.kind === "book_structure" && g.task.descriptor.kind === "structure_outline")!;
  if (outline.kind !== "book_structure") throw Error("outline expected");
  f.submit(outline.task);
  const page = Object.values(f.get().generation_tasks!).find(g => g.kind === "book_structure" && "shard_kind" in g.task.input
    && g.task.input.shard_kind === "graph_edge" && g.task.input.excerpt_slice)!;
  if (page.kind !== "book_structure") throw Error("relation page expected");
  const allowed = bookStructureReferenceScope(page.task.input).evidence_by_unit["1"];
  expect(allowed).toHaveLength(1);
  const unread = ["1.2", "1.3"].find(lid => !allowed.includes(lid))!;
  const candidate = structureProductionResponse(page.task) as { summary_fragments: Array<{ text: string; evidence_lids: string[] }> };
  const bad = structuredClone(candidate);
  bad.summary_fragments[0].evidence_lids = [unread];
  expect(() => f.submit(page.task, bad)).toThrow(/evidence/);
  f.submit(page.task, candidate);
  expect(f.get().pending_tasks).not.toContain(page.task.descriptor.work_unit_id);
}, 120000);

it("resumes individual slices, preserves focused preparation and counts a leaf only after every slice commits", () => {
  const f = structureProductionFixture(1, undefined, `# Chapter\n\n${longText(5800)}\n\nA short following paragraph.\n`);
  const outline = Object.values(f.get().generation_tasks!).find(g => g.kind === "book_structure" && g.task.descriptor.kind === "structure_outline")!;
  if (outline.kind !== "book_structure") throw Error("outline expected");
  f.submit(outline.task);
  expect(prepareBookStructureInputs(f.target, "full")).toMatchObject({ status: "prepared" });
  const state = f.get();
  const tasks = Object.values(state.generation_tasks!).flatMap(g => g.kind === "book_structure" && g.task.output_role === "unit_observation" ? [g.task] : []);
  const slices = tasks.filter(t => t.descriptor.input_basis.kind === "source_slices");
  expect(slices.length).toBeGreaterThan(1);
  for (const task of tasks.filter(t => !slices.includes(t))) f.submit(task);
  const before = f.get().book_structure_progress!.discovery!.core_leaves.done;
  const first = slices[0];
  const focused = readAutomaticBuildTaskStage(f.target, { stage: "book_structure", work_unit_id: first.descriptor.work_unit_id, parent_lid: first.parent_unit_lid }, "full")!;
  expect(focused.generation_tasks![first.descriptor.work_unit_id]).toEqual({ kind: "book_structure", task: first });
  const bad = structureProductionResponse(first) as { evidence_lids: string[] };
  bad.evidence_lids = ["1.3"];
  expect(() => f.submit(first, bad)).toThrow(/evidence/);
  f.submit(first);
  expect(f.get().book_structure_progress!.discovery!.core_leaves.done).toBe(before);
  expect(f.get().pending_tasks).not.toContain(first.descriptor.work_unit_id);
  for (const task of slices.slice(1)) f.submit(task);
  expect(f.get().book_structure_progress!.discovery!.core_leaves.done).toBe(before + 1);
  expect(f.get().quality_routing!.coverage).toHaveLength(1);
  for (let round = 0; round < 24; round++) {
    const state = f.get();
    if (!state.pending_tasks.length) break;
    for (const id of state.pending_tasks) {
      const generation = state.generation_tasks![id];
      if (generation.kind !== "book_structure") throw Error("structure task expected");
      f.submit(generation.task);
    }
  }
  expect(f.get().pending_tasks).toEqual([]);
  expect(runAutomaticBuildCloseStage(f.source, f.root, "book_structure", { quality_profile: "full" })).toMatchObject({ status: "closed" });
}, 120000);
