import { t as uiText } from '@understand-book/web/reader/i18n';
import type { AccountUsageGroup } from './account-allowance';

export interface AccountCostSummary {
  request_count: number;
  unknown_cost_requests: number;
  pending_requests: number;
  cost_breakdown: Record<AccountCostCategory, AccountCostCategoryMetrics>;
  period_groups: (AccountUsageGroup & { starts_at: number | null; expires_at: number | null })[];
}
export const accountCostCategories = [['model', uiText('模型')], ['web_search', uiText('搜索')], ['web_fetch', uiText('网页读取')], ['service', uiText('其他服务')]] as const;
export type AccountCostCategory = typeof accountCostCategories[number][0];
export interface AccountCostCategoryMetrics {
  request_count: number;
  unknown_cost_requests: number;
  pending_requests: number;
  estimated_cost_requests: number;
  reconciliation_requests: number;
}
export interface AccountCostTask extends AccountCostSummary {
  cost_owner: 'account'; user_id: string; parent_task_ref: string; latest_at: number;
  learning_workspace_ids: string[]; related_learning_workspace_ids: string[]; purposes: string[];
}
export interface AccountCostReport {
  as_of: number;
  account: AccountCostSummary;
  workspaces: (AccountCostSummary & { learning_workspace_id: string; workspace_goal?: string | null })[];
  account_level: AccountCostSummary[];
  tasks: { items: AccountCostTask[]; total: number; limit: number; offset: number };
}
export type AccountCostFilter = { parent_task_ref: string } | { learning_workspace_id: string } | { account_level: true };
export function accountCostQuery(filter: AccountCostFilter, offset = 0): string {
  return new URLSearchParams({ ...Object.fromEntries(Object.entries(filter).map(([k, v]) => [k, String(v)])), limit: '20', offset: String(offset) }).toString();
}
