// @vitest-environment happy-dom
import { mount } from '@vue/test-utils';
import { expect, it } from 'vitest';
import ConversationLearningGoal from './ConversationLearningGoal.vue';
import type { TutorSession } from '../generated/TutorSession';

const defaults = { enabled:true,busy:false,available:true,ready:true,otherSession:false,error:'' };

it('leaves the first question to the chat input without an empty goal card', async () => {
  const w = mount(ConversationLearningGoal,{ props:{ ...defaults,session:null,enabled:false } });
  expect(w.find('section').exists()).toBe(false);
  expect(w.find('input').exists()).toBe(false);
  expect(w.emitted()).toEqual({});
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
  w.unmount();
});

it('offers a direct goal entry when the current book has no matching learning session', async () => {
  const w = mount(ConversationLearningGoal,{ props:{ ...defaults,session:null,otherSession:true } });
  expect(w.text()).toContain('当前学习属于其他书籍');
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
