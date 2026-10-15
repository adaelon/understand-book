import { mkdtempSync, existsSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { describe, expect, it } from "vitest";
import { fixture } from "../testdata/semantic-retrieval/gold";
import { fakeEmbedding } from "./fixtures/embedding-provider";
import { projectRetrievalCatalog, retrievalCatalog, retrieveHybrid, retrievalPage, RETRIEVAL_POLICY,
  type SemanticRetrievalRecord } from "../src/semantic-retrieval";
import { prepareSemanticRetrieval, type PreparedRetrieval } from "../src/semantic-retrieval-preparation";
import { advanceObjectAlignment, alignmentRetrievalRequest, objectAlignmentInput } from "../src/teaching-object-alignment";

const root = () => mkdtempSync(path.join(tmpdir(), "sr-hybrid-"));
const record = (key: string, meaning = key, aliases: string[] = []): SemanticRetrievalRecord => ({ key, meaning, aliases,
  lexical_fields: ["concept", meaning, ...aliases], embedding_text: meaning, content_digest: meaning });
describe("hybrid retrieval lanes and preparation", () => {
  it("keeps all exact/alias hits first, alternates lanes, fills past self/retired/duplicate keys", () => {
    const records = [record("self", "needle"), record("alias", "other", ["needle"]), record("exact", "Needle"),
      ...Array.from({ length: 18 }, (_, i) => record(`lex/${i.toString().padStart(2, "0")}`, `needle ${i}`)),
      ...Array.from({ length: 18 }, (_, i) => record(`sem/${i.toString().padStart(2, "0")}`))];
    const keys = ["retired", "self", "alias", "exact", "lex/00", ...records.filter(r => r.key.startsWith("sem/")).map(r => r.key)];
    const ranks = keys.flatMap((key, i) => [{ key, cosine: 100 - i }, { key, cosine: 100 - i }]);
    const result = retrieveHybrid(records, { kind: "search", query: "NEEDLE", focus_key: "self" }, ranks);
    expect(result.hits.slice(0, 6).map(h => h.key)).toEqual(["alias", "exact", "lex/00", "sem/00", "lex/01", "sem/01"]);
    expect(result.hits[0].match_reasons).toEqual(["alias", "lexical", "semantic"]);
    expect(result.semantic_contributed).toBe(12);
    expect(result.semantic_truncated).toBe(true);
    expect(result.candidate_count).toBe(32);
    expect(result.hits.map(h => h.key)).not.toContain("self");
    expect(result.hits.map(h => h.key)).not.toContain("retired");
    const pages = [];
    for (let offset: number | null = 0; offset !== null;) { const page = retrievalPage(result, offset); pages.push(...page.items); offset = page.next_offset; }
    expect(pages).toEqual(result.hits);
    expect(() => retrievalPage(result, result.candidate_count + 1)).toThrow("offset");
    const manyExact = Array.from({ length: 20 }, (_, i) => record(`exact/${i}`, "needle"));
    expect(retrieveHybrid(manyExact, { kind: "search", query: "needle" }).hits).toHaveLength(20);
  });
  it("preserves complete empty browsing and semantic-only/lexical-only fallback", () => {
    const records = [record("b"), record("a"), record("c")];
    expect(retrieveHybrid(records, { kind: "search", query: "  ", focus_key: "a" }).hits.map(r => r.key)).toEqual(["a", "b", "c"]);
    expect(retrieveHybrid(records, { kind: "search", query: "missing" }, [{ key: "b", cosine: 0.1 }]).hits).toEqual([{ key: "b", match_reasons: ["semantic"] }]);
    expect(retrieveHybrid(records, { kind: "search", query: "B" }).hits).toEqual([{ key: "b", match_reasons: ["exact_meaning", "lexical"] }]);
    const focus = [record("f", "focus", ["shared"]), record("g", "shared"), record("h", "different", ["focus"])];
    expect(retrieveHybrid(focus, { kind: "focus", focus_key: "f" }).hits.map(r => r.key)).toEqual(["g", "h"]);
  });
  it("reuses serializable preparation across pages/reads and policy changes without document/query calls", async () => {
    const { work, previous } = fixture(), provider = fakeEmbedding(), workspace = root();
    provider.limits.max_input_tokens = 10000;
    const records = projectRetrievalCatalog(retrievalCatalog(work.proposal, previous));
    const input = { workspace, provider, records, request: { kind: "search" as const, query: "速度", focus_key: "base/speed" }, mode: "semantic_required" as const };
    const first = await prepareSemanticRetrieval(input);
    const calls = provider.calls.length;
    retrievalPage(first.sequence, 0); retrievalPage(first.sequence, 6);
    const saved = JSON.parse(JSON.stringify(first));
    expect(await prepareSemanticRetrieval({ ...input, previous: saved })).toEqual(first);
    expect(provider.calls).toHaveLength(calls);
    const reranked = await prepareSemanticRetrieval({ ...input, previous: saved, policy: { ...RETRIEVAL_POLICY, semantic_top_k: 4 } });
    expect(reranked.sequence.semantic_contributed).toBe(4);
    expect(provider.calls).toHaveLength(calls);
    // Accepted preparation remains usable without its regenerable vector cache or a live adapter.
    rmSync(path.join(workspace, ".build", "semantic-retrieval"), { recursive: true });
    expect(await prepareSemanticRetrieval({ ...input, provider: undefined, provider_identity: provider.identity, previous: first })).toEqual(first);
    const changed = structuredClone(records);
    changed.at(-1)!.aliases.push("novel-alias"); changed.at(-1)!.lexical_fields.push("novel-alias");
    await expect(prepareSemanticRetrieval({ ...input, records: changed, provider: undefined, provider_identity: provider.identity, previous: first })).rejects.toThrow("requires provider");
  });
  it("does not embed empty browsing or lexical mode, rejects oversized query before document calls", async () => {
    const { work } = fixture(), provider = fakeEmbedding(), workspace = root();
    const records = projectRetrievalCatalog(retrievalCatalog(work.proposal));
    const input = { workspace, provider, records, mode: "semantic_required" as const, request: { kind: "search" as const, query: "" } };
    const browse = await prepareSemanticRetrieval(input);
    expect(browse.sequence.hits).toHaveLength(records.length);
    expect(existsSync(path.join(workspace, ".build"))).toBe(false);
    await prepareSemanticRetrieval({ ...input, mode: "lexical_only", request: { kind: "search", query: "速度" } });
    await expect(prepareSemanticRetrieval({ ...input, request: { kind: "search", query: "x".repeat(129) } })).rejects.toThrow("token");
    expect(provider.calls).toEqual([]);
  });
  it("refreshes lexical-only changes while retaining identical embedding text and query vectors", async () => {
    const { work } = fixture(), provider = fakeEmbedding(), workspace = root();
    provider.limits.max_input_tokens = 10000;
    work.proposal.objects[0].meaning = "long".repeat(300);
    const records = () => projectRetrievalCatalog(retrievalCatalog(work.proposal));
    const request = { kind: "search" as const, query: "new-alias", focus_key: "base/velocity" };
    const first = await prepareSemanticRetrieval({ workspace, provider, records: records(), request, mode: "semantic_required" });
    const calls = provider.calls.length;
    work.proposal.objects[0].aliases.push("new-alias");
    const second = await prepareSemanticRetrieval({ workspace, provider, records: records(), request, mode: "semantic_required", previous: first });
    expect(second.sequence.hits[0]).toMatchObject({ key: "base/speed", match_reasons: expect.arrayContaining(["alias"]) });
    expect(second.dependencies).not.toEqual(first.dependencies);
    expect(provider.calls).toHaveLength(calls);
  });
  it("keeps explicit search through inspect/read and starts a new automatic focus after resolve", async () => {
    const { work: initial, source, previous } = fixture(), provider = fakeEmbedding(), workspace = root();
    provider.limits.max_input_tokens = 10000;
    let work = initial, prepared: PreparedRetrieval | undefined;
    const prepare = async () => prepared = await prepareSemanticRetrieval({ workspace, provider, mode: "semantic_required",
      records: projectRetrievalCatalog(retrievalCatalog(work.proposal, previous)), request: alignmentRetrievalRequest(work, previous), previous: prepared });
    const step = (action: unknown) => { work = advanceObjectAlignment({ work, action, source, previous, retrieval: prepared, operation_id: "sr3" }); };
    await prepare();
    expect(objectAlignmentInput(work, source, previous, prepared).search).toMatchObject({ mode: "focus", offset: 0 });
    const count = provider.calls.length;
    step({ kind: "search", query: "温度", offset: 0 });
    expect(provider.calls).toHaveLength(count);
    expect(() => objectAlignmentInput(work, source, previous, prepared)).toThrow("stale");
    expect(() => objectAlignmentInput(work, source, previous)).toThrow("preparation required");
    await prepare();
    const search = objectAlignmentInput(work, source, previous, prepared).search;
    expect(search).toMatchObject({ mode: "explicit", query: "温度", items: expect.arrayContaining([expect.objectContaining({ key: "base/temperature" })]) });
    expect(JSON.stringify(search)).not.toContain("cosine");
    const afterSearch = provider.calls.length;
    step({ kind: "inspect", key: "base/temperature" });
    const object = work.proposal.objects[0];
    for (const b of object.source_bindings) step({ kind: "read", lid: b.lid, start: 0, end: source.passages.find(p => p.lid === b.lid)!.text.length });
    await prepare();
    expect(objectAlignmentInput(work, source, previous, prepared).search).toEqual(search);
    expect(provider.calls).toHaveLength(afterSearch);
    step({ kind: "search", query: "温度", offset: 6 });
    await prepare();
    expect(provider.calls).toHaveLength(afterSearch);
    expect(objectAlignmentInput(work, source, previous, prepared).search!.offset).toBe(6);
    step({ kind: "resolve", keys: [object.key], object });
    expect(work.search).toBeUndefined();
    expect(work.read_ranges).toEqual([]);
    expect(() => objectAlignmentInput(work, source, previous, prepared)).toThrow("stale");
    await prepare();
    expect(objectAlignmentInput(work, source, previous, prepared).search).toMatchObject({ mode: "focus", offset: 0 });
  });
  it("updates summaries from the current catalog and rejects retired preparation after merge", async () => {
    const { work, source } = fixture(), workspace = root();
    const prepared = await prepareSemanticRetrieval({ workspace, mode: "lexical_only",
      records: projectRetrievalCatalog(retrievalCatalog(work.proposal)), request: alignmentRetrievalRequest(work) });
    const changed = structuredClone(work);
    changed.proposal.objects.at(-1)!.meaning += " edited";
    expect(() => objectAlignmentInput(changed, source, undefined, prepared)).toThrow("stale");
    // The canonical survivor remains searchable even after it has been resolved.
    changed.resolved = [changed.proposal.objects[0].key];
    changed.redirects["retired/key"] = changed.resolved[0];
    const current = projectRetrievalCatalog(retrievalCatalog(changed.proposal));
    const hits = retrieveHybrid(current, { kind: "search", query: "总路程" }, [{ key: "retired/key", cosine: 1 }, { key: changed.resolved[0], cosine: 0.9 }]);
    expect(hits.hits.map(h => h.key)).toContain(changed.resolved[0]);
    expect(hits.hits.map(h => h.key)).not.toContain("retired/key");
  });
  it("clears an explicit request after an identity decision and does not carry its cursor", async () => {
    const { work: initial, source, previous } = fixture(), workspace = root();
    let work = initial;
    work.resolved = work.proposal.objects.map(o => o.key);
    work.search = { query: "行驶", offset: 0, keys: [], next_offset: null, preparation_required: true };
    const prepared = await prepareSemanticRetrieval({ workspace, mode: "lexical_only",
      records: projectRetrievalCatalog(retrievalCatalog(work.proposal, previous)), request: alignmentRetrievalRequest(work, previous) });
    const step = (action: unknown) => { work = advanceObjectAlignment({ work, source, previous, action, retrieval: prepared, operation_id: "identity-sr3" }); };
    step({ kind: "inspect", key: "later/speed-reworded" });
    const target = work.proposal.objects.find(o => o.key === "later/speed-reworded")!;
    for (const b of target.source_bindings) step({ kind: "read", lid: b.lid, start: 0, end: source.passages.find(p => p.lid === b.lid)!.text.length });
    step({ kind: "identity", from: previous.active_refs, to_keys: [target.key], reason: "原文同一定义", source_bindings: target.source_bindings });
    expect(work.search).toBeUndefined();
    expect(work.read_ranges).toEqual([]);
    expect(alignmentRetrievalRequest(work, previous)).toEqual({ kind: "search", query: "" });
    expect(() => objectAlignmentInput(work, source, previous, prepared)).toThrow("stale");
  });
});
