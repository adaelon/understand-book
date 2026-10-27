// @vitest-environment happy-dom
import { mount, flushPromises } from '@vue/test-utils';
import { afterEach, expect, it, vi } from 'vitest';
import GuestAccess from './GuestAccess.vue';
import { network } from '../network-context';

let wrapper: ReturnType<typeof mount> | undefined;
const guest = { user_id: 'guest-one', is_guest: true, email: null, csrf_token: 'csrf' };
afterEach(() => {
  wrapper?.unmount(); wrapper = undefined; vi.unstubAllGlobals();
  localStorage.clear(); sessionStorage.clear();
  network.value = { ...network.value, identity: null, workspace: null, enabled: false };
});
it('redeems only a code, blocks duplicate clicks and enters the returned guest identity', async () => {
  let finish!: (response: Response) => void;
  const fetcher = vi.fn(async (url: string, init: RequestInit) => {
    if (url.endsWith('/me')) return Response.json({}, { status: 401 });
    expect(url).toBe('/api/auth/guest');
    expect(JSON.parse(String(init.body))).toEqual({ invite_code: 'ABCDE-ABCDE-ABCDE-ABCDE' });
    return new Promise<Response>(resolve => { finish = resolve; });
  });
  vi.stubGlobal('fetch', fetcher); wrapper = mount(GuestAccess); await flushPromises();
  expect(wrapper.findAll('input')).toHaveLength(1);
  await wrapper.get('input').setValue(' ABCDE-ABCDE-ABCDE-ABCDE ');
  await wrapper.get('form').trigger('submit'); await flushPromises();
  await wrapper.get('form').trigger('submit'); await flushPromises();
  expect(fetcher.mock.calls.filter(([url]) => url.endsWith('/guest'))).toHaveLength(1);
  expect(wrapper.get('.guest-submit').attributes('disabled')).toBeDefined();
  finish(Response.json(guest)); await flushPromises();
  expect(wrapper.emitted('entered')).toEqual([[guest]]);
  expect(JSON.stringify(localStorage) + JSON.stringify(sessionStorage)).not.toContain('ABCDE');
});
it('restores a persisted session without redeeming a code', async () => {
  const fetcher = vi.fn(async () => Response.json(guest)); vi.stubGlobal('fetch', fetcher);
  wrapper = mount(GuestAccess); await flushPromises();
  expect(wrapper.emitted('entered')).toEqual([[guest]]);
  expect(fetcher).toHaveBeenCalledTimes(1);
});
it('explains unavailable codes and retries a lost response through the installed cookie', async () => {
  let signedIn = false, failCode = true, redemptions = 0;
  vi.stubGlobal('fetch', vi.fn(async (url: string) => {
    if (url.endsWith('/me')) return signedIn ? Response.json(guest) : Response.json({}, { status: 401 });
    redemptions++;
    if (failCode) return Response.json({ error_code: 'INVITE_UNAVAILABLE' }, { status: 409 });
    signedIn = true; throw new TypeError('response body lost');
  }));
  wrapper = mount(GuestAccess); await flushPromises();
  await wrapper.get('input').setValue('ABCDE-ABCDE-ABCDE-ABCDE');
  await wrapper.get('form').trigger('submit'); await flushPromises();
  expect(wrapper.get('[role="alert"]').text()).toContain('内测码已使用');
  failCode = false; await wrapper.get('form').trigger('submit'); await flushPromises();
  expect(wrapper.get('[role="alert"]').text()).toContain('连接暂时中断');
  await wrapper.get('form').trigger('submit'); await flushPromises();
  expect(wrapper.emitted('entered')).toEqual([[guest]]);
  expect(redemptions).toBe(2);
});
