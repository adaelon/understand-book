import type { UsageTask } from './types';

export function hongKongDay(now = Date.now()): string {
  return new Date(now + 8 * 3600 * 1000).toISOString().slice(0, 10);
}
export function usageQuery(from: string, to: string, offset = 0, user = ''): string {
  if (!/^\d{4}-\d{2}-\d{2}$/.test(from) || !/^\d{4}-\d{2}-\d{2}$/.test(to) || from > to)
    throw new Error('请选择有效日期，结束日期不能早于开始日期。');
  const params = new URLSearchParams({ from, to, limit: '20', offset: String(offset) });
  if (user.trim()) params.set('user_id', user.trim());
  return params.toString();
}
export function taskChargesPath(task: UsageTask, from: string, to: string, offset: number): string {
  const params = new URLSearchParams(usageQuery(from, to, offset, task.user_id));
  params.set(task.reference_kind === 'run' ? 'run_ref' : 'task_ref', task.reference);
  return `/admin/charges?${params}`;
}
