// @vitest-environment happy-dom
import { mount } from '@vue/test-utils';
import { expect, it } from 'vitest';
import FocusReadingControls from './FocusReadingControls.vue';

it('offers exit immediately and gives an open menu or input priority on Escape', async () => {
  const pane = document.createElement('main');
  pane.className = 'reader-pane';
  document.body.append(pane);
  const wrapper = mount(FocusReadingControls, { attachTo: pane, props: { active: true, open: false } });
  const pressEscape = (target: EventTarget) => target.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true, cancelable: true }));
  try {
    expect(wrapper.get('.focus-exit').text()).toBe('退出专注');
    const input = document.createElement('textarea');
    pane.append(input);
    pressEscape(input);
    expect(wrapper.emitted('exit')).toBeUndefined();
    await wrapper.setProps({ open: true });
    pressEscape(document.body);
    expect(wrapper.emitted('update:open')).toEqual([[false]]);
    expect(wrapper.emitted('exit')).toBeUndefined();
    await wrapper.setProps({ open: false });
    pressEscape(document.body);
    expect(wrapper.emitted('exit')).toHaveLength(1);
    await wrapper.setProps({ active: false });
    pressEscape(document.body);
    expect(wrapper.emitted('exit')).toHaveLength(1);
  } finally {
    wrapper.unmount();
    pane.remove();
  }
});
