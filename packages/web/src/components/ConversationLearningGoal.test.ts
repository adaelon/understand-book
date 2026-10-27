// @vitest-environment happy-dom
import { mount } from '@vue/test-utils';
import { expect, it } from 'vitest';
import ConversationLearningGoal from './ConversationLearningGoal.vue';
import type { TutorSession } from '../generated/TutorSession';

const defaults = { enabled:true,busy:false,available:true,ready:true,error:'' };

it('shows only the restored stage for the current workspace material and never resumes it automatically', () => {
  const session = { id:'learning', user_intent:'复习链式法则', status:'paused' } as TutorSession;
  const w = mount(ConversationLearningGoal, { props: { ...defaults, session, workspaceScoped: true } });
  expect(w.text()).toContain('当前阶段'); expect(w.text()).toContain('复习链式法则');
  expect(w.find('.goal-resume').exists()).toBe(true);
  expect(w.find('[aria-label="调整阶段"]').exists()).toBe(true);
  expect(w.emitted('resume')).toBeUndefined();
  w.unmount();
});

it('offers this book its own empty goal entry without starting teaching', async () => {
  const w = mount(ConversationLearningGoal,{ props:{ ...defaults,session:null,enabled:false } });
  expect(w.text()).toContain('这本书还没有学习目标');
  expect(w.find('input').exists()).toBe(false);
  expect(w.emitted()).toEqual({});
  await w.get('button').trigger('click');
  expect(w.get('input').element.value).toBe('');
  w.unmount();
});

it('edits the visible goal without sending a chat turn or opening the management panel', async () => {
  const session = { id:'learning', user_intent:'理解注意力机制', status:'active' } as TutorSession;
  const w = mount(ConversationLearningGoal,{ props:{ ...defaults,session } });
  expect(w.text()).toContain(session.user_intent);
  await w.get('button').trigger('click');
  await w.get('input').setValue('理解 KV 缓存');
  await w.get('form').trigger('submit');
  expect(w.emitted('goal')).toEqual([['理解 KV 缓存']]);
  expect(w.emitted('enable')).toBeUndefined();
  await w.setProps({error:'保存失败'});
  expect(w.get('input').element.value).toBe('理解 KV 缓存');
  await w.get('[role=alert] button').trigger('click');
  expect(w.emitted('retry')).toEqual([[]]);
  expect(w.emitted('goal')).toHaveLength(1);
  await w.setProps({ error:'',session:{ ...session,revision:2 } });
  expect(w.get('input').element.value).toBe('理解 KV 缓存');
  await w.setProps({ session:{ ...session,id:'next',user_intent:'理解 KV 缓存' } });
  expect(w.find('input').exists()).toBe(false);
  expect(w.get('.goal-resume').text()).toContain('学习目标已保存');
  expect(w.get('.goal-resume button').text()).toContain('开始这次学习');
  w.unmount();
});

it.each([{ musicMode: true, label: '为这首作品设定目标', purpose: '带我听懂' }, { videoMode: true, label: '为这段视频设定目标', purpose: '带我看懂' }])('uses media language and saves a goal while teaching prepares: $label', async media => {
  const w = mount(ConversationLearningGoal, { props: { ...defaults, ...media, session: null, enabled: false,
    ready: false, learningPurposeAvailable: true, learningPurposeReason: '来源仍在准备' } });
  expect(w.text()).toContain('还没有学习目标');
  expect(w.get('button').text()).toBe(media.label);
  await w.get('button').trigger('click');
  expect(w.get('[data-purpose=understand]').text()).toContain(media.purpose);
  expect(w.get('[data-purpose=discover]').attributes('disabled')).toBeUndefined();
  expect(w.get('.purpose-notice').text()).toContain('来源仍在准备');
  await w.get('input').setValue('能解释当前材料的重点');
  await w.get('form').trigger('submit');
  expect(w.emitted('goal')).toEqual([['能解释当前材料的重点']]);
  await w.setProps({ session: { id: 'media-goal', user_intent: '能解释当前材料的重点', status: 'active' } as TutorSession, ready: true });
  expect(w.get('.goal-resume button').text()).toContain('开启并开始这次学习');
  await w.get('.goal-resume button').trigger('click');
  expect(w.emitted('resume')).toHaveLength(1);
  w.unmount();
});

it('offers a direct goal entry when the current book has no matching learning session', async () => {
  const w = mount(ConversationLearningGoal,{ props:{ ...defaults,session:null } });
  expect(w.text()).toContain('这本书还没有学习目标');
  await w.get('button').trigger('click');
  await w.get('input').setValue('理解这本书的核心论证');
  await w.get('form').trigger('submit');
  expect(w.emitted('goal')).toEqual([['理解这本书的核心论证']]);
  expect(w.emitted('enable')).toBeUndefined();
  w.unmount();
});

it.each(['paused','ended'] as const)('requires an explicit resume for %s and respects unavailable/readiness gates', async status => {
  const session = { id:'learning',user_intent:'理解注意力',status } as TutorSession;
  const w = mount(ConversationLearningGoal,{ props:{ ...defaults,session,enabled:false } });
  const resume = w.get('.goal-resume button');
  expect(resume.text()).toContain('开启并继续');
  expect(w.emitted('resume')).toBeUndefined();
  await resume.trigger('click');
  expect(w.emitted('resume')).toHaveLength(1);
  for (const values of [{available:false,ready:true,busy:false},{available:true,ready:false,busy:false},{available:true,ready:true,busy:true}]) {
    await w.setProps(values);
    expect(resume.attributes('disabled')).toBeDefined();
    await resume.trigger('click');
    expect(w.emitted('resume')).toHaveLength(1);
  }
  w.unmount();
});

it('opens details while pending and refuses a goal submission until state is available', async () => {
  const session = { id:'learning',user_intent:'理解注意力',status:'active' } as TutorSession;
  const w = mount(ConversationLearningGoal,{ props:{ ...defaults,session } });
  await w.get('.goal-actions button').trigger('click');
  await w.get('input').setValue('新的目标');
  await w.setProps({ available:false });
  await w.get('form').trigger('submit');
  expect(w.emitted('goal')).toBeUndefined();
  await w.findAll('.goal-heading button')[1].trigger('click');
  expect(w.emitted('manage')).toHaveLength(1);
  w.unmount();
});
