import { expect, it } from 'vitest';
import { cloudPlanForm, positiveUnit, type CloudPlanProduct } from './cloud-plans';
const form = (values: Record<string, string>) => { const data = new FormData(); Object.entries(values).forEach(([key, value]) => data.set(key, value)); return data; };

it('keeps undefined commercial terms as null and a new product disabled', () => {
  expect(cloudPlanForm(form({ title: '冲刺草稿', tier: 'basic', duration: 'day1' }), null)).toMatchObject({ title: '冲刺草稿', revision: 0, enabled: false, test_only: false, amount_fen: null, basis: null, grant_milli_percent: null, storage_bytes: null, max_file_bytes: null, retention_seconds: null });
});
it('preserves the edited revision and converts exact declared price, quota and storage units', () => {
  const existing = { product_id: 'original', revision: 3 } as CloudPlanProduct;
  expect(cloudPlanForm(form({ title: '测试配置', tier: 'plus', duration: 'month', amount: '12.50', basis_version: 'v1', basis_amount: '10', grant: '150.025', storage_mb: '1024', max_file_mb: '1.5', retention_days: '30', enabled: 'on' }), existing)).toMatchObject({ product_id: 'original', revision: 3, amount_fen: 1250, basis: { version: 'v1', basis_micro_cny: 10000000 }, grant_milli_percent: 150025, storage_bytes: 1073741824, max_file_bytes: 1572864, retention_seconds: 2592000, enabled: true });
});
it('rejects half-configured basis and unit values that cannot be represented exactly', () => {
  expect(() => cloudPlanForm(form({ tier: 'basic', duration: 'day1', basis_version: 'v1' }), null)).toThrow('同时填写');
  expect(() => positiveUnit('0.0000001', 1024 ** 2)).toThrow('精确换算');
  expect(() => positiveUnit('-1', 86400)).toThrow('非负数');
});
