import { moneyInput } from './format';
import { percentageInput } from './allowance';

export type CloudPlanTier = 'basic' | 'plus' | 'pro';
export type CloudPlanDuration = 'day1' | 'day3' | 'day7' | 'month';
export const tierLabels: Record<CloudPlanTier, string> = { basic: '基础版', plus: '进阶版', pro: '专业版' };
export const durationLabels: Record<CloudPlanDuration, string> = { day1: '1 天冲刺', day3: '3 天冲刺', day7: '7 天冲刺', month: '按月学习' };
export const unavailableLabels: Record<string, string> = {
  disabled: '尚未启用', test_only: '仅供测试', price_required: '售价待定', basis_required: '额度基准待定',
  allowance_required: '授予额度待定', storage_required: '个人空间待定', file_limit_required: '单文件上限待定',
  retention_required: '资料保留时长待定', payment_instructions_required: '需填写联系与开通说明',
};
export interface CloudPlanProduct {
  product_id: string; title: string; tier: CloudPlanTier; duration: CloudPlanDuration; revision: number;
  enabled: boolean; test_only: boolean; amount_fen: number | null;
  basis: { version: string; basis_micro_cny: number } | null;
  grant_milli_percent: number | null; storage_bytes: number | null; max_file_bytes: number | null;
  retention_seconds: number | null; instructions: string; saleable: boolean; unavailable_reasons: string[];
}
export interface CloudEntitlement {
  period_id: string; product_id: string; product_revision: number; title: string;
  tier: CloudPlanTier; duration: CloudPlanDuration; starts_at: number; expires_at: number;
  grant_milli_percent: number; storage_bytes: number; max_file_bytes: number; retention_seconds: number; amount_fen: number;
}
export interface UserCloudPlans { current: CloudEntitlement | null; next: CloudEntitlement | null; catalog: { items: CloudPlanProduct[] } }

function optional(text: string, parse: (text: string) => number): number | null { return text ? parse(text) : null; }
export function positiveUnit(text: string, scale: number): number {
  if (!/^\d+(?:\.\d+)?$/.test(text)) throw new Error('请输入有效的非负数。');
  const value = Number(text) * scale;
  if (!Number.isSafeInteger(value) || value < 0) throw new Error('该数值无法精确换算，请调整小数位数。');
  return value;
}
export function cloudPlanForm(data: FormData, existing: CloudPlanProduct | null): Record<string, unknown> {
  const text = (name: string) => String(data.get(name) ?? '').trim();
  const version = text('basis_version'), amount = text('basis_amount');
  if (!!version !== !!amount) throw new Error('额度基准名称和金额须同时填写，或同时留空。');
  const tier = text('tier') as CloudPlanTier, duration = text('duration') as CloudPlanDuration;
  if (!(tier in tierLabels) || !(duration in durationLabels)) throw new Error('请选择套餐档位和时长。');
  return {
    product_id: existing?.product_id ?? crypto.randomUUID(), revision: existing?.revision ?? 0,
    title: text('title'), tier, duration, enabled: data.get('enabled') === 'on', test_only: data.get('test_only') === 'on',
    amount_fen: optional(text('amount'), value => moneyInput(value, 2)),
    basis: version ? { version, basis_micro_cny: moneyInput(amount) } : null,
    grant_milli_percent: optional(text('grant'), percentageInput),
    storage_bytes: optional(text('storage_mb'), value => positiveUnit(value, 1024 ** 2)),
    max_file_bytes: optional(text('max_file_mb'), value => positiveUnit(value, 1024 ** 2)),
    retention_seconds: optional(text('retention_days'), value => positiveUnit(value, 86400)),
    instructions: text('instructions'),
  };
}
