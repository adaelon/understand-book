// @vitest-environment happy-dom
import { flushPromises, mount } from '@vue/test-utils';
import { afterEach, expect, it, vi } from 'vitest';
import { installIdentity, network } from '../network-context';
import DesktopAuthorization from './DesktopAuthorization.vue';

const wrappers: ReturnType<typeof mount>[] = [];
const query = new URLSearchParams({
  desktop_authorize: '1', redirect_uri: 'http://127.0.0.1:49152/callback',
  code_challenge: 'A'.repeat(43), state: 'B'.repeat(43),
});
afterEach(() => {
  wrappers.splice(0).forEach(wrapper => wrapper.unmount());
  vi.unstubAllGlobals();
  network.value = { ...network.value, identity: null, workspace: null, enabled: false };
});
function start() {
  const assign = vi.fn();
  vi.stubGlobal('location', { search: `?${query}`, assign });
  installIdentity({ user_id: 'A', csrf_token: 'csrf-a' });
  let respond!: (response: Response) => void;
  const fetcher = vi.fn(() => new Promise<Response>(resolve => { respond = resolve; }));
  vi.stubGlobal('fetch', fetcher);
  const wrapper = mount(DesktopAuthorization); wrappers.push(wrapper);
  return { wrapper, assign, fetcher, respond: (response: Response) => respond(response) };
}

it('keeps an issued desktop authorization when the same account restores a different reading scene', async () => {
  const { wrapper, assign, fetcher, respond } = start();
  await wrapper.get('button').trigger('click');
  expect(fetcher).toHaveBeenCalledWith('/api/auth/desktop/authorize', expect.objectContaining({
    method: 'POST', credentials: 'same-origin', cache: 'no-store',
    headers: { 'Content-Type': 'application/json', 'X-CSRF-Token': 'csrf-a' },
  }));
  // Reader restoration changes the scene without changing the consenting account.
  network.value = { ...network.value, epoch: network.value.epoch + 1 };
  respond(Response.json({ redirect_url: 'http://127.0.0.1:49152/callback?code=issued&state=bound' }));
  await flushPromises();
  expect(assign).toHaveBeenCalledWith('http://127.0.0.1:49152/callback?code=issued&state=bound');
  expect(wrapper.find('[role="alert"]').exists()).toBe(false);
});

it('does not complete the original desktop authorization after the account changes', async () => {
  const { wrapper, assign, respond } = start();
  await wrapper.get('button').trigger('click');
  installIdentity({ user_id: 'B', csrf_token: 'csrf-b' });
  respond(Response.json({ redirect_url: 'http://127.0.0.1:49152/callback?code=original-account' }));
  await flushPromises();
  expect(assign).not.toHaveBeenCalled();
  expect(wrapper.get('[role="alert"]').text()).toContain('账号已切换');
  expect(wrapper.get('button').attributes('disabled')).toBeUndefined();
});
