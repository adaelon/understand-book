// @vitest-environment happy-dom
import { flushPromises, mount } from '@vue/test-utils';
import { nextTick } from 'vue';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import YeyeMascot from './YeyeMascot.vue';
import { yeyeAssets } from '../yeye-assets';
import idleAnimation from '../assets/yeye/idle.json';

const runtime = vi.hoisted(() => ({ loadAnimation: vi.fn() }));
vi.mock('lottie-web', () => ({ default: runtime }));

it('is decorative by default and exposes one named image when it supplies the identity', async () => {
  const wrapper = mount(YeyeMascot);
  expect(wrapper.attributes('aria-hidden')).toBe('true');
  expect(wrapper.attributes('role')).toBeUndefined();
  expect(wrapper.get('img').attributes('alt')).toBe('');
  expect(wrapper.find('[tabindex]').exists()).toBe(false);
  await wrapper.setProps({ decorative: false });
  expect(wrapper.attributes('aria-hidden')).toBeUndefined();
  expect(wrapper.attributes('role')).toBe('img');
  expect(wrapper.attributes('aria-label')).toBe('页页，阅读伙伴');
  await wrapper.setProps({ label: '阅读伙伴页页' });
  expect(wrapper.attributes('aria-label')).toBe('阅读伙伴页页');
  await wrapper.setProps({ decorative: true });
  expect(wrapper.attributes('aria-label')).toBeUndefined();
  wrapper.unmount();
});

it('switches between the cream-book reading image and the dedicated compact avatar', async () => {
  const wrapper = mount(YeyeMascot);
  expect(wrapper.get('img').attributes('src')).toBe(yeyeAssets.idle);
  expect(wrapper.get('img').attributes('width')).toBe('160');
  await wrapper.setProps({ variant: 'reading' });
  expect(wrapper.get('img').attributes('src')).toBe(yeyeAssets.reading);
  await wrapper.setProps({ variant: 'avatar' });
  expect(wrapper.get('img').attributes('src')).toBe(yeyeAssets.avatar);
  expect(wrapper.get('img').attributes('width')).toBe('32');
  expect(wrapper.element.style.width).toBe('32px');
  wrapper.unmount();
});

it('keeps the host-selected footprint when the image changes or fails to load', async () => {
  const wrapper = mount(YeyeMascot, { props: { size: 128 } });
  await wrapper.setProps({ variant: 'reading' });
  await wrapper.get('img').trigger('error');
  expect(wrapper.element.style.width).toBe('128px');
  expect(wrapper.element.style.height).toBe('128px');
  expect(wrapper.get('img').attributes('height')).toBe('128');
  await wrapper.setProps({ variant: 'avatar', size: 24 });
  expect(wrapper.element.style.width).toBe('24px');
  expect(wrapper.element.style.height).toBe('24px');
  wrapper.unmount();
});

