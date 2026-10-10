// @vitest-environment happy-dom
import { DOMWrapper, flushPromises, mount } from '@vue/test-utils';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { api } from '../api';
import SessionRecap from './SessionRecap.vue';
import ShareImagePanel from './ShareImagePanel.vue';
import { renderShare, saveShare } from '../reading-share';
import { network } from '../network-context';
vi.mock('../reading-share', async original => ({ ...await original<typeof import('../reading-share')>(), renderShare: vi.fn(), saveShare: vi.fn() }));
import type { SessionRecap as Recap } from '../session-recap';

function fixture(session = 'chat', seq = 12): Recap {
  const evidence = [{ turn_id: 'turn', event_seq: 2 }];
  return { session_id: session, through_seq: seq, through_at: '2026-10-02T09:00:00+08:00', generated_at: '2026-10-02T09:01:00+08:00',
    questions: [{ text: '这个结论的前提是什么？'.repeat(15), status: 'running', evidence }],
    sources: [{ source_ref_id: 'source', label: '第一章 · 前提', quote: '原文证据', published_book_ref: null, evidence, unavailable_reason: null }],
    effects: [{ effect_id: 'presentation:p:1', label: '演示 · 版本 1', status: 'delivered', effect: { kind: 'presentation', reference: { presentation_id: 'p', revision: 1 } }, object_id: null, published_book_ref: null, evidence, unavailable_reason: null }],
    continuations: [{ goal_id: 'goal', text: '继续比较两个前提', status: 'open', evidence }] };
}
const wrappers: ReturnType<typeof mount>[] = [];
function render(navigate = vi.fn(async () => {}), realTeleport = false) {
  const wrapper = mount(SessionRecap, { props: { sessionId: 'chat', navigate }, attachTo: realTeleport ? document.body : undefined,
    global: { stubs: { teleport: !realTeleport } } });
  wrappers.push(wrapper); return wrapper;
}
afterEach(() => { wrappers.splice(0).forEach(w => w.unmount()); document.body.innerHTML = ''; vi.restoreAllMocks(); });
beforeEach(() => {
  vi.spyOn(HTMLDialogElement.prototype, 'showModal').mockImplementation(() => {});
  vi.spyOn(HTMLDialogElement.prototype, 'close').mockImplementation(() => {});
  vi.spyOn(URL, 'createObjectURL').mockReturnValue('blob:recap');
  vi.spyOn(URL, 'revokeObjectURL').mockImplementation(() => {});
  vi.mocked(renderShare).mockReset(); vi.mocked(saveShare).mockReset();
});

