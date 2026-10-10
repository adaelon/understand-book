import { expect, it } from "vitest";
import { readFileSync } from "node:fs";
import { structureProductionFixture, structureProductionResponse } from "./helpers/book-structure-production";
import { BOOK_STRUCTURE_EXECUTION_PROMPTS_V2, createBookStructureExecutionContractsV2, routeBookStructureUnitWorkUnitsV2,
  type BookStructureUnitSource, type BookStructureFragmentInputV1 } from "../src/book-structure";
import { STRUCTURE_DISCOVERY_PROMPT, structureDiscoverySources } from "../src/book-structure-discovery";
import { collectStructureCandidates } from "../src/book-structure-candidates";
import { bookStructureReferenceScope } from "../src/book-structure-evidence";
import { resolveContentProfile } from "../src/content-profile";
import type { LidNode } from "../src/generated/LidNode";
import { readAutomaticBuildTaskStage } from "../src/build-orchestrator";
import { automaticBuildCandidateCorrection, automaticBuildFailureDiagnosticFromWriterError } from "../src/extractor-contract";

it("covers every complete leaf exactly once, splitting long natural sections without crossing their boundaries", () => {
  const excerpts = Array.from({ length: 25 }, (_, i) => ({ lid: `1.${i < 20 ? 1 : 2}.${i + 1}`, text: `Mechanism ${i}. ` + "condition and consequence ".repeat(80) + "END" }));
  let canonical = "";
  const nodes: LidNode[] = excerpts.map(e => {
    const start = canonical.length; canonical += e.text + "\n";
    return { lid: e.lid, path: e.lid.split(".").map(Number), kind: "paragraph", span: { start, end: canonical.length - 1 }, children: [] };
  });
  const source: BookStructureUnitSource = { job_id: "unit:1", unit_lid: "1", unit_kind: "chapter", title_path: [],
    leaf_lids: excerpts.map(e => e.lid), excerpts: excerpts.map(e => ({ ...e, text: e.text.slice(0, 1200) })),
    graph_nodes: [], graph_edges: [], discourse_items: [], formula_semantics: [], pass2_edges: [] };
  const full = structureDiscoverySources([source], nodes, canonical)[0];
  expect(full.excerpts).toEqual(excerpts);
  const result = routeBookStructureUnitWorkUnitsV2({ source: full, lid_nodes: nodes,
    target: { version: "build_target_ref.v2", workspace_dir: "C:/test", book_id: "test", profile_id: "technical_learning", input_fingerprint: "a".repeat(64) },
    source_fingerprint: "a".repeat(64), contracts: createBookStructureExecutionContractsV2({ profile: resolveContentProfile("technical_learning"), quality_profile: "full",
      prompts: { ...BOOK_STRUCTURE_EXECUTION_PROMPTS_V2, fragment: STRUCTURE_DISCOVERY_PROMPT } }),
    discovery: { outline: { chapters: [], themes: [] }, section_by_leaf: Object.fromEntries(excerpts.map(e => [e.lid, { lid: e.lid.split(".").slice(0, 2).join("."), title: "Natural section" }])) } });
  expect(result.status).toBe("ready"); if (result.status !== "ready") throw Error(JSON.stringify(result));
  expect(result.mode).toBe("fragmented"); expect(result.work_units.length).toBeGreaterThan(2);
  expect(result.coverage).toMatchObject({ covered_leaf_count: 25, expected_leaf_count: 25, gap_count: 0, core_overlap_count: 0 });
  expect(result.coverage.core_ranges.flatMap(r => r.leaf_lids)).toEqual(source.leaf_lids);
  for (const work of result.work_units) {
    const p = work.input as BookStructureFragmentInputV1;
    expect(p.core_leaf_lids.every(lid => lid.startsWith(p.discovery!.section_lid + "."))).toBe(true);
    expect(p.excerpts.every(e => e.text.endsWith("END"))).toBe(true);
  }
});

