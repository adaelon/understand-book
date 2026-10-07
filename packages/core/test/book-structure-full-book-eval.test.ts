import { it, expect, vi, onTestFinished } from "vitest";
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import path from "node:path";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import { fullBookExperiment } from "../../../evals/book-structure/full-book";
import { verifyFullBook } from "../../../evals/book-structure/verify-full-book";
import { structureProductionFixture, structureProductionResponse } from "./helpers/book-structure-production";
import type { BookStructureGenerationTaskV1 } from "../src/book-structure-generation";
import { prepareExplicitLegacyBuildPlan } from "../../../skills/build/automatic-build";
import { configureBuildRetrieval, reviseBuildRetrieval } from "../src/build-retrieval-config";
import { attachBuildPlanDigest, transitionBuildPlan } from "../src/build-intent";
import * as retrievalConfig from "../src/build-retrieval-config";
import { fakeEmbedding } from "./fixtures/embedding-provider";

it.each(["lexical_only", "semantic_required"] as const)("keeps incomplete work unpublished, counts failures, resumes exact delivery and closes the full book through Engine (%s)", async mode => {
  const f = structureProductionFixture(2), dir = path.join(f.root, "full-book-eval");
  mkdirSync(dir);
  writeFileSync(path.join(f.target.workspace_dir, "source.txt"), readFileSync(f.source, "utf8"));
  f.write("source_manifest.json", { book_id: f.target.book_id, canonical_source: { kind: "markdown", truth_file: "source.txt" } });
  f.write("profile_metadata.json", { header: { book_id: f.target.book_id, profile_id: "technical_learning" } });
  const save = (name: string, value: unknown) => writeFileSync(path.join(dir, name), JSON.stringify(value));
  const read = (name: string) => JSON.parse(readFileSync(path.join(dir, name), "utf8"));
  const legacy = prepareExplicitLegacyBuildPlan(f.source, f.root, { pass2: "disabled" }).plan;
  let build_plan = transitionBuildPlan(reviseBuildRetrieval(legacy, configureBuildRetrieval("lexical_only",
    { max_documents: 0, max_queries: 0, max_calls: 0 }, undefined, "book_structure_projections_and_queries")), "confirmed",
    { at: new Date().toISOString(), confirmation_source: "codex_conversation" });
  const provider = fakeEmbedding(); provider.limits.max_input_tokens = 10000;
  if (mode === "semantic_required") {
    build_plan = attachBuildPlanDigest({ ...build_plan, retrieval: { selection: { retrieval_mode: mode,
      provider: { location: "local", identity: provider.identity }, data_scope: "book_structure_projections_and_queries" },
      estimate: { records: null, queries: null, basis: "unknown_until_fragments" }, budget: { max_documents: 100, max_queries: 100, max_calls: 100 } } });
    const runtime = vi.spyOn(retrievalConfig, "configuredBuildRetrievalRuntime").mockImplementation(plan => ({ selection: plan.retrieval!.selection, provider }));
    onTestFinished(() => runtime.mockRestore());
  }
  save("plan.json", { version: "book_structure_full_book_experiment.v1", status: "confirmed", source_workspace: f.target.workspace_dir,
    workspace: f.target.workspace_dir, execution: "codex_subagent", max_submissions: 80, max_minutes: 60,
    retrieval_mode: mode, build_plan, prepared_at: new Date().toISOString() });
  save("session.json", { steps: [] });
  const oldBytes = JSON.stringify({ old: "retained" });
  f.write("book_structure.json", JSON.parse(oldBytes));
  const first = await fullBookExperiment("next", dir) as { task_file: string; input_file: string };
  const entry = fileURLToPath(new URL("../../../evals/book-structure/full-book.ts", import.meta.url));
  const cli = spawnSync(process.execPath, ["--import", "tsx", entry, "next", dir], {
    cwd: fileURLToPath(new URL("../../../", import.meta.url)), encoding: "utf8" });
  expect(cli.status, cli.stderr).toBe(0);
  expect(JSON.parse(cli.stdout)).toEqual(first);
  expect(await fullBookExperiment("next", dir)).toEqual(first);
  expect(await fullBookExperiment("report", dir)).toMatchObject({ status: "incomplete" });
  await expect(fullBookExperiment("close", dir)).rejects.toThrow(/Complete and validate/);
  expect(readFileSync(path.join(f.target.workspace_dir, "book_structure.json"), "utf8")).toBe(oldBytes);
  const response = path.join(dir, "generator.json");
  writeFileSync(response, "{bad");
  expect(await fullBookExperiment("submit", dir, response)).toMatchObject({ ordinal: 1, status: "rejected" });
  for (let ordinal = 2; ordinal <= 3; ordinal++) {
    await fullBookExperiment("next", dir);
    expect(await fullBookExperiment("submit", dir, response)).toMatchObject({ ordinal, status: "rejected" });
  }
  await expect(fullBookExperiment("next", dir)).rejects.toThrow(/Three consecutive rejections/);
  await expect(fullBookExperiment("resume-after-fix", dir)).rejects.toThrow(/repair note/);
  const failedHistory = read("session.json");
  expect(await fullBookExperiment("resume-after-fix", dir, "Corrected the invalid JSON response before resuming"))
    .toMatchObject({ status: "resumed", after_ordinal: 3 });
  expect(read("session.json").steps).toEqual(failedHistory.steps);
  expect(read("session.json").started_at).toBe(failedHistory.started_at);
  expect(await fullBookExperiment("status", dir)).toMatchObject({ submissions: 3, rejected: 3, pending: 0 });
  const outline = await fullBookExperiment("next", dir) as { task_file: string };
  writeFileSync(response, JSON.stringify(structureProductionResponse(JSON.parse(readFileSync(outline.task_file, "utf8")))));
  expect(await fullBookExperiment("submit", dir, response)).toMatchObject({ status: "accepted" });
  const registered = await fullBookExperiment("discovery-units", dir) as { units: { unit_lid: string }[] };
  expect(registered.units).toHaveLength(2);
  const deliveries = await Promise.all(registered.units.map(u => fullBookExperiment("next", dir, undefined, { unit: u.unit_lid }))) as { task_file: string; input_file: string }[];
  expect(deliveries[0].input_file).not.toBe(deliveries[1].input_file);
  await expect(fullBookExperiment("next", dir)).rejects.toThrow(/Finish active discovery lanes/);
  for (let i = deliveries.length - 1; i >= 0; i--) {
    const unit = registered.units[i].unit_lid;
    expect(await fullBookExperiment("next", dir, undefined, { unit })).toEqual(deliveries[i]);
    writeFileSync(response, JSON.stringify(structureProductionResponse(JSON.parse(readFileSync(deliveries[i].task_file, "utf8")))));
    expect(await fullBookExperiment("submit", dir, response, { unit })).toMatchObject({ status: "accepted" });
    expect(await fullBookExperiment("next", dir, undefined, { unit })).toMatchObject({ status: "complete_unit", unit_lid: unit });
    expect(read(`session.discovery-${unit}.json`).started_at).toBe(read("session.json").started_at);
  }
  let comparisonDir: string | undefined;
  let selectedUnit: string | undefined, revisionRequested = false, revisionAccepted = false, selectionAccepted = false;
  for (let turn = 0; turn < 30; turn++) {
    const next = await fullBookExperiment("next", dir) as { status?: string; task_file: string };
    if (next.status === "complete") break;
    const task = JSON.parse(readFileSync(next.task_file, "utf8")) as BookStructureGenerationTaskV1;
    if (selectedUnit && !revisionRequested) {
      const requestFile = path.join(dir, "revision-request.json");
      const candidates = "phase" in task.input && task.input.phase === "chapter"
        ? task.input.context.catalog.candidates.filter(c => c.unit_lid === selectedUnit) : [];
      writeFileSync(requestFile, JSON.stringify({ id: "omission-1", unit_lid: selectedUnit, issue: "Include the omitted transfer condition in the macro route", candidate_refs: [candidates[1].ref] }));
      expect(await fullBookExperiment("request-chapter-revision", dir, requestFile)).toMatchObject({ status: "requested" });
      expect(await fullBookExperiment("next", dir)).toEqual(next);
      if (!("phase" in task.input) || task.input.phase !== "chapter") throw Error("Expected another unfinished chapter");
      const selectionRequest = path.join(dir, "selection-request.json");
      const prefix = `structure:chapter:${task.input.work.unit_lid}:`;
      const actionOrdinal = Number(task.descriptor.work_unit_id.slice(prefix.length).split("-")[0]);
      writeFileSync(selectionRequest, JSON.stringify({ id: "selection-1", unit_lid: task.input.work.unit_lid,
        from_action_ordinal: actionOrdinal + 1, issue: "Deliver the full chosen reference set in bounded parts without removing chapter content" }));
      expect(await fullBookExperiment("request-chapter-selection", dir, selectionRequest)).toMatchObject({ status: "requested" });
      expect(await fullBookExperiment("next", dir)).toEqual(next);
      revisionRequested = true;
    }
    let action = structureProductionResponse(task);
    if ("phase" in task.input && task.input.phase === "chapter" && task.input.work.revision) {
      expect(revisionRequested).toBe(true);
      expect(task.descriptor.work_unit_id).toContain(":revision:omission-1:");
      expect(task.input.work.evidence_lids.length).toBeGreaterThan(0);
      const previous = task.input.work.revision.previous_selection;
      action = { kind: "select", selection: { ...previous, macro_stop_refs: previous.accepted_stop_refs.slice(0, 2) } };
      revisionAccepted = true;
    }
    if ("phase" in task.input && task.input.phase === "chapter" && task.input.work.selection_draft) {
      expect(task.descriptor.kind).toBe("structure_chapter_selection");
      const full = action as { kind: string; selection?: { accepted_stop_refs: string[] } };
      if (full.kind === "select") {
        if (!task.input.work.selection_draft.accepted_stop_refs.length) {
          action = { kind: "select_stops", refs: full.selection!.accepted_stop_refs };
        } else {
          const { accepted_stop_refs, ...selection } = full.selection!;
          action = { kind: "select", selection };
          selectionAccepted = true;
        }
      }
    }
    writeFileSync(response, JSON.stringify(action));
    expect(await fullBookExperiment("submit", dir, response)).toMatchObject({ status: "accepted" });
    if (!selectedUnit && "phase" in task.input && task.input.phase === "chapter" && (action as { kind?: string }).kind === "select")
      selectedUnit = task.input.work.unit_lid;
    if (mode === "semantic_required" && task.descriptor.kind === "structure_theme_plan") {
      const fork = await fullBookExperiment("fork-lexical", dir) as { comparison_dir: string };
      comparisonDir = fork.comparison_dir;
      for (let i = 0; i < 12; i++) {
        const b = await fullBookExperiment("next", comparisonDir) as { status?: string; task_file: string };
        if (b.status === "complete") break;
        writeFileSync(response, JSON.stringify(structureProductionResponse(JSON.parse(readFileSync(b.task_file, "utf8")))));
        expect(await fullBookExperiment("submit", comparisonDir, response)).toMatchObject({ status: "accepted" });
      }
      expect(await fullBookExperiment("report", comparisonDir)).toMatchObject({ status: "ready" });
    }
  }
  expect(revisionAccepted).toBe(true);
  expect(selectionAccepted).toBe(true);
  expect(read("chapter-revision-events.json").map((e: { kind: string }) => e.kind)).toEqual(["requested", "opened", "accepted"]);
  expect(read("chapter-selection-events.json").map((e: { kind: string }) => e.kind))
    .toEqual(["requested", "opened", "accepted", "opened", "accepted"]);
  for (const runDir of [dir, ...(comparisonDir ? [comparisonDir] : [])]) {
    const initial = JSON.parse(readFileSync(path.join(runDir, "session.json"), "utf8"));
    const acceptedTheme = initial.steps.filter((step: any) => step.status === "accepted" && step.kind === "structure_theme")
      .map((step: any) => ({ step, action: JSON.parse(readFileSync(step.response_file, "utf8")) }))
      .find((value: any) => value.action.kind === "resolve");
    const priorResponse = readFileSync(acceptedTheme.step.response_file, "utf8");
    const requestFile = path.join(runDir, "theme-review-request.json");
    const revisionId = runDir === dir ? "theme-review-C" : "theme-review-B";
    writeFileSync(requestFile, JSON.stringify({ id: revisionId, theme_ref: acceptedTheme.action.result.ref,
      issue: "Verify the same mechanism and applicability condition without changing other resolved themes" }));
    expect(await fullBookExperiment("request-theme-revision", runDir, requestFile)).toMatchObject({ status: "requested" });
    const revisionStep = await fullBookExperiment("next", runDir) as { task_file: string };
    const revisionTask = JSON.parse(readFileSync(revisionStep.task_file, "utf8")) as BookStructureGenerationTaskV1;
    expect(revisionTask.descriptor.work_unit_id).toContain(`:revision:${revisionId}:`);
    if (!("phase" in revisionTask.input) || revisionTask.input.phase !== "theme") throw Error("Expected targeted theme revision");
    expect(revisionTask.input.revision_request?.id).toBe(revisionId);
    expect(revisionTask.input.directory.revision?.previous_result).toEqual(acceptedTheme.action.result);
    writeFileSync(response, JSON.stringify({ kind: "resolve", result: revisionTask.input.directory.revision!.previous_result }));
    expect(await fullBookExperiment("submit", runDir, response)).toMatchObject({ status: "accepted" });
    expect(await fullBookExperiment("report", runDir)).toMatchObject({ status: "ready" });
    expect(readFileSync(acceptedTheme.step.response_file, "utf8")).toBe(priorResponse);
    expect(JSON.parse(readFileSync(path.join(runDir, "theme-revision-events.json"), "utf8"))
      .map((event: { kind: string }) => event.kind)).toEqual(["requested", "opened", "accepted"]);
  }
  const plan = read("plan.json"); plan.max_submissions = read("session.json").steps.length
    + registered.units.reduce((n, u) => n + read(`session.discovery-${u.unit_lid}.json`).steps.length, 0)
    + (comparisonDir ? JSON.parse(readFileSync(path.join(comparisonDir, "session.json"), "utf8")).steps.length : 0); save("plan.json", plan);
  expect(await fullBookExperiment("next", dir)).toMatchObject({ status: "complete" });
  expect(await fullBookExperiment("report", dir)).toMatchObject({ status: "ready", candidates: 4,
    submissions: { rejected: 3, pending: 0 } });
  const report = read("report.json");
  expect(report.actual_model_calls).toBeNull(); expect(report.actual_input_tokens).toBeNull();
  expect(report.discovery.submissions + report.organization.submissions).toBe(report.submissions.submissions);
  if (comparisonDir) {
    const comparison = JSON.parse(readFileSync(path.join(comparisonDir, "report.json"), "utf8"));
    expect(comparison.structure).toEqual(report.structure);
    expect(comparison.discovery.submissions).toBe(0);
    expect(comparison.embedding.calls).toBe(0);
    expect(report.embedding.calls).toBeGreaterThan(0);
    const { compareFullBookExperiments } = await import("../../../evals/book-structure/compare-full-book");
    const matched = compareFullBookExperiments(dir);
    expect(matched.matched_theme_phase.C.submissions).toBe(matched.matched_theme_phase.B.submissions);
    expect(matched.shared_prefix.submissions + matched.matched_theme_phase.C.submissions).toBe(report.submissions.submissions);
    expect(matched.matched_theme_phase.C.submissions).toBeLessThan(report.organization.submissions);
    expect(matched.aggregate_experiment_submissions).toBe(plan.max_submissions);
  }
  expect(report.progress.organization.chapters).toEqual({ done: 2, total: 2 });
  const beforeReplay = provider.calls.length;
  expect(verifyFullBook(dir)).toMatchObject({ status: "passed", same_inputs: true, same_structure: true, provider_calls: 0 });
  if (comparisonDir) expect(verifyFullBook(comparisonDir)).toMatchObject({ status: "passed", shared_steps: expect.any(Number), provider_calls: 0 });
  expect(provider.calls.length).toBe(beforeReplay);
  expect(await fullBookExperiment("close", dir)).toMatchObject({ status: "published" });
  expect(read("publication.json")).toMatchObject({ status: "closed" });
  const published = readFileSync(path.join(f.target.workspace_dir, "book_structure.json"), "utf8");
  expect(await fullBookExperiment("close", dir)).toMatchObject({ status: "published" });
  expect(readFileSync(path.join(f.target.workspace_dir, "book_structure.json"), "utf8")).toBe(published);
}, 120000);
