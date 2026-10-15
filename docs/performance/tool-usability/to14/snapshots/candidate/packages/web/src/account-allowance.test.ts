import { expect, it } from 'vitest';
import { allowanceAmount, allowanceTime, spendNotice, usageOccupation, type AccountUsage } from './account-allowance';

it('keeps micro amounts, negative balances and unknown charges distinct', () => {
  expect([0, 1, 1_000_001, -200_001].map(allowanceAmount)).toEqual(['0.00', '0.000001', '1.000001', '-0.200001']);
  expect(allowanceAmount(null)).toBe('待核算');
  expect(allowanceTime(0)).toBe('1970/01/01 08:00');
  for (const state of ['reserved', 'sent', 'pending', 'settled', 'released'] as const) {
    expect(usageOccupation({ state, reserved_micro_cny: 123 } as AccountUsage)).toBe(['reserved', 'sent', 'pending'].includes(state) ? 123 : 0);
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
