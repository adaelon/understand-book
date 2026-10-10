// @vitest-environment happy-dom
import { mount } from '@vue/test-utils';
import { expect, it } from 'vitest';
import NoteEditorPanel from './NoteEditorPanel.vue';

it('keeps composition Enter out of save and emits explicit collapse separately from discard', async () => {
  const panel = mount(NoteEditorPanel, { props: { content: '想法', title: '演示', revision: 2, saving: false } });
  await panel.get('textarea').trigger('keydown', { key: 'Enter', ctrlKey: true, isComposing: true });
  expect(panel.emitted('save')).toBeUndefined();
  await panel.get('textarea').trigger('keydown', { key: 'Enter', ctrlKey: true, isComposing: false });
  expect(panel.emitted('save')).toHaveLength(1);
  await panel.findAll('button').find(b => b.text() === '收起')!.trigger('click');
  expect(panel.emitted('collapse')).toHaveLength(1);
  expect(panel.emitted('discard')).toBeUndefined();
  panel.unmount();
});


it('saves excerpt-only notes, guards the composition lifecycle and locks text during a mutation', async () => {
  const panel = mount(NoteEditorPanel, { props: { content: '', title: '原文', excerpt: { kind: 'original', text: '原句' }, saving: false } });
  const textarea = panel.get('textarea');
  await textarea.trigger('compositionstart');
  await textarea.trigger('keydown', { key: 'Enter', metaKey: true, isComposing: false });
  expect(panel.emitted('save')).toBeUndefined();
  await textarea.trigger('compositionend');
  await textarea.trigger('keydown', { key: 'Enter', metaKey: true });
  expect(panel.emitted('save')).toHaveLength(1);
  await panel.setProps({ saving: true });
  expect((textarea.element as HTMLTextAreaElement).readOnly).toBe(true);
  await textarea.trigger('keydown', { key: 'Enter', ctrlKey: true });
  expect(panel.emitted('save')).toHaveLength(1);
  panel.unmount();
});
