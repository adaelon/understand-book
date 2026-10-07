import { existsSync, mkdirSync, readFileSync, renameSync, writeFileSync } from "node:fs";
import path from "node:path";
import { isDeepStrictEqual } from "node:util";
import { fixture } from "../../packages/core/testdata/semantic-retrieval/gold";
import { configureBuildRetrieval, configuredBuildRetrievalRuntime, buildRetrievalReview } from "../../packages/core/src/build-retrieval-config";
import { compileBuildMode } from "../../packages/core/src/build-capability";
import { transitionBuildPlan, validateBuildPlanV1, type BuildPlanV1 } from "../../packages/core/src/build-intent";
import { prepareBuildRetrieval, readBuildRetrievalState } from "../../packages/core/src/automatic-build-retrieval";
import { summarizeRetrievalUsage } from "../../packages/core/src/automatic-build-budget";
import { retrievalDependencies, prepareSemanticRetrieval, type PreparedRetrieval } from "../../packages/core/src/semantic-retrieval-preparation";
import { GROUPS, experimentContract, decisionBudget, goldDependencies, goldInput, preparedGoldPage, acceptGoldAction, goldSummary, readRanking, releaseDecision, type GoldRun } from "./sr6";

const root = path.resolve(import.meta.dirname, "../..");
if (existsSync(path.join(root, ".env"))) process.loadEnvFile(path.join(root, ".env"));
const [command, directory, configFile, fourth] = process.argv.slice(2);
const onlyGroup = command === "run" ? fourth : undefined;
if (onlyGroup && !(GROUPS as readonly string[]).includes(onlyGroup)) throw new Error("Optional resume group must be A, B or C");
if (!directory || !["prepare", "prepare-revision", "confirm", "run", "report"].includes(command)) throw new Error("Usage: sr6-run.ts prepare|prepare-revision|confirm|run|report OUTPUT_DIR [LOCAL_EMBEDDING_CONFIG] [PRIOR_DIR for prepare-revision, GROUP for run]");
const dir = path.resolve(directory), file = (name: string) => path.join(dir, name);
const read = <T>(name: string): T => JSON.parse(readFileSync(file(name), "utf8"));
const save = (name: string, value: unknown) => {
  mkdirSync(dir, { recursive: true }); writeFileSync(file(`${name}.tmp`), JSON.stringify(value, null, 2) + "\n");
  renameSync(file(`${name}.tmp`), file(name));
};
const prompt = readFileSync(path.join(root, "agents/formal-objects-extractor.md"), "utf8");
let contract = experimentContract(prompt);
const model = process.env.FLUID_LLM_MODEL, base = process.env.OPENCODE_BASE_URL;
const providerMetadata = () => {
  if (!model || !base || !process.env.OPENCODE_API_KEY) throw new Error("Existing generation provider is not configured");
  return { model, origin: new URL(base).origin, endpoint: new URL("chat/completions", base.replace(/\/?$/, "/")).href };
};
interface Plan {
  version: string; status: "draft" | "confirmed"; confirmed_at?: string;
  contract: ReturnType<typeof experimentContract>; generation: ReturnType<typeof providerMetadata>;
  max_calls: number; max_wall_minutes: number; embedding_plan: BuildPlanV1;
  prior_directory?: string; prior_elapsed_ms?: number; embedding_workspace?: string;
}
if (command === "prepare" || command === "prepare-revision") {
  if (existsSync(file("plan.json"))) throw new Error("Plan already exists; resume this experiment instead of resetting its budget");
  if (!configFile) throw new Error("Local embedding config required for preview");
  const retrieval = configureBuildRetrieval("semantic_required", { max_documents: 1024, max_queries: 256, max_calls: 512 }, path.resolve(configFile));
  const embedding_plan = compileBuildMode({ mode: "standard_deep", book_id: "sr6-gold", source_fingerprint: "rates-source-v1",
    content_profile: { id: "technical_learning", version: "technical_learning_v0" }, plan_id: "sr6-gold-local", revision: 1,
    created_at: new Date().toISOString(), budget: { on_exceed: "needs_user" }, public_freshness: [], retrieval }).plan!;
  const plan: Plan = { version: "sr6-execution-plan.v1", status: "draft", contract, generation: providerMetadata(),
    max_calls: GROUPS.length * decisionBudget.max_steps, max_wall_minutes: 120, embedding_plan };
  if (command === "prepare-revision") {
    if (!fourth) throw new Error("Prior experiment directory required");
    const priorDir = path.resolve(fourth), prior: Plan = JSON.parse(readFileSync(path.join(priorDir, "plan.json"), "utf8"));
    if (prior.status !== "confirmed" || prior.prior_directory) throw new Error("Expected the confirmed initial experiment");
    const spent = GROUPS.map(g => JSON.parse(readFileSync(path.join(priorDir, `${g}.json`), "utf8")) as GoldRun);
    const steps = decisionBudget.max_steps - Math.max(...spent.map(r => r.calls.length));
    if (steps <= 0) throw new Error("No equal decision budget remains for a controlled rerun");
    plan.contract = contract = experimentContract(prompt, { thinking: "disabled", max_steps: steps });
    plan.prior_directory = priorDir;
    plan.prior_elapsed_ms = Date.now() - Date.parse(prior.confirmed_at!);
    plan.embedding_workspace = priorDir;
    plan.embedding_plan = prior.embedding_plan;
    plan.max_calls = prior.max_calls; plan.max_wall_minutes = prior.max_wall_minutes;
  }
  save("plan.json", plan);
  save("ranking.json", readRanking(path.join(root, "docs/performance/semantic-retrieval-sr1-vectors-20261001.json")));
  console.log(JSON.stringify({ status: plan.status, model: plan.generation.model, origin: plan.generation.origin,
    groups: GROUPS, per_group_max_steps: contract.decision_budget.max_steps, max_calls_including_prior: plan.max_calls,
    thinking: contract.thinking ?? "provider default", prior_directory: plan.prior_directory,
    max_output_tokens_per_call: contract.max_output_tokens, max_wall_minutes: plan.max_wall_minutes,
    source: "Fixed authored rates Gold only; no user book is sent in this experiment", plan_file: file("plan.json"),
    retrieval_review: buildRetrievalReview(plan.embedding_plan), embedding_plan_id: plan.embedding_plan.plan_id,
    embedding_plan_digest: plan.embedding_plan.plan_digest, long_material: "separate acceptance, not authorized by this Gold plan" }, null, 2));
} else {
  const plan = read<Plan>("plan.json");
  contract = experimentContract(prompt, { thinking: plan.contract.thinking?.type, max_steps: plan.contract.decision_budget.max_steps });
  if (plan.version !== "sr6-execution-plan.v1" || !isDeepStrictEqual(plan.contract, contract)) throw new Error("Experiment prompt, Gold or contract changed; preserve old run and review a new experiment");
  validateBuildPlanV1(plan.embedding_plan);
  if (command === "confirm") {
    // Operator invokes this only after the displayed exact plan has been approved.
    if (plan.status !== "confirmed") {
      plan.status = "confirmed"; plan.confirmed_at = new Date().toISOString();
      if (plan.embedding_plan.status !== "confirmed") plan.embedding_plan = transitionBuildPlan(plan.embedding_plan, "confirmed", { at: plan.confirmed_at, confirmation_source: "codex_conversation" });
      save("plan.json", plan);
    }
    console.log(JSON.stringify({ status: plan.status, plan_file: file("plan.json") }));
  } else {
    const loadRuns = () => GROUPS.map(group => existsSync(file(`${group}.json`)) ? read<GoldRun>(`${group}.json`)
      : { group, work: fixture().work, calls: [] } satisfies GoldRun);
    const runs = loadRuns();
    const priorRuns = (): GoldRun[] => plan.prior_directory ? GROUPS.map(g => JSON.parse(readFileSync(path.join(plan.prior_directory!, `${g}.json`), "utf8"))) : [];
    const embeddingWorkspace = plan.embedding_workspace ?? dir;
    const report = () => {
      const ranking = read<ReturnType<typeof readRanking>>("ranking.json");
      const value = { version: "sr6-agent-report.v1", recorded_at: new Date().toISOString(), generation: plan.generation,
        groups: loadRuns().map(goldSummary), ranking,
        prior_groups: priorRuns().map(goldSummary), contract: plan.contract,
        embedding: summarizeRetrievalUsage(readBuildRetrievalState(embeddingWorkspace).calls),
        release: releaseDecision(ranking, loadRuns()), long_material: null };
      save("report.json", value); return value;
    };
    if (command === "run") {
      if (plan.status !== "confirmed") throw new Error("Approve the displayed SR6 plan and invoke confirm before real calls");
      if (!isDeepStrictEqual(plan.generation, providerMetadata())) throw new Error("Generation provider changed since plan confirmation");
      if (!configFile) throw new Error("Local embedding config required");
      const runtime = configuredBuildRetrievalRuntime(plan.embedding_plan, path.resolve(configFile));
      const usageFile = "elapsed.json";
      let elapsed = existsSync(file(usageFile)) ? read<{ elapsed_ms: number }>(usageFile).elapsed_ms : plan.prior_elapsed_ms ?? 0;
      const spentCalls = () => [...loadRuns(), ...priorRuns()].reduce((n, r) => n + r.calls.length, 0);
      const start = performance.now();
      const persist = (run: GoldRun) => { save(`${run.group}.json`, run); save(usageFile, { elapsed_ms: elapsed + performance.now() - start }); report(); };
      try {
        const active = runs.filter(run => !onlyGroup || run.group === onlyGroup);
        const outcomes = await Promise.allSettled(active.map(async run => {
          while (!run.work.result) {
            if (elapsed + performance.now() - start >= plan.max_wall_minutes * 60000) throw new Error("confirmed wall budget exhausted");
            if (run.calls.at(-1)?.status !== "received" && (run.calls.length >= contract.decision_budget.max_steps
              || spentCalls() >= plan.max_calls)) throw new Error("confirmed model call budget exhausted");
            const d = goldDependencies(run);
            let prepared: PreparedRetrieval | undefined;
            if (run.group === "B") prepared = await prepareSemanticRetrieval({ workspace: dir, ...d, mode: "lexical_only" });
            if (run.group === "C") {
              const request = { slot: `sr6/${path.basename(dir)}/${run.group}/${run.calls.length}`, dependencies: retrievalDependencies(d.records, d.request,
                "semantic_required", plan.embedding_plan.retrieval!.selection.provider!.identity) };
              const failure = await prepareBuildRetrieval({ workspace: embeddingWorkspace, plan: plan.embedding_plan, runtime, request });
              if (failure) throw new Error(`retrieval ${failure.reason}: ${failure.message}`);
              prepared = preparedGoldPage(readBuildRetrievalState(embeddingWorkspace).prepared, request.slot, request.dependencies);
            }
            const input = goldInput(run, prepared);
            let call = run.calls.at(-1);
            if (call?.status === "reserved") throw new Error("Previous model call was interrupted with unknown outcome; retain its quota and inspect the run before resuming");
            if (call?.status === "received") {
              if (call.input !== input) throw new Error("Interrupted response no longer matches current input");
            } else {
              if (run.calls.length >= contract.decision_budget.max_steps || spentCalls() >= plan.max_calls)
                throw new Error("confirmed model call budget exhausted");
              const remainingMs = plan.max_wall_minutes * 60000 - elapsed - (performance.now() - start);
              if (remainingMs <= 0) throw new Error("confirmed wall budget exhausted");
              call = { input, status: "reserved" }; run.calls.push(call); persist(run);
              const began = performance.now();
              try {
                const last = run.calls.at(-2);
                const feedback = last?.status === "rejected" ? `\nCore rejected your preceding action: ${last.error}\nPreceding action: ${JSON.stringify(last.action)}` : "";
                call.request_input = input + feedback; persist(run);
                const response = await fetch(plan.generation.endpoint, { method: "POST", signal: AbortSignal.timeout(Math.min(300000, Math.ceil(remainingMs))),
                  headers: { authorization: `Bearer ${process.env.OPENCODE_API_KEY}`, "content-type": "application/json" },
                  body: JSON.stringify({ model: plan.generation.model, temperature: contract.temperature, max_tokens: contract.max_output_tokens,
                    ...(contract.thinking ? { thinking: contract.thinking } : {}),
                    response_format: { type: "json_object" }, messages: [{ role: "system", content: prompt }, { role: "user", content: call.request_input }] }) });
                if (!response.ok) throw new Error(`generation HTTP ${response.status}`);
                const body = await response.json() as { model?: string; choices?: Array<{ finish_reason?: string; message?: { content?: string } }>; usage?: { prompt_tokens?: number; completion_tokens?: number } };
                call.response = body.choices?.[0]?.message?.content ?? "";
                call.reported_model = body.model; call.finish_reason = body.choices?.[0]?.finish_reason;
                call.input_tokens = body.usage?.prompt_tokens; call.output_tokens = body.usage?.completion_tokens;
                call.elapsed_ms = performance.now() - began; call.status = "received"; persist(run);
              } catch (error) {
                call.elapsed_ms = performance.now() - began; call.status = "failed";
                call.error = error instanceof Error ? error.message : String(error); persist(run); throw error;
              }
            }
            try {
              call.action = JSON.parse(call.response!);
              run.work = acceptGoldAction(run, call.action, prepared); call.status = "accepted";
            } catch (error) {
              call.status = "rejected"; call.error = error instanceof Error ? error.message : String(error);
            }
            persist(run);
            console.log(JSON.stringify({ group: run.group, call: run.calls.length, status: call.status,
              action: (call.action as { kind?: string })?.kind, complete: !!run.work.result }));
            if (run.calls.slice(-3).length === 3 && run.calls.slice(-3).every(c => c.status === "rejected")) throw new Error("Three consecutive rejected actions; inspect Core errors before spending more");
          }
        }));
        const failures = outcomes.flatMap((outcome, i) => outcome.status === "rejected" ? [{ group: active[i].group,
          error: outcome.reason instanceof Error ? outcome.reason.message : String(outcome.reason) }] : []);
        save(onlyGroup ? `failures-${onlyGroup}.json` : "failures.json", failures);
        if (failures.length) throw new Error(JSON.stringify(failures));
      } finally { report(); save(usageFile, { elapsed_ms: elapsed + performance.now() - start }); }
    }
    const result = report(); console.log(JSON.stringify({ groups: result.groups, embedding: result.embedding, release: result.release }, null, 2));
  }
}
