import { mkdtempSync, readFileSync, writeFileSync, existsSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { describe, expect, it } from "vitest";
import { buildReproducibleProfileArtifactHeader } from "../src/profile-artifact";
import { routeTeachingBuildStages, freezeTeachingTask, acceptTeachingCandidate, closeTeachingStage,
  reopenTeachingBuild, type TeachingBuildInput } from "../src/teaching-build";
import { nextAutomaticBuildAction } from "../src/build-orchestrator";
import { validateFormalFragmentCoverage } from "../src/teaching-object-fragments";
import { renderAutomaticBuildTaskInput, runAutomaticBuildStageWriter } from "../../../skills/build/automatic-build";

const provenance = { executor: "fixture-source-reader", attempt: 1, generated_at: "2026-09-29T00:00:00.000Z" };
function fixture(single = false): TeachingBuildInput {
  const workspace = mkdtempSync(path.join(tmpdir(), "teaching-fragments-"));
  const passages = single
    ? [{ lid: "1.1", unit_lid: "1", text: "平均速率是总路程除以总时间。".repeat(1600) + "等距离时使用调和平均。" }]
    : Array.from({ length: 36 }, (_, i) => ({ lid: `${i + 1}.1`, unit_lid: String(i + 1),
      text: `第${i + 1}个单元讨论平均速率。` + "等距离与等时间是不同的条件。".repeat(40) }));
  return {
    target: { kind: "source_file", profile_id: "technical_learning", book_id: "large-rates", root_dir: workspace,
      workspace_dir: workspace, source_path: path.join(workspace, "source.md"), target_ref: { version: "build_target_ref.v2",
        workspace_dir: workspace, book_id: "large-rates", profile_id: "technical_learning", input_fingerprint: "source-v1" } },
    source: { source_id: "large-rates", source_revision: "source-v1", passages }, units: [],
    structure: { header: buildReproducibleProfileArtifactHeader({ book_id: "large-rates" }),
      spine: [...new Set(passages.map(p => p.unit_lid))].map(lid => ({ lid, role: "foundation", key_stop_ids: [], depends_on: [],
        summary: { text: "速率的定义与条件", evidence_lids: passages.filter(p => p.unit_lid === lid).map(p => p.lid) } })),
      throughlines: [], key_stops: [] },
  };
}
function tasks(input: TeachingBuildInput) {
  const state = routeTeachingBuildStages(input)[0];
  return { state, tasks: Object.values(state.generation_tasks ?? {}).flatMap(t => t.kind === "teaching" ? [t.task] : []) };
}
function emptyProposal(task: ReturnType<typeof tasks>["tasks"][number]) {
  return { objects: [], prerequisites: [], correspondences: [], coverage: [...new Set(task.source.passages.map(p => p.unit_lid))].map(unit_lid => ({
    unit_lid, object_keys: [], explanation: "本片段仅展开已有定义，没有新的独立对象；留待跨块对齐。",
    source_bindings: task.source.passages.filter(p => p.unit_lid === unit_lid).map(p => ({ source_id: task.source.source_id, source_revision: task.source.source_revision, lid: p.lid })),
  })) };
}
describe("large teaching object fragments", () => {
  it("reopens only affected source-unit fragments after an object review failure", () => {
    const input = fixture(), initial = tasks(input);
    const write = (task: typeof initial.tasks[number], candidate: unknown) => {
      freezeTeachingTask(input.target, task); acceptTeachingCandidate(input.target, task, candidate, provenance);
    };
    for (const [index, task] of initial.tasks.entries()) {
      const candidate = emptyProposal(task);
      write(task, index === 0 ? { ...candidate, objects: [{ key: "speed", meaning: "总路程除以总时间", kind: "concept", aliases: [], conditions: [],
        source_bindings: candidate.coverage[0].source_bindings, candidate_refs: [], participants: [], component_keys: [] }],
        coverage: candidate.coverage.map(c => ({ ...c, object_keys: ["speed"] })) } : candidate);
    }
    let task = tasks(input).tasks.at(-1)!;
    for (const binding of task.alignment!.proposal.objects[0].source_bindings) {
      const passage = input.source.passages.find(p => p.lid === binding.lid)!;
      for (let start = 0; start < passage.text.length; start += 2000) {
        write(task, { kind: "read", lid: passage.lid, start, end: Math.min(start + 2000, passage.text.length) });
        task = tasks(input).tasks.at(-1)!;
      }
    }
    write(task, { kind: "resolve", keys: [task.alignment!.proposal.objects[0].key], object: task.alignment!.proposal.objects[0] });
    task = tasks(input).tasks.at(-1)!; write(task, { kind: "finish" });
    closeTeachingStage(input, "formal_objects"); closeTeachingStage(input, "cognitive_materials");
    const state = routeTeachingBuildStages(input).find(s => s.stage === "teaching_publish")!;
    for (const generation of Object.values(state.generation_tasks!)) if (generation.kind === "teaching") write(generation.task, { samples: generation.task.samples!.map(s => ({
      sample_id: s.sample_id, verdict: s.sample_id.startsWith("object:") ? "fail" : "pass", reason: "修正第一单元的对象条件", source_bindings: s.evidence_bindings,
    })) });
    reopenTeachingBuild(input);
    const repaired = tasks(input);
    expect(repaired.state.pending_tasks).toHaveLength(1);
    const pending = repaired.tasks.find(t => repaired.state.pending_tasks.includes(t.descriptor.work_unit_id))!;
    expect(pending.source.passages.map(p => p.unit_lid)).toEqual(["1"]);
    expect(JSON.parse(pending.rendered_input).feedback[0].verdict).toBe("fail");
    expect(repaired.tasks.slice(1).map(t => t.descriptor.work_unit_id)).toEqual(initial.tasks.slice(1).map(t => t.descriptor.work_unit_id));
    expect(existsSync(path.join(input.target.workspace_dir, "teaching_readiness.json"))).toBe(false);
  });
  it("accepts fragmented extraction and alignment through the production writer, resuming stable final identities", () => {
    const input = fixture(true), first = tasks(input);
    const write = (task: typeof first.tasks[number], candidate: unknown) => {
      freezeTeachingTask(input.target, task);
      expect(renderAutomaticBuildTaskInput(input.target, task.descriptor.stage, task.descriptor.work_unit_id,
        { policy_generation_id: task.policy_generation_id }).stdout).toBe(task.rendered_input);
      const file = path.join(input.target.workspace_dir, "candidate.json");
      writeFileSync(file, JSON.stringify(candidate));
      runAutomaticBuildStageWriter(input.target, task.descriptor.stage, task.descriptor.work_unit_id, file,
        { policy_generation_id: task.policy_generation_id, ...provenance });
    };
    for (const [index, task] of first.tasks.entries()) {
      const candidate = emptyProposal(task);
      write(task, index < 2 ? { ...candidate, objects: [{ key: "speed", meaning: "总路程除以总时间", kind: "concept", aliases: ["平均速率"], conditions: [],
        source_bindings: candidate.coverage[0].source_bindings, candidate_refs: [], participants: [], component_keys: [] }],
        coverage: candidate.coverage.map(c => ({ ...c, object_keys: ["speed"] })) } : candidate);
    }
    let task = tasks(input).tasks.at(-1)!;
    const keys = task.alignment!.proposal.objects.map(o => o.key);
    write(task, { kind: "inspect", key: keys[1] });
    task = tasks(input).tasks.at(-1)!;
    expect(task.descriptor.input_budget_proof.estimated_rendered_tokens).toBeLessThanOrEqual(6000);
    const pendingId = task.descriptor.work_unit_id;
    expect(tasks(input).tasks.at(-1)!.descriptor.work_unit_id).toBe(pendingId);
    const passage = input.source.passages[0];
    for (let start = 0; start < passage.text.length; start += 2000) {
      write(task, { kind: "read", lid: passage.lid, start, end: Math.min(start + 2000, passage.text.length) });
      task = tasks(input).tasks.at(-1)!;
    }
    write(task, { kind: "resolve", keys, object: task.alignment!.proposal.objects[0] });
    task = tasks(input).tasks.at(-1)!;
    write(task, { kind: "finish" });
    closeTeachingStage(input, "formal_objects");
    const file = path.join(input.target.workspace_dir, "formal_objects.json"), firstBytes = readFileSync(file, "utf8");
    expect(JSON.parse(firstBytes).active_refs).toHaveLength(1);
    expect(tasks(input).state.closed).toBe(true);
    closeTeachingStage(input, "formal_objects");
    expect(readFileSync(file, "utf8")).toBe(firstBytes);
    closeTeachingStage(input, "cognitive_materials");
    for (let step = 0; step < 40; step++) {
      const reviewState = routeTeachingBuildStages(input).find(s => s.stage === "teaching_publish")!;
      const next = Object.values(reviewState.generation_tasks!).flatMap(t => t.kind === "teaching" && reviewState.pending_tasks.includes(t.task.descriptor.work_unit_id) ? [t.task] : [])[0];
      if (!next) break;
      expect(next.descriptor.input_budget_proof.estimated_rendered_tokens).toBeLessThanOrEqual(6000);
      expect(next.reviewWork).toBeDefined();
      const body = JSON.parse(next.rendered_input);
      if (body.next_range) {
        expect(() => closeTeachingStage(input, "teaching_publish")).toThrow("incomplete");
        write(next, { kind: "read", ...body.next_range });
      } else write(next, { kind: "finish", review: { samples: next.samples!.map(s => ({ sample_id: s.sample_id, verdict: "pass", reason: "逐段核对完整来源", source_bindings: s.evidence_bindings })) } });
    }
    closeTeachingStage(input, "teaching_publish");
    expect(JSON.parse(readFileSync(path.join(input.target.workspace_dir, "teaching_readiness.json"), "utf8")).status).toBe("ready");
  }, 30000);
  it.each([false, true])("routes complete source coverage within the task budget (single long LID: %s)", single => {
    const input = fixture(single);
    const result = tasks(input);
    expect(result.state.teaching_blocked).toBeUndefined();
    expect(result.tasks.length).toBeGreaterThan(1);
    expect(nextAutomaticBuildAction({ target: input.target, stages: [result.state] }).kind).toBe("extract");
    for (const task of result.tasks) {
      expect(task.descriptor.input_budget_proof.estimated_rendered_tokens).toBeLessThanOrEqual(6000);
      expect(JSON.parse(task.rendered_input).mode).toBe("fragment");
    }
    const ranges = result.tasks.flatMap(t => t.fragment?.source_ranges ?? []);
    for (const passage of input.source.passages) {
      const covered = ranges.filter(r => r.lid === passage.lid).sort((a, b) => a.start - b.start);
      expect(covered[0].start).toBe(0);
      expect(covered.at(-1)!.end).toBe(passage.text.length);
      for (let i = 1; i < covered.length; i++) expect(covered[i].start).toBe(covered[i - 1].end);
    }
  });
  it("persists local proposals and resumes accepted fragments without publishing formal identities", () => {
    const input = fixture();
    const first = tasks(input);
    const task = first.tasks[0];
    freezeTeachingTask(input.target, task);
    const file = acceptTeachingCandidate(input.target, task, emptyProposal(task), provenance);
    const bytes = readFileSync(file, "utf8");
    expect(JSON.parse(bytes).payload.version).toBe("formal_object_fragment.v1");
    const resumed = tasks(input);
    expect(resumed.tasks.map(t => t.descriptor.work_unit_id)).toEqual(first.tasks.map(t => t.descriptor.work_unit_id));
    expect(resumed.state.pending_tasks).not.toContain(task.descriptor.work_unit_id);
    expect(readFileSync(acceptTeachingCandidate(input.target, task, emptyProposal(task), provenance), "utf8")).toBe(bytes);
    expect(() => closeTeachingStage(input, "formal_objects")).toThrow();
    expect(existsSync(path.join(input.target.workspace_dir, "formal_objects.json"))).toBe(false);
    expect(existsSync(path.join(input.target.workspace_dir, "teaching_readiness.json"))).toBe(false);
  });
  it("partitions large candidate metadata and blocks formal publication until reconciliation", () => {
    const input = fixture();
    input.source.passages = input.source.passages.slice(0, 1);
    input.structure.spine = input.structure.spine.slice(0, 1);
    input.units = [{ job_id: "unit-1", unit_lid: "1", unit_kind: "chapter", title_path: ["速率"], leaf_lids: ["1.1"], excerpts: [],
      graph_nodes: Array.from({ length: 120 }, (_, i) => ({ id: `concept:${i}`, type: "concept", name: `条件${i}`, occurrences: ["1.1"], source_lid: null })),
      graph_edges: [], discourse_items: [], formula_semantics: [], pass2_edges: [] }];
    // One large record as well as many small records must survive routing intact.
    input.units[0].graph_nodes[0].name = "平均速率的适用条件".repeat(2000);
    const routed = tasks(input);
    const fragments = routed.tasks.map(t => t.fragment!);
    expect(fragments.length).toBeGreaterThan(3);
    const candidateParts = fragments.flatMap(f => f.input.candidates);
    const itemId = "1/graph_nodes/0";
    expect(candidateParts.filter(p => p.item_id === itemId).length).toBeGreaterThan(1);
    const text = candidateParts.filter(p => p.item_id === itemId).sort((a, b) => a.start - b.start).map(p => p.text).join("");
    expect(JSON.parse(text)).toEqual(input.units[0].graph_nodes[0]);
    expect(new Set(candidateParts.map(p => p.item_id)).size).toBe(121);
    expect(() => validateFormalFragmentCoverage(input.source, fragments.slice(1))).toThrow("coverage");
    for (const task of routed.tasks) {
      expect(task.descriptor.input_budget_proof.estimated_rendered_tokens).toBeLessThanOrEqual(6000);
      freezeTeachingTask(input.target, task);
      acceptTeachingCandidate(input.target, task, emptyProposal(task), provenance);
    }
    const { state: complete, tasks: aligned } = tasks(input);
    expect(complete.pending_tasks).toHaveLength(1);
    expect(complete.closed).toBe(false);
    expect(JSON.parse(aligned.at(-1)!.rendered_input).focus.kind).toBe("finish");
    expect(() => closeTeachingStage(input, "formal_objects")).toThrow("incomplete");
    freezeTeachingTask(input.target, aligned.at(-1)!);
    acceptTeachingCandidate(input.target, aligned.at(-1)!, { kind: "finish" }, provenance);
    closeTeachingStage(input, "formal_objects");
    const objects = JSON.parse(readFileSync(path.join(input.target.workspace_dir, "formal_objects.json"), "utf8"));
    expect(objects.version).toBe("formal_objects.v1");
    expect(objects.coverage.map((c: { unit_lid: string }) => c.unit_lid)).toEqual(["1"]);
    expect(tasks(input).state.closed).toBe(true);
  });
  it("rejects out-of-fragment evidence and premature formal identity assignments", () => {
    const input = fixture();
    const task = tasks(input).tasks[0];
    freezeTeachingTask(input.target, task);
    const forged = emptyProposal(task);
    forged.coverage[0].source_bindings[0].lid = input.source.passages.at(-1)!.lid;
    expect(() => acceptTeachingCandidate(input.target, task, forged, provenance)).toThrow("source binding");
    const candidate = { ...emptyProposal(task), objects: [{ key: "speed", meaning: "平均速率", kind: "concept", aliases: [], conditions: [],
      candidate_refs: [], participants: [], component_keys: [], source_bindings: emptyProposal(task).coverage[0].source_bindings,
      existing_ref: { source_id: input.source.source_id, object_id: "old" } }] };
    expect(() => acceptTeachingCandidate(input.target, task, candidate, provenance)).toThrow("local candidates");
  });
});
