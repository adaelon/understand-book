// @vitest-environment happy-dom
import { afterEach, describe, expect, it } from 'vitest';
import { mount } from '@vue/test-utils';
import BoardReadingPane from './components/BoardReadingPane.vue';
import { boardFixtureItems } from './fixtures/board-workspace';

let wrapper: ReturnType<typeof mount> | undefined;
afterEach(() => { wrapper?.unmount(); wrapper = undefined; });

describe('board explanation source discovery', () => {
  it('places retained source access before a long answer and emits the exact selected source', async () => {
    const item = structuredClone(boardFixtureItems.find(item => item.target.kind === 'answer')!);
    item.readable_content = '很长的讲解。'.repeat(300);
    item.source_bindings = [{ source_ref_id: 'exact-source', book_id: 'book', evidence_text_digest: 'retained', label_snapshot: '第三章 · 距离', preview_snapshot: '普攻距离是六格。' }];
    wrapper = mount(BoardReadingPane, { props: { boardId: 'board', item } });
    const sources = wrapper.get('.board-reading-heading .board-reading-sources');
    expect(sources.get('summary').text()).toContain('原文引用 · 1 处');
    expect(wrapper.get('.board-reading-content').find('.board-reading-sources').exists()).toBe(false);
    expect(sources.text()).toContain('第三章 · 距离');
    await sources.get('button').trigger('click');
    expect(wrapper.emitted('source')?.[0]).toEqual(['exact-source', sources.get('button').element]);
  });
  it('explicitly identifies an answer without source bindings and never borrows another card’s sources', async () => {
    const item = structuredClone(boardFixtureItems.find(item => item.target.kind === 'answer')!);
    item.source_bindings = [];
    wrapper = mount(BoardReadingPane, { props: { boardId: 'board', item, items: boardFixtureItems } });
    expect(wrapper.get('.board-reading-heading').text()).toContain('本条讲解未附原文引用');
    expect(wrapper.find('.board-reading-sources').exists()).toBe(false);
  });
});
