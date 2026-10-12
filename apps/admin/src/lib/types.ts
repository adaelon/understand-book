export interface Page<T> {
  items: T[];
  total: number;
  limit: number;
  offset: number;
}
export interface McpCallPack {
  pack_id: string;
  receipt_id: string;
  pack_count: number;
  unit_price_fen: number;
  units_per_pack: number;
  price_fen: number;
  total_units: number;
  adjusted_units: number;
  used_units: number;
  remaining_units: number;
  available_units: number;
  opened_at: string;
  expires_at: string;
  revision: number;
}
export interface McpUsage {
  month_hk: string;
  free_granted: number;
  free_used: number;
  free_remaining: number;
  free_resets_at: string;
  paid_remaining: number;
  paid_next_expires_at: string | null;
  call_packs: McpCallPack[];
}
export interface BookRef {
  book_id: string;
  publication_id: string;
}
export interface Balance {
  granted_micro_cny: number;
  debited_micro_cny: number;
  active_reserved_micro_cny: number;
  pending_micro_cny: number;
  available_micro_cny: number;
}
export interface Period {
  period_id: string;
  starts_at: number;
  expires_at: number;
  revision: number;
  balance: Balance;
}
export interface User {
  activity: Activity;
  user_id: string;
  email: string | null;
  disabled: boolean;
  is_admin: boolean;
  current_allowance: Period | null;
  book_grants: Page<BookRef>;
}
export interface Activity {
  first_used_at: number | null;
  last_read_at: number | null;
  last_question_at: number | null;
  active_days: number;
  return_days: number;
  completed_runs: number;
}
export interface UsageMetrics {
  request_count: number;
  confirmed_cost_micro_cny: number;
  account_debit_micro_cny: number;
  active_reserved_micro_cny: number;
  active_requests: number;
  pending_micro_cny: number;
  pending_requests: number;
  unknown_cost_requests: number;
}
export interface UsageSummary extends UsageMetrics {
  receipt_original_fen: number;
  receipt_correction_fen: number;
  receipt_net_fen: number;
  active_users: number;
  returning_users: number;
  completed_runs: number;
}
export interface UsageTask extends UsageMetrics {
  user_id: string;
  reference_kind: 'run' | 'task';
  reference: string;
}
export interface UsageReport {
  from: string;
  to: string;
  today_date: string;
  as_of: number;
  today: UsageSummary;
  summary: UsageSummary;
  tasks: Page<UsageTask>;
}
export interface Charge {
  call_id: string;
  user_id: string;
  revision: number;
  state: string;
  model: string;
  purpose: string;
  reserved_micro_cny: number;
  account_debit_micro_cny: number | null;
  provider_cost_micro_cny: number | null;
  provider_cost_status: string;
  created_at: number;
  settled_at: number | null;
  needs_reconciliation: boolean;
  rate_snapshot: unknown;
  reservation_estimate: unknown;
  provider_usage: unknown;
  reports: Page<Record<string, unknown>>;
  reconciliations: Page<Record<string, unknown>>;
}
