// @vitest-environment happy-dom
import { afterEach, describe, expect, it, vi } from 'vitest';
import { flushPromises, mount } from '@vue/test-utils';
const mocks = vi.hoisted(() => ({ context: vi.fn(), save: vi.fn() }));
vi.mock('./api', () => ({ api: { musicCalibrationContext: mocks.context, musicCalibrate: mocks.save } }));
vi.mock('./video-api', () => ({ videoCatalog: async () => ({ videos: [], active: null }), videoUrl: () => '' }));
vi.mock('./App.vue', () => ({ default: { template: '<div>阅读工作区</div>' } }));
vi.mock('./components/MusicCalibrationPane.vue', () => ({ default: { props: ['context'], emits: ['close'], template: '<section data-test="calibration"><h1>{{ context.manifest.work.title }}</h1><button @click="$emit(\'close\')">关闭校准</button></section>' } }));
import LocalApp from './LocalApp.vue';
const context = (title: string) => ({ manifest: { work: { title }, occurrences: [{}] }, audio_url: '/api/build_workbench/music/audio', score_pages: [], history: [] });
afterEach(() => vi.clearAllMocks());
describe('local calibration workspace identity', () => {
  it('reloads the current source before reopening and clears old material while the request is pending or failed', async () => {
    mocks.context.mockResolvedValueOnce(context('舒伯特'));
    const wrapper = mount(LocalApp); await flushPromises();
    expect(wrapper.get('h1').text()).toBe('舒伯特');
    await wrapper.get('[data-test="calibration"] button').trigger('click');
    let finish!: (value: unknown) => void;
    mocks.context.mockReturnValueOnce(new Promise(resolve => { finish = resolve; }));
    await wrapper.get('nav[aria-label="本地音乐校准"] button').trigger('click');
    expect(wrapper.find('[data-test="calibration"]').exists()).toBe(false);
    finish(context('贝九')); await flushPromises(); expect(wrapper.get('h1').text()).toBe('贝九');
    await wrapper.get('[data-test="calibration"] button').trigger('click');
    mocks.context.mockRejectedValueOnce(new Error('读取当前材料失败'));
    window.dispatchEvent(new Event('local-workspace-opened')); await flushPromises();
    expect(wrapper.find('nav[aria-label="本地音乐校准"]').exists()).toBe(false);
    expect(wrapper.get('[role="alert"]').text()).toBe('读取当前材料失败');
    wrapper.unmount();
  });
  it('adds the entry after a non-music workspace switches to music without taking over the reader', async () => {
    mocks.context.mockResolvedValueOnce(null);
    const wrapper = mount(LocalApp); await flushPromises();
    expect(wrapper.find('nav[aria-label="本地音乐校准"]').exists()).toBe(false);
    mocks.context.mockResolvedValueOnce(context('新音乐'));
    window.dispatchEvent(new Event('local-workspace-opened')); await flushPromises();
    expect(wrapper.get('nav[aria-label="本地音乐校准"]').text()).toContain('新音乐');
    expect(wrapper.find('[data-test="calibration"]').exists()).toBe(false);
    wrapper.unmount();
  });
});
