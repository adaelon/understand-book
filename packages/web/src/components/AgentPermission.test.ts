// @vitest-environment happy-dom
import { flushPromises, mount } from '@vue/test-utils';
import { beforeEach, expect, it, vi } from 'vitest';
import AgentPermission from './AgentPermission.vue';
import { localRequest } from '../reader-host';

vi.mock('../reader-host', () => ({localRequest:vi.fn()}));
beforeEach(() => vi.mocked(localRequest).mockReset());

it.each([['Allow once','allow'],['取消此请求',null]])('sends the chosen ACP response for the original turn and request: %s', async (label, optionId) => {
  vi.mocked(localRequest).mockResolvedValueOnce({permission:{
    request_id:'request-17',turn_id:'turn-3',title:'Read the selected chapter',details:'Chapter I',
    options:[{name:'Allow once',optionId:'allow',kind:'allow_once'}],
  }}).mockResolvedValueOnce({ok:true});
  const card = mount(AgentPermission, { props: {turnId:'turn-3'} });
  await flushPromises();
  expect(card.text()).toContain('Read the selected chapter');
  expect(card.find('pre').text()).toBe('Chapter I');
  expect(localRequest).toHaveBeenCalledTimes(1);
  await card.findAll('button').find(button => button.text() === label)!.trigger('click');
  expect(localRequest).toHaveBeenLastCalledWith('POST','/local-agent/permission',{
    action:'respond',turn_id:'turn-3',request_id:'request-17',option_id:optionId,
  });
  await flushPromises();
  expect(card.find('section').exists()).toBe(false);
});
