// @vitest-environment happy-dom
import { act, type ReactNode } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { QueryClientProvider } from '@tanstack/react-query';
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import { queryClient, session } from '@/lib/api';
import type { Period, AllowanceOffer } from '@/lib/types';
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
  sessionStorage.clear(); queryClient.clear(); offers = [];
  mutate = async (_path, body) => Response.json(receipt(body));
  lookup = async () => Response.json({ error_code: 'NOT_FOUND' }, { status: 404 });
  fetcher = vi.fn(async (url: string, init?: RequestInit) => {
    if (url === '/api/auth/me') return Response.json({ user_id: 'operator', csrf_token: 'csrf', capabilities: { admin: true } });
    if (init?.method === 'POST') return mutate(url, JSON.parse(String(init.body)));
    if (url.startsWith('/api/admin/operations/')) return lookup();
    if (url === '/api/admin/allowance-offers') return Response.json({ items: offers });
    if (url.startsWith(`${base}/allowance-periods`)) return Response.json(page([period]));
    throw new Error(`Unexpected request ${url}`);
  });
  vi.stubGlobal('fetch', fetcher); await session.refresh();
});
afterEach(async () => { await unmount(); queryClient.clear(); vi.unstubAllGlobals(); });

it('shows fixed percentages and sends signed exact percentage gifts without local grants', async () => {
  await mount(); expect(text()).toContain('130%'); expect(text()).toContain('150%'); expect(text()).not.toContain('AI 使用额度（元）');
  await click('赠送 / 调整'); await fill('delta', '0.0001'); await fill('reason', '赠送'); await submit(); expect(writes()).toHaveLength(0);
  await fill('delta', '50.025'); await submit();
  expect(writes()[0][0]).toBe(`${base}/allowance-percentage-adjustments`);
  expect(JSON.parse(writes()[0][1].body)).toMatchObject({ period_id: 'period-a', revision: 2, basis_version: 'standard-v1', delta_milli_percent: 50025, reason: '赠送' });
  expect(text()).toContain('提交后可用额度：130%'); expect(text()).toContain('50.025%');
});

it('requires an explicit basis for new or legacy periods and never guesses commercial values', async () => {
  await mount(<AllowancePanel userId="A" current={{ ...period, balance: { ...period.balance, allowance_basis: null, percentages: null } }} />);
  expect(text()).toContain('待配置比例'); expect([...document.querySelector('section > div')!.querySelectorAll('button')].find(b => b.textContent === '登记收款')).toBeUndefined();
  await click('配置百分比基准'); expect(document.querySelector<HTMLInputElement>('[name="basis_amount"]')?.value).toBe('');
  await fill('basis_version', 'configured-v2'); await fill('basis_amount', '20'); await submit();
  expect(writes()[0][0]).toBe(`${base}/allowance-basis`); expect(JSON.parse(writes()[0][1].body)).toMatchObject({ period_id: 'period-a', revision: 2, basis: { version: 'configured-v2', basis_micro_cny: 20000000 } });
  await click('新建额度期'); await fill('basis_version', 'configured-v3'); await fill('basis_amount', '30'); await submit();
  expect(JSON.parse(writes()[1][1].body).basis).toEqual({ version: 'configured-v3', basis_micro_cny: 30000000 });
});

it('preserves cash, percentage and original operation through unknown receipt retry', async () => {
  mutate = async () => { throw new TypeError('network lost'); };
  await mount(); await click('登记收款'); expect(text()).toContain('授予 AI 使用额度（%）'); await fill('amount', '12.50'); await fill('delta', '50'); await fill('note', '已确认到账'); await submit();
  const original = JSON.parse(writes()[0][1].body);
  expect(original).toMatchObject({ amount_fen: 1250, delta_milli_percent: 50000, basis_version: 'standard-v1' });
  expect(text()).toContain('实际收款（元）12.50'); expect(text()).toContain('增加 / 调整额度50%');
  await unmount(); await mount(); expect(text()).toContain(original.operation_id);
  mutate = async (_path, body) => Response.json(receipt(body)); await click('按原操作重试');
  expect(writes()).toHaveLength(2); expect(JSON.parse(writes()[1][1].body)).toEqual(original); expect(sessionStorage.length).toBe(0);
  const calls = fetcher.mock.calls; const retryIndex = calls.length - 1 - [...calls].reverse().findIndex(([, init]) => init?.method === 'POST');
  expect(fetcher.mock.calls[retryIndex - 1][0]).toBe(`/api/admin/operations/${original.operation_id}`);
});

it('creates and edits a priced percentage offer without granting to the displayed account', async () => {
  await mount(); await click('新增补充方案');
  await fill('title', '学习补充'); await fill('amount', '12.50'); await fill('delta', '50'); await fill('basis_version', 'standard-v1'); await fill('instructions', '人工确认后到账');
  mutate = async (_path, body) => { offers = [{ ...body } as unknown as AllowanceOffer]; return Response.json(receipt(body, 'allowance-offers')); };
  await submit(); const original = JSON.parse(writes()[0][1].body);
  expect(writes()[0][0]).toBe('/api/admin/allowance-offers'); expect(original).toMatchObject({ amount_fen: 1250, delta_milli_percent: 50000, enabled: true, basis_version: 'standard-v1' });
  expect(original.period_id).toBeUndefined(); expect(text()).toContain('12.50 元 · 增加 50%');
  await click('编辑方案'); await fill('amount', '15'); await submit();
  expect(JSON.parse(writes()[1][1].body)).toMatchObject({ offer_id: original.offer_id, amount_fen: 1500, delta_milli_percent: 50000 });
});
