import { readFileSync, mkdirSync, mkdtempSync, writeFileSync } from "node:fs";
import { execFileSync } from "node:child_process";
import { tmpdir } from "node:os";
import path from "node:path";
import { describe, expect, it } from "vitest";
import { fixture, cases } from "../testdata/semantic-retrieval/gold";
import { alignmentFocus } from "../src/teaching-object-alignment";
import { prepareSemanticRetrieval } from "../src/semantic-retrieval-preparation";
import { GROUPS, acceptGoldAction, goldDependencies, preparedGoldPage, goldInput, goldSummary, identityQuality, legacySequence, readRanking, rankingReport, releaseDecision,
  type GoldRun } from "../../../evals/semantic-retrieval/sr6";

function complete(group: GoldRun["group"], mergeSynonyms = true, wrongMerge = false): GoldRun {
  const f = fixture(), run: GoldRun = { group, work: f.work, calls: [] };
  const act = (action: unknown) => {
    const input = goldInput(run); run.work = acceptGoldAction(run, action);
    run.calls.push({ input, action, status: "accepted" });
  };
  for (let i = 0; i < 60 && !run.work.result; i++) {
    const focus = alignmentFocus(run.work, f.previous);
    if (focus.kind === "finish") { act({ kind: "finish" }); break; }
    if (focus.kind === "identity") {
      const key = run.work.redirects["base/speed"] ?? "base/speed";
      const target = run.work.proposal.objects.find(o => o.key === key)!;
      act({ kind: "inspect", key });
      for (const b of target.source_bindings) act({ kind: "read", lid: b.lid, start: 0, end: f.source.passages.find(p => p.lid === b.lid)!.text.length });
      act({ kind: "identity", from: [focus.object.ref], to_keys: [key], reason: "same definition", source_bindings: target.source_bindings });
      continue;
    }
    const keys = [focus.key];
    if (mergeSynonyms && focus.key === "base/speed") keys.push("later/speed-reworded");
    if (mergeSynonyms && focus.key === "base/measure") keys.push("later/measure-reworded");
    if (wrongMerge && focus.key === "base/speed") keys.push("base/velocity");
    for (const key of keys.slice(1)) act({ kind: "inspect", key });
    for (const b of focus.object.source_bindings) act({ kind: "read", lid: b.lid, start: 0, end: f.source.passages.find(p => p.lid === b.lid)!.text.length });
    act({ kind: "resolve", keys, object: focus.object });
  }
  expect(run.work.result).toBeDefined(); return run;
}

