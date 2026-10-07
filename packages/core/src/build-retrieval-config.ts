import { readFileSync } from "node:fs";
import path from "node:path";
import { z } from "zod";
import { BuildRetrievalPlanZ, attachBuildPlanDigest, validateBuildPlanV1, type BuildPlanV1, type BuildRetrievalPlan, type BuildRetrievalSelection, retrievalScopeIncludes } from "./build-intent";
import type { BuildRetrievalRuntime } from "./automatic-build-retrieval";
import { localEmbeddingIdentity, openLocalEmbeddingProvider } from "./local-embedding-provider";

const ConfigZ = z.object({ version: z.literal("local_embedding_config.v1"),
  runtime_dir: z.string().min(1), model_dir: z.string().min(1) }).strict();
export function readLocalEmbeddingConfig(file = process.env.UNDERSTAND_BOOK_EMBEDDING_CONFIG) {
  if (!file) throw new Error("Set UNDERSTAND_BOOK_EMBEDDING_CONFIG to the local embedding configuration file");
  const config = ConfigZ.parse(JSON.parse(readFileSync(file, "utf8")));
  return { runtime_dir: path.resolve(path.dirname(file), config.runtime_dir), model_dir: path.resolve(path.dirname(file), config.model_dir) };
}

export function configureBuildRetrieval(mode: "lexical_only" | "semantic_required", budget: BuildRetrievalPlan["budget"], configFile?: string, data_scope: BuildRetrievalSelection["data_scope"] = "current_and_previous_formal_object_projections_and_queries"): BuildRetrievalPlan {
  return BuildRetrievalPlanZ.parse({ selection: { retrieval_mode: mode,
    ...(mode === "semantic_required" ? { provider: { location: "local", identity: localEmbeddingIdentity(readLocalEmbeddingConfig(configFile)) } } : {}),
    data_scope },
    estimate: { records: null, queries: null, basis: "unknown_until_fragments",
      ...(mode === "semantic_required" ? { cost: { amount: 0, currency: "USD" } } : {}) }, budget });
}

export function buildRetrievalScope(plan: BuildPlanV1): BuildRetrievalSelection["data_scope"] {
  const structure = plan.content_profile.id === "technical_learning" && plan.public_stage_closure.includes("book_structure");
  const formal = plan.public_stage_closure.includes("formal_objects");
  return structure ? formal ? "book_structure_and_formal_object_projections_and_queries" : "book_structure_projections_and_queries"
    : "current_and_previous_formal_object_projections_and_queries";
}

/** Host location is resolved only if Core needs new vectors. Harness is deliberately absent. */
export function configuredBuildRetrievalRuntime(plan: BuildPlanV1, configFile?: string): BuildRetrievalRuntime {
  const selection = plan.retrieval!.selection;
  return { selection, ...(selection.retrieval_mode === "semantic_required" ? { open_provider: async (signal?: AbortSignal) => {
    if (selection.provider?.location !== "local") throw new Error("This build supports only the configured local MiniLM provider");
    const config = readLocalEmbeddingConfig(configFile);
    return openLocalEmbeddingProvider(config, selection.provider!.identity, signal);
  } } : {}) };
}

export function reviseBuildRetrieval(input: BuildPlanV1, retrieval: BuildRetrievalPlan): BuildPlanV1 {
  const plan = structuredClone(validateBuildPlanV1(input));
  delete plan.confirmed_at; delete plan.confirmation_source;
  return attachBuildPlanDigest({ ...plan, revision: plan.revision + 1, status: "draft", retrieval: BuildRetrievalPlanZ.parse(retrieval) });
}

export function buildRetrievalReview(plan: BuildPlanV1): string {
  const r = plan.retrieval;
  if (!r) return "候选召回：既有本地词法检索。";
  const p = r.selection.provider, local = p?.location === "local";
  return ["## 候选召回配置", `模式：${r.selection.retrieval_mode}`,
    `执行位置：${p ? local ? "local（本机）" : "remote（远端）" : "本机，无 embedding provider"}`,
    ...(p ? [`Provider：${p.identity.provider_id}；模型：${p.identity.model_id}；revision：${p.identity.model_revision ?? "未知"}；维度：${p.identity.dimensions}`,
      `向量配置：${JSON.stringify(p.identity.embedding_config)}`] : []),
    `数据范围：${r.selection.data_scope}。`,
    ...(retrievalScopeIncludes(r.selection.data_scope, "book_structure") ? ["结构投影：章节问题与概述、候选含义／条件／已有别名，以及主题搜索 query；完整原文通过既有证据读取交付。"] : []),
    ...(retrievalScopeIncludes(r.selection.data_scope, "formal_objects") ? ["正式对象投影：当前及 previous 正式对象的 kind、meaning、aliases、conditions，以及关系角色/复合组件的一跳摘要；另含 focus 投影或显式搜索 query。"] : []),
    `数据去向：${r.selection.retrieval_mode === "lexical_only" ? "仅本机词法处理" : local ? "仅本机推理，不发送远端" : "发送至所选远端 provider"}。`,
    `待 embedding 文档估计：${r.estimate.records ?? "未知，fragments 就绪后展开"}；query 估计：${r.estimate.queries ?? "未知，随对齐决策产生"}。`,
    `执行上限（含失败与重试）：documents=${r.budget.max_documents}，queries=${r.budget.max_queries}，calls=${r.budget.max_calls}，input tokens=${r.budget.max_input_tokens ?? "由记录/调用额度约束"}。`,
    `总计划上限：tokens=${plan.budget.max_total_tokens ?? "未另设"}；分钟=${plan.budget.max_wall_clock_minutes ?? "未另设"}。`,
    `费用类别：${r.selection.retrieval_mode === "lexical_only" ? "无推理 API 费用" : local ? "本地推理 API 费用 0 USD；实际 tokenizer 用量可得，电力费用未测" : r.estimate.cost ? JSON.stringify(r.estimate.cost) : "未知，以 provider 可得 usage 记录"}。`,
    "语义召回默认关闭；SR6 质量与长材料验收仍待完成。",
  ].join("\n\n");
}
