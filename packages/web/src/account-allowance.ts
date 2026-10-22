export interface AllowancePercentage { value: string; nonzero: boolean; below_tenth: boolean }
export interface AllowanceBasis { version: string; basis_micro_cny: number }
export interface AllowancePercentages {
  granted: AllowancePercentage; debited: AllowancePercentage; active_reserved: AllowancePercentage;
  pending: AllowancePercentage; available: AllowancePercentage; deficit: AllowancePercentage;
}
export interface AllowancePeriod {
  period_id: string;
  starts_at: number;
  expires_at: number;
  basis_status: 'bound' | 'unbound';
  validity_state: 'active' | 'scheduled' | 'expired' | 'closed';
  balance: {
    granted_micro_cny: number;
    debited_micro_cny: number;
    active_reserved_micro_cny: number;
    pending_micro_cny: number;
    available_micro_cny: number;
    allowance_basis: AllowanceBasis | null;
    percentages: AllowancePercentages | null;
  };
}
export interface AccountAllowanceSummary {
  current_allowance: AllowancePeriod | null;
  latest_allowance: AllowancePeriod | null;
  status: 'active' | 'scheduled' | 'expired' | 'closed' | 'no_period';
  replenishment: { offers: AllowanceOffer[]; contact?: string | null };
}
export interface AllowanceOffer { offer_id: string; title: string; amount_fen: number; delta_milli_percent: number; basis_version: string; instructions: string }
export function offerPercentage(milli: number): string { return `${milli / 1000}%`; }
export function paymentAmount(fen: number): string { return `${Math.trunc(fen / 100)}.${String(fen % 100).padStart(2, '0')}`; }
export interface AccountUsage {
  call_id: string;
  period_id: string;
  run_ref: string | null;
  task_ref: string | null;
  purpose: string;
  model: string | null;
  entry_kind?: 'model' | 'service' | 'web_search' | 'web_fetch';
  service_kind?: string;
  quantity_ms?: number | null;
  state: 'reserved' | 'sent' | 'pending' | 'settled' | 'released';
  reserved_micro_cny: number;
  account_debit_micro_cny: number | null;
  provider_cost_status?: string;
  needs_reconciliation?: boolean;
  created_at: number;
  settled_at: number | null;
  allowance_basis: AllowanceBasis | null;
  allowance_percentages: { reserved: AllowancePercentage | null; debited: AllowancePercentage | null } | null;
}
export interface AccountUsageGroup {
  period_id: string; allowance_basis: AllowanceBasis | null; unknown_provider_cost_count: number;
  percentages: Pick<AllowancePercentages, 'debited' | 'active_reserved' | 'pending'> | null;
}
export interface AccountUsagePage { items: AccountUsage[]; total: number; limit: number; offset: number; period_groups: AccountUsageGroup[] }

/** Server projections use the immutable original-period basis; never recompute from current grants. */
export function allowancePercent(value: AllowancePercentage | null | undefined): string {
  if (!value) return '待配置比例';
  if (value.below_tenth && value.nonzero) return value.value.startsWith('-') ? '减少低于 0.1%' : '低于 0.1%';
  return `${value.value}%`;
}
export function allowanceTime(seconds: number): string {
  return new Intl.DateTimeFormat('zh-CN', { timeZone: 'Asia/Hong_Kong', year: 'numeric', month: '2-digit', day: '2-digit', hour: '2-digit', minute: '2-digit', hourCycle: 'h23' }).format(new Date(seconds * 1000));
}
export const usageState = { reserved: '正在预留', sent: '调用中', pending: '待核算', settled: '已扣减', released: '已释放（不扣减）' };
export function usageOccupation(item: AccountUsage): string {
  return ['reserved', 'sent', 'pending'].includes(item.state) ? allowancePercent(item.allowance_percentages?.reserved) : '0%';
}
const purposes: Record<string, string> = { web_search: '搜索', web_fetch: '网页读取', video_transcription: '视频转写', outer: '学习对话', main: '学习对话', memory: '记忆整理', memory_update: '记忆整理', memory_compaction: '记忆整理', compaction: '对话整理', context_compaction: '对话整理', build: '资料准备', book_build: '资料准备', prebuild: '预构建回答' };
export function usagePurpose(purpose: string): string { return purpose === 'video_audio_review' ? '原声复核' : purposes[purpose] ?? (purpose.includes('memory') ? '记忆整理' : purpose.includes('build') ? '资料准备' : purpose.includes('compact') ? '对话整理' : '学习服务'); }

const spendReasons: Record<string, string> = {
  ALLOWANCE_INSUFFICIENT: '本期可用额度不足以开始下一次 AI 调用。',
  ALLOWANCE_EXPIRED: '当前没有有效的 AI 使用额度期。',
  MODEL_RATE_UNAVAILABLE: 'AI 计价配置暂不可用，请联系运营者。',
  MODEL_SPEND_STORAGE_UNAVAILABLE: '费用记录暂时无法保存，AI 调用已停止，请稍后重试或联系运营者。',
  MODEL_SPEND_SCOPE_MISSING: '本次 AI 调用无法确认费用归属，请联系运营者。',
  RUN_PERMISSION_REVOKED: '本次 AI 调用的访问权限已失效，请联系运营者。',
  MODEL_CHARGE_RECONCILIATION_REQUIRED: '本次调用费用需要核对，请联系运营者。',
};
export interface SpendError { error_code: string; category: string; message: string; execution_error?: SpendError | null }
export interface SpendNotice { code: string; message: string }
export function spendNotice(error: SpendError | null | undefined, persistence: 'saved' | 'pending' | 'failed' | 'unsubmitted'): SpendNotice | undefined {
  const cause = error?.error_code === 'TURN_UNSAVED' ? error.execution_error : error;
  if (cause?.category !== 'model_spend' || !spendReasons[cause.error_code]) return;
  const saved = persistence === 'saved' ? '本次已完成内容已保存。'
    : persistence === 'failed' ? '本次结果尚未保存，请先重试保存。'
    : persistence === 'unsubmitted' ? '本次问题未提交。' : '正在确认本次内容的保存状态。';
  return { code: cause.error_code, message: `${spendReasons[cause.error_code]}${saved}` };
}
