// @vitest-environment happy-dom
import { act, type ReactNode } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { QueryClientProvider } from '@tanstack/react-query';
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import { queryClient, session } from '@/lib/api';
import type { Period, AllowanceOffer } from '@/lib/types';
import type { CloudPlanProduct } from '@/lib/cloud-plans';
import { OperationsProvider } from '@/components/operation';
import { AllowancePanel } from './allowance';

const base = '/api/admin/users/A';
const pct = (value: string) => ({ value, nonzero: value !== '0', below_tenth: false });
const period: Period = { period_id: 'period-a', starts_at: 1791648000, expires_at: 1891648000, revision: 2,
  balance: { granted_micro_cny: 15000000, debited_micro_cny: 1000000, active_reserved_micro_cny: 1000000, pending_micro_cny: 0, available_micro_cny: 13000000,
    allowance_basis: { version: 'standard-v1', basis_micro_cny: 10000000 }, percentages: { granted: pct('150'), debited: pct('10'), active_reserved: pct('10'), pending: pct('0'), available: pct('130'), deficit: pct('0') } } };
let root: Root | undefined; let host: HTMLDivElement;
let fetcher: ReturnType<typeof vi.fn>;
let mutate: (path: string, body: Record<string, unknown>) => Promise<Response>;
let lookup: () => Promise<Response>;
let offers: AllowanceOffer[];
let cloudPlans: CloudPlanProduct[];
const cloudProduct: CloudPlanProduct = { product_id: 'cloud-fixture', revision: 2, title: '测试用三天套餐', tier: 'basic', duration: 'day3', enabled: true, test_only: false, amount_fen: 1250, basis: { version: 'fixture', basis_micro_cny: 1000000 }, grant_milli_percent: 100000, storage_bytes: 1024 ** 3, max_file_bytes: 1024 ** 2, retention_seconds: 86400, instructions: '联系运营', saleable: true, unavailable_reasons: [] };
const page = (items: unknown[]) => ({ items, total: items.length, limit: 20, offset: 0 });
const receipt = (body: Record<string, unknown>, kind = 'percentage-receipts') => ({ operation_id: body.operation_id, user_id: 'A', actor: 'operator', kind, created_at: 1791684000, ok: true, parameters: body, allowance_period: period });
const writes = () => fetcher.mock.calls.filter(([, init]) => init?.method === 'POST');
const text = () => document.body.textContent ?? '';
async function settle() { await act(async () => { await new Promise(r => setTimeout(r, 25)); }); }
async function mount(children: ReactNode = <AllowancePanel userId="A" current={period} />) {
  host = document.createElement('div'); document.body.append(host); root = createRoot(host);
  await act(async () => root!.render(<QueryClientProvider client={queryClient}><OperationsProvider>{children}</OperationsProvider></QueryClientProvider>)); await settle();
}
async function unmount() { if (root) { await act(async () => root!.unmount()); root = undefined; host.remove(); } }
async function click(label: string, index = 0) {
  const button = [...document.querySelectorAll('button')].filter(b => b.textContent === label)[index]; expect(button, label).toBeDefined();
  await act(async () => button.click()); await settle();
}
async function fill(name: string, value: string) {
  const input = document.querySelector<HTMLInputElement>(`[name="${name}"]`)!; expect(input, name).not.toBeNull();
  await act(async () => { Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value')!.set!.call(input, value); input.dispatchEvent(new Event('input', { bubbles: true })); input.dispatchEvent(new Event('change', { bubbles: true })); });
}
async function submit() { await act(async () => document.querySelector('[role=dialog] form')!.dispatchEvent(new Event('submit', { bubbles: true, cancelable: true }))); await settle(); }
beforeEach(async () => {
  (globalThis as typeof globalThis & { IS_REACT_ACT_ENVIRONMENT: boolean }).IS_REACT_ACT_ENVIRONMENT = true;
  sessionStorage.clear(); queryClient.clear(); offers = []; cloudPlans = [];
  mutate = async (_path, body) => Response.json(receipt(body));
  lookup = async () => Response.json({ error_code: 'NOT_FOUND' }, { status: 404 });
  fetcher = vi.fn(async (url: string, init?: RequestInit) => {
    if (url === '/api/auth/me') return Response.json({ user_id: 'operator', csrf_token: 'csrf', capabilities: { admin: true } });
    if (init?.method === 'POST') return mutate(url, JSON.parse(String(init.body)));
    if (url.startsWith('/api/admin/operations/')) return lookup();
    if (url === '/api/admin/allowance-offers') return Response.json({ items: offers });
    if (url === '/api/admin/cloud-plans') return Response.json({ items: cloudPlans });
    if (url === `${base}/cloud-plan-purchases`) return Response.json({ current: null, next: null, catalog: { items: [] }, purchase_starts_at: Math.floor(Date.now() / 1000), purchase_blocked_by_unlimited: false });
    if (url.startsWith(`${base}/allowance-periods`)) return Response.json(page([period]));
    throw new Error(`Unexpected request ${url}`);
  });
  vi.stubGlobal('fetch', fetcher); await session.refresh();
});
afterEach(async () => { await unmount(); queryClient.clear(); vi.unstubAllGlobals(); });

it('manages exact cost budgets without asking for percentages or internal versions', async () => {
  await mount(); expect(text()).toContain('当前可用预算13.00 元'); expect(text()).toContain('本期预算总额15.00 元');
  await click('赠送 / 扣回预算'); await fill('budget', '0.0000001'); await fill('reason', '赠送'); await submit(); expect(writes()).toHaveLength(0);
  await fill('budget', '3.333333'); await submit();
  expect(writes()[0][0]).toBe(`${base}/allowance-adjustments`);
  expect(JSON.parse(writes()[0][1].body)).toMatchObject({ period_id: 'period-a', revision: 2, delta_micro_cny: 3333333, reason: '赠送' });
  expect(text()).toContain('提交后可用额度：13.00 元 AI 预算');
});

it('shows unlimited beta access without low balance or an absurd expiry date', async () => {
  await mount(<AllowancePanel userId="A" current={{ ...period, expires_at: 253402300799, balance: { ...period.balance, unlimited: true, granted_micro_cny: 0, available_micro_cny: -12000000, debited_micro_cny: 12000000 } }} />);
  expect(text()).toContain('当前可用不限额度'); expect(text()).toContain('累计已用12.00 元');
  expect(text()).not.toContain('当前预算不足'); expect(text()).not.toContain('9999'); expect(writes()).toHaveLength(0);
});

it('opens a free grant with its dates and initial budget in one operation', async () => {
  await mount(<AllowancePanel userId="A" current={null} />); await click('单独赠送 AI 额度');
  expect(document.querySelector('[name="basis_version"]')).toBeNull();
  await fill('budget', '10'); await fill('reason', '体验'); await submit();
  expect(writes()).toHaveLength(1); expect(writes()[0][0]).toBe(`${base}/allowance-periods`);
  expect(JSON.parse(writes()[0][1].body)).toMatchObject({ initial_grant_micro_cny: 10000000, reason: '体验' });
  expect(JSON.parse(writes()[0][1].body).basis).toBeUndefined();
});

it('preserves form values and shows a rejected operation in the dialog', async () => {
  mutate = async () => Response.json({ error_code: 'ALLOWANCE_PERIOD_OVERLAP' }, { status: 409 });
  await mount(<AllowancePanel userId="A" current={null} />); await click('单独赠送 AI 额度');
  await fill('budget', '10'); await fill('reason', '体验'); await submit();
  expect(document.querySelector('[role="dialog"]')?.textContent).toContain('有效期与已有额度期重叠');
  expect(document.querySelector<HTMLInputElement>('[name="budget"]')?.value).toBe('10');
  expect(sessionStorage.length).toBe(0);
  mutate = async (_path, body) => Response.json(receipt(body)); await submit();
  expect(document.querySelector('[role="dialog"]')).toBeNull(); expect(writes()).toHaveLength(2);
});

it('preserves cash and budget through an uncertain payment retry', async () => {
  mutate = async () => { throw new TypeError('network lost'); };
  await mount(); await click('收款并补充本期'); await fill('amount', '12.50'); await fill('budget', '5'); await submit();
  const original = JSON.parse(writes()[0][1].body);
  expect(original).toMatchObject({ amount_fen: 1250, delta_micro_cny: 5000000 });
  await click('关闭并核对原操作'); expect(text()).toContain('实际收款（元）12.50');
  await unmount(); await mount(); expect(text()).toContain(original.operation_id);
  mutate = async (_path, body) => Response.json(receipt(body)); await click('按原操作重试');
  expect(writes()).toHaveLength(2); expect(JSON.parse(writes()[1][1].body)).toEqual(original); expect(sessionStorage.length).toBe(0);
});

it('creates an add-on with independent sale price and AI budget', async () => {
  await mount(); await click('新增加购商品');
  await fill('title', '学习补充'); await fill('amount', '12.50'); await fill('budget', '5'); await fill('instructions', '人工确认后到账');
  await submit(); expect(writes()[0][0]).toBe('/api/admin/allowance-offers');
  expect(JSON.parse(writes()[0][1].body)).toMatchObject({ amount_fen: 1250, budget_micro_cny: 5000000, enabled: true });
  expect(JSON.parse(writes()[0][1].body).period_id).toBeUndefined();
});

it('saves an unpriced cloud-plan draft without making up commercial values', async () => {
  await mount(); await click('新增套餐商品');
  for (const name of ['amount', 'budget', 'storage_mb', 'max_file_mb', 'retention_days']) expect(document.querySelector<HTMLInputElement>(`[name="${name}"]`)!.value).toBe('');
  expect(document.querySelector('[name="basis_version"]')).toBeNull();
  await fill('title', '待配置冲刺'); await submit();
  expect(writes()[0][0]).toBe('/api/admin/cloud-plans');
  expect(JSON.parse(writes()[0][1].body)).toMatchObject({ title: '待配置冲刺', revision: 0, enabled: false, amount_fen: null, budget_micro_cny: null, storage_bytes: null, max_file_bytes: null, retention_seconds: null });
});

it('opens a paid plan with a pinned product version and no manually entered dates or basis', async () => {
  cloudPlans = [cloudProduct];
  await mount(<AllowancePanel userId="A" current={null} />); await click('开通 / 续费套餐');
  const dialog = document.querySelector('[role="dialog"]')!;
  expect(dialog.textContent).toContain('确认实收 12.50 元'); expect(dialog.textContent).toContain('发放 1.00 元 AI 预算');
  expect(dialog.querySelector('[name="basis_version"]')).toBeNull(); expect(dialog.querySelector('[name="starts_at"]')).toBeNull();
  await submit(); const original = JSON.parse(writes()[0][1].body);
  expect(writes()[0][0]).toBe(`${base}/cloud-plan-purchases`);
  expect(original).toMatchObject({ product_id: 'cloud-fixture', product_revision: 2, channel: '微信' });
});

it('does not offer direct date editing for sold plan periods', async () => {
  await mount(<AllowancePanel userId="A" current={{ ...period, is_plan: true }} />);
  const section = document.querySelector('[aria-label="本期 AI 使用预算"]')!;
  expect(section.textContent).not.toContain('编辑有效期'); expect(section.textContent).toContain('需要延长使用时间');
});
