import type { AllowanceBasis, Page, Percentage, UsageMetrics } from './types';

export interface CostPeriod {
  user_id: string; period_id: string; allowance_basis: AllowanceBasis | null;
  percentages: { debited: Percentage; active_reserved: Percentage; pending: Percentage } | null;
}
export interface CostCategoryMetrics extends UsageMetrics {
  estimated_cost_micro_cny: number; estimated_cost_requests: number;
  platform_covered_micro_cny: number; reconciliation_requests: number;
  provider_cost_complete: boolean; provider_cost_micro_cny: number | null;
}
export interface CostSummary extends CostCategoryMetrics {
  cost_breakdown: Record<CostCategory, CostCategoryMetrics>;
  period_groups: CostPeriod[];
}
export const costCategories = [['model', '模型'], ['web_search', '搜索'], ['web_fetch', '网页读取'], ['service', '其他服务']] as const;
export type CostCategory = typeof costCategories[number][0];
export interface CostTask extends CostSummary {
  cost_owner: 'account' | 'platform'; user_id: string | null; parent_task_ref: string;
  learning_workspace_ids: string[]; related_learning_workspace_ids: string[]; purposes: string[];
}
export interface CostReport {
  as_of: number; from: string | null; to: string | null;
  account: CostSummary; platform: CostSummary | null;
  accounts: (CostSummary & { user_id: string })[];
  workspaces: (CostSummary & { user_id: string; learning_workspace_id: string; workspace_goal: string | null; archived: boolean })[];
  account_level: (CostSummary & { user_id: string })[];
  tasks: Page<CostTask>;
}
export interface CostFilter { user_id?: string; cost_owner?: 'account' | 'platform'; parent_task_ref?: string; learning_workspace_id?: string; account_level?: boolean }
export function costChargesPath(filter: CostFilter, from: string, to: string, offset = 0): string {
  const params = new URLSearchParams({ from, to, limit: '20', offset: String(offset) });
  for (const [key, value] of Object.entries(filter)) if (value !== undefined && value !== '') params.set(key, String(value));
  return `/admin/charges?${params}`;
}
