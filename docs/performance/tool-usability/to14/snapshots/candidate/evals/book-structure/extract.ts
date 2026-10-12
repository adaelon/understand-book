import { existsSync, mkdirSync, readFileSync, readdirSync, writeFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { isDeepStrictEqual } from "node:util";
import { readBookStructureGenerationArtifact, readBookStructureGenerationTask, type BookStructureGenerationTaskV1 } from "../../packages/core/src/book-structure-generation";
import { listAutomaticBuildAttemptDirectories } from "../../packages/core/src/automatic-build-task-store";
import { readAutomaticBuildUsageReceipt, type AutomaticBuildTaskMetricsV1 } from "../../packages/core/src/automatic-build-metrics";
import { automaticBuildGenerationArtifactPath } from "../../packages/core/src/semantic-artifact";
import type { AutomaticBuildTarget } from "../../packages/core/src/build-orchestrator";
import type { BookStructureCandidate, BookStructureUnitArtifact, BookStructureUnitCard, BookStructureFragmentObservationV1, BookStructureStitchFragmentInputV1 } from "../../packages/core/src/book-structure";
import type { BookStructureRelationDelta } from "../../packages/core/src/book-structure-relations";
import { materializeBookStructureContributions } from "../../packages/core/src/book-structure-materialization";
import { applyBookStructureRelationDeltas } from "../../packages/core/src/book-structure-relations";
import { MAX_BOOK_STRUCTURE_EXCERPT_LEN } from "../../packages/core/src/book-structure";

export const DEFAULT_BOOK = "E:/allwork/download/agent/lifebook/.understand-book/ai-infra-book-complete";
const ROOT = fileURLToPath(new URL("../../", import.meta.url));
const FIXTURES = path.join(ROOT, "packages/core/testdata/book-structure");
const json = <T>(file: string): T => JSON.parse(readFileSync(file, "utf8"));
const requireValue = <T>(value: T | undefined, message: string): T => {
  if (value === undefined) throw new Error(message);
  return value;
};
interface SourceNode { lid: string; kind: string; span: { start: number; end: number }; children: string[] }
export interface SourceRange { from: string; to: string }
interface SampleLocations { recovered_stops: Array<{ work_unit_id: string; stop_id: string }>; theme_ranges: SourceRange[]; chapter_ranges: SourceRange[] }

/** Uses the same UTF-16 source offsets as Core's book.text/BookStructure input. */
export function sourceExcerpt(nodes: SourceNode[], source: string, range: SourceRange) {
  const first = requireValue(nodes.find(n => n.lid === range.from), `missing source LID ${range.from}`);
  const last = requireValue(nodes.find(n => n.lid === range.to), `missing source LID ${range.to}`);
  if (first.span.start >= last.span.end) throw new Error("reversed source range");
  return { ...range, span: { start: first.span.start, end: last.span.end },
    text: source.slice(first.span.start, last.span.end),
    lids: nodes.filter(n => !n.children.length && n.span.start >= first.span.start && n.span.end <= last.span.end).map(n => n.lid) };
}

export function assertSourceExcerpt(nodes: SourceNode[], source: string, excerpt: { lid: string; text: string }) {
  const actual = sourceExcerpt(nodes, source, { from: excerpt.lid, to: excerpt.lid }).text.trim().slice(0, MAX_BOOK_STRUCTURE_EXCERPT_LEN);
  if (excerpt.text !== actual) throw new Error(`frozen excerpt differs from canonical source: ${excerpt.lid}`);
}

/** Unknown tokens remain null, including when a metrics record is absent. */
export function summarizeUsage(attempts: Array<{ usage: { input_tokens?: number; output_tokens?: number; cached_input_tokens?: number } }>) {
  const fields = ["input_tokens", "output_tokens", "cached_input_tokens"] as const;
  return Object.fromEntries(fields.map(field => {
    const known = attempts.flatMap(a => a.usage[field] === undefined ? [] : [a.usage[field]!]);
    return [field, { known_subtotal: known.length ? known.reduce((a, b) => a + b, 0) : null,
      known_attempts: known.length, unknown_attempts: attempts.length - known.length }];
  }));
}

/** Only reads the source workspace; output paths are owned by this evaluation. */
export function extractBaseline(bookRoot = DEFAULT_BOOK) {
  const workspace = path.resolve(bookRoot);
  const rel = (file: string) => path.relative(workspace, file).replaceAll("\\", "/");
  const taskRoot = path.join(workspace, ".build/automatic-build/v4/tasks/book_structure");
  const taskFiles = readdirSync(taskRoot).sort().flatMap(g => readdirSync(path.join(taskRoot, g)).sort().map(f => path.join(taskRoot, g, f)));
  const first = json<BookStructureGenerationTaskV1>(taskFiles[0]);
  const target: AutomaticBuildTarget = { kind: "source_file", profile_id: "technical_learning", book_id: first.target_ref.book_id,
    root_dir: path.dirname(workspace), workspace_dir: workspace, source_path: path.join(workspace, "source.txt"),
    target_ref: { ...first.target_ref, workspace_dir: workspace } };
  const nodes = json<{ lid_nodes: SourceNode[] }>(path.join(workspace, "base.json")).lid_nodes;
  const source = readFileSync(target.source_path, "utf8");
  const published = json<BookStructureCandidate>(path.join(workspace, "book_structure.json"));
  const rows = taskFiles.map(file => {
    const identity = json<BookStructureGenerationTaskV1>(file);
    const task = requireValue(readBookStructureGenerationTask(target, identity.policy_generation_id, identity.descriptor.work_unit_id), `missing task ${file}`);
    const artifact = requireValue(readBookStructureGenerationArtifact(target, task), `unaccepted task ${file}`);
    if ("excerpts" in task.input) for (const excerpt of task.input.excerpts) assertSourceExcerpt(nodes, source, excerpt);
    return { task, artifact, task_path: rel(file), artifact_path: rel(automaticBuildGenerationArtifactPath(target, "book_structure", task.policy_generation_id, task.descriptor.work_unit_id)) };
  });
  const unitOrder = requireValue(published.spine, "published spine missing").map(u => u.lid);
  const cards = rows.filter(r => r.task.output_role === "unit_artifact").map(row => {
    const card = (row.artifact.payload as BookStructureUnitArtifact).output.unit_card;
    const node = requireValue(nodes.find(n => n.lid === card.unit_lid), "unit node missing");
    const heading = requireValue(nodes.find(n => n.lid === node.children[0]), "unit heading missing");
    const title = source.slice(heading.span.start, heading.span.end).replace(/^#+\s*/, "").trim();
    const chapter = /^第\s*(\d+)\s*章/u.exec(title);
    return { unit_lid: card.unit_lid, title, display_chapter: chapter ? Number(chapter[1]) : null,
      heading: sourceExcerpt(nodes, source, { from: heading.lid, to: heading.lid }),
      contribution_ref: row.artifact.work_unit_id, artifact_path: row.artifact_path, task_path: row.task_path, card };
  }).sort((a, b) => unitOrder.indexOf(a.unit_lid) - unitOrder.indexOf(b.unit_lid));
  if (!isDeepStrictEqual(cards.map(c => c.unit_lid), unitOrder)) throw new Error("final chapter cards do not match published units");
  const locals = rows.filter(r => r.task.output_role === "stitch_candidate").map(row => {
    const input = row.task.input as BookStructureStitchFragmentInputV1;
    for (const card of input.unit_cards) {
      if (!isDeepStrictEqual(card, cards.find(c => c.unit_lid === card.unit_lid)?.card)) throw new Error(`stitch input differs from accepted card ${card.unit_lid}`);
    }
    return { work_unit_id: row.artifact.work_unit_id, artifact_path: row.artifact_path,
      // Existing contribution identity is carried for the existing materializer, never recomputed here.
      artifact_hash: row.artifact.artifact_hash, unit_card_range: input.unit_card_range,
      payload: row.artifact.payload as BookStructureCandidate };
  });
  const materialized = materializeBookStructureContributions(locals, unitOrder).candidate;
  const relations = rows.filter(r => r.task.output_role === "relation_delta").map(r => ({ work_unit_id: r.artifact.work_unit_id, delta: r.artifact.payload as BookStructureRelationDelta }));
  const replay = applyBookStructureRelationDeltas(materialized, relations).candidate;
  for (const field of ["spine", "throughlines", "key_stops"] as const) {
    if (!isDeepStrictEqual(replay[field], published[field])) throw new Error(`published ${field} differs from accepted contributions`);
  }
  const attempts = listAutomaticBuildAttemptDirectories(target, "book_structure").map(a => {
    const metricsPath = path.join(a.attempt_dir, "metrics.json");
    const metrics = existsSync(metricsPath) ? json<AutomaticBuildTaskMetricsV1>(metricsPath) : undefined;
    const usage = metrics?.usage ?? readAutomaticBuildUsageReceipt(path.join(a.attempt_dir, "lease.json"));
    return { work_unit_id: a.work_unit_id, attempt: a.physical_attempt, path: rel(a.attempt_dir),
      status: metrics?.status ?? null, executor_ms: metrics?.executor_ms ?? null,
      usage: { source: usage.source, input_tokens: usage.input_tokens, output_tokens: usage.output_tokens, cached_input_tokens: usage.cached_input_tokens } };
  });
  const tasks = rows.map(r => ({ work_unit_id: r.artifact.work_unit_id, kind: r.task.descriptor.kind,
    output_role: r.task.output_role, task_path: r.task_path, artifact_path: r.artifact_path,
    accepted_attempt: r.artifact.provenance.attempt, budget: {
      estimator_version: r.task.descriptor.execution_budget_proof.estimator_version,
      estimated_rendered_tokens: r.task.descriptor.execution_budget_proof.estimated_rendered_tokens,
      estimated_prompt_tokens: r.task.descriptor.execution_budget_proof.estimated_prompt_tokens,
      input_delivery_overhead_tokens: r.task.descriptor.execution_budget_proof.input_delivery_overhead_tokens,
      max_candidate_tokens: r.task.descriptor.execution_budget_proof.max_candidate_tokens,
    } }));
  for (const task of tasks) {
    if (!attempts.some(a => a.work_unit_id === task.work_unit_id && a.attempt === task.accepted_attempt && a.status === "committed")) {
      throw new Error(`accepted artifact lacks committed attempt metrics: ${task.work_unit_id}`);
    }
  }
  const budgetFields = ["estimated_rendered_tokens", "estimated_prompt_tokens", "input_delivery_overhead_tokens", "max_candidate_tokens"] as const;
  const groups = [...new Set(tasks.map(t => t.kind))].sort().map(kind => {
    const selected = tasks.filter(t => t.kind === kind);
    return { kind, count: selected.length, estimates: Object.fromEntries(budgetFields.map(k => [k, selected.reduce((sum, t) => sum + t.budget[k], 0)])) };
  });
  const references: Array<{ pointer: string; lid: string }> = [];
  const visit = (value: unknown, pointer: string) => {
    if (!value || typeof value !== "object") return;
    if (Array.isArray(value)) { value.forEach((v, i) => visit(v, `${pointer}/${i}`)); return; }
    for (const [key, v] of Object.entries(value)) {
      if (["lid", "lids", "evidence_lids", "depends_on"].includes(key)) {
        for (const lid of Array.isArray(v) ? v : [v]) {
          if (!nodes.some(n => n.lid === lid)) throw new Error(`unresolved published LID ${lid}`);
          references.push({ pointer: `${pointer}/${key}`, lid });
        }
      } else visit(v, `${pointer}/${key}`);
    }
  };
  visit(published, "");
  const settings = json<SampleLocations>(path.join(ROOT, "evals/book-structure/sample-locations.json"));
  const chapter = requireValue(cards.find(c => c.unit_lid === "10"), "chapter 8 missing");
  const recovered = settings.recovered_stops.map(location => {
    const row = requireValue(rows.find(r => r.artifact.work_unit_id === location.work_unit_id), "fragment missing");
    const payload = row.artifact.payload as BookStructureFragmentObservationV1;
    const stop = requireValue(payload.candidate_key_stops.find(s => s.id === location.stop_id), "fragment stop missing");
    for (const lid of [stop.lid, ...stop.reason.evidence_lids]) {
      if (!row.task.allowed_evidence_lids.includes(lid)) throw new Error(`recovered stop outside delivered scope ${lid}`);
    }
    return { ref: `${row.artifact.work_unit_id}#${stop.id}`, contribution_ref: row.artifact.work_unit_id,
      artifact_path: row.artifact_path, task_path: row.task_path, stop,
      evidence: [...new Set([stop.lid, ...stop.reason.evidence_lids])].map(lid => sourceExcerpt(nodes, source, { from: lid, to: lid })) };
  });
  const chapterInput = { chapter, recovered_candidates: recovered,
    evidence: settings.chapter_ranges.map(r => sourceExcerpt(nodes, source, r)),
    published: { spine: published.spine!.find(u => u.lid === "10"), key_stops: published.key_stops!.filter(s => s.lid.startsWith("10.")) } };
  const themeEvidence = settings.theme_ranges.map(r => sourceExcerpt(nodes, source, r));
  const themeLids = new Set(themeEvidence.flatMap(e => e.lids));
  const themeCandidates = rows.filter(r => r.task.output_role === "unit_observation" && r.task.descriptor.kind === "structure_fragment").flatMap(r =>
    (r.artifact.payload as BookStructureFragmentObservationV1).candidate_key_stops.filter(s => themeLids.has(s.lid)).map(stop => ({
      ref: `${r.artifact.work_unit_id}#${stop.id}`, contribution_ref: r.artifact.work_unit_id, artifact_path: r.artifact_path, stop })));
  const themeInput = { question: "状态复用如何改变容量、传输和恢复成本", chapter_cards: cards.filter(c => ["4", "10", "11", "12", "13"].includes(c.unit_lid)),
    candidates: themeCandidates, evidence: themeEvidence };
  const wrongChapterMentions = (published.throughlines ?? []).flatMap(line => {
    const claims = [...line.summary.text.matchAll(/第\s*(\d+)\s*章/gu)].map(m => Number(m[1]));
    const suspects = cards.filter(c => c.display_chapter !== null && c.display_chapter !== Number(c.unit_lid)
      && claims.includes(Number(c.unit_lid)) && line.lids.includes(c.unit_lid));
    return suspects.length ? [{ id: line.id, summary: line.summary, members: line.lids,
      mentioned_numbers: claims, suspected_lid_as_chapter: suspects.map(c => ({ unit_lid: c.unit_lid, title: c.title })) }] : [];
  });
  const baseline = { source_workspace: workspace.replaceAll("\\", "/"), book_id: target.book_id,
    counts: { cards: cards.length, chapter_candidates: cards.reduce((n, c) => n + c.card.candidate_key_stops.length, 0), tasks: tasks.length,
      spine: published.spine!.length, throughlines: published.throughlines!.length, two_unit_throughlines: published.throughlines!.filter(l => l.lids.length === 2).length,
      key_stops: published.key_stops!.length, dependencies: published.spine!.reduce((n, u) => n + u.depends_on.length, 0), reference_occurrences: references.length },
    estimates: groups, actual_usage: { attempts: attempts.length, tokens: summarizeUsage(attempts),
      executor_ms_sum: attempts.reduce((s, a) => s + (a.executor_ms ?? 0), 0), unknown_executor_ms_attempts: attempts.filter(a => a.executor_ms === null).length,
      wall_clock_ms: null, monetary_cost: null },
    chapter_cards: cards, final: published, local_contributions: locals.map(({ artifact_hash: _existingIdentity, ...local }) => local),
    chapter_number_suspects: wrongChapterMentions, references, tasks, attempts };
  return { "baseline.json": baseline, "chapter-8.input.json": chapterInput, "state-reuse.input.json": themeInput };
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const [mode, bookRoot] = process.argv.slice(2);
  if (!["--write", "--check"].includes(mode)) throw new Error("usage: node --import tsx evals/book-structure/extract.ts --write|--check [book-directory]");
  const outputs = extractBaseline(bookRoot);
  for (const [name, value] of Object.entries(outputs)) {
    const file = path.join(FIXTURES, name);
    if (mode === "--check") {
      // JSON omits unavailable optional usage fields; compare the persisted shape.
      if (!isDeepStrictEqual(json(file), JSON.parse(JSON.stringify(value)))) throw new Error(`source replay differs: ${name}`);
    } else {
      mkdirSync(FIXTURES, { recursive: true });
      writeFileSync(file, JSON.stringify(value, null, 2) + "\n");
    }
  }
  console.log(JSON.stringify({ mode, counts: outputs["baseline.json"].counts,
    estimates: outputs["baseline.json"].estimates, actual_usage: outputs["baseline.json"].actual_usage,
    chapter_number_suspects: outputs["baseline.json"].chapter_number_suspects.length }, null, 2));
}
