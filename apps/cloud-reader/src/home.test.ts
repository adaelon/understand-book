// @vitest-environment happy-dom
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
vi.mock('vue', () => ({ createApp: () => ({ mount: vi.fn() }) }));
vi.mock('./components/ProductHome.vue', () => ({ default: {} }));

beforeEach(() => { vi.resetModules(); });
afterEach(() => { vi.unstubAllGlobals(); });

it('takes a signed-in reader from the public root back to the reader', async () => {
  const replace = vi.fn();
  vi.stubGlobal('location', { pathname: '/', hash: '', replace });
  vi.stubGlobal('fetch', vi.fn(async () => Response.json({ user_id: 'reader' })));
  await import('./home');
  expect(replace).toHaveBeenCalledWith('/index.html');
  expect(fetch).toHaveBeenCalledWith('/api/auth/me', { credentials: 'same-origin', cache: 'no-store' });
});

it.each(['/home.html', '/#learning-story'])('keeps an explicit introduction visit at %s', async address => {
  const url = new URL(address, 'https://reader.example'), replace = vi.fn();
  vi.stubGlobal('location', { pathname: url.pathname, hash: url.hash, replace });
  vi.stubGlobal('fetch', vi.fn());
  await import('./home');
  expect(fetch).not.toHaveBeenCalled(); expect(replace).not.toHaveBeenCalled();
});

it.each(['signed-out', 'offline'])('keeps the introduction usable when %s', async state => {
  const replace = vi.fn();
  vi.stubGlobal('location', { pathname: '/', hash: '', replace });
  vi.stubGlobal('fetch', vi.fn(async () => {
    if (state === 'offline') throw new Error('offline');
    return Response.json({}, { status: 401 });
  }));
  await import('./home');
  expect(replace).not.toHaveBeenCalled();
});

it('lets a reader choose an introduction section while the login check is pending', async () => {
  const current = { pathname: '/', hash: '', replace: vi.fn() };
  let finish!: (response: Response) => void;
  vi.stubGlobal('location', current);
  vi.stubGlobal('fetch', vi.fn(() => new Promise<Response>(resolve => { finish = resolve; })));
  await import('./home');
  current.hash = '#learning-story';
  finish(Response.json({ user_id: 'reader' }));
  await Promise.resolve();
  expect(current.replace).not.toHaveBeenCalled();
});
