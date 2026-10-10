// @vitest-environment happy-dom
import { mount } from '@vue/test-utils';
import { expect, it, vi } from 'vitest';
import NoteCard from './NoteCard.vue';
import AnnotationPreview from './AnnotationPreview.vue';
import { shareNoteKey } from '../reading-share';
import type { MemoryRecord } from '../api';

it('uses the same selected record from PDF and Markdown details', async () => {
  const note: MemoryRecord = { mem_id: 'same-record', book_id: 'book', type: 'note', layer: 'long_term', anchor: {}, content: '同一条记录' };
  const share = vi.fn(); const global = { provide: { [shareNoteKey as symbol]: share } };
  const pdf = mount(NoteCard, { props: { note, renderMarkdown: text => text }, global });
  const markdown = mount(AnnotationPreview, { props: { records: [note], active: note, sourceText: '', renderMarkdown: text => text }, global });
  try {
    for (const detail of [pdf, markdown]) {
      await detail.get('[data-share-note]').trigger('click');
      expect(share).toHaveBeenLastCalledWith(note);
      expect(detail.emitted('edit')).toBeUndefined(); expect(detail.emitted('open-note')).toBeUndefined();
    }
    expect(share).toHaveBeenCalledTimes(2);
  } finally { pdf.unmount(); markdown.unmount(); }
});
