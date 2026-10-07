import { mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import path from "node:path";
import { tmpdir } from "node:os";
import { expect, onTestFinished } from "vitest";
import { buildAutomaticBuildSnapshot, resolveAutomaticBuildTarget } from "../../src/build-orchestrator";
import { freezeBookStructureGenerationTask, writeBookStructureGenerationCandidate, type BookStructureGenerationTaskV1 } from "../../src/book-structure-generation";
import { bookStructureReferenceScope } from "../../src/book-structure-evidence";
import { runAutomaticBuildCloseStage } from "../../../../skills/build/automatic-build";
import { freezeAutomaticBuildStagePolicySet } from "../../src/automatic-build-policy-generation";
import { freezePass1ShadowTask } from "../../src/pass1-reduction";
import { writePass1ProductionTaskArtifact } from "./model-input-routability-fixture";
import { freezeProfileSidecarSemanticFastPathTask, writeProfileSidecarSemanticFastPathCandidate } from "../../src/profile-sidecar-reduction";
import type { BuildRetrievalSelection } from "../../src/build-intent";
import type { BuildExecutionProfileV1 } from "../../src/build-execution-profile";

export function structureProductionFixture(count = 3, execution_profile?: BuildExecutionProfileV1, sourceText?: string) {
  const root = mkdtempSync(path.join(tmpdir(), "ub-bsr5-"));
  onTestFinished(() => rmSync(root, { recursive: true, force: true }));
  const source = path.join(root, "book.md");
  writeFileSync(source, sourceText ?? Array.from({ length: count }, (_, i) => `# Chapter ${i + 1}\n\nOwnership mechanism ${i + 1} has explicit conditions.\n\nTransfer mechanism ${i + 1} has a separate cost.\n`).join("\n"));
  const target = resolveAutomaticBuildTarget(source, root);
  mkdirSync(target.workspace_dir, { recursive: true });
  const write = (name: string, value: unknown) => writeFileSync(path.join(target.workspace_dir, name), JSON.stringify(value));
  const header = { book_id: target.book_id, profile_id: target.profile_id };
  write("base.json", { graph_nodes: [], graph_edges: [] });
  write("discourse_index.json", { header, items: [] }); write("formula_semantics.json", { header, items: [] });
  for (const prerequisite of ["pass1", "profile_sidecar"] as const) {
    for (let round = 0; round < 8; round++) {
      const state = buildAutomaticBuildSnapshot(target).stages.find(item => item.stage === prerequisite)!;
      if (!state.pending_tasks.length) break;
      freezeAutomaticBuildStagePolicySet(target, state.policy_set!);
      for (const id of state.pending_tasks) {
        const generation = state.generation_tasks![id];
        if (generation.kind === "pass1") {
          freezePass1ShadowTask(target, generation.task);
          writePass1ProductionTaskArtifact({ target, policy_generation_id: generation.task.policy_generation_id, work_unit_id: id, marker: "Ownership claim" });
        } else if (generation.kind === "profile_sidecar_fast_path") {
          freezeProfileSidecarSemanticFastPathTask(target, generation.task);
          writeProfileSidecarSemanticFastPathCandidate({ target, source: readFileSync(source, "utf8"), task: generation.task,
            candidate: { discourse_items: [{ lid: generation.task.packet.visible_lids[0], mode: "informative", relations: [] }] },
            provenance: { executor: "bsr5-fixture", attempt: 1, generated_at: "2026-10-01T00:00:00.000Z" } });
        } else throw Error("unexpected prerequisite task");
      }
    }
    expect(runAutomaticBuildCloseStage(source, root, prerequisite, { quality_profile: "full" })).toMatchObject({ status: "closed" });
  }
  const get = (retrieval?: BuildRetrievalSelection) => buildAutomaticBuildSnapshot(target, { retrieval, execution_profile }).stages.find(s => s.stage === "book_structure")!;
  const submit = (task: BookStructureGenerationTaskV1, candidate = structureProductionResponse(task)) => {
    freezeBookStructureGenerationTask(target, task);
    return writeBookStructureGenerationCandidate({ target, task, candidate,
      provenance: { executor: "bsr5-fixture", attempt: 1, generated_at: "2026-10-01T00:00:00.000Z" } });
  };
  return { root, source, target, get, submit, write };
}

/** Deterministic pipeline fixture, not a content-quality generator. */
export function structureProductionResponse(task: BookStructureGenerationTaskV1): unknown {
  if (task.output_role === "unit_observation") {
    const lid = task.parent_unit_lid, evidence = bookStructureReferenceScope(task.input).evidence_by_unit[lid];
    return { version: "book_structure_fragment_observation.v1", parent_unit_lid: lid,
      summary_fragments: evidence.length ? [{ text: "Resource ownership and transfer conditions.", evidence_lids: [evidence[0]] }] : [],
      candidate_key_stops: evidence.slice(0, 2).map((e, i) => ({ id: `stop-${i}`, lid: e, type: "claim",
        meaning: `Condition ${i}: ownership is established before transfer.`, conditions: [],
        reason: { text: `Condition ${i}: ownership is established before transfer.`, evidence_lids: [e] } })),
      role_hints: ["foundation"], dependency_hints: [], evidence_lids: evidence };
  }
  if (task.output_role === "unit_artifact") {
    const lid = task.parent_unit_lid, evidence = bookStructureReferenceScope(task.input).evidence_by_unit[lid];
    return { unit_card: { unit_lid: lid, role: "foundation", summary: { text: "Resource ownership and transfer conditions.", evidence_lids: [evidence[0]] },
      candidate_key_stops: evidence.slice(0, 2).map((e, i) => ({ id: `stop-${i}`, lid: e, type: "claim",
        reason: { text: `Condition ${i}: ownership is established before transfer.`, evidence_lids: [e] } })), depends_on: [], evidence_lids: evidence } };
  }
  if (!("phase" in task.input)) throw Error(`unsupported fixture ${task.output_role}`);
  const p = task.input;
  switch (p.phase) {
    case "outline": return { chapters: p.context.chapters.map(c => ({ unit_lid: c.unit_lid, question: c.overview, progression: c.overview })), themes: [] };
    case "chapter": {
      const candidates = p.context.catalog.candidates.filter(c => c.unit_lid === p.work.unit_lid);
      if (!candidates.length) {
        const index = p.context.excerpts.findIndex(e => e.unit_lid === p.work.unit_lid);
        if (!p.work.reading.length && !p.work.evidence_lids.length) return { kind: "read", indices: [index] };
        return { kind: "select", selection: { unit_lid: p.work.unit_lid, role: "foundation",
          summary: { text: "Chapter overview.", evidence_lids: [p.context.excerpts[index].lids[0]] }, accepted_stop_refs: [], macro_stop_refs: [] } };
      }
      const unseen = candidates.filter(c => !p.work.seen_refs.includes(c.ref));
      if (unseen.length > 24) return { kind: "browse", offset: p.work.offset + 24 };
      if (!p.work.inspecting.length && !p.work.evidence_lids.length) return { kind: "inspect", refs: candidates.slice(0, 1).map(c => c.ref) };
      return { kind: "select", selection: { unit_lid: p.work.unit_lid, role: "foundation", summary: candidates[0].reason,
        accepted_stop_refs: candidates.map(c => c.ref), macro_stop_refs: candidates.slice(0, 1).map(c => c.ref) } };
    }
    case "plan": return { themes: p.context.chapters.length < 2 || !p.context.catalog.candidates.length ? [] : [{ ref: "theme:ownership", question: "How does ownership change transfer cost?",
      distinction: "Conditions for transfer", unit_lids: p.context.chapters.slice(0, 2).map(c => c.unit_lid),
      member_refs: p.context.chapters.slice(0, 2).map(c => p.context.catalog.candidates.find(s => s.unit_lid === c.unit_lid)!.ref) }] };
    case "theme": {
      const w = p.directory.works.find(w => w.plan.ref === p.ref)!;
      if (!w.notes) return { kind: "search", query: "ownership", notes: "searched" };
      if (!w.inspecting.length) return { kind: "inspect", refs: w.plan.member_refs };
      const candidates = w.plan.member_refs.map(ref => p.context.catalog.candidates.find(c => c.ref === ref)!);
      return { kind: "resolve", result: { ref: p.ref, name: "Ownership and transfer", summary: { text: "Ownership precedes transfer under explicit conditions.", evidence_lids: candidates.flatMap(c => c.evidence_lids) },
        stages: candidates.map(c => ({ unit_lid: c.unit_lid, development: c.reason, member_refs: [c.ref] })),
        dependencies: [{ unit_lid: candidates[1].unit_lid, depends_on: candidates[0].unit_lid,
          prerequisite: candidates[0].reason, application: candidates[1].reason, rationale: "Transfer applies the ownership rule." }] } };
    }
    case "reconcile": return { edits: [], merges: [], unresolved: [] };
  }
}
