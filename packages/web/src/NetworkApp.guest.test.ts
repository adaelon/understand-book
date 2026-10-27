// @vitest-environment happy-dom
import { mount, flushPromises } from '@vue/test-utils';
import { afterEach, expect, it, vi } from 'vitest';
import { network } from './network-context';
vi.mock('./App.vue', () => ({ default: { template: '<div />' } }));
import NetworkApp from './NetworkApp.vue';

let wrapper: ReturnType<typeof mount> | undefined;
afterEach(() => { wrapper?.unmount(); vi.unstubAllGlobals(); localStorage.clear(); sessionStorage.clear(); history.replaceState({}, '', '/'); network.value = { ...network.value, identity: null, workspace: null, enabled: false }; });
it('enters a populated guest library from the invite link and restores it on refresh', async () => {
  history.replaceState({}, '', '/?account=guest');
  let signedIn = false, redemptions = 0;
  const identity = { user_id: 'guest-one', is_guest: true, csrf_token: 'csrf' };
  vi.stubGlobal('fetch', vi.fn(async (url: string) => {
    if (url === '/api/auth/me') return signedIn ? Response.json(identity) : Response.json({}, { status: 401 });
    if (url === '/api/auth/guest') { redemptions++; signedIn = true; return Response.json(identity); }
    if (url === '/api/library') return Response.json({ books: [{ title: '量化的本质', published_book_ref: { book_id: 'example', publication_id: 'current' }, is_default: true }] });
    if (url === '/api/videos') return Response.json({ videos: [] });
    if (url === '/api/learning-workspaces') return Response.json({ workspaces: [] });
    if (url === '/api/me/reading-resumption') return Response.json(null);
    throw new Error(url);
  }));
  wrapper = mount(NetworkApp); await flushPromises();
  await wrapper.get('input[name="invite_code"]').setValue('ABCDE-ABCDE-ABCDE-ABCDE');
  await wrapper.get('form').trigger('submit'); await flushPromises();
  expect(network.value.identity).toEqual(identity);
  expect(wrapper.text()).toContain('量化的本质');
  expect(wrapper.text()).toContain('已赠送 5 元额度');
  expect(wrapper.find('input[type="password"]').exists()).toBe(false);
  wrapper.unmount(); network.value = { ...network.value, identity: null, workspace: null };
  wrapper = mount(NetworkApp); await flushPromises();
  expect(wrapper.text()).toContain('量化的本质'); expect(redemptions).toBe(1);
  vi.stubGlobal('confirm', vi.fn(() => false));
  await wrapper.findAll('button').find(b => b.text() === '退出登录')!.trigger('click');
  expect(network.value.identity?.user_id).toBe('guest-one');
});
