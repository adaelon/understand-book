// @vitest-environment happy-dom
import { mount } from '@vue/test-utils';
import { expect, it } from 'vitest';
import ConversationLearningGoal from './ConversationLearningGoal.vue';
import type { TutorSession } from '../generated/TutorSession';

it('uses the chat input as the first goal and only enables teaching on request', async () => {
  const w = mount(ConversationLearningGoal,{ props:{ session:null,enabled:false,busy:false,available:true,error:'' } });
  expect(w.text()).toContain('这次想弄懂什么');
  expect(w.find('input').exists()).toBe(false);
  expect(w.emitted('enable')).toBeUndefined();
  await w.get('button').trigger('click');
  expect(w.emitted('enable')).toHaveLength(1);
  w.unmount();
});

it('edits the visible goal without sending a chat turn or opening the management panel', async () => {
  const session = { id:'learning', user_intent:'理解注意力机制', status:'active' } as TutorSession;
  const w = mount(ConversationLearningGoal,{ props:{ session,enabled:true,busy:false,available:true,error:'' } });
  expect(w.text()).toContain(session.user_intent);
  await w.get('button').trigger('click');
  await w.get('input').setValue('理解 KV 缓存');
  await w.get('form').trigger('submit');
  expect(w.emitted('goal')).toEqual([['理解 KV 缓存']]);
  expect(w.emitted('enable')).toBeUndefined();
  await w.setProps({error:'保存失败'});
  expect(w.get('input').element.value).toBe('理解 KV 缓存');
  w.unmount();
});

it('offers a direct goal entry when the current book has no matching learning session', async () => {
  const w = mount(ConversationLearningGoal,{ props:{ session:null,enabled:true,busy:false,available:true,error:'' } });
  await w.get('button').trigger('click');
  await w.get('input').setValue('理解这本书的核心论证');
  await w.get('form').trigger('submit');
  expect(w.emitted('goal')).toEqual([['理解这本书的核心论证']]);
  expect(w.emitted('enable')).toBeUndefined();
  w.unmount();
});
