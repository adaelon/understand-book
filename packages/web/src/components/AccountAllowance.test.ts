// @vitest-environment happy-dom
import { flushPromises, mount } from '@vue/test-utils';
import { afterEach, expect, it, vi } from 'vitest';
import AccountAllowance from './AccountAllowance.vue';
import { installIdentity, network } from '../network-context';
import type { AllowancePeriod, AccountAllowanceSummary } from '../account-allowance';

const wrappers: ReturnType<typeof mount>[] = [];
afterEach(() => { wrappers.splice(0).forEach(w => w.unmount()); vi.unstubAllGlobals(); network.value = { ...network.value, identity: null, enabled: false }; });
const percent = (value: string, below_tenth = false, nonzero = value !== '0') => ({ value, below_tenth, nonzero });
const basis = { version: 'standard-v1', basis_micro_cny: 10000000 };
const period: AllowancePeriod = { period_id: 'p', starts_at: 0, expires_at: 2_000_000_000, basis_status: 'bound', validity_state: 'active',
  balance: { granted_micro_cny: 15000000, debited_micro_cny: 1000001, active_reserved_micro_cny: 200000, pending_micro_cny: 300000, available_micro_cny: 13499999,
    allowance_basis: basis, percentages: { granted: percent('150'), debited: percent('10.00001'), active_reserved: percent('2'), pending: percent('3'), available: percent('134.99999'), deficit: percent('0') } } };
const summary = (current: AllowancePeriod | null = period, status: AccountAllowanceSummary['status'] = 'active'): AccountAllowanceSummary => ({ current_allowance: current, latest_allowance: current ?? { ...period, validity_state: 'expired' }, status, replenishment: { offers: [{ offer_id: 'half', title: '学习补充', amount_fen: 1250, delta_milli_percent: 50000, basis_version: basis.version, instructions: '联系运营人工收款' }], contact: '运营联系入口' } });
const empty = { items: [], period_groups: [], total: 0, limit: 20, offset: 0 };
function start() { installIdentity({ user_id: 'A', csrf_token: 'a' }); const w = mount(AccountAllowance); wrappers.push(w); return w; }
const button = (w: ReturnType<typeof mount>, text: string) => w.findAll('button').find(b => b.text() === text)!;

it('shows >100% and tiny usage, fixed original-period pages and payment offers without granting on click', async () => {
  let amount = period; const offsets: number[] = [];
  const fetch = vi.fn(async (url: string, init: RequestInit) => {
    expect(init.method).toBe('GET'); expect(init.credentials).toBe('same-origin'); expect(init.cache).toBe('no-store');
    if (url === '/api/account/allowance') return Response.json(summary(amount));
    const offset = Number(new URL(url, 'https://reader.test').searchParams.get('offset')); offsets.push(offset);
    return Response.json({ total: 21, limit: 20, offset, period_groups: [{ period_id: 'original-period', allowance_basis: { ...basis, version: 'old-basis' }, percentages: { debited: percent('0.14997'), active_reserved: percent('0'), pending: percent('3') }, unknown_provider_cost_count: 1 }], items: [{ call_id: `call-${offset}`, period_id: offset ? 'old-period' : 'p', model: 'test-model', purpose: 'memory_compaction', state: 'settled', account_debit_micro_cny: 1, reserved_micro_cny: 3000000, allowance_basis: { ...basis, version: offset ? 'old-basis' : 'standard-v1' }, allowance_percentages: { debited: percent('0.00001', true), reserved: percent('30') }, created_at: 0 }] });
  });
  vi.stubGlobal('fetch', fetch);
  const w = start(); await flushPromises();
  expect(w.text()).toContain('134.99999%'); expect(w.text()).toContain('待核算占用'); expect(w.text()).toContain('低于 0.1%'); expect(w.text()).toContain('记忆整理');
  expect(w.text()).toContain('0.14997%'); expect(w.text()).toContain('old-basis'); expect(w.text()).not.toContain('AI 使用额度（元）');
  await button(w, '补充额度').trigger('click');
  expect(w.text()).toContain('增加 50%'); expect(w.text()).toContain('支付 ¥12.50'); expect(w.text()).toContain('运营联系入口'); expect(fetch).toHaveBeenCalledTimes(2);
  await button(w, '下一页').trigger('click'); await flushPromises();
  expect(w.text()).toContain('old-period'); expect(button(w, '下一页').attributes('disabled')).toBeDefined();
  await button(w, '上一页').trigger('click'); await flushPromises();
  amount = { ...period, balance: { ...period.balance, percentages: { ...period.balance.percentages!, available: percent('184.99999') } } };
  await button(w, '刷新额度与明细').trigger('click'); await flushPromises();
  expect(w.text()).toContain('184.99999%'); expect(w.text()).toContain('0.14997%'); expect(offsets).toEqual([0, 20, 0, 0]);
  await button(w, '返回阅读与对话').trigger('click'); expect(w.emitted('close')).toHaveLength(1);
});

