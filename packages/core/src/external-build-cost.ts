import { existsSync, mkdirSync, readFileSync, readdirSync, writeFileSync } from "node:fs";
import path from "node:path";
import type { BuildExecutionProfileSelectionV1 } from "./build-execution-profile";
import { TranscriptServiceLedger } from "./service-usage";

/** Ownership is durable; authentication belongs only to the running operator host. */
export interface ExternalBuildCostBinding {
  version: "external_build_cost.v1";
  metering?: "provider_calls_v1";
  cost_owner: "platform" | "account";
  user_id?: string;
  learning_workspace_id?: string;
  max_micro_cny: number;
  reserved_per_execution_micro_cny: number;
  confirmation: string;
  /** A multi-material operator build shares one confirmed parent budget. */
  task_ref?: string;
}

export interface ExternalBuildCostConnection {
  base_url: string;
  cookie: string;
  csrf_token: string;
  origin: string;
}

export const EXTERNAL_BUILD_COST_CAPABILITY = Object.freeze({
  codex_mcp_v3: Object.freeze({ budget_enforced: false, receipt_capability: "none" as const }),
  dsh_native_v4: Object.freeze({ budget_enforced: false, receipt_capability: "partial" as const }),
  codex_cli_video: Object.freeze({ budget_enforced: false, receipt_capability: "partial" as const }),
});

export function readExternalBuildCostBinding(value: unknown, profile: BuildExecutionProfileSelectionV1): ExternalBuildCostBinding {
  if (!value || typeof value !== "object" || Array.isArray(value)) throw new Error("external_build_cost_invalid");
  const v = value as Record<string, unknown>;
  if (Object.keys(v).some(k => !["version", "cost_owner", "user_id", "learning_workspace_id", "max_micro_cny", "reserved_per_execution_micro_cny", "confirmation", "task_ref", "metering"].includes(k))
    || v.version !== "external_build_cost.v1" || !["platform", "account"].includes(String(v.cost_owner))
    || !Number.isSafeInteger(v.max_micro_cny) || (v.max_micro_cny as number) < 1
    || !Number.isSafeInteger(v.reserved_per_execution_micro_cny) || (v.reserved_per_execution_micro_cny as number) < 1
    || (v.reserved_per_execution_micro_cny as number) > (v.max_micro_cny as number)
    || typeof v.confirmation !== "string" || !v.confirmation.trim()
    || [v.user_id, v.learning_workspace_id].some(id => id !== undefined && (typeof id !== "string" || !id.trim()))
    || (v.task_ref !== undefined && (typeof v.task_ref !== "string" || !/^[A-Za-z0-9_-]{1,128}$/u.test(v.task_ref)))) {
    throw new Error("external_build_cost_invalid");
  }
  // Published profiles, never caller-supplied capability claims, decide paid eligibility.
  if (v.metering !== undefined && (v.metering !== "provider_calls_v1" || profile.profile_id !== "dsh_native_v4")) throw new Error("external_build_metering_unsupported");
  const capability = v.metering === "provider_calls_v1" ? {budget_enforced:true,receipt_capability:"complete"} : EXTERNAL_BUILD_COST_CAPABILITY[profile.profile_id];
  if (v.cost_owner === "account" && (!capability.budget_enforced || capability.receipt_capability !== ("complete" as string))) {
    throw new Error("external_build_personal_payment_unsupported: executor cannot enforce a monetary budget and provide complete cost receipts");
  }
  if (v.cost_owner === "platform" && (v.user_id !== undefined || v.learning_workspace_id !== undefined)) throw new Error("platform_build_has_account_scope");
  return { version: "external_build_cost.v1", ...(v.metering ? {metering:"provider_calls_v1" as const} : {}), cost_owner: v.cost_owner as ExternalBuildCostBinding["cost_owner"],
    max_micro_cny: v.max_micro_cny as number, reserved_per_execution_micro_cny: v.reserved_per_execution_micro_cny as number,
    confirmation: v.confirmation, ...(v.task_ref ? { task_ref: v.task_ref as string } : {}), ...(v.user_id ? { user_id: v.user_id as string } : {}),
    ...(v.learning_workspace_id ? { learning_workspace_id: v.learning_workspace_id as string } : {}) };
}

