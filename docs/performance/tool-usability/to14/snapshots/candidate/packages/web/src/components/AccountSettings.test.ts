// @vitest-environment happy-dom
import { mount, flushPromises } from '@vue/test-utils';
import { afterEach, expect, it, vi } from 'vitest';
import AccountSettings from './AccountSettings.vue';
import { installIdentity, installWorkspace, network, type NetworkWorkspace } from '../network-context';
const wrappers: ReturnType<typeof mount>[] = [];
afterEach(() => { wrappers.splice(0).forEach(w => w.unmount()); vi.useRealTimers(); vi.unstubAllGlobals(); sessionStorage.clear(); network.value = { ...network.value, identity: null, workspace: null, enabled: false }; });
function start(email: string | null = null) {
  installIdentity({ user_id: 'legacy', csrf_token: 'csrf', email });
  const wrapper = mount(AccountSettings); wrappers.push(wrapper); return wrapper;
}
const button = (w: ReturnType<typeof mount>, text: string) => w.findAll('button').find(b => b.text() === text)!;
async function send(w: ReturnType<typeof mount>) {
  await w.get('input[type="email"]').setValue('Reader@Example.com');
  await w.get('input[type="password"]').setValue('fixture-only-password');
  await w.get('form').trigger('submit'); await flushPromises();
}
const pending = () => ({ request_id: 'request', expires_at: Math.floor(Date.now()/1000) + 900, resend_after: 60 });

it('keeps a rejected or timed-out send recoverable, clears password, and binds without changing workspace ownership', async () => {
  vi.useFakeTimers();
  const calls: string[] = [];
  vi.stubGlobal('fetch', vi.fn(async (url: string, init: RequestInit) => {
    calls.push(url); expect(init.headers).toMatchObject({ 'X-CSRF-Token': 'csrf' });
    if (url.endsWith('/start')) {
      expect(JSON.parse(String(init.body))).toEqual({ email: 'Reader@Example.com', current_password: 'fixture-only-password' });
      return Response.json({ ...pending(), error_code: 'ACCOUNT_MAIL_TIMEOUT' }, { status: 503 });
    }
    if (url.endsWith('/resend')) return Response.json(pending());
    expect(JSON.parse(String(init.body))).toEqual({ request_id: 'request', verification_code: '123456' });
    return Response.json({ user_id: 'legacy', email: 'reader@example.com', completed: true });
  }));
  const w = start();
  const workspace = { workspace_id: 'original', generation: 1, revision: 0, published_book_ref: { book_id: 'b', publication_id: 'p' }, selected_chat: 'chat', reader: {} } as NetworkWorkspace;
  installWorkspace(workspace); const before = network.value;
  await send(w);
  expect(w.get('[role="alert"]').text()).toContain('可能已送达');
  expect(w.find('input[type="password"]').exists()).toBe(false);
  expect(button(w, '60 秒后可重发').attributes('disabled')).toBeDefined();
  vi.advanceTimersByTime(61_000); await flushPromises();
  await button(w, '重发验证码').trigger('click'); await flushPromises();
  await w.get('input[autocomplete="one-time-code"]').setValue('123456');
  await w.get('form').trigger('submit'); await flushPromises();
  expect(w.text()).toContain('已验证邮箱：reader@example.com');
  expect(w.emitted('bound')).toHaveLength(1);
  expect(network.value.identity?.user_id).toBe('legacy');
  expect(network.value.workspace).toBe(before.workspace);
  expect(network.value.epoch).toBe(before.epoch);
  expect(network.value.attachment).toBe(before.attachment);
  expect(JSON.stringify(sessionStorage)).not.toContain('fixture-only-password');
  expect(JSON.stringify(localStorage)).not.toContain('fixture-only-password');
  expect(calls).toEqual(['/api/account/email/start','/api/account/email/resend','/api/account/email/complete']);
});

it('wrong password retains the current login and shows a cleared password field', async () => {
  vi.stubGlobal('fetch', vi.fn(async () => Response.json({ error_code: 'CURRENT_PASSWORD_INVALID' }, { status: 400 })));
  const w = start(); await send(w);
  expect(w.get('[role="alert"]').text()).toContain('当前密码不正确');
  expect((w.get('input[type="password"]').element as HTMLInputElement).value).toBe('');
  expect(network.value.identity?.user_id).toBe('legacy');
});

it('expiry offers a new request, and an already bound account offers no replacement form', async () => {
  vi.useFakeTimers();
  vi.stubGlobal('fetch', vi.fn(async () => Response.json(pending())));
  const w = start(); await send(w);
  vi.advanceTimersByTime(901_000); await flushPromises();
  expect(button(w,'确认绑定').attributes('disabled')).toBeDefined();
  expect(w.text()).toContain('验证码已过期');
  await button(w,'重新开始绑定').trigger('click');
  expect((w.get('input[type="password"]').element as HTMLInputElement).value).toBe('');
  const bound = start('known@example.com'); expect(bound.text()).toContain('已验证邮箱：known@example.com');
  expect(bound.find('form').exists()).toBe(false);
});

it('discards pending responses after account switching or closing settings', async () => {
  let finish!: (r: Response) => void;
  vi.stubGlobal('fetch', vi.fn(() => new Promise<Response>(resolve => { finish=resolve; })));
  const w=start(); await send(w);
  installIdentity({ user_id:'other', csrf_token:'other', email:null });
  finish(Response.json({ completed:true, user_id:'legacy', email:'old@example.com' })); await flushPromises();
  expect(network.value.identity?.email).toBeNull(); expect(w.emitted('bound')).toBeUndefined();
  const closed=start(); await send(closed); closed.unmount();
  finish(Response.json({ completed:true, user_id:'legacy', email:'old@example.com' })); await flushPromises();
  expect(network.value.identity?.email).toBeNull();
});
