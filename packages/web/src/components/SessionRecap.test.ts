// @vitest-environment happy-dom
import { flushPromises, mount } from '@vue/test-utils';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { api } from '../api';
import SessionRecap from './SessionRecap.vue';
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
function render(navigate = vi.fn(async () => {})) {
  const wrapper = mount(SessionRecap, { props: { sessionId: 'chat', navigate }, global: { stubs: { teleport: true } } });
  wrappers.push(wrapper); return wrapper;
}
afterEach(() => { wrappers.splice(0).forEach(w => w.unmount()); vi.restoreAllMocks(); });

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
