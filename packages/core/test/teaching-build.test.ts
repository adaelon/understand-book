import { mkdirSync, mkdtempSync, readFileSync, writeFileSync, existsSync, unlinkSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { afterEach, describe, expect, it, vi } from "vitest";
import fs from "node:fs";
import { syncBuiltinESMExports } from "node:module";
import { source, proposal, binding, targets } from "./fixtures/teaching-source";
import { buildReproducibleProfileArtifactHeader } from "../src/profile-artifact";
import { acceptTeachingCandidate, closeTeachingStage, routeTeachingBuildStages, freezeTeachingTask, readTeachingTask, teachingTaskPath, reopenTeachingBuild, resumeTeachingCognitiveBudget,
  type TeachingBuildInput, type TeachingGenerationTask } from "../src/teaching-build";
import { nextAutomaticBuildAction, nextPlannedAutomaticBuildAction, readTeachingBuildInput, type AutomaticBuildTarget } from "../src/build-orchestrator";
import { compileBuildMode, standardDeepStageClosure } from "../src/build-capability";
import { transitionBuildPlan, attachBuildPlanDigest } from "../src/build-intent";
import { renderAutomaticBuildTaskInput, runAutomaticBuildStageWriter, resolveAutomaticBuildPromptAsset } from "../../../skills/build/automatic-build";
import { TEACHING_STAGES, TEACHING_POLICIES, TEACHING_EXTRACTORS } from "../src/teaching-policy";
import { automaticBuildFailureDiagnosticFromWriterError } from "../src/extractor-contract";
import { acceptFormalObjects } from "../src/teaching-map";
import { objectAlignmentInput, newObjectAlignment } from "../src/teaching-object-alignment";
import { canonicalBuildJson } from "../src/build-intent";
import { prepareBuildRetrieval, readBuildRetrievalState, type BuildRetrievalRuntime } from "../src/automatic-build-retrieval";
import { summarizeRetrievalUsage } from "../src/automatic-build-budget";
import { fakeEmbedding } from "./fixtures/embedding-provider";
import { observeAutomaticBuildRemainingWork } from "../src/automatic-build-observation";
import { buildSemanticArtifactEnvelopeV3 } from "../src/semantic-artifact";
import { claimAutomaticBuildTask } from "../src/automatic-build-lease";
import { failAutomaticBuildTask } from "../src/automatic-build-mailbox";
import { readAutomaticBuildCandidateRetryFeedback } from "../src/automatic-build-task-store";
import { taskPolicyBindingForWorkUnit } from "../src/stage-work-unit";
import { createAutomaticBuildFailureDiagnosticV3 } from "../src/extractor-contract";

const provenance = { executor: "fixture-source-reader", attempt: 1, generated_at: "2026-09-29T00:00:00.000Z" };
const serialReplayInputs = new WeakSet<TeachingBuildInput>();
// The first turn queues the report; the second lets IPC deliver it before another long replay.
afterEach(async () => {
  await new Promise<void>(resolve => setImmediate(resolve));
  await new Promise<void>(resolve => setImmediate(resolve));
});
function fixture(): TeachingBuildInput {
  const workspace = mkdtempSync(path.join(tmpdir(), "teaching-build-"));
  const target: AutomaticBuildTarget = { kind: "source_file", profile_id: "technical_learning", book_id: source.source_id,
    root_dir: workspace, workspace_dir: workspace, source_path: path.join(workspace, "source.txt"),
    target_ref: { version: "build_target_ref.v2", workspace_dir: workspace, book_id: source.source_id, profile_id: "technical_learning", input_fingerprint: source.source_revision } };
  writeFileSync(target.source_path, source.passages.map(p => p.text).join("\n\n"));
  const input: TeachingBuildInput = { source, target, units: [], structure: { header: buildReproducibleProfileArtifactHeader({ book_id: source.source_id }),
    spine: ["1", "2"].map(lid => ({ lid, role: "foundation", summary: { text: "来源单元", evidence_lids: source.passages.filter(p => p.unit_lid === lid).map(p => p.lid) }, key_stop_ids: lid === "2" ? targets.map(t => t.id) : [], depends_on: [] })),
    throughlines: [{ id: "rates", name: "平均速率", summary: { text: "定义、条件、方法和局限", evidence_lids: ["1.1", "2.1"] }, lids: ["1", "2"], key_stop_ids: targets.map(t => t.id) }], key_stops: targets } };
  serialReplayInputs.add(input);
  return input;
}
function tasks(input: TeachingBuildInput) {
  let state = routeTeachingBuildStages(input).find(s => !s.closed)!;
  if (serialReplayInputs.has(input)) {
    // The existing replay tests model a book whose serial action was already issued.
    const next = Object.values(state.generation_tasks ?? {}).find(t => t.kind === "teaching"
      && state.pending_tasks.includes(t.task.descriptor.work_unit_id) && /-parallel-\d+-0-0-0-/u.test(t.task.descriptor.work_unit_id));
    if (next?.kind === "teaching") {
      const legacy = structuredClone(next.task);
      legacy.descriptor.work_unit_id = legacy.descriptor.work_unit_id.replace(/-parallel-(\d+)-0-0-0-/u, "-$1-");
      delete legacy.alignment!.focus_key;
      freezeTeachingTask(input.target, legacy);
      state = routeTeachingBuildStages(input).find(s => !s.closed)!;
    }
  }
  return { state, tasks: Object.values(state.generation_tasks ?? {}).filter(t => t.kind === "teaching").map(t => t.task) };
}
function write(input: TeachingBuildInput, task: TeachingGenerationTask, candidate: unknown) {
  freezeTeachingTask(input.target, task);
  // Exercise the exact production input/writer bridge used by executor sessions.
  expect(renderAutomaticBuildTaskInput(input.target, task.descriptor.stage, task.descriptor.work_unit_id,
    { policy_generation_id: task.policy_generation_id }).stdout).toBe(task.rendered_input);
  const file = path.join(input.target.workspace_dir, "candidate.json");
  writeFileSync(file, JSON.stringify(candidate));
  return runAutomaticBuildStageWriter(input.target, task.descriptor.stage, task.descriptor.work_unit_id, file,
    { policy_generation_id: task.policy_generation_id, ...provenance });
}
function finishMaterials(input: TeachingBuildInput) {
  for (let i = 0; i < 20; i++) {
    const { state, tasks: work } = tasks(input);
    if (state.stage !== "cognitive_materials") return;
    if (!state.pending_tasks.length) { closeTeachingStage(input, "cognitive_materials"); return; }
    for (const task of work.filter(t => state.pending_tasks.includes(t.descriptor.work_unit_id))) {
      const target = task.target!;
      const readings = task.work!.readings;
      if (!readings.some(p => p.lid === target.lid)) { write(input, task, { kind: "read", lid: target.lid }); continue; }
      if (target.id === "harmonic" && !readings.some(p => p.lid === "1.1")) { write(input, task, { kind: "read", lid: "1.1" }); continue; }
      if (target.id === "gap" && !task.work!.searches.length) { write(input, task, { kind: "search", query: "测量记录", offset: 0, limit: 12 }); continue; }
      write(input, task, { kind: "finish", material: { target_id: target.id, kind: target.id === "measurement" ? "method" : "reasoning_episode",
        object_refs: [task.objects!.active_refs[target.id === "harmonic" ? 5 : 0]], purpose: target.reason.text,
        steps: (target.id === "harmonic" ? [
          { id: "definition", content: "平均速率等于总路程除以总时间", source_bindings: [binding("1.1")] },
          { id: "substitution", content: "等距离时总路程为2d，总时间为d/u+d/v；代入定义得2uv/(u+v)", source_bindings: [binding("2.1")] },
        ] : readings.map((p, index) => ({ id: `s${index}`, content: p.text, source_bindings: [binding(p.lid)] }))),
        connections: target.id === "harmonic" ? [{ from: "definition", to: "substitution", meaning: "将两段总路程与总时间代入定义并化简", source_bindings: [binding("1.1"), binding("2.1")] }] : [],
        conditions: target.id === "harmonic" ? ["两段等距离，d、u、v为正"] : [],
        gaps: target.id === "gap" ? [{ description: "缺少各段测量记录，无法断言每段都变快", source_bindings: [binding("2.3")], searched_queries: ["测量记录"], inspected_lids: ["2.3"] }] : [], patterns: [] } });
    }
  }
  throw new Error("fixture did not finish");
}
function reviews(input: TeachingBuildInput, pass: boolean) {
  const { state, tasks: work } = tasks(input);
  expect(state.stage).toBe("teaching_publish");
  for (const task of work) write(input, task, { samples: task.samples!.map(s => ({ sample_id: s.sample_id, verdict: pass ? "pass" : "fail",
    reason: pass ? "逐项对照原文，定义、条件和来源缺口保持一致" : "需要修正对象条件", source_bindings: s.evidence_lids.map(binding) })) });
}
function replayFixture(semantic = true, linked = semantic, serial = true) {
  const input = fixture(), provider = fakeEmbedding();
  if (!serial) serialReplayInputs.delete(input);
  provider.limits.max_input_tokens = 10000;
  input.source = structuredClone(source);
  input.source.passages[0].text = source.passages[0].text.repeat(500);
  const fragmentFiles: string[] = [];
  for (const [i, task] of tasks(input).tasks.entries()) {
    const bindings = task.source.passages.map(p => ({ ...binding(p.lid), range_utf16: { start: 0, end: Math.min(20, p.text.length) } }));
    const objects = Array.from({ length: linked ? 4 : 1 }, (_, j) => ({ ...proposal.objects[0], key: `o${i}-${j}`,
      meaning: `块 ${i} 定义 ${j}`, aliases: [], candidate_refs: [], source_bindings: [bindings[0]] }));
    if (linked) {
      objects[2] = { ...objects[2], kind: "relation", participants: [{ role: "输入", object_key: objects[0].key }, { role: "输出", object_key: objects[1].key }] };
      objects[3] = { ...objects[3], kind: "composite", component_keys: [objects[0].key, objects[1].key] };
    }
    fragmentFiles.push(write(input, task, { objects, prerequisites: [], correspondences: [],
      coverage: [...new Set(task.source.passages.map(p => p.unit_lid))].map(unit_lid => ({ unit_lid,
        object_keys: objects.map(o => o.key), explanation: "本块对象", source_bindings: bindings.filter(b => task.source.passages.some(p => p.unit_lid === unit_lid && p.lid === b.lid)),
      })) }).artifact_path);
  }
  const selection = { retrieval_mode: "semantic_required" as const, provider: { identity: structuredClone(provider.identity), location: "local" as const },
    data_scope: "current_and_previous_formal_object_projections_and_queries" as const };
  if (semantic) input.retrieval = selection;
  let plan = transitionBuildPlan(compileBuildMode({ mode: "standard_deep", book_id: source.source_id, source_fingerprint: source.source_revision,
    content_profile: { id: "technical_learning", version: "technical_learning_v0" }, plan_id: "sr4b-plan", revision: 1,
    created_at: provenance.generated_at, budget: { on_exceed: "needs_user" }, public_freshness: [],
    retrieval: { selection, estimate: { records: null, queries: null, basis: "unknown_until_fragments" },
      budget: { max_documents: 1000, max_queries: 100, max_calls: 200 } },
  }).plan!, "confirmed", { at: provenance.generated_at, confirmation_source: "codex_conversation" });
  const prepare = async () => {
    for (let i = 0; i < 32; i++) {
      const state = tasks(input).state;
      if (!state.retrieval_preparation) return tasks(input).tasks.at(-1)!;
      const failure = await prepareBuildRetrieval({ workspace: input.target.workspace_dir, request: state.retrieval_preparation,
        plan, runtime: { selection, provider } });
      if (failure) throw new Error(failure.message);
    }
    throw new Error(`preparation did not converge: ${JSON.stringify(tasks(input).state.retrieval_preparation?.dependencies.request)}`);
  };
  const changeProvider = () => {
    provider.identity.embedding_config.normalize = !provider.identity.embedding_config.normalize;
    selection.provider.identity = structuredClone(provider.identity);
    plan.revision++; plan.retrieval!.selection = structuredClone(selection); plan = attachBuildPlanDigest(plan);
  };
  return { input, provider, prepare, changeProvider, fragmentFiles };
}
describe("whole-material teaching build", () => {
  it("routes supplemental evidence reads after parallel retrieval exhaustion and preserves field diagnostics and receipts", async () => {
    const f = replayFixture(true, false, false);
    await f.prepare();
    const first = tasks(f.input).tasks.find(t => t.alignment?.focus_key)!;
    const focus = first.alignment!.focus_key!;
    for (let step = 0; step < 32; step++) {
      await new Promise<void>(resolve => setImmediate(resolve));
      const routed = tasks(f.input);
      const current = routed.tasks.find(t => routed.state.pending_tasks.includes(t.descriptor.work_unit_id)
        && t.alignment?.focus_key === focus)!;
      write(f.input, current, step === 31
        ? { kind: "search", query: "source", offset: 0 }
        : { kind: "inspect", key: focus });
    }
    const state = tasks(f.input).state;
    expect(state.teaching_blocked).toBeUndefined();
    expect(state.retrieval_preparation).toBeUndefined();
    expect(state.object_alignment).toMatchObject({ resolved_objects: 0 });
    expect(state.work_units!.length).toBeGreaterThanOrEqual(32);
    expect(nextAutomaticBuildAction({ target: f.input.target, stages: [state] }).kind).toBe("extract");
    const decision = tasks(f.input).tasks.find(t => state.pending_tasks.includes(t.descriptor.work_unit_id) && t.alignment?.focus_key === focus)!;
    const budget = JSON.parse(decision.rendered_input).retrieval_budget;
    expect(budget).toMatchObject({ steps_used: 32, step_limit: 32, allowed_actions: ["resolve", "read"] });
    expect(decision.retrieval).toBeUndefined();
    expect(() => write(f.input, decision, { kind: "inspect", key: focus })).toThrow("retrieval steps are exhausted");
    const object = decision.alignment!.proposal.objects.find(o => o.key === focus)!;
    for (const [candidate, pointer] of [
      [{ kind: "resolve", keys: [], object }, "/keys"],
      [{ kind: "resolve", keys: [focus], object: { ...object, source_bindings: [] } }, "/object/source_bindings"],
      [{ kind: "resolve", keys: [focus], object }, "/object/source_bindings/0"],
    ] as const) {
      let failure: unknown;
      try { write(f.input, decision, candidate); } catch (error) { failure = error; }
      expect(automaticBuildFailureDiagnosticFromWriterError(failure, { writer_started: true }))
        .toMatchObject({ code: "schema_invalid", json_pointer: pointer });
    }
    const frozenInput = decision.rendered_input;
    const readReceipt = write(f.input, decision, budget.next_read);
    const supplemented = tasks(f.input);
    const ready = supplemented.tasks.find(t => supplemented.state.pending_tasks.includes(t.descriptor.work_unit_id) && t.alignment?.focus_key === focus)!;
    expect(ready.alignment!.read_ranges.length).toBeGreaterThan(0);
    expect(readTeachingTask(f.input.target, decision.policy_generation_id, decision.descriptor.work_unit_id).rendered_input).toBe(frozenInput);
    expect(existsSync(readReceipt.artifact_path)).toBe(true);
    const receipt = write(f.input, ready, { kind: "resolve", keys: [focus], object });
    expect(JSON.parse(readFileSync(receipt.artifact_path, "utf8")).payload.steps_since_progress).toBe(0);
    const after = tasks(f.input);
    expect(after.tasks.some(t => after.state.pending_tasks.includes(t.descriptor.work_unit_id) && t.alignment?.focus_key === focus)).toBe(false);
    expect(existsSync(path.join(f.input.target.workspace_dir, "teaching_readiness.json"))).toBe(false);
  }, 120000);
  it("rebuilds a truncated unaccepted frozen task but leaves accepted task records untouched", () => {
    const f = replayFixture(false, true, false), routed = tasks(f.input);
    const task = routed.tasks.find(t => routed.state.pending_tasks.includes(t.descriptor.work_unit_id))!;
    const file = teachingTaskPath(f.input.target, task.policy_generation_id, task.descriptor.work_unit_id);
    mkdirSync(path.dirname(file), { recursive: true });
    writeFileSync(file, "{");
    expect(() => freezeTeachingTask(f.input.target, task)).not.toThrow();
    expect(readTeachingTask(f.input.target, task.policy_generation_id, task.descriptor.work_unit_id).rendered_input).toBe(task.rendered_input);
    write(f.input, task, { kind: "inspect", key: JSON.parse(task.rendered_input).focus.key });
    writeFileSync(file, "{");
    expect(() => freezeTeachingTask(f.input.target, task)).toThrow(SyntaxError);
    expect(readFileSync(file, "utf8")).toBe("{");
  });
  it("removes an interrupted first freeze and keeps the last complete writer context on ENOSPC", () => {
    const f = replayFixture(false, true, false), routed = tasks(f.input);
    const task = routed.tasks.find(t => routed.state.pending_tasks.includes(t.descriptor.work_unit_id))!;
    const file = teachingTaskPath(f.input.target, task.policy_generation_id, task.descriptor.work_unit_id);
    const originalWrite = fs.writeFileSync;
    let failedFile = file;
    const spy = vi.spyOn(fs, "writeFileSync").mockImplementation((name, data, options) => {
      if (name === failedFile) {
        originalWrite(name, "{", options);
        throw Object.assign(new Error("disk full"), { code: "ENOSPC" });
      }
      return originalWrite(name, data, options);
    });
    syncBuiltinESMExports();
    try {
      expect(() => freezeTeachingTask(f.input.target, task)).toThrow("disk full");
      expect(existsSync(file)).toBe(false);
      failedFile = "";
      freezeTeachingTask(f.input.target, task);
      const previous = structuredClone(task);
      previous.alignment!.proposal.objects.at(-1)!.meaning = "last complete writer ledger";
      freezeTeachingTask(f.input.target, previous);
      const next = structuredClone(previous);
      next.alignment!.proposal.objects.at(-1)!.meaning = "new writer ledger";
      failedFile = `${file}.current.json.tmp`;
      expect(() => freezeTeachingTask(f.input.target, next)).toThrow("disk full");
      expect(existsSync(failedFile)).toBe(false);
      expect(readTeachingTask(f.input.target, task.policy_generation_id, task.descriptor.work_unit_id, true)
        .alignment!.proposal.objects.at(-1)!.meaning).toBe("last complete writer ledger");
    } finally { spy.mockRestore(); syncBuiltinESMExports(); }
  });
  it("dispatches three independent alignment focuses and keeps sibling inputs frozen while one advances", () => {
    const f = replayFixture(false, true, false);
    const initial = tasks(f.input);
    const pending = initial.tasks.filter(t => initial.state.pending_tasks.includes(t.descriptor.work_unit_id));
    expect(pending).toHaveLength(3);
    const focuses = pending.map(t => JSON.parse(t.rendered_input).focus.key);
    expect(new Set(focuses).size).toBe(3);
    pending.forEach(t => freezeTeachingTask(f.input.target, t));
    const first = pending[0], object = JSON.parse(first.rendered_input).focus.object;
    write(f.input, first, { kind: "read", lid: object.source_bindings[0].lid, start: 0, end: 20 });
    const next = tasks(f.input);
    expect(next.state.pending_tasks).toHaveLength(3);
    for (const sibling of pending.slice(1)) {
      expect(next.tasks.find(t => t.descriptor.work_unit_id === sibling.descriptor.work_unit_id)?.rendered_input).toBe(sibling.rendered_input);
    }
    expect(next.tasks.filter(t => next.state.pending_tasks.includes(t.descriptor.work_unit_id))
      .find(t => JSON.parse(t.rendered_input).focus.key === focuses[0])!.alignment!.read_ranges).toHaveLength(1);
  });
  it("reduces parallel rounds to complete formal coverage and reuses the published identities", () => {
    const f = replayFixture(false, true, false);
    for (let round = 0; round < 50; round++) {
      const routed = tasks(f.input);
      if (!routed.state.pending_tasks.length) break;
      for (const task of routed.tasks.filter(t => routed.state.pending_tasks.includes(t.descriptor.work_unit_id))) {
        const focus = JSON.parse(task.rendered_input).focus;
        if (focus.kind === "finish") { write(f.input, task, { kind: "finish" }); continue; }
        if (!task.alignment!.read_ranges.length) write(f.input, task, { kind: "read", lid: focus.object.source_bindings[0].lid, start: 0, end: 20 });
        else write(f.input, task, { kind: "resolve", keys: [focus.key], object: focus.object });
      }
    }
    const ready = tasks(f.input);
    expect(ready.state.pending_tasks).toEqual([]);
    expect(ready.state.object_alignment?.remaining_objects).toBe(0);
    closeTeachingStage(f.input, "formal_objects");
    const published = readFileSync(path.join(f.input.target.workspace_dir, "formal_objects.json"), "utf8");
    expect(routeTeachingBuildStages(f.input)[0].closed).toBe(true);
    closeTeachingStage(f.input, "formal_objects");
    expect(readFileSync(path.join(f.input.target.workspace_dir, "formal_objects.json"), "utf8")).toBe(published);
  }, 60000);
  it("finishes an issued serial action before entering parallel rounds without losing its reads", () => {
    const f = replayFixture(false, true, false);
    const routed = tasks(f.input), first = routed.tasks.find(t => routed.state.pending_tasks.includes(t.descriptor.work_unit_id))!;
    const serial = structuredClone(first);
    serial.descriptor.work_unit_id = serial.descriptor.work_unit_id.replace(/-parallel-0-0-0-0-/u, "-0-");
    delete serial.alignment!.focus_key;
    freezeTeachingTask(f.input.target, serial);
    const frozen = readFileSync(teachingTaskPath(f.input.target, serial.policy_generation_id, serial.descriptor.work_unit_id), "utf8");
    expect(tasks(f.input).state.pending_tasks).toEqual([serial.descriptor.work_unit_id]);
    const focus = JSON.parse(serial.rendered_input).focus;
    const receipt = write(f.input, serial, { kind: "read", lid: focus.object.source_bindings[0].lid, start: 0, end: 20 });
    const bytes = readFileSync(receipt.artifact_path, "utf8");
    const next = tasks(f.input);
    expect(next.state.pending_tasks).toHaveLength(3);
    const continuing = next.tasks.find(t => next.state.pending_tasks.includes(t.descriptor.work_unit_id) && JSON.parse(t.rendered_input).focus.key === focus.key)!;
    expect(continuing.alignment!.read_ranges).toHaveLength(1);
    expect(readFileSync(teachingTaskPath(f.input.target, serial.policy_generation_id, serial.descriptor.work_unit_id), "utf8")).toBe(frozen);
    expect(readFileSync(receipt.artifact_path, "utf8")).toBe(bytes);
  });
  it("prepares retrieval for each parallel focus and refreshes only the branch's explicit query", async () => {
    const f = replayFixture(true, true, false);
    await f.prepare();
    const routed = tasks(f.input), pending = routed.tasks.filter(t => routed.state.pending_tasks.includes(t.descriptor.work_unit_id));
    expect(pending).toHaveLength(3);
    for (const task of pending) expect(task.retrieval!.dependencies.request).toMatchObject({
      kind: "focus", focus_key: JSON.parse(task.rendered_input).focus.key,
    });
    const first = pending[0];
    write(f.input, first, { kind: "search", query: "parallel-explicit", offset: 0 });
    await f.prepare();
    const next = tasks(f.input);
    expect(next.state.pending_tasks).toHaveLength(3);
    for (const sibling of pending.slice(1)) expect(next.tasks.find(t => t.descriptor.work_unit_id === sibling.descriptor.work_unit_id)?.rendered_input).toBe(sibling.rendered_input);
    const changed = next.tasks.find(t => next.state.pending_tasks.includes(t.descriptor.work_unit_id)
      && JSON.parse(t.rendered_input).focus.key === JSON.parse(first.rendered_input).focus.key)!;
    expect(changed.retrieval!.dependencies.request).toMatchObject({ kind: "search", query: "parallel-explicit" });
  }, 60000);
  it("keeps formal object cardinality requirements in transport retry feedback", () => {
    const input = fixture(), task = tasks(input).tasks[0];
    freezeTeachingTask(input.target, task);
    const options = { owner: "feedback-test", descriptor: task.descriptor,
      binding: taskPolicyBindingForWorkUnit(task.descriptor, task.policy_generation_id),
      policy_generation: "v3_only" as const };
    const claim = claimAutomaticBuildTask(input.target, "formal_objects", task.descriptor.work_unit_id, options);
    if (claim.status !== "leased") throw new Error("expected first formal object attempt");
    failAutomaticBuildTask(input.target, claim.lease_ref, claim.lease.token, {
      failure_diagnostic: createAutomaticBuildFailureDiagnosticV3({ category: "transport", phase: "generation",
        code: "candidate_request_too_large", expected: "Candidate request measured 2144 tokens; limit is 2048. Shorten output." }),
    });
    const retry = claimAutomaticBuildTask(input.target, "formal_objects", task.descriptor.work_unit_id, options);
    if (retry.status !== "leased") throw new Error("expected retry formal object attempt");
    const feedback = readAutomaticBuildCandidateRetryFeedback(input.target, "formal_objects", task.descriptor.work_unit_id, retry.lease.attempt)!;
    expect(feedback).toMatchObject({ code: "candidate_request_too_large", json_pointer: "/" });
    expect(feedback.expected).toContain("2144 tokens");
    expect(feedback.expected).toContain("component_keys");
    expect(feedback.expected).toContain("participants");
    expect(feedback.expected).toContain("request envelope");
  });
  it.each(["composite", "relation"] as const)("returns a correctable field diagnostic for an incomplete %s and accepts its correction", kind => {
    const input = fixture(), task = tasks(input).tasks[0];
    const b = { source_id: task.source.source_id, source_revision: task.source.source_revision, lid: task.source.passages[0].lid };
    const base = { ...proposal.objects[0], key: "part", kind: "concept" as const, source_bindings: [b], participants: [], component_keys: [] };
    const candidate: typeof proposal = { objects: [base, { ...base, key: "whole", kind }], prerequisites: [], correspondences: [],
      coverage: [...new Set(task.source.passages.map(p => p.unit_lid))].map(unit_lid => ({ unit_lid,
        object_keys: unit_lid === task.source.passages[0].unit_lid ? ["part", "whole"] : [], explanation: "local",
        source_bindings: [{ ...b, lid: task.source.passages.find(p => p.unit_lid === unit_lid)!.lid }] })) };
    let failure: unknown;
    try { write(input, task, candidate); } catch (error) { failure = error; }
    expect(automaticBuildFailureDiagnosticFromWriterError(failure, { writer_started: true })).toMatchObject({
      category: "schema", code: "schema_invalid", phase: "artifact_writer",
      json_pointer: `/objects/1/${kind === "composite" ? "component_keys" : "participants"}`,
    });
    if (kind === "composite") candidate.objects[1].component_keys = ["part"];
    else candidate.objects[1].participants = [{ object_key: "part", role: "input" }, { object_key: "whole", role: "output" }];
    expect(write(input, task, candidate).artifact_path).toBeTruthy();
  });
  it("SR4b refreshes an explicit query after provider change while reusing its unchanged automatic-focus search action", async () => {
    const f = replayFixture();
    const originalQuery = f.provider.embed_query;
    let changed = false;
    f.provider.embed_query = async (text, options) => text === "explicit-sr4b"
      ? { vector: changed ? [0, 0, 1] : [0, 1, 0] } : originalQuery(text, options);
    const initial = await f.prepare();
    const searchReceipt = write(f.input, initial, { kind: "search", query: "explicit-sr4b", offset: 0 });
    const bytes = readFileSync(searchReceipt.artifact_path, "utf8");
    const searched = await f.prepare();
    write(f.input, searched, { kind: "read", lid: "1.1", start: 0, end: 20 });
    changed = true; f.changeProvider();
    const refreshed = await f.prepare();
    const chain = tasks(f.input).tasks.filter(t => t.alignment);
    expect(chain[0].rendered_input).toBe(initial.rendered_input);
    expect(chain[0].descriptor.work_unit_id).toBe(initial.descriptor.work_unit_id);
    expect(refreshed.alignment!.search).toMatchObject({ query: "explicit-sr4b", offset: 0, preparation_required: true });
    expect(JSON.parse(refreshed.rendered_input).search.keys).not.toEqual(JSON.parse(searched.rendered_input).search.keys);
    expect(refreshed.descriptor.work_unit_id).not.toBe(searched.descriptor.work_unit_id);
    expect(refreshed.alignment!.read_ranges).toEqual([]);
    expect(readFileSync(searchReceipt.artifact_path, "utf8")).toBe(bytes);
  }, 120000);
  it("SR4b reuses actions across provider/config changes with identical input and keeps the frozen task unchanged", async () => {
    const f = replayFixture();
    const initial = await f.prepare();
    const receipt = write(f.input, initial, { kind: "read", lid: "1.1", start: 0, end: 20 });
    const next = await f.prepare();
    freezeTeachingTask(f.input.target, next);
    const file = teachingTaskPath(f.input.target, next.policy_generation_id, next.descriptor.work_unit_id), bytes = readFileSync(file, "utf8");
    const artifactBytes = readFileSync(receipt.artifact_path, "utf8");
    f.changeProvider();
    const current = await f.prepare();
    expect(current.rendered_input).toBe(next.rendered_input);
    expect(current.alignment!.read_ranges).toEqual(next.alignment!.read_ranges);
    expect(current.retrieval!.dependencies.provider).not.toEqual(next.retrieval!.dependencies.provider);
    write(f.input, current, { kind: "inspect", key: current.alignment!.proposal.objects[0].key });
    expect(readFileSync(file, "utf8")).toBe(bytes);
    expect(readFileSync(receipt.artifact_path, "utf8")).toBe(artifactBytes);
    const after = await f.prepare();
    expect(after.alignment!.inspected).toEqual([current.alignment!.proposal.objects[0].key]);
    expect(after.retrieval!.dependencies.provider).toEqual(f.provider.identity);
    unlinkSync(path.join(f.input.target.workspace_dir, ".build/semantic-retrieval/cache.json"));
    const calls = f.provider.calls.length;
    expect((await f.prepare()).rendered_input).toBe(after.rendered_input);
    expect(f.provider.calls).toHaveLength(calls);
  }, 120000);
  it("SR4b propagates a resolved meaning to relation/component projections through actual writer and preparation", async () => {
    const f = replayFixture();
    const first = await f.prepare();
    const object = first.alignment!.proposal.objects[0];
    write(f.input, first, { kind: "read", lid: object.source_bindings[0].lid, start: 0, end: 20 });
    const readTask = await f.prepare();
    const before = f.provider.calls.length;
    write(f.input, readTask, { kind: "resolve", keys: [object.key], object: { ...object, meaning: "原文核对后的含义" } });
    expect(tasks(f.input).state.preparation_required).toBe(true);
    const next = await f.prepare();
    const documents = f.provider.calls.slice(before).filter(c => c.role === "document").flatMap(c => c.texts);
    expect(documents).toHaveLength(3);
    expect(documents.every(text => text.includes("原文核对后的含义"))).toBe(true);
    expect(next.alignment!.resolved).toContain(object.key);
    expect(next.alignment!.read_ranges).toEqual([]);
  }, 120000);
  it("SR4b restores finish identities after cache deletion and rematerializes changed Core dependencies only at stage close", async () => {
    const f = replayFixture(true, false);
    let finish!: TeachingGenerationTask;
    for (let i = 0; i < 100; i++) {
      const task = await f.prepare();
      const focus = JSON.parse(task.rendered_input).focus;
      if (focus.kind === "finish") { finish = task; write(f.input, task, { kind: "finish" }); break; }
      if (!task.alignment!.read_ranges.length) write(f.input, task, { kind: "read", lid: focus.object.source_bindings[0].lid, start: 0, end: 20 });
      else write(f.input, task, { kind: "resolve", keys: [focus.key], object: focus.object });
    }
    expect(finish).toBeDefined();
    closeTeachingStage(f.input, "formal_objects");
    const file = path.join(f.input.target.workspace_dir, "formal_objects.json"), bytes = readFileSync(file, "utf8");
    unlinkSync(path.join(f.input.target.workspace_dir, ".build/semantic-retrieval/cache.json"));
    const calls = f.provider.calls.length;
    f.provider.embed_query = async () => { throw new Error("offline"); };
    f.provider.embed_documents = async () => { throw new Error("offline"); };
    expect(routeTeachingBuildStages(f.input)[0].closed).toBe(true);
    closeTeachingStage(f.input, "formal_objects");
    expect(readFileSync(file, "utf8")).toBe(bytes);
    // A changed upstream coverage obligation is not in the bounded alignment inputs.
    const fragmentFile = f.fragmentFiles.at(-1)!;
    const fragment = JSON.parse(readFileSync(fragmentFile, "utf8"));
    fragment.payload.proposal.coverage[0].explanation += "；更新覆盖说明";
    writeFileSync(fragmentFile, JSON.stringify(buildSemanticArtifactEnvelopeV3(fragment)));
    const routed = tasks(f.input);
    expect(routed.state.closed).toBe(false);
    expect(routed.state.pending_tasks).toEqual([]);
    expect(routed.tasks.at(-1)!.descriptor.work_unit_id).toBe(finish.descriptor.work_unit_id);
    expect(readFileSync(file, "utf8")).toBe(bytes);
    closeTeachingStage(f.input, "formal_objects");
    const rematerialized = readFileSync(file, "utf8");
    expect(JSON.parse(rematerialized).coverage.some((c: { explanation: string }) => c.explanation.includes("更新覆盖说明"))).toBe(true);
    expect(JSON.parse(rematerialized).active_refs).not.toEqual(JSON.parse(bytes).active_refs);
    expect(routeTeachingBuildStages(f.input)[0].closed).toBe(true);
    closeTeachingStage(f.input, "formal_objects");
    expect(readFileSync(file, "utf8")).toBe(rematerialized);
    expect(f.provider.calls).toHaveLength(calls);
  }, 120000);
  it("SR4b rejects a reused action that fails the current gate and gives its replacement an independent receipt", () => {
    const f = replayFixture(false);
    const first = tasks(f.input).tasks.at(-1)!;
    const oldKey = first.alignment!.proposal.objects.at(-1)!.key;
    const receipt = write(f.input, first, { kind: "inspect", key: oldKey });
    const bytes = readFileSync(receipt.artifact_path, "utf8");
    const file = f.fragmentFiles.at(-1)!;
    const fragment = JSON.parse(readFileSync(file, "utf8"));
    const object = fragment.payload.proposal.objects.at(-1);
    for (const coverage of fragment.payload.proposal.coverage) coverage.object_keys = ["replacement"];
    object.key = "replacement";
    writeFileSync(file, JSON.stringify(buildSemanticArtifactEnvelopeV3(fragment)));
    const replacement = tasks(f.input).tasks.at(-1)!;
    expect(replacement.rendered_input).toBe(first.rendered_input);
    expect(replacement.descriptor.work_unit_id).toBe(first.descriptor.work_unit_id + "-replay");
    expect(replacement.alignment!.inspected).toEqual([]);
    const newKey = replacement.alignment!.proposal.objects.at(-1)!.key;
    write(f.input, replacement, { kind: "inspect", key: newKey });
    expect(tasks(f.input).tasks.at(-1)!.alignment!.inspected).toEqual([newKey]);
    expect(readFileSync(receipt.artifact_path, "utf8")).toBe(bytes);
  });
  it("SR4a persists prepared pages, keeps search through provider failure/cancellation and resumes without a model attempt", async () => {
    const input = fixture(), provider = fakeEmbedding();
    provider.limits.max_input_tokens = 10000;
    const selection = { retrieval_mode: "semantic_required" as const,
      provider: { identity: structuredClone(provider.identity), location: "local" as const },
      data_scope: "current_and_previous_formal_object_projections_and_queries" as const };
    input.retrieval = selection;
    input.source = structuredClone(input.source);
    input.source.passages[0].text = source.passages[0].text.repeat(500);
    for (const [i, task] of tasks(input).tasks.entries()) {
      const bindings = task.source.passages.map(p => ({ source_id: source.source_id, source_revision: source.source_revision, lid: p.lid }));
      write(input, task, { objects: [{ ...proposal.objects[0], key: `o${i}`, meaning: `第 ${i} 块定义`, source_bindings: [bindings[0]] }],
        prerequisites: [], correspondences: [], coverage: [...new Set(task.source.passages.map(p => p.unit_lid))].map(unit_lid => ({
          unit_lid, object_keys: [`o${i}`], explanation: "本块定义", source_bindings: bindings.filter(b => task.source.passages.some(p => p.unit_lid === unit_lid && p.lid === b.lid)),
        })) });
    }
    const plan = transitionBuildPlan(compileBuildMode({ mode: "standard_deep", book_id: source.source_id, source_fingerprint: source.source_revision,
      content_profile: { id: "technical_learning", version: "technical_learning_v0" }, plan_id: "retrieval-plan", revision: 1,
      created_at: provenance.generated_at, budget: { on_exceed: "needs_user" }, public_freshness: [],
      retrieval: { selection, estimate: { records: null, queries: null, basis: "unknown_until_fragments" },
        budget: { max_documents: 1000, max_queries: 100, max_calls: 100 } },
    }).plan!, "confirmed", { at: provenance.generated_at, confirmation_source: "codex_conversation" });
    const runtime: BuildRetrievalRuntime = { selection, provider };
    const prepare = (signal?: AbortSignal) => {
      const state = tasks(input).state;
      expect(state.preparation_required).toBe(true);
      expect(nextPlannedAutomaticBuildAction({ target: input.target, stages: [state] }, plan))
        .toMatchObject({ kind: "needs_user", reason: "preparation_required" });
      return prepareBuildRetrieval({ workspace: input.target.workspace_dir, request: state.retrieval_preparation!, plan, runtime, signal });
    };
    const pending = tasks(input).state;
    expect(pending.retrieval_remaining).toMatchObject({ queries: 1, calls: null });
    expect(pending.pending_tasks).toEqual([]);
    expect(observeAutomaticBuildRemainingWork({ target: input.target, stages: [pending] })[0].retrieval).toEqual(pending.retrieval_remaining);
    expect(provider.calls).toEqual([]);
    expect(existsSync(path.join(input.target.workspace_dir, ".build/teaching/retrieval.json"))).toBe(false);
    expect(() => closeTeachingStage(input, "formal_objects")).toThrow("incomplete");
    expect(await prepare()).toBeUndefined();
    const first = tasks(input).tasks.at(-1)!;
    expect(JSON.parse(first.rendered_input).search.mode).toBe("focus");
    const calls = provider.calls.length;
    const receipt = write(input, first, { kind: "search", query: "第", offset: 0 });
    const acceptedBytes = readFileSync(receipt.artifact_path, "utf8");
    expect(provider.calls).toHaveLength(calls);
    expect(JSON.parse(acceptedBytes).payload.search).toMatchObject({ query: "第", preparation_required: true });
    expect(tasks(input).state.pending_tasks).toEqual([]);
    const query = provider.embed_query;
    provider.embed_query = async () => { throw new Error("network offline"); };
    expect(await prepare()).toMatchObject({ reason: "provider_failed", message: "network offline" });
    const failureState = readBuildRetrievalState(input.target.workspace_dir);
    expect(failureState.calls.at(-1)).toMatchObject({ role: "query", status: "failed" });
    const failedBytes = readFileSync(path.join(input.target.workspace_dir, ".build/teaching/retrieval.json"), "utf8");
    tasks(input); tasks(input);
    expect(readFileSync(path.join(input.target.workspace_dir, ".build/teaching/retrieval.json"), "utf8")).toBe(failedBytes);
    provider.embed_query = query;
    expect(await prepare()).toBeUndefined();
    const searched = tasks(input).tasks.at(-1)!;
    expect(JSON.parse(searched.rendered_input).search).toMatchObject({ mode: "explicit", query: "第" });
    expect(readFileSync(receipt.artifact_path, "utf8")).toBe(acceptedBytes);
    expect(JSON.parse(acceptedBytes).provenance.attempt).toBe(1);
    const afterSearch = provider.calls.length;
    write(input, searched, { kind: "inspect", key: first.alignment!.proposal.objects[0].key });
    const inspected = tasks(input).tasks.at(-1)!;
    write(input, inspected, { kind: "read", lid: "1.1", start: 0, end: 20 });
    const afterRead = tasks(input).tasks.at(-1)!;
    expect(afterRead.alignment!.read_ranges).toHaveLength(1);
    expect(JSON.parse(afterRead.rendered_input).search).toEqual(JSON.parse(searched.rendered_input).search);
    expect(provider.calls).toHaveLength(afterSearch);
    const controller = new AbortController();
    write(input, afterRead, { kind: "search", query: "取消", offset: 0 });
    provider.embed_query = async (text, options) => {
      expect(options.signal).toBeDefined(); controller.abort(new Error("control cancelled")); return query(text, options);
    };
    expect(await prepare(controller.signal)).toMatchObject({ reason: "cancelled" });
    expect(readBuildRetrievalState(input.target.workspace_dir).calls.at(-1)!.status).toBe("cancelled");
    provider.embed_query = query;
    expect(await prepare()).toBeUndefined();
    expect(tasks(input).tasks.at(-1)!.alignment!.read_ranges).toEqual(afterRead.alignment!.read_ranges);
    expect(summarizeRetrievalUsage(readBuildRetrievalState(input.target.workspace_dir).calls).unknown_usage_calls).toBeGreaterThan(0);
    const finalRead = tasks(input).tasks.at(-1)!;
    const focus = JSON.parse(finalRead.rendered_input).focus;
    const documentCalls = () => readBuildRetrievalState(input.target.workspace_dir).calls.filter(c => c.role === "document").reduce((n, c) => n + c.records, 0);
    const beforeResolve = documentCalls();
    write(input, finalRead, { kind: "resolve", keys: [focus.key], object: { ...focus.object, meaning: "已核对的新定义",
      source_bindings: [{ ...binding("1.1"), range_utf16: { start: 0, end: 20 } }] } });
    expect(tasks(input).state.retrieval_preparation).toBeDefined();
    expect(await prepare()).toBeUndefined();
    expect(documentCalls() - beforeResolve).toBe(1);
    expect(tasks(input).tasks.at(-1)!.alignment!.search).toBeUndefined();
    // A valid page is sufficient while the adapter is offline and the vector cache is absent.
    unlinkSync(path.join(input.target.workspace_dir, ".build/semantic-retrieval/cache.json"));
    const stateFile = path.join(input.target.workspace_dir, ".build/teaching/retrieval.json");
    const stateBytes = readFileSync(stateFile, "utf8");
    const restored = { ...input, retrieval: undefined };
    expect(tasks(restored).tasks.at(-1)!.rendered_input).toBe(tasks(input).tasks.at(-1)!.rendered_input);
    expect(readFileSync(stateFile, "utf8")).toBe(stateBytes);
  }, 120000);
  it.each([1, 2])("keeps v%i contracts in their generation and rejects old alignment payloads", version => {
    const input = fixture();
    const task = tasks(input).tasks[0];
    const legacy = structuredClone(task);
    legacy.policy_generation_id = `formal_objects.full.v${version}`;
    Object.assign(legacy.descriptor.policy_fingerprint, { stage_policy_version: `formal_objects.v${version}`, schema_version: `formal_objects.v${version}`,
      prompt_sha256: version === 1 ? "b84919b2e559cc371ad0aace3b9353512a75bbaa9ef663424561cc87d93d9dd1"
        : "b4261f010580cbbd5f2721f01454f89ee72053741cc0089510d3bb10594da01a" });
    freezeTeachingTask(input.target, legacy);
    const oldFile = acceptTeachingCandidate(input.target, legacy, proposal, provenance);
    const oldBytes = readFileSync(oldFile, "utf8");
    expect(tasks(input).state.pending_tasks).toEqual([task.descriptor.work_unit_id]);
    expect(task.policy_generation_id).toBe("formal_objects.full.v3");
    expect(task.descriptor.policy_fingerprint).toMatchObject({ stage_policy_version: "formal_objects.v3", schema_version: "formal_objects.v2" });
    const current = write(input, task, proposal);
    expect(current.artifact_path).not.toBe(oldFile);
    expect(readFileSync(oldFile, "utf8")).toBe(oldBytes);
    closeTeachingStage(input, "formal_objects");

    // A legacy frozen alignment task can still be submitted after an upgrade.
    // It must not return an old accepted payload or be advanced under the new contract.
    const oldAlignment = { ...legacy, alignment: { ...newObjectAlignment([]), version: `formal_object_alignment.v${version}` } } as unknown as TeachingGenerationTask;
    const saved = JSON.parse(oldBytes);
    saved.payload = { ...oldAlignment.alignment, result: saved.payload };
    delete saved.payload.read_ranges;
    writeFileSync(oldFile, JSON.stringify(saved));
    expect(() => acceptTeachingCandidate(input.target, oldAlignment, { kind: "finish" }, provenance)).toThrow("contract is stale");
  });
  it("SR4b replays actions on the current ledger and preserves immutable input and cumulative reads", async () => {
    const input = fixture();
    input.source = structuredClone(input.source);
    input.source.passages[0].text = source.passages[0].text.repeat(500);
    const fragments = tasks(input).tasks;
    expect(fragments.length).toBeGreaterThan(1);
    const fragmentPaths: string[] = [];
    for (const [i, task] of fragments.entries()) {
      expect(task.fragment).toBeDefined();
      const bindings = task.source.passages.map(p => ({ source_id: source.source_id, source_revision: source.source_revision, lid: p.lid }));
      fragmentPaths.push(write(input, task, { objects: [{ ...proposal.objects[0], key: `o${i}`, meaning: `第 ${i} 块定义`, source_bindings: [bindings[0]] }],
        prerequisites: [], correspondences: [], coverage: [...new Set(task.source.passages.map(p => p.unit_lid))].map(unit_lid => ({
          unit_lid, object_keys: [`o${i}`], explanation: "本块定义", source_bindings: bindings.filter(b => task.source.passages.some(p => p.unit_lid === unit_lid && p.lid === b.lid)),
        })) }).artifact_path);
    }
    const initial = tasks(input).tasks.at(-1)!;
    const key = initial.alignment!.proposal.objects[0].key;
    const receipt = write(input, initial, { kind: "inspect", key });
    const restored = tasks(input).tasks.at(-1)!;
    expect(restored.alignment!.inspected).toEqual([key]);
    const saved = JSON.parse(readFileSync(receipt.artifact_path, "utf8"));
    expect(saved.payload.accepted_action).toEqual({ kind: "inspect", key });
    expect(saved.payload.proposal).toEqual(restored.alignment!.proposal);
    const changed = structuredClone(initial);
    changed.alignment!.proposal.objects.at(-1)!.meaning = "未展示候选被重新整理";
    expect(canonicalBuildJson(objectAlignmentInput(changed.alignment!, input.source, changed.previous))).toBe(initial.rendered_input);
    expect(changed.descriptor.input_hash).toBe(initial.descriptor.input_hash);
    const frozenFile = teachingTaskPath(input.target, initial.policy_generation_id, initial.descriptor.work_unit_id);
    const frozenBytes = readFileSync(frozenFile, "utf8");
    freezeTeachingTask(input.target, restored);
    expect(() => freezeTeachingTask(input.target, changed)).not.toThrow();
    expect(readFileSync(frozenFile, "utf8")).toBe(frozenBytes);
    const reused = acceptTeachingCandidate(input.target, changed, { kind: "inspect", key }, provenance);
    expect(reused).toBe(receipt.artifact_path);
    // The historical receipt stays intact; current route state comes from the current fragments.
    expect(JSON.parse(readFileSync(reused, "utf8")).payload.proposal).toEqual(initial.alignment!.proposal);
    const fragmentFile = fragmentPaths.at(-1)!;
    const replacement = JSON.parse(readFileSync(fragmentFile, "utf8"));
    replacement.payload.proposal.objects.at(-1).meaning = "未展示候选被重新整理";
    writeFileSync(fragmentFile, JSON.stringify(buildSemanticArtifactEnvelopeV3(replacement)));
    const current = tasks(input);
    expect(current.state.pending_tasks).toEqual([current.tasks.at(-1)!.descriptor.work_unit_id]);
    expect(current.tasks.at(-1)!.alignment!.proposal).toEqual(changed.alignment!.proposal);
    expect(current.tasks.at(-1)!.alignment!.inspected).toEqual([key]);
    expect(current.tasks.at(-1)!.rendered_input).toBe(restored.rendered_input);
    expect(restored.alignment!.reading).toBeUndefined();
    write(input, current.tasks.at(-1)!, { kind: "read", lid: "1.1", start: 0, end: 20 });
    const afterRead = tasks(input).tasks.at(-1)!;
    expect(afterRead.alignment!.reading!.text).toBe(input.source.passages[0].text.slice(0, 20));
    expect(afterRead.alignment!.inspected).toEqual([key]);
    expect(afterRead.alignment!.read_ranges).toEqual([{ ...binding("1.1"), start: 0, end: 20 }]);
    expect(JSON.parse(afterRead.rendered_input).read_ranges).toEqual(afterRead.alignment!.read_ranges);
    const object = afterRead.alignment!.proposal.objects[0];
    expect(() => write(input, afterRead, { kind: "resolve", keys: [key], object })).toThrow("unread source range");
    write(input, afterRead, { kind: "read", lid: "1.1", start: 20, end: 40 });
    const resumed = tasks(input).tasks.at(-1)!;
    expect(resumed.alignment!.read_ranges).toEqual([{ ...binding("1.1"), start: 0, end: 40 }]);
    const resolved = write(input, resumed, { kind: "resolve", keys: [key], object: {
      ...object, source_bindings: [{ ...binding("1.1"), range_utf16: { start: 0, end: 40 } }],
    } });
    expect(JSON.parse(readFileSync(resolved.artifact_path, "utf8")).payload.read_ranges).toEqual([]);
    const next = tasks(input).tasks.at(-1)!;
    expect(next.alignment!.read_ranges).toEqual([]);
    expect(next.alignment!.reading).toBeUndefined();
    const nextObject = next.alignment!.proposal.objects.find(o => !next.alignment!.resolved.includes(o.key))!;
    expect(() => write(input, next, { kind: "resolve", keys: [nextObject.key], object: {
      ...nextObject, source_bindings: [{ ...binding("1.1"), range_utf16: { start: 0, end: 40 } }],
    } })).toThrow("unread source range");
    let pending = next;
    let lastArtifact = "";
    for (let i = 0; i < 32; i++) {
      await new Promise<void>(resolve => setImmediate(resolve));
      lastArtifact = write(input, pending, { kind: "read", lid: "1.1", start: 0, end: 20 }).artifact_path;
      if (i < 31) pending = tasks(input).tasks.at(-1)!;
    }
    const stopped = tasks(input);
    expect(stopped.state.teaching_blocked).toBeUndefined();
    expect(stopped.state.work_units!.length).toBeGreaterThanOrEqual(32);
    const decision = stopped.tasks.at(-1)!;
    expect(JSON.parse(decision.rendered_input).retrieval_budget).toMatchObject({ steps_used: 32, step_limit: 32, allowed_actions: ["resolve", "read"] });
    expect(() => write(input, decision, { kind: "read", lid: "1.1", start: 0, end: 20 })).toThrow("retrieval steps are exhausted");
    expect(JSON.parse(readFileSync(lastArtifact, "utf8")).payload.read_ranges).toEqual([{ ...binding("1.1"), start: 0, end: 20 }]);
    expect(() => closeTeachingStage(input, "formal_objects")).toThrow("incomplete");
    expect(existsSync(path.join(input.target.workspace_dir, "teaching_readiness.json"))).toBe(false);
  }, 120000);
  it("keeps existing formal identities when a new teaching policy rebuilds the same source", () => {
    const input = fixture();
    const existing = acceptFormalObjects({ source, proposal, operation_id: "prior-policy" });
    writeFileSync(path.join(input.target.workspace_dir, "formal_objects.json"), JSON.stringify(existing));
    const root = path.join(input.target.workspace_dir, ".build", "teaching");
    mkdirSync(root, { recursive: true });
    writeFileSync(path.join(root, `baseline-${source.source_revision}.json`), "{}");
    const task = tasks(input).tasks[0];
    expect(task.previous!.active_refs).toEqual(existing.active_refs);
    const next = structuredClone(proposal);
    next.objects.forEach((o, i) => o.existing_ref = existing.active_refs[i]);
    write(input, task, next); closeTeachingStage(input, "formal_objects");
    expect(JSON.parse(readFileSync(path.join(input.target.workspace_dir, "formal_objects.json"), "utf8")).active_refs).toEqual(existing.active_refs);
  });
  it("binds installed prompt assets and reports invalid candidates as correctable semantic failures", () => {
    for (const stage of TEACHING_STAGES) expect(resolveAutomaticBuildPromptAsset(`${TEACHING_EXTRACTORS[stage]}.md`).sha256).toBe(TEACHING_POLICIES[stage].prompt_sha256);
    const input = fixture();
    try { write(input, tasks(input).tasks[0], {}); throw new Error("expected invalid candidate"); }
    catch (error) { expect(automaticBuildFailureDiagnosticFromWriterError(error, { writer_started: true })).toMatchObject({ category: "schema", code: "semantic_output_invalid" }); }
  });
  it("loads full source spans, including definitions beyond structure previews", () => {
    const input = fixture();
    const tail = "必要定义在段落末尾：总路程包括往返两程。";
    writeFileSync(input.target.source_path, `# 定义\n\n${"上下文。".repeat(500)}${tail}\n`);
    writeFileSync(path.join(input.target.workspace_dir, "base.json"), JSON.stringify({ graph_nodes: [], graph_edges: [] }));
    writeFileSync(path.join(input.target.workspace_dir, "discourse_index.json"), JSON.stringify({ items: [] }));
    writeFileSync(path.join(input.target.workspace_dir, "formula_semantics.json"), "[]");
    writeFileSync(path.join(input.target.workspace_dir, "book_structure.json"), JSON.stringify(input.structure));
    const loaded = readTeachingBuildInput(input.target);
    expect(loaded.source.passages.some(p => p.text.includes(tail))).toBe(true);
    expect(JSON.stringify(loaded.units.flatMap(u => u.excerpts))).not.toContain(tail);
  });
  it("routes large paragraphs, keeps oversized atomic input incomplete and resumes accepted reads", () => {
    const input = fixture();
    const large = structuredClone(input.source);
    large.passages[0].text = "来源".repeat(20000);
    const oversized = routeTeachingBuildStages({ ...input, source: large });
    expect(nextAutomaticBuildAction({ target: input.target, stages: oversized }).kind).toBe("extract");
    large.passages[0].kind = "formula";
    const atomic = routeTeachingBuildStages({ ...input, source: large });
    expect(nextAutomaticBuildAction({ target: input.target, stages: atomic })).toMatchObject({ kind: "needs_user", reason: "teaching_preparation_incomplete" });
    expect(existsSync(path.join(input.target.workspace_dir, "teaching_readiness.json"))).toBe(false);
    write(input, tasks(input).tasks[0], proposal); closeTeachingStage(input, "formal_objects");
    let task = tasks(input).tasks.find(t => t.target?.id === "harmonic")!;
    write(input, task, { kind: "read", lid: "2.1" });
    task = tasks(input).tasks.filter(t => t.target?.id === "harmonic").at(-1)!;
    // Exhaust the bounded search allowance; the last result remains a durable checkpoint.
    for (let i = 0; i < 13; i++) {
      write(input, task, { kind: "search", query: `not-found-${i}`, offset: 0, limit: 12 });
      task = tasks(input).tasks.filter(t => t.target?.id === "harmonic").at(-1)!;
    }
    expect(tasks(input).state.teaching_blocked).toContain("预算耗尽");
    expect(() => resumeTeachingCognitiveBudget(input, "harmonic", {} as never)).toThrow();
    resumeTeachingCognitiveBudget(input, "harmonic", { searches: 24, preview_chars: 4000, read_chars: 24000, context_chars: 24000 });
    task = tasks(input).tasks.filter(t => t.target?.id === "harmonic").at(-1)!;
    expect(task.work!.readings.map(p => p.lid)).toEqual(["2.1"]);
    expect(task.work!.used.searches).toBe(12);
    write(input, task, { kind: "read", lid: "1.1" });
    finishMaterials(input); reviews(input, true); closeTeachingStage(input, "teaching_publish");
    expect(routeTeachingBuildStages(input).every(s => s.closed)).toBe(true);
  });
  it.each(["enabled", "disabled"] as const)("publishes source-bound assets through confirmed plan with Pass2 %s and resumes accepted work", (pass2) => {
    const input = fixture();
    const closure = standardDeepStageClosure({ id: "technical_learning", version: "technical_learning_v0" }, { pass2 });
    expect(closure.includes("pass2")).toBe(pass2 === "enabled");
    expect(closure.slice(-3)).toEqual(["formal_objects", "cognitive_materials", "teaching_publish"]);
    const routed = routeTeachingBuildStages(input);
    const snapshot = { target: input.target, stages: routed };
    const plan = transitionBuildPlan(compileBuildMode({ mode: "standard_deep", book_id: source.source_id, source_fingerprint: source.source_revision,
      content_profile: { id: "technical_learning", version: "technical_learning_v0" }, plan_id: "teaching-plan", revision: 1,
      created_at: provenance.generated_at, public_freshness: [], budget: { on_exceed: "needs_user" }, pass2 }).plan!, "confirmed", {
      at: provenance.generated_at, confirmation_source: "reader_ui",
    });
    expect(nextPlannedAutomaticBuildAction(snapshot, plan).kind).toBe("extract");
    const task = tasks(input).tasks[0];
    const first = write(input, task, proposal);
    const bytes = readFileSync(first.artifact_path, "utf8");
    expect(write(input, task, proposal).artifact_path).toBe(first.artifact_path);
    expect(readFileSync(first.artifact_path, "utf8")).toBe(bytes);
    expect(tasks(input).state.pending_tasks).toEqual([]);
    closeTeachingStage(input, "formal_objects");
    expect(() => closeTeachingStage(input, "cognitive_materials")).toThrow("incomplete");
    finishMaterials(input);
    expect(existsSync(path.join(input.target.workspace_dir, "teaching_readiness.json"))).toBe(false);
    reviews(input, true);
    closeTeachingStage(input, "teaching_publish");
    expect(nextAutomaticBuildAction({ target: input.target, stages: routeTeachingBuildStages(input) }).kind).toBe("done");
    const receipt = JSON.parse(readFileSync(path.join(input.target.workspace_dir, "teaching_readiness.json"), "utf8"));
    expect(receipt).toMatchObject({ status: "ready", limitations: ["缺少各段测量记录，无法断言每段都变快"] });
    expect(existsSync(path.join(input.target.workspace_dir, receipt.map_path))).toBe(true);
  });
  it("blocks failed source review, then repairs with old identities retained", () => {
    const input = fixture();
    write(input, tasks(input).tasks[0], proposal);
    closeTeachingStage(input, "formal_objects"); finishMaterials(input); reviews(input, false);
    expect(() => closeTeachingStage(input, "teaching_publish")).toThrow("source review failed");
    reopenTeachingBuild(input);
    const task = tasks(input).tasks[0];
    expect(task.previous?.active_refs).toHaveLength(proposal.objects.length);
    const corrected = structuredClone(proposal);
    corrected.objects.forEach((o, i) => o.existing_ref = task.previous!.active_refs[i]);
    write(input, task, corrected);
    closeTeachingStage(input, "formal_objects"); finishMaterials(input); reviews(input, true);
    closeTeachingStage(input, "teaching_publish");
  });
  it("repairs only the rejected material and reuses unrelated extraction and source reviews", () => {
    const input = fixture();
    write(input, tasks(input).tasks[0], proposal); closeTeachingStage(input, "formal_objects"); finishMaterials(input);
    const formalBytes = readFileSync(path.join(input.target.workspace_dir, "formal_objects.json"), "utf8");
    const before = tasks(input).tasks;
    for (const task of before) write(input, task, { samples: task.samples!.map(s => ({ sample_id: s.sample_id,
      verdict: s.sample_id === "material:gap" ? "fail" : "pass", reason: "核对缺失的各段测量依据", source_bindings: s.evidence_lids.map(binding) })) });
    reopenTeachingBuild(input);
    const rerouted = tasks(input);
    expect(rerouted.state.stage).toBe("cognitive_materials");
    expect(rerouted.tasks.filter(t => rerouted.state.pending_tasks.includes(t.descriptor.work_unit_id)).map(t => t.target!.id)).toEqual(["gap"]);
    expect(JSON.parse(rerouted.tasks.at(-1)!.rendered_input).feedback[0].sample_id).toBe("material:gap");
    expect(readFileSync(path.join(input.target.workspace_dir, "formal_objects.json"), "utf8")).toBe(formalBytes);
    finishMaterials(input);
    const after = tasks(input);
    expect(after.state.pending_tasks).toHaveLength(1);
    expect(after.tasks.find(t => after.state.pending_tasks.includes(t.descriptor.work_unit_id))!.samples![0].sample_id).toBe("material:gap");
    const preserved = before.filter(t => t.samples![0].sample_id !== "material:gap").map(t => t.descriptor.work_unit_id);
    expect(after.tasks.filter(t => !after.state.pending_tasks.includes(t.descriptor.work_unit_id)).map(t => t.descriptor.work_unit_id)).toEqual(preserved);
    reviews(input, true); closeTeachingStage(input, "teaching_publish");
  });
  it("rejects missing structural coverage, stale source and review evidence omissions", () => {
    const input = fixture();
    expect(() => routeTeachingBuildStages({ ...input, structure: { ...input.structure, spine: [] } })).toThrow("coverage");
    const task = tasks(input).tasks[0];
    expect(() => acceptTeachingCandidate(input.target, { ...task, source: { ...source, source_revision: "wrong" } }, proposal, provenance)).toThrow("stale");
    write(input, task, proposal); closeTeachingStage(input, "formal_objects"); finishMaterials(input);
    const review = tasks(input).tasks[0];
    expect(() => write(input, review, { samples: [] })).toThrow();
  });
});