interface CostReceipt {
  call_id: string;
  status: "partial" | "unknown" | "not_sent";
  usage?: Record<string, unknown>;
  evidence: Record<string, unknown>;
  succeeded: boolean;
}
interface LaunchRecord {
  call_id: string;
  execution_id: string;
  logical_call_id: string;
  attempt: number;
  state: "prepared" | "admitted" | "receipt_pending" | "recorded" | "not_admitted";
  receipt?: CostReceipt;
  metered?: boolean;
}

/** One execution total per actual host launch. Model token observations are never a confirmed bill. */
export class ExternalBuildCostLedger {
  private readonly directory: string;
  constructor(readonly taskRef: string, readonly binding: ExternalBuildCostBinding,
    private readonly connection: ExternalBuildCostConnection, registryRoot: string,
    private readonly fetcher: typeof fetch = fetch) {
    this.directory = path.join(registryRoot, "external-build-cost", taskRef);
  }
  transcriptMeter(workspace: string, processingConfirmation?: string) {
    return new TranscriptServiceLedger(this.taskRef, workspace, this.connection, this.fetcher, processingConfirmation);
  }
  private file(callId: string) {
    if (!/^[a-zA-Z0-9_-]+$/u.test(callId)) throw new Error("external_build_call_id_invalid");
    return path.join(this.directory, `${callId}.json`);
  }
  private write(record: LaunchRecord) {
    mkdirSync(this.directory, { recursive: true });
    writeFileSync(this.file(record.call_id), JSON.stringify(record));
  }
  private records(): LaunchRecord[] {
    return existsSync(this.directory) ? readdirSync(this.directory).filter(f => f.endsWith(".json"))
      .map(f => JSON.parse(readFileSync(path.join(this.directory, f), "utf8"))) : [];
  }
  private async request(suffix: string, body?: unknown, missing = false): Promise<any> {
    const url = `${this.connection.base_url.replace(/\/$/u, "")}/api/admin/build-tasks/${encodeURIComponent(this.taskRef)}${suffix}`;
    const response = await this.fetcher(url, { method: body === undefined ? "GET" : "POST",
      headers: { "Content-Type": "application/json", Cookie: this.connection.cookie,
        "X-CSRF-Token": this.connection.csrf_token, Origin: this.connection.origin },
      ...(body === undefined ? {} : { body: JSON.stringify(body) }), signal: AbortSignal.timeout(15000) });
    if (missing && response.status === 404) return undefined;
    if (!response.ok) { const failure = await response.json().catch(()=>null); throw new Error(failure?.error_code ?? `external_build_cost_http_${response.status}`); }
    return response.json();
  }
  async prepare() {
    const { cost_owner, user_id, learning_workspace_id, max_micro_cny, confirmation } = this.binding;
    if (this.binding.task_ref) {
      // The operator's start/continue transaction already confirmed the shared budget.
      const budget = await this.request('');
      if (budget.cost_owner !== cost_owner || budget.max_micro_cny !== max_micro_cny) throw new Error('external_build_budget_changed');
    } else {
      await this.request("/budget", { cost_owner, user_id, learning_workspace_id, max_micro_cny, confirmation });
    }
    // A dead host's launch is never resumed as if it had not spent money. Preserve uncertainty.
    for (const record of this.records()) {
      if (record.state === "recorded" || record.state === "not_admitted") continue;
      const existing = await this.request(`/charges/${encodeURIComponent(record.call_id)}`, undefined, true);
      if (!existing) { record.state = "not_admitted"; this.write(record); continue; }
      if (["settled", "released"].includes(existing.charge?.state)) {
        record.state = "recorded"; this.write(record); continue;
      }
      record.receipt ??= { call_id: record.call_id, status: "unknown", succeeded: false,
        evidence: { source: "external_host_recovery", reason: "terminal_receipt_missing", execution_id: record.execution_id } };
      await this.sendReceipt(record);
    }
  }
  async admit(launchId: string, logicalCallId: string, provider: string, model: string,
    executor: keyof typeof EXTERNAL_BUILD_COST_CAPABILITY = "dsh_native_v4", metered?: {execution_id:string;request:unknown}) {
    const callId = `ext_${launchId}`;
    if (existsSync(this.file(callId))) throw new Error("external_build_launch_already_recorded");
    const attempt = this.records().filter(r => r.logical_call_id === logicalCallId && r.state !== "not_admitted").length + 1;
    const record: LaunchRecord = { call_id: callId, execution_id: metered?.execution_id ?? launchId, logical_call_id: logicalCallId, attempt, state: "prepared", ...(metered ? {metered:true} : {}) };
    this.write(record);
    try {
      const response = await this.request(metered ? "/metered-admit" : "/admit", { ...(metered ? {request:metered.request} : {}), execution_id: record.execution_id, call_id: callId, logical_call_id: logicalCallId,
        attempt, granularity: metered ? "calls" : "execution_total", reserved_micro_cny: this.binding.reserved_per_execution_micro_cny,
        provider, model, purpose: "external_build", ...EXTERNAL_BUILD_COST_CAPABILITY[executor] });
      if (response.admitted !== true || response.call_id !== callId) throw new Error("external_build_admission_invalid");
      record.state = "admitted"; this.write(record);
      return callId;
    } catch (error) {
      // The child has not started. If admission committed but its response was lost, release this reservation.
      record.receipt = { call_id: callId, status: "not_sent", succeeded: false,
        evidence: { source: "external_host", reason: "admission_response_failed_before_launch" } };
      this.write(record);
      const existing = await this.request(`/charges/${encodeURIComponent(callId)}`, undefined, true).catch(() => undefined);
      if (existing) {
        await this.sendReceipt(record).catch(() => undefined);
      }
      throw error;
    }
  }
  async settle(callId: string, input: { model_calls: number; usages: Record<string, unknown>[]; succeeded: boolean; outcome: string;
    source?: "dsh_host_stream_observations" | "codex_cli_stream_observations" }) {
    const record = JSON.parse(readFileSync(this.file(callId), "utf8")) as LaunchRecord;
    record.receipt = { call_id: callId,
      status: input.model_calls === 0 ? "not_sent" : input.usages.length ? "partial" : "unknown",
      ...(input.usages.length ? { usage: { observations: input.usages } } : {}),
      evidence: { source: input.source ?? "dsh_host_stream_observations", outcome: input.outcome, model_calls_observed: input.model_calls,
        billing_granularity: "execution_total", monetary_cost_available: false }, succeeded: input.succeeded };
    await this.sendReceipt(record);
  }
  async settleCall(callId: string, usage: Record<string, unknown> | undefined, succeeded: boolean) {
    const record = JSON.parse(readFileSync(this.file(callId), "utf8")) as LaunchRecord;
    const number = (key: string): number | undefined => typeof usage?.[key] === "number" ? usage[key] as number : undefined;
    const input = number("inputTokens"), cached = number("cacheReadTokens"), written = number("cacheWriteTokens");
    record.receipt = {call_id:callId,status:usage ? "partial" : "unknown", succeeded,
      ...(usage ? {usage:{ input_tokens: input === undefined ? null : input + (cached ?? 0) + (written ?? 0),
        output_tokens:number("outputTokens") ?? null,cached_input_tokens:cached ?? null,cache_creation_input_tokens:written ?? null,
        reasoning_output_tokens:number("reasoningTokens") ?? null,total_tokens:number("totalTokens") ?? null }} : {}),
      evidence:{source:"dsh_provider_stream",raw_usage:usage ?? null,billing_granularity:"calls"}};
    await this.sendReceipt(record);
  }
  private async sendReceipt(record: LaunchRecord) {
    record.state = "receipt_pending"; this.write(record);
    await this.request(record.metered ? "/metered-receipts" : "/receipts", record.receipt);
    record.state = "recorded"; this.write(record);
  }
}
