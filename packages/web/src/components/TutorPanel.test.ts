// @vitest-environment happy-dom
import { mount } from '@vue/test-utils';
import { expect, it } from 'vitest';
import TutorPanel from './TutorPanel.vue';
import type { TutorState } from '../generated/TutorState';

it('opens learning with optional assets absent and preserves explicit control and source ownership', async () => {
  const state: TutorState = { control: { enabled: true, revision: 2, current_tutor_session_id: 'learn' }, sessions: {
    learn: { id: 'learn', revision: 1, status: 'active', user_intent: '理解速度', explicit_constraints: [],
      current_focus: { interpretation: '', target_object_refs: [], capability_targets: [] },
      material_scope: [{ source_id: 'book', scope_refs: [], role: 'primary' }], default_teaching_intent: null, path_instance_ref: null, progress_ref: null },
  } };
  const w = mount(TutorPanel, { props: { state, busy: false, error: '', pending: false, sourceId: 'book', label: 'Tutor 已开启',
    readiness: { status: 'ready', reason: '可用', limitations: [] } }, global: { stubs: { Teleport: true, UnderstandingSpace: true } } });
  const button = (name: string) => w.findAll('button').find(b => b.text() === name)!;
  try {
    await w.get('textarea').setValue('从原文解释速度');
    expect(button('开始学习').attributes('disabled')).toBeUndefined();
    await w.get('form').trigger('submit');
    expect(w.emitted('action')?.[0][0]).toMatchObject({ kind: 'start', material_scope: [{ source_id: 'book' }] });
    await button('继续教学').trigger('click');
    expect(w.emitted('action')?.[1][0]).toEqual({ kind: 'resume', session_id: 'learn' });
    await w.setProps({ readiness: { status: 'preparing', reason: '原文或全书结构尚未就绪', limitations: [] } });
    expect(button('继续教学').attributes('disabled')).toBeDefined();
    expect(button('保存学习意图').exists()).toBe(true);
    await w.setProps({ readiness: { status: 'ready', reason: '可用', limitations: [] }, sourceId: 'other' });
    expect(button('继续教学').attributes('disabled')).toBeDefined();
    expect(w.text()).toContain('请回到本次学习的书籍继续');
    await w.setProps({ sourceId: 'book', state: { ...state, control: { ...state.control, enabled: false } } });
    expect(button('保存学习意图').exists()).toBe(true);
    expect(button('继续教学').attributes('disabled')).toBeDefined();
  } finally { w.unmount(); }
});
