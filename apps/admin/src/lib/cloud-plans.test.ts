import { expect, it } from 'vitest';
import { cloudPlanForm, positiveUnit, unitInput, type CloudPlanProduct } from './cloud-plans';
const form = (values: Record<string, string>) => { const data = new FormData(); Object.entries(values).forEach(([key, value]) => data.set(key, value)); return data; };

it('keeps undefined commercial terms as null and a new product disabled', () => {
  expect(cloudPlanForm(form({ title: '冲刺草稿', tier: 'basic', duration: 'day1' }), null)).toMatchObject({ title: '冲刺草稿', revision: 0, enabled: false, test_only: false, amount_fen: null, basis: null, grant_milli_percent: null, storage_bytes: null, max_file_bytes: null, retention_seconds: null });
});
it('preserves the edited revision and converts exact declared price, quota and storage units', () => {
  const existing = { product_id: 'original', revision: 3 } as CloudPlanProduct;
  expect(cloudPlanForm(form({ title: '测试配置', tier: 'plus', duration: 'month', amount: '12.50', budget: '15.0025', storage_mb: '1024', max_file_mb: '1.5', retention_days: '30', enabled: 'on' }), existing)).toMatchObject({ product_id: 'original', revision: 3, amount_fen: 1250, budget_micro_cny: 15002500, basis: null, grant_milli_percent: null, storage_bytes: 1073741824, max_file_bytes: 1572864, retention_seconds: 2592000, enabled: true });
});
it('rejects nonpositive budgets and unit values that cannot be represented exactly', () => {
  expect(() => cloudPlanForm(form({ tier: 'basic', duration: 'day1', budget: '0' }), null)).toThrow('大于零');
  expect(() => positiveUnit('0.0000001', 1024 ** 2)).toThrow('精确换算');
  expect(() => positiveUnit('-1', 86400)).toThrow('非负数');
});
it.each([['1.1', 95040], ['0.07', 6048], ['0.0003125', 27]])('converts %s days to an exact integer number of seconds', (days, seconds) => {
  expect(cloudPlanForm(form({ tier: 'basic', duration: 'day1', retention_days: days }), null).retention_seconds).toBe(seconds);
});
it('preserves unchanged integer units while editing another product field', () => {
  const existing = { product_id: 'original', revision: 3, storage_bytes: 11, max_file_bytes: 7, retention_seconds: 11 } as CloudPlanProduct;
  const values = { title: '更新标题', tier: 'basic', duration: 'day1', storage_mb: unitInput(11, 1024 ** 2), max_file_mb: unitInput(7, 1024 ** 2), retention_days: unitInput(11, 86400) };
  expect(cloudPlanForm(form(values), existing)).toMatchObject({ title: '更新标题', storage_bytes: 11, max_file_bytes: 7, retention_seconds: 11 });
  expect(cloudPlanForm(form({ ...values, retention_days: '1.1' }), existing).retention_seconds).toBe(95040);
  expect(cloudPlanForm(form({ ...values, retention_days: '' }), existing).retention_seconds).toBeNull();
});
it('rejects decimal fractions of a second and integers outside the safe range', () => {
  expect(() => positiveUnit('0.00001', 86400)).toThrow('精确换算');
  expect(() => positiveUnit('9007199254740992', 1)).toThrow('精确换算');
  expect(positiveUnit('9007199254740991', 1)).toBe(Number.MAX_SAFE_INTEGER);
});