describe('controlled motion', () => {
  let wrapper: ReturnType<typeof mount<typeof YeyeMascot>>;
  let intersection: (entries: unknown[]) => void;
  let media: EventTarget & { matches: boolean };
  let visible: string;
  const disconnect = vi.fn();
  let players: { pause: ReturnType<typeof vi.fn>; destroy: ReturnType<typeof vi.fn>; goToAndPlay: ReturnType<typeof vi.fn>; fire: (name: string) => void }[];

  beforeEach(() => {
    vi.useFakeTimers();
    players = [];
    runtime.loadAnimation.mockReset().mockImplementation(() => {
      const events: Record<string, () => void> = {};
      const player = { pause: vi.fn(), destroy: vi.fn(), goToAndPlay: vi.fn(),
        addEventListener: (name: string, callback: () => void) => { events[name] = callback; },
        fire: (name: string) => events[name]?.() };
      players.push(player); return player;
    });
    disconnect.mockClear();
    vi.stubGlobal('IntersectionObserver', class {
      constructor(callback: typeof intersection) { intersection = callback; }
      observe() {} disconnect = disconnect;
    });
    media = Object.assign(new EventTarget(), { matches: false });
    vi.spyOn(window, 'matchMedia').mockReturnValue(media as MediaQueryList);
    visible = 'visible';
    vi.spyOn(document, 'visibilityState', 'get').mockImplementation(() => visible as DocumentVisibilityState);
    vi.spyOn(HTMLImageElement.prototype, 'decode').mockResolvedValue(undefined);
    vi.stubGlobal('fetch', vi.fn(async () => Response.json(idleAnimation)));
  });
  afterEach(() => { wrapper?.unmount(); vi.restoreAllMocks(); vi.unstubAllGlobals(); vi.useRealTimers(); });
  async function mountMotion(props: Record<string, unknown> = {}) {
    wrapper = mount(YeyeMascot, { props: { motion: 'idle', ...props } });
    intersection([{ isIntersecting: true, intersectionRect: { width: 160, height: 160 } }]);
    await nextTick();
  }
  async function advance(ms: number) { await vi.advanceTimersByTimeAsync(ms); await flushPromises(); }
  async function ready() { players.at(-1)!.fire('DOMLoaded'); await nextTick(); }

  it('keeps the first screen static, loads only idle after 16 seconds, and rests between one-shot blinks', async () => {
    await mountMotion();
    await advance(15_999); expect(fetch).not.toHaveBeenCalled(); expect(runtime.loadAnimation).not.toHaveBeenCalled();
    await advance(1); expect(fetch).toHaveBeenCalledTimes(1);
    expect(runtime.loadAnimation).toHaveBeenCalledWith(expect.objectContaining({ autoplay: false, loop: false, renderer: 'svg',
      animationData: expect.objectContaining({ assets: [expect.objectContaining({ p: yeyeAssets.idle, u: '', e: 1 })] }) }));
    expect(wrapper.attributes('data-yeye-playing')).toBe('false');
    await ready(); expect(players[0].goToAndPlay).toHaveBeenCalledTimes(1);
    players[0].fire('complete'); await nextTick();
    expect(wrapper.attributes('data-yeye-playing')).toBe('false');
    await advance(15_999); expect(players[0].goToAndPlay).toHaveBeenCalledTimes(1);
    await advance(1); expect(players[0].goToAndPlay).toHaveBeenCalledTimes(2);
    expect(fetch).toHaveBeenCalledTimes(1);
  });

  it('avoids short-wait flicker, loops a sustained wait and stops immediately when the fact ends', async () => {
    await mountMotion({ variant: 'reading', motion: 'waiting' });
    await advance(1_199); expect(fetch).not.toHaveBeenCalled();
    await wrapper.setProps({ motion: 'static' }); await advance(5_000); expect(fetch).not.toHaveBeenCalled();
    await wrapper.setProps({ motion: 'waiting' }); await advance(1_200); await ready();
    expect(runtime.loadAnimation).toHaveBeenCalledWith(expect.objectContaining({ loop: true }));
    expect(vi.mocked(fetch).mock.calls[0][0]).toContain('reading.json');
    await wrapper.setProps({ motion: 'static' });
    expect(players[0].pause).toHaveBeenCalledOnce(); expect(players[0].destroy).toHaveBeenCalledOnce();
    expect(wrapper.attributes('data-yeye-playing')).toBe('false');
    await advance(30_000); expect(fetch).toHaveBeenCalledTimes(1);
  });

  it.each(['host', 'page', 'viewport', 'preference'] as const)('stops and clears idle time for %s, then starts a fresh interval', async gate => {
    await mountMotion(); await advance(16_000); await ready();
    const toggle = async (active: boolean) => {
      if (gate === 'host') await wrapper.setProps({ active });
      if (gate === 'page') { visible = active ? 'visible' : 'hidden'; document.dispatchEvent(new Event('visibilitychange')); }
      if (gate === 'viewport') intersection([{ isIntersecting: active, intersectionRect: { width: active ? 160 : 0, height: 160 } }]);
      if (gate === 'preference') { media.matches = !active; media.dispatchEvent(new Event('change')); }
      await nextTick();
    };
    await toggle(false); expect(players[0].destroy).toHaveBeenCalledOnce();
    expect(wrapper.attributes('data-yeye-playing')).toBe('false');
    await advance(60_000); expect(fetch).toHaveBeenCalledTimes(1); expect(vi.getTimerCount()).toBe(0);
    await toggle(true); await advance(15_999); expect(fetch).toHaveBeenCalledTimes(1);
    await advance(1); expect(fetch).toHaveBeenCalledTimes(2);
  });

  it('does not load motion for an initial reduced-motion preference or a compact avatar', async () => {
    media.matches = true; await mountMotion(); await advance(60_000); expect(fetch).not.toHaveBeenCalled();
    await wrapper.setProps({ variant: 'avatar' }); media.matches = false; media.dispatchEvent(new Event('change'));
    await nextTick(); await advance(60_000); expect(fetch).not.toHaveBeenCalled();
  });

  it('nods once on an explicit completion, returns to still and never loads Lottie for it', async () => {
    await mountMotion({ variant: 'avatar', motion: 'static' });
    const feedback = wrapper.vm.prepareCompletionFeedback()!;
    feedback(); await nextTick();
    expect(wrapper.attributes('data-yeye-nodding')).toBe('true');
    expect(wrapper.get('img').classes()).not.toContain('yeye-still-hidden');
    await advance(720); expect(wrapper.attributes('data-yeye-nodding')).toBe('false');
    feedback(); await advance(30_000);
    expect(wrapper.attributes('data-yeye-nodding')).toBe('false');
    expect(fetch).not.toHaveBeenCalled(); expect(runtime.loadAnimation).not.toHaveBeenCalled();
  });

  it.each(['host', 'page', 'viewport', 'preference'] as const)('consumes hidden completions and invalidates pending feedback across %s changes', async gate => {
    await mountMotion({ variant: 'avatar', motion: 'static' });
    const feedback = wrapper.vm.prepareCompletionFeedback()!;
    const toggle = async (active: boolean) => {
      if (gate === 'host') await wrapper.setProps({ active });
      if (gate === 'page') { visible = active ? 'visible' : 'hidden'; document.dispatchEvent(new Event('visibilitychange')); }
      if (gate === 'viewport') intersection([{ isIntersecting: active, intersectionRect: { width: active ? 24 : 0, height: 24 } }]);
      if (gate === 'preference') { media.matches = !active; media.dispatchEvent(new Event('change')); }
      await nextTick();
    };
    feedback(); await nextTick(); expect(wrapper.attributes('data-yeye-nodding')).toBe('true');
    const pending = wrapper.vm.prepareCompletionFeedback()!;
    await toggle(false); expect(wrapper.attributes('data-yeye-nodding')).toBe('false');
    expect(wrapper.vm.prepareCompletionFeedback()).toBeUndefined();
    expect(vi.getTimerCount()).toBe(0);
    await toggle(true); pending(); await nextTick();
    expect(wrapper.attributes('data-yeye-nodding')).toBe('false');
  });

  it('discards an unmounted completion callback and a remount has no queued feedback', async () => {
    await mountMotion({ variant: 'avatar', motion: 'static' });
    const pending = wrapper.vm.prepareCompletionFeedback()!;
    wrapper.unmount(); pending(); expect(vi.getTimerCount()).toBe(0);
    await mountMotion({ variant: 'avatar', motion: 'static' });
    await advance(30_000); expect(wrapper.attributes('data-yeye-nodding')).toBe('false');
    expect(fetch).not.toHaveBeenCalled();
  });

  it('ignores a late load after hiding and releases observers, listeners and timers on unmount', async () => {
    let release!: (response: Response) => void;
    vi.mocked(fetch).mockImplementationOnce(() => new Promise(resolve => { release = resolve; }));
    const remove = vi.spyOn(document, 'removeEventListener');
    const removeMedia = vi.spyOn(media, 'removeEventListener');
    await mountMotion(); await advance(16_000);
    const signal = vi.mocked(fetch).mock.calls[0][1]!.signal!;
    await wrapper.setProps({ active: false }); expect(signal.aborted).toBe(true);
    release(Response.json(idleAnimation)); await flushPromises(); expect(runtime.loadAnimation).not.toHaveBeenCalled();
    await wrapper.setProps({ active: true }); wrapper.unmount();
    expect(disconnect).toHaveBeenCalled();
    expect(remove).toHaveBeenCalledWith('visibilitychange', expect.any(Function));
    expect(removeMedia).toHaveBeenCalledWith('change', expect.any(Function));
    await advance(60_000); expect(fetch).toHaveBeenCalledTimes(1); expect(vi.getTimerCount()).toBe(0);
  });

  it.each(['request', 'image', 'player'] as const)('retains the static footprint after a %s failure without retrying continuously', async failure => {
    if (failure === 'request') vi.mocked(fetch).mockResolvedValueOnce(new Response('', { status: 503 }));
    if (failure === 'image') vi.mocked(HTMLImageElement.prototype.decode).mockRejectedValueOnce(new Error('decode failed'));
    await mountMotion(); await advance(16_000);
    if (failure === 'player') { await ready(); players[0].fire('data_failed'); await nextTick(); }
    expect(wrapper.attributes('data-yeye-playing')).toBe('false');
    expect(wrapper.get('img').classes()).not.toContain('yeye-still-hidden');
    expect(wrapper.element.style.width).toBe('160px');
    await advance(60_000); expect(fetch).toHaveBeenCalledTimes(1);
  });
});
