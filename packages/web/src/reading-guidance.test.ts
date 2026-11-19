// @vitest-environment happy-dom
import { flushPromises, mount } from '@vue/test-utils';
import { defineComponent, h, ref } from 'vue';
import { expect, it, vi } from 'vitest';
import { provideReadingGuidance, useReadingGuidance, type GuidanceContext } from './reading-guidance';

const api = vi.hoisted(() => ({ readingGuidance: vi.fn() }));
vi.mock('./api', () => ({ api }));
vi.mock('./reader-context', () => ({ bindSceneApi: (value: unknown) => value, readerKey: () => 'reader' }));

it.each([null, { question: '你想补充哪一点？', draft: '我想补充的是……' }])('reuses a generated or quiet decision without calls for typing and reopening: %j', async result => {
  api.readingGuidance.mockReset().mockResolvedValue(result);
  const context = ref<GuidanceContext>({ key: 'book:chat-1', title: '材料', intent: '', medium: 'text', selection: null,
    scopes: [{ value: 'material', label: '整份材料' }] });
  const active = ref(true), text = ref('');
  const Child = defineComponent({ setup() {
    const guide = useReadingGuidance({ purpose: () => 'followup', scope: () => 'material', active: () => active.value, text: () => text.value,
      recentTurns: () => [{ turn_id: 'turn', user: '我的理由' + '字'.repeat(1500) + '最后的想法',
        assistant: '解释' + '字'.repeat(4000) + '已经问过的问题？', source_quote: '原文' }] });
    return () => h('p', text.value ? 'typing' : guide.value?.draft ?? 'none');
  } });
  const wrapper = mount(defineComponent({ setup() { provideReadingGuidance(context); return () => h(Child); } }));
  try {
    await flushPromises();
    expect(api.readingGuidance).toHaveBeenCalledTimes(1);
    const request = api.readingGuidance.mock.calls[0][0];
    expect(request.recent_turns[0].user.length).toBeLessThanOrEqual(1000);
    expect(request.recent_turns[0].user).toMatch(/最后的想法$/);
    expect(request.recent_turns[0].assistant.length).toBeLessThanOrEqual(2400);
    expect(request.recent_turns[0].assistant).toMatch(/已经问过的问题？$/);
    text.value = '自己写的内容'; await flushPromises();
    expect(wrapper.text()).toBe('typing');
    text.value += '继续写'; await flushPromises();
    text.value = ''; await flushPromises();
    active.value = false; await flushPromises();
    active.value = true; await flushPromises();
    expect(api.readingGuidance).toHaveBeenCalledTimes(1);
    expect(wrapper.text()).toBe(result?.draft ?? 'none');
    context.value = { ...context.value, key: 'book:chat-2' }; await flushPromises();
    expect(api.readingGuidance).toHaveBeenCalledTimes(2);
  } finally { wrapper.unmount(); }
});
