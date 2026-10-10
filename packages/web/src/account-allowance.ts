export interface AllowancePeriod {
  period_id: string;
  starts_at: number;
  expires_at: number;
  balance: {
    granted_micro_cny: number;
    debited_micro_cny: number;
    active_reserved_micro_cny: number;
    pending_micro_cny: number;
    available_micro_cny: number;
  };
}
export interface AccountUsage {
  call_id: string;
  period_id: string;
  run_ref: string | null;
  task_ref: string | null;
  purpose: string;
  model: string;
  state: 'reserved' | 'sent' | 'pending' | 'settled' | 'released';
  reserved_micro_cny: number;
  account_debit_micro_cny: number | null;
  created_at: number;
  settled_at: number | null;
}
export interface AccountUsagePage { items: AccountUsage[]; total: number; limit: number; offset: number }

/** Display all six stored decimal places when needed; never round a small charge to zero. */
export function allowanceAmount(micro: number | null): string {
  if (micro === null) return '待核算';
  const value = BigInt(micro), magnitude = value < 0n ? -value : value;
  const fraction = (magnitude % 1_000_000n).toString().padStart(6, '0').replace(/0+$/, '').padEnd(2, '0');
  return `${value < 0n ? '-' : ''}${magnitude / 1_000_000n}.${fraction}`;
}
export function allowanceTime(seconds: number): string {
  return new Intl.DateTimeFormat('zh-CN', { timeZone: 'Asia/Hong_Kong', year: 'numeric', month: '2-digit', day: '2-digit', hour: '2-digit', minute: '2-digit', hourCycle: 'h23' }).format(new Date(seconds * 1000));
}
export const usageState = { reserved: '正在预留', sent: '调用中', pending: '待核算', settled: '已扣减', released: '已释放（不扣减）' };
export function usageOccupation(item: AccountUsage): number {
  return ['reserved', 'sent', 'pending'].includes(item.state) ? item.reserved_micro_cny : 0;
}

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
