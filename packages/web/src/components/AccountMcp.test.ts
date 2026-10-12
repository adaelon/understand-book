// @vitest-environment happy-dom
import { mount, flushPromises } from '@vue/test-utils';
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import AccountMcp from './AccountMcp.vue';
import { installIdentity, network } from '../network-context';
import type { AccountMcpState, McpCredential } from '../api';

let wrapper: ReturnType<typeof mount>;
const credential: McpCredential = { credential_id: 'c1', label: '我的 Agent', created_at: 1791676800, revoked_at: null, active: true };
let state: AccountMcpState;
const button = (text: string) => wrapper.findAll('button').find(b => b.text() === text)!;
beforeEach(() => {
  installIdentity({ user_id: 'alice', csrf_token: 'csrf' });
  state = { credentials: [], mcp_url: 'https://reader.example/mcp', usage: {
    month_hk: '2026-10', free_granted: 1000, free_used: 12, free_remaining: 988, free_resets_at: '2026-11-01T00:00:00+08:00',
    paid_remaining: 700, paid_next_expires_at: '2027-10-11T08:00:00+08:00',
    call_packs: [
      { pack_id: 'old', pack_count: 1, total_units: 1000, adjusted_units: 0, used_units: 700, remaining_units: 300, available_units: 0, opened_at: '2025-09-01T00:00:00+08:00', expires_at: '2026-09-01T00:00:00+08:00' },
      { pack_id: 'new', pack_count: 1, total_units: 1000, adjusted_units: 0, used_units: 300, remaining_units: 700, available_units: 700, opened_at: '2026-10-11T08:00:00+08:00', expires_at: '2027-10-11T08:00:00+08:00' },
    ], top_up_url: 'https://reader.example/?account=mcp', top_up_instruction: 'purchase',
  } };
  vi.stubGlobal('fetch', vi.fn(async (url: string, init: RequestInit) => {
    expect(init.credentials).toBe('same-origin'); expect(init.cache).toBe('no-store');
    expect(init.headers).toMatchObject({ 'X-CSRF-Token': 'csrf' });
    if (url === '/api/account/mcp') return Response.json(state);
    if (url === '/api/account/mcp/credentials') {
      expect(JSON.parse(String(init.body))).toEqual({ label: '我的 Agent' });
      state.credentials = [{ ...credential }];
      return Response.json({ credential, token: 'test-creation-only-secret' });
    }
    expect(url).toBe('/api/account/mcp/credentials/c1/revoke');
    state.credentials[0] = { ...credential, active: false, revoked_at: 1791676900 };
    return Response.json({ revoked: true });
  }));
});
afterEach(() => { wrapper?.unmount(); vi.unstubAllGlobals(); sessionStorage.clear(); localStorage.clear(); network.value = { ...network.value, identity: null, workspace: null, enabled: false }; });
async function start() { wrapper = mount(AccountMcp); await flushPromises(); }

it('works without a book, shows balances and every expiry, and creates a one-time credential then revokes it', async () => {
  await start();
  expect(network.value.workspace).toBeNull();
  expect(wrapper.get('[aria-label="MCP 次数"]').text()).toContain('免费剩余 988 次 · 已购可用 700 次');
  expect(wrapper.findAll('.packs li')).toHaveLength(2);
  expect(wrapper.text()).toContain('2027年10月11日'); expect(wrapper.text()).toContain('未用次数不可使用');
  expect(wrapper.get('[aria-label="购买调用包"]').text()).toContain('12 个日历月');
  await wrapper.get('form').trigger('submit'); await flushPromises();
  expect((wrapper.get('textarea').element as HTMLTextAreaElement).value).toBe('test-creation-only-secret');
  expect(wrapper.get('.credentials').text()).not.toContain('test-creation-only-secret');
  expect(JSON.stringify(sessionStorage) + JSON.stringify(localStorage)).not.toContain('test-creation-only-secret');
  await button('撤销 我的 Agent').trigger('click'); await flushPromises();
  expect(wrapper.find('textarea').exists()).toBe(false); expect(wrapper.get('.credentials').text()).toContain('已撤销');
  wrapper.unmount(); await start(); expect(wrapper.find('textarea').exists()).toBe(false);
});

it('clears credential and balance immediately when identity changes and discards an old response', async () => {
  await start(); await wrapper.get('form').trigger('submit'); await flushPromises();
  let resolve!: (r: Response) => void;
  vi.stubGlobal('fetch', vi.fn(() => new Promise<Response>(r => { resolve = r; })));
  await button('刷新').trigger('click');
  installIdentity({ user_id: 'bob', csrf_token: 'other' });
  await flushPromises();
  expect(wrapper.find('textarea').exists()).toBe(false); expect(wrapper.find('.packs').exists()).toBe(false);
  resolve(Response.json(state)); await flushPromises();
  expect(wrapper.find('.packs').exists()).toBe(false); expect(wrapper.emitted('close')).toHaveLength(1);
});

it('ignores a credential delivered after the window closes and explains uncertain creation', async () => {
  await start();
  vi.stubGlobal('fetch', vi.fn(async () => { throw new Error('connection lost'); }));
  await wrapper.get('form').trigger('submit'); await flushPromises();
  expect(wrapper.get('[role="alert"]').text()).toContain('刷新列表核对');
  let resolve!: (r: Response) => void;
  vi.stubGlobal('fetch', vi.fn(() => new Promise<Response>(r => { resolve = r; })));
  await wrapper.get('form').trigger('submit'); wrapper.unmount();
  resolve(Response.json({ credential, token: 'late-secret' })); await flushPromises();
  expect(JSON.stringify(sessionStorage) + JSON.stringify(localStorage)).not.toContain('late-secret');
});

it('keeps the purchase and recovery explanation available at zero balance', async () => {
  state.usage.free_remaining = 0; state.usage.paid_remaining = 0; state.usage.call_packs = [];
  await start(); expect(wrapper.text()).toContain('次数已用完'); expect(wrapper.text()).toContain('每包 10 元');
  expect(wrapper.text()).toContain('目录和次数查询仍可使用');
});