it("starts with source outline, persists distinct same-LID candidates and rejects heading evidence", () => {
  const f = structureProductionFixture(1);
  const tasks = () => f.get().pending_tasks.map(id => { const g = f.get().generation_tasks![id]; if (g.kind !== "book_structure") throw Error("structure"); return g.task; });
  expect(tasks().map(t => t.descriptor.kind)).toEqual(["structure_outline"]);
  const outline = tasks()[0];
  expect("phase" in outline.input && outline.input.phase === "outline" && outline.input.context.chapters[0].overview.text).toContain("Ownership mechanism");
  f.submit(outline);
  const task = tasks()[0];
  expect(task.descriptor.kind).toBe("structure_fragment");
  const candidate = structureProductionResponse(task) as any;
  const evidence = bookStructureReferenceScope(task.input).evidence_by_unit[task.parent_unit_lid];
  candidate.candidate_key_stops[1].lid = evidence[0];
  candidate.candidate_key_stops[1].reason.evidence_lids = [evidence[0]];
  candidate.candidate_key_stops[1].meaning = "Different mechanism with a separate condition";
  candidate.candidate_key_stops[1].conditions = ["A transfer is in progress"];
  const bad = structuredClone(candidate); bad.candidate_key_stops[0].reason.evidence_lids = [`${task.parent_unit_lid}.1`];
  expect(() => f.submit(task, bad)).toThrow(/evidence/);
  f.submit(task, candidate);
  const next = tasks()[0]; if (!("phase" in next.input) || next.input.phase !== "chapter") throw Error("chapter expected");
  const catalog = next.input.context.catalog;
  expect(catalog.candidates).toHaveLength(2);
  expect(new Set(catalog.candidates.map(c => c.lid)).size).toBe(1);
  expect(new Set(catalog.candidates.map(c => c.ref)).size).toBe(2);
  expect(catalog.candidates[1].conditions).toEqual(["A transfer is in progress"]);
  const contribution = { contribution_ref: task.descriptor.work_unit_id, unit_lid: task.parent_unit_lid,
    stops: candidate.candidate_key_stops, reference_scope: bookStructureReferenceScope(task.input) };
  expect(collectStructureCandidates([contribution, contribution])).toEqual(catalog);
  expect(f.get().work_units!.some(w => w.kind === "structure_reduce" || w.kind === "structure_unit")).toBe(false);
  expect(f.get().book_structure_progress?.discovery).toMatchObject({ fragments: { done: 1, total: 1 }, core_leaves: { done: 3, total: 3 }, candidates: 2 });
  expect(readFileSync(new URL("../../../agents/book-structure-discovery-extractor.md", import.meta.url), "utf8")).toBe(STRUCTURE_DISCOVERY_PROMPT);
}, 120000);

it("identifies missing and unexpected observation fields in the next-generation correction", () => {
  const f = structureProductionFixture(1);
  const outline = Object.values(f.get().generation_tasks!)[0];
  if (outline.kind !== "book_structure") throw Error("outline");
  f.submit(outline.task);
  const generation = Object.values(f.get().generation_tasks!).find(g => g.kind === "book_structure"
    && g.task.descriptor.kind === "structure_fragment");
  if (!generation || generation.kind !== "book_structure") throw Error("fragment");
  const candidate = structureProductionResponse(generation.task) as any;
  const invalid = structuredClone(candidate);
  delete invalid.evidence_lids;
  invalid.notes = "extra field";
  let error: unknown;
  try { f.submit(generation.task, invalid); } catch (caught) { error = caught; }
  const diagnostic = automaticBuildFailureDiagnosticFromWriterError(error, { writer_started: true });
  const correction = automaticBuildCandidateCorrection(diagnostic);
  expect(correction).toMatchObject({ code: "schema_invalid", json_pointer: "/" });
  expect(correction!.expected).toContain("missing: evidence_lids");
  expect(correction!.expected).toContain("unexpected: notes");
  expect(() => f.submit(generation.task, candidate)).not.toThrow();
}, 120000);

it("points invalid role values at their role_hints array item", () => {
  const f = structureProductionFixture(1);
  const outline = Object.values(f.get().generation_tasks!)[0];
  if (outline.kind !== "book_structure") throw Error("outline");
  f.submit(outline.task);
  const generation = Object.values(f.get().generation_tasks!).find(g => g.kind === "book_structure"
    && g.task.descriptor.kind === "structure_fragment");
  if (!generation || generation.kind !== "book_structure") throw Error("fragment");
  const candidate = structureProductionResponse(generation.task) as any;
  candidate.role_hints = ["foundation", "overview"];
  let error: unknown;
  try { f.submit(generation.task, candidate); } catch (caught) { error = caught; }
  const diagnostic = automaticBuildFailureDiagnosticFromWriterError(error, { writer_started: true });
  expect(automaticBuildCandidateCorrection(diagnostic)).toEqual({ code: "schema_invalid",
    json_pointer: "/role_hints/1", expected: "setup | foundation | method | application | case | synthesis" });
}, 120000);

