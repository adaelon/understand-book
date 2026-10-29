// @vitest-environment happy-dom
import { mount, flushPromises } from '@vue/test-utils';
import { afterEach, expect, it, vi } from 'vitest';
import { network } from './network-context';
vi.mock('./App.vue', () => ({ default: { template: '<div />' } }));
import NetworkApp from './NetworkApp.vue';

let wrapper: ReturnType<typeof mount> | undefined;
afterEach(() => { wrapper?.unmount(); vi.unstubAllGlobals(); localStorage.clear(); sessionStorage.clear(); history.replaceState({}, '', '/'); network.value = { ...network.value, identity: null, workspace: null, enabled: false }; });

it('opens email registration from an old guest link without redeeming an invite', async () => {
  history.replaceState({}, '', '/?account=guest');
  const fetcher = vi.fn(async () => Response.json({}, { status: 401 }));
  vi.stubGlobal('fetch', fetcher);
  wrapper = mount(NetworkApp); await flushPromises();
  expect(location.search).toBe('?account=register');
  expect(wrapper.get('h1').text()).toBe('创建阅读账号');
  expect(wrapper.find('input[name="email"]').exists()).toBe(true);
  expect(wrapper.find('input[name="invite_code"]').exists()).toBe(true);
  expect(wrapper.text()).toContain('内测期间 AI Token 免费使用');
  expect(fetcher).not.toHaveBeenCalled();
});

it('offers registration and password recovery without a guest entry on the sign-in page', async () => {
  vi.stubGlobal('fetch', vi.fn(async () => Response.json({}, { status: 401 })));
  wrapper = mount(NetworkApp); await flushPromises();
  expect(wrapper.text()).not.toContain('免注册');
  expect(wrapper.text()).toContain('忘记密码');
  await wrapper.findAll('button').find(b => b.text() === '注册账号')!.trigger('click');
  expect(wrapper.get('h1').text()).toBe('创建阅读账号');
});

it('restores an existing guest session and keeps its reader and logout notice', async () => {
  const identity = { user_id: 'guest-one', is_guest: true, csrf_token: 'csrf' };
  const requests: string[] = [];
  vi.stubGlobal('fetch', vi.fn(async (url: string) => {
    requests.push(url);
    if (url === '/api/auth/me') return Response.json(identity);
    if (url === '/api/library') return Response.json({ books: [{ title: '量化的本质', published_book_ref: { book_id: 'example', publication_id: 'current' }, is_default: true }] });
    if (url === '/api/videos') return Response.json({ videos: [] });
    if (url === '/api/learning-workspaces') return Response.json({ workspaces: [] });
    if (url === '/api/me/reading-resumption') return Response.json(null);
    throw new Error(url);
  }));
  wrapper = mount(NetworkApp); await flushPromises();
  expect(network.value.identity?.user_id).toBe('guest-one');
  expect(wrapper.text()).toContain('量化的本质');
  expect(requests).not.toContain('/api/auth/guest');
  vi.stubGlobal('confirm', vi.fn(() => false));
  await wrapper.findAll('button').find(b => b.text() === '退出登录')!.trigger('click');
  expect(network.value.identity?.user_id).toBe('guest-one');
});
