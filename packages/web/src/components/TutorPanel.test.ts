// @vitest-environment happy-dom
import { mount, type VueWrapper } from '@vue/test-utils';
import { nextTick } from 'vue';
import { afterEach, expect, it } from 'vitest';
import TutorPanel from './TutorPanel.vue';
import type { TutorState } from '../generated/TutorState';
import type { TutorSession } from '../generated/TutorSession';
import type { CourseLessonView } from '../generated/CourseLessonView';

const session = (id = 'learn', patch: Partial<TutorSession> = {}): TutorSession => ({
  id, revision: 1, workspace_goal_revision: null, status: 'active', user_intent: '理解速度', explicit_constraints: [],
  current_focus: { interpretation: '', target_object_refs: [], capability_targets: [] },
  material_scope: [{ source_id: 'book', scope_refs: [], role: 'primary' }],
  default_teaching_intent: null, path_instance_ref: null, progress_ref: null, ...patch,
});
const state = (sessions: TutorState['sessions'] = { learn: session() }, current: string | null = 'learn'): TutorState =>
  ({ control: { enabled: true, revision: 2, current_tutor_session_id: current }, sessions, workspace_goal_revision: null, learning_map_enabled: false });
const wrappers: VueWrapper[] = [];
function setup(value: TutorState | null = state(), initialView?: 'current' | 'understanding' | 'history' | 'knowledge') {
  const w = mount(TutorPanel, { attachTo: document.body, props: { state: value, busy: false, error: '', pending: false, sourceId: 'book', label: 'Tutor 已开启',
    readiness: { status: 'ready', reason: '可用', limitations: [] }, initialView }, global: { stubs: { Teleport: true, UnderstandingSpace: true, KnowledgeTree: true } } });
  wrappers.push(w); return w;
}
const button = (w: VueWrapper, name: string) => w.findAll('button').find(b => b.text() === name)!;
afterEach(() => { wrappers.splice(0).forEach(w => w.unmount()); });

it('shows the active course when the global Tutor is off and routes pause to the course',async()=>{
  const course={progress:{status:'active'},summary:{question:'两项时间怎样决定下界？',objective:'区分计算与读取的约束'}} as CourseLessonView;
  const value=state();value.control.enabled=false;
  const w=mount(TutorPanel,{props:{state:value,busy:false,error:'',pending:false,label:'学习助手已关闭',course},global:{stubs:{Teleport:true}}});
  wrappers.push(w);
  expect(w.text()).toContain(course.summary.question);expect(w.text()).toContain('本课带学中');
  expect(w.text()).not.toContain('学习助手已关闭');expect(w.find('.new-learning').exists()).toBe(false);
  await button(w,'暂停带学').trigger('click');
  expect(w.emitted('course-action')).toEqual([[{kind:'pause'}]]);expect(w.emitted('action')).toBeUndefined();
  await button(w,'回到当前课程').trigger('click');expect(w.emitted('course-return')).toHaveLength(1);
});

it('redirects previous understanding and tree entries to the primary map',async()=>{
  const w=setup(null,'understanding');expect(w.emitted('understanding')).toHaveLength(1);
  await w.setProps({state:state()});
  const panel=w.vm as unknown as {openView:(view:'knowledge')=>void};panel.openView('knowledge');await nextTick();
  expect(w.emitted('understanding')).toHaveLength(2);expect(w.find('#tutor-knowledge').exists()).toBe(false);
  await button(w,'新学习').trigger('click');await w.get('textarea').setValue('继续理解偏移量');
  await button(w,'我的理解 ↗').trigger('click');expect(w.emitted('update:draft')?.at(-1)).toEqual(['继续理解偏移量']);
  expect(w.emitted('understanding')).toHaveLength(3);expect(w.get('textarea').element.value).toBe('继续理解偏移量');
});

it('prioritizes the current goal, preserves drafts across tabs, and switches only after a confirmed start', async () => {
  const w = setup();
  expect(w.get('#tutor-current').isVisible()).toBe(true);
  expect(w.get('#tutor-new').isVisible()).toBe(false);
  await button(w, '新学习').trigger('click');
  await w.get('textarea').setValue('从原文解释速度');
  await w.get('#tutor-tab-history').trigger('click');
  await button(w, '新学习').trigger('click');
  expect(w.get('textarea').element.value).toBe('从原文解释速度');
  await w.get('form').trigger('submit');
  expect(w.emitted('action')?.[0][0]).toMatchObject({ kind: 'start', user_intent: '从原文解释速度', material_scope: [{ source_id: 'book' }] });
  expect(w.get('#tutor-new').isVisible()).toBe(true);
  await w.setProps({ pending: true, error: '保存失败' });
  expect(w.get('textarea').element.value).toBe('从原文解释速度');
  expect(button(w, '开始学习').attributes('disabled')).toBeDefined();
  await button(w, '重试').trigger('click');
  expect(w.emitted('retry')).toHaveLength(1);
  await w.setProps({ pending: false, error: '', state: state({ learn: session(), next: session('next', { user_intent: '从原文解释速度' }) }, 'next') });
  expect(w.get('#tutor-current').isVisible()).toBe(true);
  expect(w.get('.session-goal h3').text()).toBe('从原文解释速度');
  expect(w.get('textarea').element.value).toBe('');
});

