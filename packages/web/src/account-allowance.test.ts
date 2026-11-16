import { expect, it } from 'vitest';
import { allowancePercent, allowanceTime, offerPercentage, paymentAmount, remainingPercentages, spendNotice, usageOccupation, type AccountUsage, type AllowancePeriod } from './account-allowance';

it('recomputes remaining share after top-ups, subtracts occupations, and handles empty or overdrawn budgets', () => {
  const balance: AllowancePeriod['balance'] = { granted_micro_cny: 10000000, debited_micro_cny: 5000000, active_reserved_micro_cny: 1000000, pending_micro_cny: 0, available_micro_cny: 4000000, allowance_basis: null, percentages: null };
  expect(remainingPercentages(balance).available.value).toBe('40');
  expect(remainingPercentages({ ...balance, granted_micro_cny: 20000000, available_micro_cny: 14000000 }).available.value).toBe('70');
  expect(remainingPercentages({ ...balance, available_micro_cny: -1 }).available.value).toBe('0');
  expect(remainingPercentages({ ...balance, available_micro_cny: 1 }).available.below_tenth).toBe(true);
  expect(remainingPercentages({ ...balance, granted_micro_cny: 0, available_micro_cny: 0 }).available.value).toBe('0');
});

it('preserves server percentages, nonzero tiny usage and unknown basis without a local currency conversion', () => {
  expect(allowancePercent({ value: '120.7', nonzero: true, below_tenth: false })).toBe('120.7%');
  expect(allowancePercent({ value: '0', nonzero: true, below_tenth: true })).toBe('低于 0.1%');
  expect(allowancePercent({ value: '0', nonzero: false, below_tenth: false })).toBe('0%');
  expect(allowancePercent(null)).toBe('待配置比例');
  expect(allowanceTime(0)).toBe('1970/01/01 08:00');
  expect(offerPercentage(50250)).toBe('50.25%'); expect(paymentAmount(1250)).toBe('12.50');
  for (const state of ['reserved', 'sent', 'pending', 'settled', 'released'] as const) {
    expect(usageOccupation({ state, allowance_percentages: { reserved: { value: '3', nonzero: true, below_tenth: false } } } as AccountUsage)).toBe(['reserved', 'sent', 'pending'].includes(state) ? '3%' : '0%');
  }
});

it('only recognizes structured spend errors and never claims an unsaved result is saved', () => {
  const error = { category: 'model_spend', error_code: 'ALLOWANCE_INSUFFICIENT', message: 'server text' };
  expect(spendNotice(error, 'saved')?.message).toContain('本次已完成内容已保存');
  expect(spendNotice({ category: 'persistence', error_code: 'TURN_UNSAVED', message: '', execution_error: error }, 'failed')?.message).toContain('尚未保存');
  expect(spendNotice(error, 'unsubmitted')?.message).toContain('问题未提交');
  expect(spendNotice({ ...error, category: 'provider' }, 'saved')).toBeUndefined();
  expect(spendNotice({ ...error, error_code: 'OTHER', message: 'ALLOWANCE_INSUFFICIENT' }, 'saved')).toBeUndefined();
  for (const code of ['ALLOWANCE_EXPIRED', 'MODEL_RATE_UNAVAILABLE', 'MODEL_SPEND_STORAGE_UNAVAILABLE', 'MODEL_SPEND_SCOPE_MISSING', 'RUN_PERMISSION_REVOKED', 'MODEL_CHARGE_RECONCILIATION_REQUIRED']) {
    expect(spendNotice({ ...error, error_code: code }, 'saved')?.code).toBe(code);
  }
});
