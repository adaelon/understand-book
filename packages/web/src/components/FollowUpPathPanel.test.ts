// @vitest-environment happy-dom
import { mount } from '@vue/test-utils';
import { describe, expect, it } from 'vitest';
import type { FollowUpNode, FollowUpStart } from '../follow-up-conversations';
import Panel from './FollowUpPathPanel.vue';

const draft: FollowUpStart = {
  parent_session_id:'root', parent_turn_id:'turn', client_request_id:'request',
  anchor:{ start_part:0, start_offset:0, end_part:0, end_offset:3, excerpt:'对照组' }, question:'对照组也上课吗？',
};
const root:FollowUpNode = { session_id:'root', title:'分差', question:'这 8 分从哪里来？', answer:'需要比较两组。', status:'ready' };
const child:FollowUpNode = { session_id:'control', title:'对照组', origin:{...draft,parent_question:root.question}, question:'用的教材一样吗？', answer:'这两组用同一份教材。', status:'ready' };
const props = { nodes:[root,child], current:'root', selected:'root', drafts:[], layout:'tray' as const };

describe('follow-up path preview', () => {
  it('previews the actual latest Q&A, preserves the current chat and only enters through the dedicated action', async () => {
    const wrapper = mount(Panel,{props});
    await wrapper.get('[data-session-id="control"]').trigger('click');
    expect(wrapper.emitted('select')).toEqual([['control']]);
    expect(wrapper.emitted('enter')).toBeUndefined();
    await wrapper.setProps({ selected:'control' });
    expect(wrapper.get('[aria-current="true"]').attributes('data-session-id')).toBe('root');
    const preview = wrapper.get('.follow-up-preview');
    expect(preview.text()).toContain(child.question);
    expect(preview.text()).toContain(child.answer);
    expect(preview.text()).toContain(draft.anchor.excerpt);
    await preview.get('button').trigger('click');
    expect(wrapper.emitted('enter')).toEqual([['control']]);
  });

  it('drafts only submit on confirmation and retain the request identity for retry', async () => {
    const wrapper = mount(Panel,{props:{...props,drafts:[draft],selected:'draft:request'}});
    const editor = wrapper.get('textarea');
    await editor.setValue('');
    expect(wrapper.emitted('update-question')).toEqual([['']]);
    expect(wrapper.emitted('start')).toBeUndefined();
    await wrapper.setProps({ drafts:[{...draft,question:' '}] });
    expect(wrapper.get('.primary').attributes('disabled')).toBeDefined();
    const edited = {...draft,question:'哪组继续用原来的学习方式？'};
    await wrapper.setProps({ drafts:[edited] });
    await wrapper.get('.primary').trigger('click');
    expect(wrapper.emitted('start')).toEqual([[edited]]);
    await wrapper.setProps({ nodes:[root,{...child,status:'unsent',answer:null}],drafts:[],selected:'control',current:'control' });
    expect(wrapper.text()).toContain('首问尚未发送');
    await wrapper.get('.primary').trigger('click');
    expect(wrapper.emitted('start')?.[1]).toEqual([child.origin]);
  });

  it('keeps a surviving subtree navigable after the parent is deleted', () => {
    const grand:FollowUpNode = {...child,session_id:'hours',origin:{...child.origin!,parent_session_id:'control',anchor:{...draft.anchor,excerpt:'课时'}}};
    const wrapper = mount(Panel,{props:{...props,nodes:[grand,child],current:'control',selected:'hours'}});
    expect(wrapper.findAll('nav button').map(n=>n.attributes('data-session-id'))).toEqual(['control','hours']);
  });

  it('refreshes a pending reply on preview without selecting its chat', async () => {
    const wrapper = mount(Panel,{props:{...props,layout:'side',nodes:[root,{...child,status:'pending',answer:null}]}});
    await wrapper.get('[data-session-id="control"]').trigger('click');
    expect(wrapper.emitted('retry')).toHaveLength(1);
    expect(wrapper.emitted('enter')).toBeUndefined();
    await wrapper.setProps({selected:'control',nodes:[root,child]});
    expect(wrapper.get('.preview-answer').text()).toBe(child.answer);
  });
});
