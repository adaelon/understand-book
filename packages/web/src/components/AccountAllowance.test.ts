// @vitest-environment happy-dom
import { flushPromises, mount } from '@vue/test-utils';
import { afterEach, expect, it, vi } from 'vitest';
import AccountAllowance from './AccountAllowance.vue';
import { installIdentity, network } from '../network-context';

const wrappers: ReturnType<typeof mount>[] = [];
afterEach(() => { wrappers.splice(0).forEach(w => w.unmount()); vi.unstubAllGlobals(); network.value = { ...network.value, identity: null, enabled: false }; });
const period = { period_id: 'p', starts_at: 0, expires_at: 2_000_000_000,
  balance: { granted_micro_cny: 10_000_000, debited_micro_cny: 1_000_001, active_reserved_micro_cny: 2_000_000, pending_micro_cny: 3_000_000, available_micro_cny: 3_999_999 } };
function start() { installIdentity({ user_id: 'A', csrf_token: 'a' }); const w = mount(AccountAllowance); wrappers.push(w); return w; }
const button = (w: ReturnType<typeof mount>, text: string) => w.findAll('button').find(b => b.text() === text)!;

it('reads personal facts without a workspace, pages all periods and refreshes after a grant without sending a run', async () => {
  let amount = period, offsets: number[] = [];
  const fetch = vi.fn(async (url: string, init: RequestInit) => {
    expect(init.method).toBe('GET'); expect(init.credentials).toBe('same-origin'); expect(init.cache).toBe('no-store');
    if (url === '/api/account/allowance') return Response.json({ current_allowance: amount });
    expect(url).toMatch(/^\/api\/account\/usage\?limit=20&offset=/);
    const offset = Number(new URL(url, 'https://reader.test').searchParams.get('offset')); offsets.push(offset);
    return Response.json({ total: 21, limit: 20, offset, items: [{ call_id: `call-${offset}`, period_id: offset ? 'old-period' : 'p', model: 'test-model', purpose: 'outer', state: 'pending', account_debit_micro_cny: null, reserved_micro_cny: 3_000_000, created_at: 0 }] });
  });
  vi.stubGlobal('fetch', fetch);
  const w = start(); await flushPromises();
  expect(w.text()).toContain('3.999999'); expect(w.text()).toContain('待核算占用'); expect(w.text()).toContain('待核算');
  await button(w, '下一页').trigger('click'); await flushPromises();
  expect(w.text()).toContain('old-period'); expect(button(w, '下一页').attributes('disabled')).toBeDefined();
  await button(w, '上一页').trigger('click'); await flushPromises();
  amount = { ...period, balance: { ...period.balance, available_micro_cny: 8_000_000 } };
  await button(w, '刷新额度与明细').trigger('click'); await flushPromises();
  expect(w.text()).toContain('8.00'); expect(offsets).toEqual([0, 20, 0, 0]);
  await button(w, '返回阅读与对话').trigger('click'); expect(w.emitted('close')).toHaveLength(1);
});

it.each([null, { ...period, balance: { ...period.balance, available_micro_cny: 0 } }, { ...period, balance: { ...period.balance, available_micro_cny: -1 } }])('explains missing, zero and negative allowance without blocking reading', async current => {
  vi.stubGlobal('fetch', vi.fn(async (url: string) => Response.json(url.endsWith('/allowance') ? { current_allowance: current } : { items: [], total: 0, limit: 20, offset: 0 })));
  const w = start(); await flushPromises();
  expect(w.text()).toContain(current ? '本期可用额度已用尽' : '当前没有有效额度期');
  expect(w.text()).toContain('仍可在现有授权范围内阅读原文');
  if (current?.balance.available_micro_cny === -1) expect(w.text()).toContain('-0.000001');
});

it('discards old identity responses and makes a query failure retryable', async () => {
  const resolve: ((value: Response) => void)[] = [];
  vi.stubGlobal('fetch', vi.fn(() => new Promise<Response>(yes => { resolve.push(yes); })));
  const w = start(); await flushPromises();
  installIdentity({ user_id: 'B', csrf_token: 'b' });
  resolve[0](Response.json({ current_allowance: period }));
  resolve[1](Response.json({ items: [{ call_id: 'private-A' }], total: 1 })); await flushPromises();
  expect(w.text()).not.toContain('private-A');
  w.unmount();
  vi.stubGlobal('fetch', vi.fn(async () => Response.json({ message: '服务暂不可用' }, { status: 503 })));
  const retry = start(); await flushPromises();
  expect(retry.get('[role="alert"]').text()).toContain('服务暂不可用');
  expect(button(retry, '刷新额度与明细').attributes('disabled')).toBeUndefined();
});
