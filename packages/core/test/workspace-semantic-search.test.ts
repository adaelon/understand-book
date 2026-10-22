import { mkdirSync, mkdtempSync, readFileSync, readdirSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { afterEach, describe, expect, it } from "vitest";
import { fakeEmbedding } from "./fixtures/embedding-provider";
import { splitEmbeddingText, WorkspaceSemanticSearch, type SemanticPublication, type SemanticSearchRequest } from "../src/workspace-semantic-search";

const roots: string[] = [];
const fixture = (texts = ["第一段：模型训练与泛化。", "Second paragraph about rainfall and clouds."]) => {
  const root = mkdtempSync(path.join(tmpdir(), "ub-workspace-semantic-"));
  roots.push(root);
  const directory = path.join(root, "published"), cache_dir = path.join(root, "cache");
  mkdirSync(directory);
  let source = "";
  const leaves = texts.map((text, index) => {
    const start = source.length;
    source += text;
    const end = source.length;
    source += "\n\n";
    return { lid: `1.${index + 1}`, path: [1, index + 1], kind: "paragraph", children: [], span: { start, end } };
  });
  writeFileSync(path.join(directory, "source.txt"), source);
  writeFileSync(path.join(directory, "base.json"), JSON.stringify({ book_id: "book", lid_nodes: [
    { lid: "1", path: [1], kind: "chapter", children: leaves.map(l => l.lid), span: { start: 0, end: source.length } }, ...leaves,
  ] }));
  const publication: SemanticPublication = { book_id: "book", publication_id: "published-v1", directory, source_revision: "source-v1" };
  const request: SemanticSearchRequest = { query: "泛化能力", top_k: 20, cache_dir, publications: [publication] };
  return { root, source, leaves, publication, request };
};
const withProvider = () => {
  const provider = fakeEmbedding();
  let opens = 0, disposals = 0;
  const engine = new WorkspaceSemanticSearch(async () => { opens++; return { provider, dispose: async () => { disposals++; } }; });
  return { provider, engine, opens: () => opens, disposals: () => disposals };
};
afterEach(() => { for (const root of roots.splice(0)) rmSync(root, { recursive: true, force: true }); });

describe("workspace source semantic retrieval", () => {
  it("keeps a fitting explanation together at an available sentence boundary", () => {
    const provider = fakeEmbedding();
    const sentence = "This sentence is explanatory and ends here. ";
    const text = sentence.repeat(5);
    const parts = splitEmbeddingText(text, provider, "document");
    expect(parts.map(p => p.text).join("")).toBe(text);
    expect(parts.slice(0, -1).every(p => p.text.endsWith("."))).toBe(true);
  });

  it("covers long leaves and queries without truncating or breaking UTF-16 source positions", async () => {
    const { provider, engine } = withProvider();
    const text = "汉字🦀 punctuation. ".repeat(40) + "最终尾部";
    const { source, leaves, request } = fixture(["前言🦀", text]);
    const pieces = splitEmbeddingText(text, provider, "document");
    expect(pieces.map(p => p.text).join("")).toBe(text);
    for (const piece of pieces) {
      expect(provider.count_tokens(piece.text, "document")).toBeLessThanOrEqual(128);
      expect(piece.text).toBe(text.slice(piece.start, piece.end));
      expect(piece.text).not.toMatch(/^[\uDC00-\uDFFF]|[\uD800-\uDBFF]$/u);
    }
    request.query = "为什么更换数据后模型表现会变差？ ".repeat(25);
    request.top_k = 100;
    const result = await engine.search(request);
    expect(result.status).toBe("ready");
    expect(result.hits.filter(h => h.lid === "1.2").sort((a, b) => a.start_utf16 - b.start_utf16)
      .map(h => source.slice(h.start_utf16, h.end_utf16)).join("")).toBe(text);
    expect(Math.max(...result.hits.map(h => h.end_utf16))).toBe(leaves[1].span.end);
    expect(provider.calls.filter(c => c.role === "query").length).toBeGreaterThan(1);
    expect(provider.calls.filter(c => c.role === "query").flatMap(c => c.texts).join("")).toBe(request.query);
    expect(result.query_tokens).toBe(provider.calls.filter(c => c.role === "query").flatMap(c => c.texts)
      .reduce((sum, t) => sum + provider.count_tokens(t, "query"), 0));
    await engine.dispose();
  });

  it("persists complete publication indices outside the publication and reuses the model and vectors", async () => {
    const { provider, engine, opens, disposals } = withProvider();
    const { request, publication } = fixture();
    const first = await engine.search(request);
    expect(first.status).toBe("ready");
    expect(first.embedded_chunks).toBe(2);
    expect(first.indexed_publications).toBe(1);
    expect(first.document_tokens).toBeGreaterThan(0);
    expect(readdirSync(publication.directory).sort()).toEqual(["base.json", "source.txt"]);
    // A hit uses frozen publication identity, so cached searches do not reread full source files.
    rmSync(path.join(publication.directory, "source.txt"));
    const second = await engine.search(request);
    expect(second.hits).toEqual(first.hits);
    expect(second.embedded_chunks).toBe(0);
    expect(second.cache_hits).toBe(2);
    expect(second.document_tokens).toBe(0);
    expect(opens()).toBe(1);
    expect(provider.calls.filter(c => c.role === "document")).toHaveLength(1);
    await engine.dispose();
    expect(disposals()).toBe(1);
  });

  it("rebuilds for a changed source revision or provider and never mixes publication identities", async () => {
    const { provider, engine } = withProvider();
    const { request, publication } = fixture();
    await engine.search(request);
    publication.source_revision = "source-v2";
    expect((await engine.search(request)).embedded_chunks).toBe(2);
    provider.identity.model_revision = "next-model";
    expect((await engine.search(request)).embedded_chunks).toBe(2);
    publication.publication_id = "published-v2";
    const revised = await engine.search(request);
    expect(revised.embedded_chunks).toBe(2);
    expect(revised.hits.every(h => h.publication_id === "published-v2")).toBe(true);
    await engine.dispose();
  });

  it("does not publish a partial cache when a later batch fails", async () => {
    const { provider, engine } = withProvider();
    const { request } = fixture(Array.from({ length: 10 }, (_, i) => `Paragraph ${i}: model evaluation`));
    let batches = 0;
    const embed = provider.embed_documents;
    provider.embed_documents = async (texts, options) => {
      if (++batches === 2) throw new Error("inference failed");
      return embed(texts, options);
    };
    const failed = await engine.search(request);
    expect(failed.status).toBe("unavailable");
    expect(failed.embedded_chunks).toBe(8);
    expect(failed.hits).toEqual([]);
    expect(failed.indexed_publications).toBe(0);
    provider.embed_documents = embed;
    const recovered = await engine.search(request);
    expect(recovered.embedded_chunks).toBe(10);
    expect(recovered.cache_hits).toBe(0);
    expect(recovered.status).toBe("ready");
    await engine.dispose();
  });

  it("repairs invalid cached vectors and orders only the explicitly supplied publications", async () => {
    const { provider, engine } = withProvider();
    const { request } = fixture();
    provider.embed_documents = async texts => ({ vectors: texts.map(() => [1, 0, 0]) });
    provider.embed_query = async () => ({ vector: [1, 0, 0] });
    const initial = await engine.search(request);
    expect(initial.hits.map(h => h.lid)).toEqual(["1.1", "1.2"]);
    const file = path.join(request.cache_dir, "book-book", "publication-published-v1.json");
    const index = JSON.parse(readFileSync(file, "utf8"));
    index.chunks[0].vector = [1];
    writeFileSync(file, JSON.stringify(index));
    expect((await engine.search(request)).embedded_chunks).toBe(2);
    expect((await engine.search({ ...request, publications: [] })).hits).toEqual([]);
    await engine.dispose();
  });

  it("reports model/configuration unavailability without inventing semantic hits", async () => {
    const { request } = fixture();
    const engine = new WorkspaceSemanticSearch(async () => { throw new Error("local model is not configured"); });
    const result = await engine.search(request);
    expect(result).toMatchObject({ status: "unavailable", reason: "local model is not configured", hits: [],
      query_tokens: 0, document_tokens: 0, embedded_chunks: 0 });
    await engine.dispose();
  });
});