describe("SR6 controlled identity evaluation", () => {
  it("keeps real execution behind confirmation and carries prior calls and embedding budget into a revised trial", () => {
    const root = mkdtempSync(path.join(tmpdir(), "sr6-plan-"));
    const json = (file: string, data: unknown) => { mkdirSync(path.dirname(file), { recursive: true }); writeFileSync(file, JSON.stringify(data)); };
    const config = path.join(root, "embedding.json");
    json(config, { version: "local_embedding_config.v1", runtime_dir: "runtime", model_dir: "model" });
    json(path.join(root, "model/model-metadata.json"), { model: "Xenova/paraphrase-multilingual-MiniLM-L12-v2", revision: "test" });
    for (const name of ["@huggingface/transformers", "onnxruntime-node"]) json(path.join(root, "runtime/node_modules", name, "package.json"), { version: "test" });
    const script = path.resolve(import.meta.dirname, "../../../evals/semantic-retrieval/sr6-run.ts");
    const invoke = (...args: string[]) => execFileSync(process.execPath, ["--import", "tsx", script, ...args], {
      cwd: path.resolve(import.meta.dirname, "../../.."), encoding: "utf8", windowsHide: true,
      env: { ...process.env, FLUID_LLM_MODEL: "test", OPENCODE_BASE_URL: "http://127.0.0.1:1", OPENCODE_API_KEY: "test" }, stdio: "pipe" });
    const prior = path.join(root, "prior"), next = path.join(root, "next");
    invoke("prepare", prior, config);
    expect(() => invoke("run", prior, config)).toThrow("Approve the displayed SR6 plan");
    invoke("confirm", prior);
    for (const [i, group] of GROUPS.entries()) json(path.join(prior, `${group}.json`), { group, work: fixture().work,
      calls: Array.from({ length: i + 1 }, () => ({ input: "sample", status: "failed", error: "unknown response" })) });
    const original = readFileSync(path.join(prior, "plan.json"), "utf8");
    invoke("prepare-revision", next, config, prior);
    const revised = JSON.parse(readFileSync(path.join(next, "plan.json"), "utf8"));
    expect(revised.status).toBe("draft"); expect(revised.contract.thinking).toEqual({ type: "disabled" });
    expect(revised.contract.decision_budget.max_steps).toBe(381);
    expect(revised.embedding_workspace).toBe(prior); expect(revised.prior_directory).toBe(prior);
    expect(revised.embedding_plan).toEqual(JSON.parse(original).embedding_plan);
    expect(revised.max_calls).toBe(1152);
    expect(readFileSync(path.join(prior, "plan.json"), "utf8")).toBe(original);
    expect(() => invoke("run", next, config)).toThrow("Approve the displayed SR6 plan");
  }, 30000);
  it("consumes an unchanged Core page across steps and refuses stale pages instead of falling back to substring", async () => {
    const run: GoldRun = { group: "C", work: fixture().work, calls: [] };
    const page = await prepareSemanticRetrieval({ workspace: "unused-lexical", ...goldDependencies(run), mode: "lexical_only" });
    const reused = preparedGoldPage({ first: page }, "next", page.dependencies);
    expect(JSON.parse(goldInput(run, reused)).search.mode).toBe("focus");
    const changed = structuredClone(page.dependencies); changed.request = { kind: "search", query: "changed" };
    expect(() => preparedGoldPage({ first: page }, "next", changed)).toThrow("prepared C page unavailable");
    expect(() => preparedGoldPage({}, "next", page.dependencies)).toThrow("prepared C page unavailable");
  });
  it("grades actual accepted redirects and stable identity, including both missed synonyms and previous reuse", () => {
    const correct = complete("C");
    expect(identityQuality(correct.work)).toMatchObject({ false_merge: [], missed_reuse_or_false_split: [] });
    const missed = identityQuality(complete("B", false).work)!;
    expect(missed.missed_reuse_or_false_split).toHaveLength(3);
    expect(missed.rows.find(r => r.id === "previous-paraphrase")!.missed_reuse_or_false_split).toEqual(["previous/old-speed"]);
    expect(identityQuality(complete("C", true, true).work)!.false_merge.length).toBeGreaterThan(0);
  });
  it("keeps unknown identity quality and usage unknown and executes the production read/inspect gate", () => {
    const f = fixture(), run: GoldRun = { group: "A", work: f.work, calls: [] };
    expect(identityQuality(run.work)).toBeNull();
    expect(() => acceptGoldAction(run, { kind: "resolve", keys: ["base/speed"], object: f.work.proposal.objects[0] })).toThrow("unread");
    run.work.steps_since_progress = 32;
    expect(() => acceptGoldAction(run, { kind: "search", query: "", offset: 0 })).toThrow("budget exhausted");
    const summary = goldSummary(complete("A"));
    expect(summary.unknown_usage_calls).toBe(summary.model_calls);
    expect(summary.read).toBeGreaterThan(0); expect(summary.inspect).toBeGreaterThan(0);
  });
  it("keeps Gold labels out of model input and exactly reproduces legacy JSON substring hits", () => {
    const f = fixture(), run: GoldRun = { group: "A", work: f.work, calls: [] };
    const input = JSON.parse(goldInput(run));
    expect(input.cases).toBeUndefined(); expect(input.policy).toBeUndefined();
    for (const c of cases) {
      const result = acceptGoldAction(run, { kind: "search", query: c.query, offset: 0 });
      expect(result.search!.keys).toEqual(legacySequence(run.work, f.previous, c.query).hits.slice(0, 6).map(h => h.key));
    }
  });
  it("reports fixed-query real-vector A/B/C metrics and refuses publication without complete Agent and long-source evidence", () => {
    const file = path.resolve(import.meta.dirname, "../../../docs/performance/semantic-retrieval-sr1-vectors-20261001.json");
    const ranking = readRanking(file);
    expect(ranking.aggregate.B[6].recall).toBe(0.125); expect(ranking.aggregate.C[6].recall).toBe(1);
    expect(ranking.rows.every(r => r.exact_retained)).toBe(true);
    expect(releaseDecision(ranking, []).reasons).toContain("three_complete_agent_runs_required");
    const runs = GROUPS.map(g => complete(g));
    expect(releaseDecision(ranking, runs)).toMatchObject({ eligible: false, default_mode: "lexical_only", reasons: ["long_material_acceptance_incomplete"] });
    runs[2] = complete("C", false);
    expect(releaseDecision(ranking, runs).reasons).toContain("identity_reuse_regression");
    const saved = JSON.parse(readFileSync(file, "utf8")); saved.documents[0].embedding_text += " changed";
    // The CLI loader uses this same validator; changed projections cannot reuse stale evidence.
    expect(() => rankingReport(saved)).toThrow("saved projection changed");
  });
});
