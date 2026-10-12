import { describe, expect, it } from "vitest";
import { callEmbedding, type EmbeddingCall } from "../src/embedding-provider";
import { fakeEmbedding } from "./fixtures/embedding-provider";

describe("async embedding contract", () => {
  it("meters each bounded transient retry and never retries cancellation or invalid output", async () => {
    const p = fakeEmbedding(), calls: EmbeddingCall[] = [];
    p.max_retries = 1;
    let attempts = 0, reserved = 0;
    p.embed_query = async () => {
      if (++attempts === 1) throw Object.assign(new Error("busy"), { code: "EBUSY" });
      return { vector: [1, 0, 0], usage: { input_tokens: 2 } };
    };
    await callEmbedding(p, p.identity, "query", ["q"], { before_call: () => reserved++, on_call: c => calls.push(c) });
    expect(reserved).toBe(2); expect(calls.map(c => c.status)).toEqual(["failed", "ok"]);
    attempts = 0;
    p.embed_query = async () => { attempts++; throw Object.assign(new Error("busy"), { code: "EAGAIN" }); };
    await expect(callEmbedding(p, p.identity, "query", ["q"])).rejects.toThrow("busy");
    expect(attempts).toBe(2);
    attempts = 0;
    p.embed_query = async () => { attempts++; return { vector: [] }; };
    await expect(callEmbedding(p, p.identity, "query", ["q"])).rejects.toThrow("response");
    expect(attempts).toBe(1);
    const controller = new AbortController(); attempts = 0;
    p.embed_query = async () => { attempts++; controller.abort(); throw Object.assign(new Error("busy"), { code: "EBUSY" }); };
    await expect(callEmbedding(p, p.identity, "query", ["q"], { signal: controller.signal })).rejects.toThrow();
    expect(attempts).toBe(1);
  });
  it("counts document/query calls and preserves unknown usage", async () => {
    const provider = fakeEmbedding(), calls: EmbeddingCall[] = [];
    expect(await callEmbedding(provider, provider.identity, "document", ["one", "two"], { on_call: c => calls.push(c) })).toHaveLength(2);
    await callEmbedding(provider, provider.identity, "query", ["one"], { on_call: c => calls.push(c) });
    expect(calls.map(c => [c.role, c.records, c.status])).toEqual([["document", 2, "ok"], ["query", 1, "ok"]]);
    expect(calls[0].usage).toBeUndefined();
    provider.embed_query = async () => ({ vector: [1, 0, 0], usage: { input_tokens: 3 } });
    await callEmbedding(provider, provider.identity, "query", ["one"], { on_call: c => calls.push(c) });
    expect(calls[2].usage).toEqual({ input_tokens: 3 });
  });
  it("rejects identity, shape, zero/nonfinite vectors, batch and actual token overflow", async () => {
    const p = fakeEmbedding();
    await expect(callEmbedding(p, { ...p.identity, model_revision: "different" }, "query", ["q"])).rejects.toThrow("identity");
    await expect(callEmbedding(p, p.identity, "document", Array(9).fill("x"))).rejects.toThrow("batch");
    await expect(callEmbedding(p, p.identity, "query", ["x".repeat(129)])).rejects.toThrow("token");
    expect(p.calls).toEqual([]);
    for (const vectors of [[], [[1]], [[NaN, 1, 0]], [[0, 0, 0]]]) {
      p.embed_documents = async () => ({ vectors });
      await expect(callEmbedding(p, p.identity, "document", ["x"])).rejects.toThrow("response");
    }
    p.embed_documents = async () => { p.identity.model_revision = "changed"; return { vectors: [[1, 0, 0]] }; };
    await expect(callEmbedding(p, p.identity, "document", ["x"])).rejects.toThrow("identity");
  });
  it("passes cancellation through and rejects late results after abort", async () => {
    const p = fakeEmbedding(), controller = new AbortController(), calls: EmbeddingCall[] = [];
    p.embed_query = async (_text, { signal }) => {
      expect(signal).toBe(controller.signal); controller.abort(); return { vector: [1, 0, 0] };
    };
    await expect(callEmbedding(p, p.identity, "query", ["q"], { signal: controller.signal, on_call: c => calls.push(c) })).rejects.toThrow();
    expect(calls[0].status).toBe("cancelled");
    await expect(callEmbedding(p, p.identity, "document", ["q"], { signal: controller.signal })).rejects.toThrow();
    expect(p.calls).toEqual([]);
  });
});
