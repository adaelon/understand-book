// @vitest-environment happy-dom
import { mount } from '@vue/test-utils';
import { expect, it } from 'vitest';
import UnderstandingSummaryCard from './UnderstandingSummary.vue';
import type { UnderstandingSummary } from '../note-thinking';

it('keeps an earlier branch expression readable without calling its difficulty current', async () => {
  const item: UnderstandingSummary = { focus_ref:'offset',source_id:'book',source_revision:'v1',workspace_id:null,
    label:'偏移量的相对关系',capability:'explanation',target:null,demonstrated:[],difficulties:['把偏移量当成固定答案'],
    unconfirmed:[],evidence_ref:'old',evidence_refs:['old'],note_refs:[],next_step:'比较两个锚框',occurred_at:null,
    source_session_id:null,source_turn_id:null,assistance:'在已提供帮助下',nature:'hypothesis',learner_quote:'偏移量应该一样',correction:null };
  const wrapper=mount(UnderstandingSummaryCard,{props:{item,showQuote:true}});
  expect(wrapper.get('.difficulties h4').text()).toBe('当前卡点');
  await wrapper.setProps({item:{...item,later_evidence_ref:'new'}});
  expect(wrapper.get('.summary-eyebrow').text()).toContain('先前记录');
  expect(wrapper.get('.difficulties h4').text()).toBe('当时卡点');
  expect(wrapper.get('blockquote').text()).toBe(item.learner_quote);
  expect(wrapper.get('.summary-next').text()).toContain('当时的建议');
  expect(wrapper.text()).toContain('最新判断见“我的理解”');
  wrapper.unmount();
});
