import { t as uiText } from '@understand-book/web/reader/i18n';
import type { AllowanceBasis } from './account-allowance';
import { networkFetch } from './network-client';

export type CloudPlanTier = 'basic' | 'plus' | 'pro';
export type CloudPlanDuration = 'day1' | 'day3' | 'day7' | 'month';
export const cloudPlanTiers: Record<CloudPlanTier, string> = { basic: uiText('基础版'), plus: uiText('进阶版'), pro: uiText('专业版') };
export const cloudPlanDurations: Record<CloudPlanDuration, string> = { day1: uiText('1 天冲刺'), day3: uiText('3 天冲刺'), day7: uiText('7 天冲刺'), month: uiText('按月学习') };
export interface CloudPlanProduct {
  product_id: string; title: string; tier: CloudPlanTier; duration: CloudPlanDuration; revision: number;
  enabled: boolean; test_only: boolean; amount_fen: number | null; basis: AllowanceBasis | null;
  grant_milli_percent: number | null; storage_bytes: number | null; max_file_bytes: number | null;
  retention_seconds: number | null; instructions: string; saleable: boolean; unavailable_reasons: string[];
}
export interface CloudPlanEntitlement {
  period_id: string; product_id: string; product_revision: number; title: string;
  tier: CloudPlanTier; duration: CloudPlanDuration; starts_at: number; expires_at: number;
  storage_bytes: number; max_file_bytes: number; retention_seconds: number;
  receipt_id: string; month_anchor_day: number | null; basis: AllowanceBasis;
  grant_milli_percent: number; amount_fen: number;
}
export interface AccountCloudPlans {
  current: CloudPlanEntitlement | null; next: CloudPlanEntitlement | null;
  catalog: { items: CloudPlanProduct[] };
}
export interface CloudStorageSummary {
  used_bytes: number; reserved_bytes: number; total_bytes: number; limit_bytes: number | null;
  max_file_bytes: number | null; available_bytes: number; can_upload: boolean; over_limit: boolean;
  expires_at: number | null; retained_until: number | null;
}
export const accountCloudPlans = {
  read: () => networkFetch<AccountCloudPlans>('GET', '/account/plans'),
  storage: () => networkFetch<CloudStorageSummary>('GET', '/account/storage'),
};
export function saleableCloudPlans(items: CloudPlanProduct[], duration: CloudPlanDuration): CloudPlanProduct[] {
  return items.filter(p => p.duration === duration && p.enabled && !p.test_only && p.saleable
    && p.amount_fen !== null && p.basis !== null && p.grant_milli_percent !== null
    && p.storage_bytes !== null && p.max_file_bytes !== null && p.retention_seconds !== null)
    .sort((a, b) => Object.keys(cloudPlanTiers).indexOf(a.tier) - Object.keys(cloudPlanTiers).indexOf(b.tier));
}
export function cloudStorageSize(bytes: number): string {
  const unit = bytes >= 1024 ** 3 ? 1024 ** 3 : bytes >= 1024 ** 2 ? 1024 ** 2 : bytes >= 1024 ? 1024 : 1;
  const label = unit === 1024 ** 3 ? 'GB' : unit === 1024 ** 2 ? 'MB' : unit === 1024 ? 'KB' : uiText('字节');
  return `${Number((bytes / unit).toFixed(2))} ${label}`;
}
export function cloudQuotaLabel(product: CloudPlanProduct, catalog: CloudPlanProduct[]): string {
  const amount = (plan: CloudPlanProduct) => plan.basis && plan.grant_milli_percent != null ? plan.basis.basis_micro_cny * plan.grant_milli_percent / 100000 : 0;
  const basics = saleableCloudPlans(catalog, product.duration).filter(plan => plan.tier === 'basic');
  const base = basics.length ? Math.min(...basics.map(amount)) : 0;
  if (!base) return uiText('含本期 AI 使用额度');
  return uiText('基础版用量的 {0} 倍', [Number((amount(product) / base).toFixed(2))]);
}
export function cloudRetention(seconds: number): string {
  return seconds % 86400 === 0 ? uiText('{0} 天', [seconds / 86400]) : seconds % 3600 === 0 ? uiText('{0} 小时', [seconds / 3600]) : uiText('{0} 秒', [seconds]);
}