describe('JL9 reading recap', () => {
  it('freezes the range until explicit refresh and opens exact evidence only on a click', async () => {
    const first = fixture(), second = fixture('chat', 19);
    second.questions.push({ text: '新问题', status: 'answered', evidence: [{ turn_id: 'next', event_seq: 18 }] });
    const read = vi.spyOn(api, 'sessionRecap').mockResolvedValueOnce(first).mockResolvedValueOnce(second);
    const navigate = vi.fn(async () => {}), wrapper = render(navigate);
    await flushPromises();
    expect(read).toHaveBeenCalledExactlyOnceWith('chat');
    expect(navigate).not.toHaveBeenCalled();
    expect(wrapper.findAll('.recap-sections > section')).toHaveLength(4);
    expect(wrapper.text()).toContain('进行中');
    expect(wrapper.text()).not.toContain('新问题');
    expect(wrapper.get('details p').text()).toBe(first.questions[0].text);
    await wrapper.findAll('button').find(b => b.text() === '查看原版本')!.trigger('click');
    expect(navigate).toHaveBeenCalledWith(expect.objectContaining({ session_id: 'chat', turn_id: 'turn', through_seq: 12,
      effect: expect.objectContaining({ effect: { kind: 'presentation', reference: { presentation_id: 'p', revision: 1 } } }) }));
    await wrapper.get('.recap-refresh').trigger('click'); await flushPromises();
    expect(wrapper.text()).toContain('新问题');
    await wrapper.findAll('button').find(b => b.text() === '查看原文')!.trigger('click');
    expect(navigate).toHaveBeenLastCalledWith(expect.objectContaining({ kind: 'source', through_seq: 19, source: first.sources[0] }));
  });
  it('clears an old chat immediately and discards late responses', async () => {
    let resolve!: (r: Recap) => void;
    vi.spyOn(api, 'sessionRecap').mockReturnValueOnce(new Promise(r => { resolve = r; })).mockResolvedValueOnce({ ...fixture('next'), questions: [], sources: [], effects: [], continuations: [] });
    const wrapper = render();
    await wrapper.setProps({ sessionId: 'next' }); await flushPromises();
    resolve(fixture()); await flushPromises();
    expect(wrapper.text()).toContain('还没有提问');
    expect(wrapper.findAll('.recap-empty')).toHaveLength(4);
    expect(wrapper.text()).not.toContain('原文证据');
  });
  it('keeps facts on refresh failure, shows unavailable objects and reports click-time deletion', async () => {
    const data = fixture(); data.sources[0].unavailable_reason = '原发布当前不可用';
    vi.spyOn(api, 'sessionRecap').mockResolvedValueOnce(data).mockRejectedValueOnce(new Error('读取失败'));
    const wrapper = render(vi.fn(async () => { throw new Error('原成果当前不可用'); }));
    await flushPromises();
    expect((wrapper.findAll('button').find(b => b.text() === '查看原文')!.element as HTMLButtonElement).disabled).toBe(true);
    await wrapper.findAll('button').find(b => b.text() === '查看原版本')!.trigger('click'); await flushPromises();
    expect(wrapper.get('[role="alert"]').text()).toBe('原成果当前不可用');
    expect(wrapper.emitted('close')).toBeUndefined();
    await wrapper.get('.recap-refresh').trigger('click'); await flushPromises();
    expect(wrapper.get('[role="alert"]').text()).toBe('读取失败');
    expect(wrapper.text()).toContain('演示 · 版本 1');
    await wrapper.get('.recap-close').trigger('click');
    expect(wrapper.emitted('close')).toHaveLength(1);
  });
});

