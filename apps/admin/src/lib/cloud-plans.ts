import { moneyInput } from './format';

export type CloudPlanTier = 'basic' | 'plus' | 'pro';
export type CloudPlanDuration = 'day1' | 'day3' | 'day7' | 'month';
export const tierLabels: Record<CloudPlanTier, string> = { basic: '基础版', plus: '进阶版', pro: '专业版' };
export const durationLabels: Record<CloudPlanDuration, string> = { day1: '1 天冲刺', day3: '3 天冲刺', day7: '7 天冲刺', month: '按月学习' };
export const unavailableLabels: Record<string, string> = {
  disabled: '尚未启用', test_only: '仅供测试', price_required: '售价待定', basis_required: 'AI 预算待定',
  allowance_required: 'AI 预算待定', storage_required: '个人空间待定', file_limit_required: '单文件上限待定',
  retention_required: '资料保留时长待定', payment_instructions_required: '需填写联系与开通说明',
};
export interface CloudPlanProduct {
  product_id: string; title: string; tier: CloudPlanTier; duration: CloudPlanDuration; revision: number;
  enabled: boolean; test_only: boolean; amount_fen: number | null;
  budget_micro_cny?: number | null;
  basis: { version: string; basis_micro_cny: number } | null;
  grant_milli_percent: number | null; storage_bytes: number | null; max_file_bytes: number | null;
  retention_seconds: number | null; instructions: string; saleable: boolean; unavailable_reasons: string[];
}
export interface CloudEntitlement {
  period_id: string; product_id: string; product_revision: number; title: string;
  tier: CloudPlanTier; duration: CloudPlanDuration; starts_at: number; expires_at: number;
  grant_milli_percent: number; storage_bytes: number; max_file_bytes: number; retention_seconds: number; amount_fen: number;
  basis: { version: string; basis_micro_cny: number };
}
export interface UserCloudPlans { current: CloudEntitlement | null; next: CloudEntitlement | null; catalog: { items: CloudPlanProduct[] }; purchase_starts_at: number; purchase_blocked_by_unlimited: boolean }

export function planBudget(plan: Pick<CloudPlanProduct, 'basis' | 'grant_milli_percent' | 'budget_micro_cny'>): number | null {
  if (plan.budget_micro_cny != null) return plan.budget_micro_cny;
  if (!plan.basis || plan.grant_milli_percent == null) return null;
  return Number(BigInt(plan.basis.basis_micro_cny) * BigInt(plan.grant_milli_percent) / 100000n);
}

function optional(text: string, parse: (text: string) => number): number | null { return text ? parse(text) : null; }
export function unitInput(value: number | null | undefined, scale: number): string {
  return value == null ? '' : String(value / scale);
}
export function positiveUnit(text: string, scale: number): number {
  if (!/^\d+(?:\.\d+)?$/.test(text)) throw new Error('请输入有效的非负数。');
  const [whole, fraction = ''] = text.split('.');
  const numerator = BigInt(whole + fraction) * BigInt(scale);
  const denominator = 10n ** BigInt(fraction.length);
  if (numerator % denominator !== 0n || numerator / denominator > BigInt(Number.MAX_SAFE_INTEGER)) {
    throw new Error('该数值无法精确换算，请调整小数位数。');
  }
  return Number(numerator / denominator);
}
export function cloudPlanForm(data: FormData, existing: CloudPlanProduct | null): Record<string, unknown> {
  const text = (name: string) => String(data.get(name) ?? '').trim();
  // An integer number of seconds may have no finite decimal representation in days.
  const unit = (name: string, scale: number, original: number | null | undefined) =>
    original != null && text(name) === unitInput(original, scale)
      ? original : optional(text(name), value => positiveUnit(value, scale));
  const tier = text('tier') as CloudPlanTier, duration = text('duration') as CloudPlanDuration;
  if (!(tier in tierLabels) || !(duration in durationLabels)) throw new Error('请选择套餐档位和时长。');
  const budget = optional(text('budget'), moneyInput);
  const amount = optional(text('amount'), value => moneyInput(value, 2));
  if ((budget != null && budget <= 0) || (amount != null && amount <= 0)) throw new Error('售价和 AI 使用预算须大于零；未确定时可留空保存草稿。');
  return {
    product_id: existing?.product_id ?? crypto.randomUUID(), revision: existing?.revision ?? 0,
    title: text('title'), tier, duration, enabled: data.get('enabled') === 'on', test_only: data.get('test_only') === 'on',
    amount_fen: amount, budget_micro_cny: budget,
    basis: null, grant_milli_percent: null,
    storage_bytes: unit('storage_mb', 1024 ** 2, existing?.storage_bytes),
    max_file_bytes: unit('max_file_mb', 1024 ** 2, existing?.max_file_bytes),
    retention_seconds: unit('retention_days', 86400, existing?.retention_seconds),
    instructions: text('instructions'),
  };
}