it('keeps readiness, enabled control, pending state and material ownership gates', async () => {
  const w = setup();
  await button(w, '继续学习').trigger('click');
  expect(w.emitted('action')?.[0][0]).toEqual({ kind: 'resume', session_id: 'learn' });
  await w.setProps({ readiness: { status: 'preparing', reason: '原文或全书结构尚未就绪', limitations: [] } });
  expect(button(w, '继续学习').attributes('disabled')).toBeDefined();
  await button(w, '新学习').trigger('click');
  await w.get('textarea').setValue('继续解释');
  expect(button(w, '保存学习意图').attributes('disabled')).toBeUndefined();
  await w.get('#tutor-tab-current').trigger('click');
  await w.setProps({ readiness: { status: 'ready', reason: '可用', limitations: ['缺少示例'] }, sourceId: 'other' });
  expect(button(w, '继续学习').attributes('disabled')).toBeDefined();
  expect(w.text()).toContain('请回到本次学习的书籍继续');
  expect(w.text()).toContain('缺少示例');
  await w.setProps({ sourceId: 'book', state: { ...state(), control: { ...state().control, enabled: false } } });
  expect(button(w, '继续学习').attributes('disabled')).toBeUndefined();
  await button(w, '继续学习').trigger('click');
  expect(w.emitted('action')?.at(-1)?.[0]).toEqual({ kind: 'resume', session_id: 'learn' });
  await w.setProps({ state: state(), pending: true });
  expect(button(w, '继续学习').attributes('disabled')).toBeDefined();
  await w.setProps({ pending: false, readinessError: '学习就绪状态读取失败' });
  expect(button(w, '继续学习').attributes('disabled')).toBeDefined();
  await button(w, '新学习').trigger('click');
  expect(button(w, '保存学习意图').exists()).toBe(true);
});

it('keeps learning goal actions without a separate teaching mode', async () => {
  const w = setup();
  expect(w.find('.teaching-mode').exists()).toBe(false);
  expect(w.text()).not.toContain('默认教法');
  await button(w, '暂停').trigger('click');
  expect(w.emitted('action')?.[0][0]).toEqual({ kind: 'pause', session_id: 'learn' });
  await button(w, '结束这次学习').trigger('click');
  await button(w, '取消').trigger('click');
  expect(w.find('.end-confirmation').exists()).toBe(false);
  await button(w, '结束这次学习').trigger('click');
  await button(w, '结束学习').trigger('click');
  expect(w.emitted('action')?.[1][0]).toEqual({ kind: 'end', session_id: 'learn' });
});

it('keeps every non-current session reachable and requires an explicit resume', async () => {
  const data = state({ learn: session(), old: session('old', { status: 'ended', user_intent: '旧目标' }),
    other: session('other', { status: 'paused', user_intent: '另一本书', material_scope: [{ source_id: 'other-book', scope_refs: [], role: 'primary' }] }),
    background: session('background', { user_intent: '另一个目标' }), absent: undefined });
  const w = setup(data);
  await w.get('#tutor-tab-history').trigger('click');
  expect(w.findAll('.history-session')).toHaveLength(3);
  await w.findAll('.history-session')[0].trigger('click');
  expect(w.get('.session-goal h3').text()).toBe('旧目标');
  expect(w.emitted('action')).toBeUndefined();
  await button(w, '新学习').trigger('click');
  await button(w, '返回当前学习').trigger('click');
  expect(w.get('.session-goal h3').text()).toBe('理解速度');
  await w.get('#tutor-tab-history').trigger('click');
  await w.findAll('.history-session')[0].trigger('click');
  await button(w, '继续学习').trigger('click');
  expect(w.emitted('action')?.[0][0]).toEqual({ kind: 'resume', session_id: 'old' });
  await w.setProps({ state: state({ ...data.sessions, old: session('old', { user_intent: '旧目标' }) }, 'old') });
  expect(w.get('#tutor-current').isVisible()).toBe(true);
  await w.get('#tutor-tab-history').trigger('click');
  await w.findAll('.history-session').find(b => b.text().includes('另一本书'))!.trigger('click');
  expect(button(w, '继续学习').attributes('disabled')).toBeDefined();
});

it('shows new learning without a session and retains drafts when state loads', async () => {
  const w = setup(null);
  expect(w.get('#tutor-new').isVisible()).toBe(true);
  await w.get('textarea').setValue('请带我一步步推导');
  expect(w.get('textarea').element.value).toContain('一步步推导');
  await w.setProps({ state: state() });
  expect(w.get('#tutor-new').isVisible()).toBe(true);
  await w.setProps({ sourceId: undefined });
  expect(button(w, '开始学习').attributes('disabled')).toBeDefined();
  await w.get('form').trigger('submit');
  expect(w.emitted('action')).toBeUndefined();
});

it('includes the paused current session in history and handles keyboard tabs and close', async () => {
  const w = setup(state({ learn: session('learn', { status: 'paused' }) }));
  await w.get('#tutor-tab-history').trigger('click');
  expect(w.findAll('.history-session')).toHaveLength(1);
  await w.get('.tutor-tabs').trigger('keydown', { key: 'Home' });
  expect(w.get('#tutor-tab-current').attributes('aria-selected')).toBe('true');
  await w.get('.tutor-tabs').trigger('keydown', { key: 'ArrowRight' });
  expect(w.get('#tutor-tab-history').attributes('aria-selected')).toBe('true');
  await w.get('.tutor-panel').trigger('keydown', { key: 'Escape' });
  expect(w.emitted('close')).toHaveLength(1);
});