it.each(['no_period', 'expired', 'scheduled', 'closed'] as const)('explains %s without blocking reading or inventing an offer', async status => {
  vi.stubGlobal('fetch', vi.fn(async (url: string) => Response.json(url.endsWith('/allowance') ? { ...summary(null, status), replenishment: { offers: [] } } : empty)));
  const w = start(); await flushPromises();
  expect(w.text()).toContain({ no_period: '当前没有有效额度期', expired: '上一额度期已到期', scheduled: '下一额度期尚未生效', closed: '上一额度期已关闭' }[status]);
  expect(w.text()).toContain('仍可在现有授权范围内阅读原文');
  await button(w, '补充额度').trigger('click'); expect(w.text()).toContain('当前暂无可直接选择的补充方案'); expect(w.text()).not.toContain('支付 ¥');
});

it('shows a zero main balance with a nonzero deficit and treats an unbound basis separately', async () => {
  let current: AllowancePeriod = { ...period, balance: { ...period.balance, available_micro_cny: -1, percentages: { ...period.balance.percentages!, available: percent('0'), deficit: percent('0', true, true) } } };
  vi.stubGlobal('fetch', vi.fn(async (url: string) => Response.json(url.endsWith('/allowance') ? summary(current) : empty)));
  const w = start(); await flushPromises();
  expect(w.get('.available dd').text()).toBe('0%'); expect(w.text()).toContain('已发生费用超出剩余额度 低于 0.1%');
  current = { ...period, balance: { ...period.balance, allowance_basis: null, percentages: null } };
  await button(w, '刷新额度与明细').trigger('click'); await flushPromises();
  expect(w.get('.available dd').text()).toBe('待配置比例'); expect(w.text()).toContain('尚未配置百分比基准');
});

it('clears loaded and in-flight personal data on account switch and allows retry', async () => {
  const pending: ((value: Response) => void)[] = [];
  vi.stubGlobal('fetch', vi.fn(() => new Promise<Response>(yes => { pending.push(yes); })));
  const w = start(); await flushPromises();
  installIdentity({ user_id: 'B', csrf_token: 'b' });
  pending[0](Response.json(summary())); pending[1](Response.json({ ...empty, items: [{ call_id: 'private-A' }] })); await flushPromises();
  expect(w.text()).not.toContain('134.99999%'); expect(w.text()).not.toContain('private-A');
  pending[2](Response.json({ ...summary(null, 'no_period'), replenishment: { offers: [] } })); pending[3](Response.json(empty)); await flushPromises();
  expect(w.text()).toContain('当前没有有效额度期');
  installIdentity({ user_id: 'C', csrf_token: 'c' }); await flushPromises(); expect(w.text()).not.toContain('当前没有有效额度期');
  pending[4](Response.json({ message: '服务暂不可用' }, { status: 503 })); pending[5](Response.json(empty)); await flushPromises();
  expect(w.get('[role="alert"]').text()).toContain('服务暂不可用'); expect(button(w, '刷新额度与明细').attributes('disabled')).toBeUndefined();
});