describe('RS4 selected recap', () => {
  it('separates viewing focus from explicit sharing, freezes the draft across refresh, and reselects new facts', async () => {
    const first = fixture(), second = fixture('chat', 19);
    second.questions[0] = { ...first.questions[0], text: '刷新后的问题', status: 'answered' };
    const read = vi.spyOn(api, 'sessionRecap').mockResolvedValueOnce(first).mockResolvedValueOnce(second);
    // Real Teleport preserves the draft instance; the built-in test stub remounts its children on updates.
    const navigate = vi.fn(async () => {}), wrapper = render(navigate, true), view = new DOMWrapper(document.body);
    await wrapper.setProps({ sessionTitle: '前提与证据' }); await flushPromises();
    const share = () => view.findAll('button').find(b => b.text() === '生成阅读回顾卡')!;
    expect(share().attributes('disabled')).toBeDefined();
    await view.findAll('.recap-focus-button')[3].trigger('click');
    expect(view.get('.recap-focus').text()).toContain('继续比较两个前提');
    expect(navigate).not.toHaveBeenCalled();
    expect(share().attributes('disabled')).toBeDefined();
    await view.get('input[aria-label="分享讨论过的问题第1项"]').setValue(true);
    await view.get('input[aria-label="分享待继续事项第1项"]').setValue(true);
    await share().trigger('click');
    const panel = wrapper.getComponent(ShareImagePanel);
    const frozen = JSON.stringify(panel.props('source'));
    expect(panel.props('source').parts).toHaveLength(2);
    expect(panel.props('source').recap).toEqual({ sessionId: 'chat', throughSeq: 12, throughAt: first.through_at });
    expect(panel.props('source').association).toBe('对话「前提与证据」');
    await panel.get('input:not([type=checkbox])').setValue('自己的标题');
    await view.get('.recap-refresh').trigger('click'); await flushPromises();
    expect(wrapper.getComponent(ShareImagePanel).element).toBe(panel.element);
    expect((panel.get('input:not([type=checkbox])').element as HTMLInputElement).value).toBe('自己的标题');
    expect(JSON.stringify(panel.props('source'))).toBe(frozen);
    const png = new Blob(['png'], { type: 'image/png' });
    vi.mocked(renderShare).mockResolvedValue([png]); vi.mocked(saveShare).mockResolvedValue(true);
    await panel.findAll('button').find(b => b.text() === '预览图片')!.trigger('click'); await flushPromises();
    expect(panel.find('[role=alert]').exists() ? panel.get('[role=alert]').text() : '').toBe('');
    expect(panel.get('img').attributes('src')).toBe('blob:recap');
    await panel.findAll('button').find(b => b.text() === '下载 PNG')!.trigger('click'); await flushPromises();
    expect(renderShare).toHaveBeenCalledWith(expect.objectContaining({ layout: 'recap', title: '自己的标题', source: JSON.parse(frozen) }));
    expect(saveShare).toHaveBeenCalledWith(png, 'blob:recap', expect.any(Function), undefined);
    expect(read).toHaveBeenCalledTimes(2);
    await panel.get('.share-reselect').trigger('click'); await flushPromises();
    expect(wrapper.findComponent(ShareImagePanel).exists()).toBe(false);
    await share().trigger('click');
    const fresh = wrapper.getComponent(ShareImagePanel).props('source');
    expect(fresh.recap?.throughSeq).toBe(19);
    expect(fresh.parts[0]).toMatchObject({ text: '刷新后的问题', label: '阅读回顾 · 讨论过的问题 · 已回答' });
    expect(JSON.stringify(fresh)).not.toContain('已懂');
  });

  it('clears selection and the image on chat change and keeps empty recaps unshareable', async () => {
    vi.spyOn(api, 'sessionRecap').mockResolvedValueOnce(fixture()).mockResolvedValueOnce({ ...fixture('next'), questions: [], sources: [], effects: [], continuations: [] });
    const wrapper = render(); await flushPromises();
    await wrapper.get('input[type=checkbox]').setValue(true);
    await wrapper.findAll('button').find(b => b.text() === '生成阅读回顾卡')!.trigger('click');
    expect(wrapper.findComponent(ShareImagePanel).exists()).toBe(true);
    await wrapper.setProps({ sessionId: 'next' }); await flushPromises();
    expect(wrapper.findComponent(ShareImagePanel).exists()).toBe(false);
    expect(wrapper.find('.recap-focus').exists()).toBe(false);
    expect(wrapper.findAll('.recap-empty')).toHaveLength(4);
    expect(wrapper.findAll('button').find(b => b.text() === '生成阅读回顾卡')!.attributes('disabled')).toBeDefined();
  });

  it.each(['identity', 'publication'])('clears the frozen draft when the %s changes', async change => {
    const original = network.value;
    vi.spyOn(api, 'sessionRecap').mockResolvedValue(fixture());
    const wrapper = render(); await flushPromises();
    await wrapper.get('input[type=checkbox]').setValue(true);
    await wrapper.findAll('button').find(b => b.text() === '生成阅读回顾卡')!.trigger('click');
    try {
      network.value = change === 'identity' ? { ...original, identity: { user_id: 'other', csrf_token: 'csrf' } }
        : { ...original, workspace: { workspace_id: 'w', generation: 2, revision: 1, selected_chat: 'chat', published_book_ref: { book_id: 'book', publication_id: 'other' }, reader: {} as never } };
      await flushPromises();
      expect(wrapper.findComponent(ShareImagePanel).exists()).toBe(false);
      expect(wrapper.get('.recap-share-bar').text()).toContain('已选 0 项');
    } finally { wrapper.unmount(); network.value = original; }
  });

  it('keeps the open draft when the same workspace only advances its revision', async () => {
    const original = network.value;
    network.value = { ...original, workspace: { workspace_id: 'w', generation: 2, revision: 1, selected_chat: 'chat', published_book_ref: { book_id: 'book', publication_id: 'publication' }, reader: {} as never } };
    const read = vi.spyOn(api, 'sessionRecap').mockResolvedValue(fixture());
    const wrapper = render(vi.fn(async () => {}), true), view = new DOMWrapper(document.body);
    try {
      await flushPromises();
      await view.get('input[type=checkbox]').setValue(true);
      await view.findAll('button').find(b => b.text() === '生成阅读回顾卡')!.trigger('click');
      const panel = wrapper.getComponent(ShareImagePanel);
      network.value = { ...network.value, workspace: { ...network.value.workspace!, revision: 2 } };
      await flushPromises();
      expect(wrapper.findComponent(ShareImagePanel).exists()).toBe(true);
      expect(wrapper.getComponent(ShareImagePanel).element).toBe(panel.element);
      expect(read).toHaveBeenCalledTimes(1);
    } finally { wrapper.unmount(); network.value = original; }
  });
});
