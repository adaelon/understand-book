// @vitest-environment happy-dom
import { mount } from '@vue/test-utils';
import { describe, expect, it, vi } from 'vitest';
import LeftRail from './components/LeftRail.vue';

vi.stubGlobal('ResizeObserver', class { observe() {} disconnect() {} });
describe('restored directory interaction', () => {
  it('follows positions between chapter anchors and navigates to the selected real leaf', async () => {
    const wrapper=mount(LeftRail,{ props:{
      outlineItems:[
        { lid:'1.114',kind:'section',depth:0,title:'引言 为什么会发生这种情况？',span:{start:4506,end:11929}},
        { lid:'1.170',kind:'section',depth:0,title:'第一章 若干事例',span:{start:11929,end:27000}},
      ],anchorLid:'1.180',anchorOffset:12500,selectedLid:null,progressPct:10,leafCount:1822,
      debugOpen:false,gotoInput:'',searchQuery:'',
    }});
    expect(wrapper.find('.outline-item.active').text()).toContain('第一章 若干事例');
    await wrapper.findAll('.outline-item')[0].trigger('click');
    expect(wrapper.emitted('goto')).toEqual([['1.114']]);
    await wrapper.setProps({ anchorLid:'1.120',anchorOffset:5500 });
    expect(wrapper.find('.outline-item.active').text()).toContain('引言');
    await wrapper.setProps({ searchQuery:'若干' });
    expect(wrapper.findAll('.outline-item')).toHaveLength(1);
    expect(wrapper.find('.outline-item').text()).toContain('第一章');
    wrapper.unmount();
  });
});