it("focused discovery keeps the whole source outline and exactly the same chapter delivery", () => {
  const f = structureProductionFixture(2), first = Object.values(f.get().generation_tasks!)[0];
  if (first.kind !== "book_structure") throw Error("outline");
  f.submit(first.task);
  const full = f.get(), work = Object.values(full.generation_tasks!).find(g => g.kind === "book_structure" && g.task.parent_unit_lid === "1")!;
  if (work.kind !== "book_structure") throw Error("discovery");
  const focused = readAutomaticBuildTaskStage(f.target, { stage: "book_structure", work_unit_id: work.task.descriptor.work_unit_id, parent_lid: "1" }, "full")!;
  expect(focused.generation_tasks![work.task.descriptor.work_unit_id]).toEqual(work);
  expect(Object.values(focused.generation_tasks!).some(g => g.kind === "book_structure" && g.task.parent_unit_lid === "2")).toBe(false);
  const outline = Object.values(focused.generation_tasks!).find(g => g.kind === "book_structure" && g.task.descriptor.kind === "structure_outline")!;
  if (outline.kind !== "book_structure" || !("phase" in outline.task.input) || outline.task.input.phase !== "outline") throw Error("outline");
  expect(outline.task.input.context.chapters).toHaveLength(2);
}, 120000);

it("accepts an empty heading-only core even when a graph edge delivers body context, while body cores still require an overview", () => {
  const f = structureProductionFixture(1, undefined, "# Chapter\n\n## Container\n\n### Mechanism\n\nOwnership has an explicit transfer cost.\n");
  const tasks = () => Object.values(f.get().generation_tasks!).flatMap(g => g.kind === "book_structure"
    && g.task.descriptor.kind === "structure_fragment" ? [g.task] : []);
  const outline = Object.values(f.get().generation_tasks!)[0];
  if (outline.kind !== "book_structure") throw Error("outline");
  f.submit(outline.task);
  const excerpts = tasks().flatMap(t => "excerpts" in t.input ? t.input.excerpts : []);
  const heading = excerpts.find(e => e.text.trim() === "## Container")!;
  const body = excerpts.find(e => e.text.startsWith("Ownership"))!;
  expect(heading).toBeDefined(); expect(body).toBeDefined();
  f.write("base.json", { graph_nodes: [
    { id: "concept:container", type: "concept", name: "Container", occurrences: [heading.lid], source_lid: null },
    { id: "claim:cost", type: "claim", name: "Transfer cost", occurrences: [], source_lid: body.lid },
  ], graph_edges: [{ source: "claim:cost", target: "concept:container", type: "defines", direction: "directed", scope: "local", weight: 1 }] });
  const headingTask = tasks().find(t => "core_leaf_lids" in t.input && t.input.core_leaf_lids.length === 1
    && t.input.core_leaf_lids[0] === heading.lid)!;
  expect(bookStructureReferenceScope(headingTask.input).evidence_by_unit[headingTask.parent_unit_lid]).toContain(body.lid);
  const empty = { version: "book_structure_fragment_observation.v1", parent_unit_lid: headingTask.parent_unit_lid,
    summary_fragments: [], candidate_key_stops: [], role_hints: [], dependency_hints: [], evidence_lids: [] };
  expect(() => f.submit(headingTask, empty)).not.toThrow();
  const bodyTask = tasks().find(t => "core_leaf_lids" in t.input && t.input.core_leaf_lids.includes(body.lid))!;
  let bodyError: unknown;
  try { f.submit(bodyTask, empty); } catch (error) { bodyError = error; }
  expect(bodyError).toBeDefined();
  const diagnostic = automaticBuildFailureDiagnosticFromWriterError(bodyError, { writer_started: true });
  expect(diagnostic).toMatchObject({ category: "schema", code: "schema_invalid", phase: "artifact_writer",
    json_pointer: "/summary_fragments" });
  expect(automaticBuildCandidateCorrection(diagnostic)).toEqual({ code: "schema_invalid",
    json_pointer: "/summary_fragments", expected: "discovery body needs a grounded overview: at least one summary fragment with text and evidence_lids from this core" });
}, 120000);
