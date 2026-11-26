// @vitest-environment happy-dom
import { enableAutoUnmount, flushPromises, mount } from '@vue/test-utils';
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import type { TeachBackComparison } from '../generated/TeachBackComparison';
import { comparisonGraphs, type StoredTeachBackComparison } from '../teach-back-history';
import { setLocale } from '../i18n';
import TeachBackComparisonCard from './TeachBackComparisonCard.vue';

const api = vi.hoisted(() => ({ teachBackDisplayed: vi.fn() }));
vi.mock('../api', () => ({ api }));
let intersect: IntersectionObserverCallback;
function setIntersection(isIntersecting: boolean, height = 120) {
  intersect([{ isIntersecting, intersectionRect: { width: 320, height } } as IntersectionObserverEntry], {} as IntersectionObserver);
}
enableAutoUnmount(afterEach);
beforeEach(() => {
  api.teachBackDisplayed.mockReset().mockResolvedValue({});
  vi.stubGlobal('IntersectionObserver', class {
    constructor(callback: IntersectionObserverCallback) { intersect = callback; }
    observe() {}
    disconnect() {}
  });
});
afterEach(() => { vi.unstubAllGlobals(); vi.restoreAllMocks(); setLocale('zh-CN'); });

const source = { source_id: 'infra', source_revision: 'edition-1' };
const legacy: Extract<StoredTeachBackComparison, { book_relations: unknown }> = {
  round_id: 'tb:turn-2', source,
  book_relations: [
    { id: 'b1', premise: '显存容量', relation_type: 'defines', conclusion: '能否装下权重', evidence: [{ lid: '3.6.3.3', quote: '容量决定能否装下权重。' }] },
    { id: 'b2', condition: '读取量不变且处于读取瓶颈', premise: '带宽翻倍', relation_type: 'causes', conclusion: '纯读取时间下界减半',
      evidence: [{ lid: '3.6.3.8', quote: '先比较两种下界。' }, { lid: '3.6.3.14', quote: '带宽翻倍，读取时间下界降为 10.45 ms。' }] },
  ],
  student_relations: [
    { id: 's1', conclusion: '放得下', relation_type: 'concludes', quotes: [{ turn_id: 'turn-2', text: '装得下。' }] },
    { id: 's2', condition: '已能装下', premise: '容量翻倍', relation_type: 'causes', conclusion: '能读得更快', quotes: [{ turn_id: 'turn-2', text: '装得下以后，容量大了就更快。' }] },
    { id: 's3', conclusion: '快一倍', relation_type: 'concludes', quotes: [{ turn_id: 'turn-2', text: '我估计快一倍。' }] },
  ],
  rows: [
    { row_id: 'capacity', book_relation_ids: ['b1'], student_relation_ids: ['s1'], status: 'consistent', summary: '都谈容纳能力。' },
    { row_id: 'speed', book_relation_ids: ['b2'], student_relation_ids: ['s2', 's3'], status: 'divergent', summary: '比较的资源不同。', divergence_condition: '只换容量更大的卡。' },
  ],
  side_notes: [{ label: '另一条思路：减少读取量', text: '也可减少要读取的数据。', evidence: [{ lid: '3.6.3.15', quote: '读取量减半。' }] }],
  follow_up: { row_id: 'speed', question: '带宽不变，只增容量会怎样？' },
};
const comparison = comparisonGraphs(legacy);
function show(data: StoredTeachBackComparison = comparison) {
  return mount(TeachBackComparisonCard, {
    props: { sessionId: 'chat', turnId: 'turn-2', comparison: data, currentSourceId: 'infra', currentSourceRevision: 'edition-1' },
    attachTo: document.body,
  });
}

it('focuses the follow-up on both graphs and shows the selected quotes, conditions and question', async () => {
  const card = show();
  const [first, second] = card.findAll('.teach-back-select');
  expect(first.get('.focus-topic').text()).toBe('放得下');
  expect(second.get('.focus-topic').text()).toBe('能读得更快');
  expect(card.get('.focus-heading').text()).toContain('能读得更快');
  expect(second.attributes('aria-pressed')).toBe('true');
  expect(card.get('[data-node-id="b2:to"]').attributes('data-active')).toBe('true');
  expect(card.get('[data-node-id="s2:to"]').attributes('data-active')).toBe('true');
  expect(card.get('[data-node-id="b1:from"]').attributes('data-active')).toBe('false');
  expect(card.get('.teach-back-row').text()).toContain('只换容量更大的卡。');
  expect(card.get('.teach-back-row').text()).toContain('读取量不变且处于读取瓶颈');
  expect(card.get('.teach-back-row').text()).toContain('已能装下');
  await card.get('[data-node-id="s1:to"]').trigger('click');
  expect(first.attributes('aria-pressed')).toBe('true');
  expect(card.get('[data-edge-id="b1"]').attributes('data-active')).toBe('true');
  await first.trigger('keydown', { key: 'ArrowDown' });
  expect(document.activeElement).toBe(second.element);
  expect(second.attributes('aria-pressed')).toBe('true');
  const evidence = card.get('.teach-back-row.selected .teach-back-evidence');
  expect(evidence.findAll('blockquote').map(q => q.text())).toEqual([
    '先比较两种下界。', '带宽翻倍，读取时间下界降为 10.45 ms。', '装得下以后，容量大了就更快。', '我估计快一倍。',
  ]);
  const links = evidence.findAll('button');
  const extra = evidence.get<HTMLDetailsElement>('details.more-quotes');
  expect(extra.element.open).toBe(false);
  extra.element.open = true;
  await links[1].trigger('click');
  await links[2].trigger('click');
  expect(card.emitted('jump')).toEqual([['3.6.3.14', 'turn-2']]);
  expect(card.emitted('locate')).toEqual([['turn-2']]);
  expect(card.get('.teach-back-row').get('.follow-up').text()).toContain('带宽不变，只增容量会怎样？');
  const notes = card.get('details.teach-back-side-note');
  expect(notes.attributes('open')).toBeUndefined();
  expect(notes.find('.comparison-status').exists()).toBe(false);
  expect(notes.get('blockquote').text()).toBe('读取量减半。');
  await card.get('[data-edge-label="b1"]').trigger('click');
  expect(card.get('.teach-back-evidence').text()).toContain('装得下。');
  await second.trigger('click');
  expect(card.get<HTMLDetailsElement>('.teach-back-evidence details').element.open).toBe(false);
  await second.trigger('click');
  expect(second.attributes('aria-pressed')).toBe('true');
});

it('draws each endpoint of a branch and merge once and exposes additional endpoint citations', async () => {
  const evidence = [{ lid: '3.6.3.3', quote: 'Quote on the edge.' }];
  const graph: TeachBackComparison['book_graph'] = {
    nodes: ['A', 'B', 'C', 'D'].map(id => ({ id, label: id, evidence: id === 'A'
      ? [{ lid: '3.6.3.3', quote: 'Additional wording on A.' }, ...evidence] : [] })),
    edges: [['A','B'], ['A','C'], ['B','D'], ['C','D']].map(([from, to]) => ({
      id: from + to, from, to, relation_type: 'supports', evidence,
    })),
  };
  const card = show({ ...comparison, book_graph: graph,
    rows: comparison.rows.map((r, i) => ({ ...r, book_edge_ids: i ? ['AB','AC'] : ['BD','CD'] })) });
  const book = card.get('[data-side="book"]');
  expect(book.findAll('.node-level').map(level => level.findAll('[data-node-id]').map(n => n.attributes('data-node-id'))))
    .toEqual([['A'], ['B', 'C'], ['D']]);
  expect(book.findAll('path[marker-end]')).toHaveLength(4);
  expect(card.get('.teach-back-evidence section').findAll('blockquote').map(q => q.text()))
    .toEqual(['Quote on the edge.', 'Additional wording on A.']);
  const shared = book.get('[data-node-id="B"]');
  await shared.trigger('click');
  expect(card.get('[data-row-id="capacity"]').attributes('aria-pressed')).toBe('true');
  await shared.trigger('click');
  expect(card.get('[data-row-id="speed"]').attributes('aria-pressed')).toBe('true');
  await book.get('[data-node-id="A"]').trigger('click');
  const firstQuote = card.get('.teach-back-evidence section > .citation blockquote');
  expect(firstQuote.text()).toBe('Additional wording on A.');
  await book.get('[data-edge-label="AC"]').trigger('click');
  expect(card.get('[data-row-id="speed"]').attributes('aria-pressed')).toBe('true');
});

it('opens historical row IDs and shared relations, drawing each original relation only once', async () => {
  const oldRows = legacy.rows.map(row => ({ ...row, book_relation_ids: ['b1'] }));
  const card = show({ ...legacy, book_relations: [legacy.book_relations[0]], rows: oldRows });
  expect(card.findAll('[data-edge-id="b1"]')).toHaveLength(1);
  expect(card.findAll('[data-node-id="s1:to"]')).toHaveLength(1);
  expect(card.get('[data-edge-id="b1"]').attributes('data-active')).toBe('true');
  expect(card.get('.teach-back-evidence').text()).toContain('容量决定能否装下权重。');
  await card.get('[data-row-id="capacity"]').trigger('click');
  expect(card.get('[data-edge-id="b1"]').attributes('data-active')).toBe('true');
  expect(card.get('.teach-back-evidence').text()).toContain('装得下。');
});

it.each([{ currentSourceId: 'another-book' }, { currentSourceRevision: 'edition-2' }, { currentSourceRevision: undefined }])(
  'keeps the quote snapshot and disables every book jump if the exact source cannot be located', async unavailable => {
    const card = show();
    await card.setProps(unavailable);
    await card.get('[data-row-id="capacity"]').trigger('click');
    expect(card.get('.teach-back-evidence').text()).toContain('容量决定能否装下权重。');
    expect(card.get('.teach-back-evidence').text()).toContain(
      unavailable.currentSourceRevision === 'edition-2' ? '原书版本已变' : '原书版本当前不可定位',
    );
    const passage = card.get('.teach-back-evidence button');
    expect(passage.attributes('disabled')).toBeDefined();
    expect(card.get('details.teach-back-side-note button').attributes('disabled')).toBeDefined();
    await passage.trigger('click');
    expect(card.emitted('jump')).toBeUndefined();
  },
);

it('names each comparison status in English and renders one-sided evidence without inventing a quotation', async () => {
  setLocale('en');
  const statuses: TeachBackComparison['rows'][number]['status'][] = [
    'consistent', 'divergent', 'omitted', 'alternative_valid', 'unverified_against_passage', 'insufficient_book_basis', 'clarified',
  ];
  const rows = statuses.map((status, i) => ({ ...comparison.rows[i === 1 ? 1 : 0], status, row_id: status }));
  rows[2].student_node_ids = [];
  rows[5].book_edge_ids = [];
  const card = show({ ...comparison, rows, follow_up: undefined });
  expect(card.findAll('.teach-back-select .focus-state').map(s => s.text())).toEqual([
    'In agreement', 'Different reasoning', 'Key point left unstated', 'Valid alternative',
    'Needs checking against the passage', 'Insufficient evidence in the book', 'Reason clarified',
  ]);
  await card.get('[data-row-id="omitted"]').trigger('click');
  expect(card.get('.teach-back-evidence').text()).toContain('There is no matching quotation from your explanation.');
  await card.get('[data-row-id="insufficient_book_basis"]').trigger('click');
  expect(card.get('.teach-back-evidence').text()).toContain('There is no source quotation for this relation.');
  expect(card.find('.follow-up').exists()).toBe(false);
});

it('records only a visible render in a foreground tab, once per round, including when an offscreen card returns', async () => {
  const visibility = vi.spyOn(document, 'visibilityState', 'get').mockReturnValue('hidden');
  const card = show(); await flushPromises();
  expect(api.teachBackDisplayed).not.toHaveBeenCalled();
  setIntersection(true);
  expect(api.teachBackDisplayed).not.toHaveBeenCalled();
  setIntersection(false);
  visibility.mockReturnValue('visible');
  document.dispatchEvent(new Event('visibilitychange'));
  setIntersection(true, 0);
  expect(api.teachBackDisplayed).not.toHaveBeenCalled();
  setIntersection(true);
  await flushPromises();
  await card.get('[data-row-id="speed"]').trigger('click');
  document.dispatchEvent(new Event('visibilitychange'));
  expect(api.teachBackDisplayed.mock.calls).toEqual([['chat', 'turn-2', 'tb:turn-2']]);
  await card.setProps({ turnId: 'turn-3', comparison: { ...comparison, round_id: 'tb:turn-3' } });
  expect(api.teachBackDisplayed).toHaveBeenCalledTimes(1);
  setIntersection(true);
  expect(api.teachBackDisplayed).toHaveBeenLastCalledWith('chat', 'turn-3', 'tb:turn-3');
});

it('keeps delivered evidence available after a failed receipt and retries just the display', async () => {
  api.teachBackDisplayed.mockRejectedValueOnce(new Error('offline'));
  const card = show(); await flushPromises();
  setIntersection(true); await flushPromises();
  expect(card.get('.teach-back-evidence').text()).toContain('带宽翻倍');
  await card.get('[role="status"] button').trigger('click'); await flushPromises();
  expect(card.find('[role="status"]').exists()).toBe(false);
  expect(api.teachBackDisplayed.mock.calls).toEqual([['chat', 'turn-2', 'tb:turn-2'], ['chat', 'turn-2', 'tb:turn-2']]);
});

it('shares an in-flight display across surfaces and reuses the confirmed receipt when reopening', async () => {
  let confirm!: (receipt: { support_delivery_refs: string[] }) => void;
  api.teachBackDisplayed.mockReturnValueOnce(new Promise(resolve => { confirm = resolve; }));
  const wrapper = mount({
    components: { TeachBackComparisonCard },
    data: () => ({ comparison, visible: true }),
    template: '<TeachBackComparisonCard v-if="visible" :comparison="comparison" session-id="chat" turn-id="turn-2" />',
  });
  await flushPromises();
  setIntersection(true);
  await wrapper.setData({ visible:false });
  await wrapper.setData({ visible:true });
  setIntersection(true);
  expect(api.teachBackDisplayed).toHaveBeenCalledTimes(1);
  confirm({support_delivery_refs:['usage:original', 'tb:earlier']});
  await flushPromises();
  expect(wrapper.get('.teach-back-support').text()).toContain('1 轮三图对照');
  await wrapper.setData({ visible:false });
  await wrapper.setData({ visible:true });
  setIntersection(true);
  await flushPromises();
  expect(wrapper.getComponent(TeachBackComparisonCard).emitted('displayed')).toEqual([['turn-2','tb:turn-2']]);
  expect(api.teachBackDisplayed).toHaveBeenCalledTimes(1);
});

it('opens a previously displayed course card with its saved assistance and no new receipt request', async () => {
  const card = show({ ...comparison, support_delivery_refs:['course:explanation', 'tb:first'] });
  await card.setProps({ alreadyDisplayed:true });
  setIntersection(true);
  await flushPromises();
  expect(card.get('.teach-back-support').text()).toContain('1 轮三图对照');
  expect(api.teachBackDisplayed).not.toHaveBeenCalled();
});

it('recovers an older card’s frozen support from its own display receipt', async () => {
  let finishFirst!: (receipt: { support_delivery_refs: string[] }) => void;
  api.teachBackDisplayed.mockReturnValueOnce(new Promise(resolve => { finishFirst = resolve; }));
  const card = show(); await flushPromises();
  setIntersection(true);
  const other = { ...comparison, round_id: 'tb:other' };
  await card.setProps({ turnId: 'other', comparison: other });
  api.teachBackDisplayed.mockResolvedValueOnce({ event_id: 'display:tb:other', support_delivery_refs: ['usage:original', 'tb:earlier'] });
  setIntersection(true); await flushPromises();
  expect(card.get('.teach-back-support').text()).toContain('1 轮三图对照');
  finishFirst({ support_delivery_refs: ['usage:original'] }); await flushPromises();
  expect(card.get('.teach-back-support').text()).toContain('1 轮三图对照');
  expect(api.teachBackDisplayed.mock.calls).toEqual([['chat', 'turn-2', 'tb:turn-2'], ['chat', 'other', 'tb:other']]);
});

it.each(['dispute_interpretation', 'revise_understanding'] as const)(
  'submits %s for exactly the selected row after its display receipt', async action => {
    const card = show();
    await card.setProps({ correctionEnabled: true });
    await card.get('[data-row-id="speed"]').trigger('click');
    await card.get(`[data-action="${action}"]`).trigger('click');
    const field = card.get('textarea');
    expect(document.activeElement).toBe(field.element);
    await field.setValue('  我说的是车道的宽度。  ');
    expect(card.get('button[type="submit"]').attributes('disabled')).toBeDefined();
    setIntersection(true);
    await flushPromises();
    expect(card.emitted('displayed')).toEqual([['turn-2', 'tb:turn-2']]);
    expect(card.get('button[type="submit"]').attributes('disabled')).toBeUndefined();
    await card.get('form').trigger('submit');
    expect(card.emitted('correct')).toEqual([[{
      action, turnId: 'turn-2', roundId: 'tb:turn-2', rowId: 'speed', text: '我说的是车道的宽度。',
    }]]);
    expect(card.find('textarea').exists()).toBe(false);
    await card.get(`[data-action="${action}"]`).trigger('click');
    await card.get('textarea').trigger('keydown', { key: 'Escape' });
    expect(card.find('textarea').exists()).toBe(false);
    expect(document.activeElement).toBe(card.get(`[data-action="${action}"]`).element);
  },
);

it('replays the selected complete round with its own source snapshot, help and turn identity', async () => {
  const revised = comparisonGraphs({
    round_id: 'tb:turn-4', source, side_notes: [], support_delivery_refs: ['usage:original', 'tb:turn-2'],
    continuation: { kind: 'revise_understanding', first_round_id: comparison.round_id, parent_round_id: comparison.round_id, row_id: 'speed' },
    book_relations: [legacy.book_relations[1]],
    student_relations: [{ id: 'new-reason', relation_type: 'causes', premise: '带宽更大', conclusion: '读取更快',
      quotes: [{ turn_id: 'turn-4', text: '容量够了就看带宽。' }] }],
    rows: [{ row_id: 'clarified', book_relation_ids: ['b2'], student_relation_ids: ['new-reason'], status: 'clarified', summary: '区分了容量与带宽。' }],
  });
  const card = show(revised);
  const rounds = [{ turnId: 'turn-2', comparison }, { turnId: 'turn-4', comparison: revised }];
  await card.setProps({ turnId: 'turn-4', rounds, correctionEnabled: true });
  expect(card.get('.teach-back-support').text()).toContain('1 轮三图对照');
  expect(card.get('.teach-back-evidence').text()).toContain('容量够了就看带宽。');
  const first = card.get('[data-round-id="tb:turn-2"]');
  await first.trigger('click');
  setIntersection(true);
  await flushPromises();
  expect(first.attributes('aria-pressed')).toBe('true');
  expect(card.emitted('view-round')).toEqual([['tb:turn-2']]);
  expect(card.emitted('displayed')).toEqual([['turn-2', 'tb:turn-2']]);
  expect(card.get('.teach-back-support').text()).toBe('表达前已看过原讲解。');
  expect(card.get('.teach-back-evidence').text()).toContain('装得下以后，容量大了就更快。');
  expect(card.get('.teach-back-side-note').text()).toContain('减少读取量');
  await card.get('[data-action="dispute_interpretation"]').trigger('click');
  await card.get('textarea').setValue('重新核对首次表达。');
  await card.get('form').trigger('submit');
  expect(card.emitted('correct')).toEqual([[{
    action: 'dispute_interpretation', turnId: 'turn-2', roundId: 'tb:turn-2', rowId: 'speed', text: '重新核对首次表达。',
  }]]);
  await card.get('.teach-back-evidence button').trigger('click');
  expect(card.emitted('jump')).toEqual([['3.6.3.8', 'turn-2']]);
  await card.get('[data-round-id="tb:turn-4"]').trigger('click');
  expect(card.find('.teach-back-side-note').exists()).toBe(false);
  expect(card.get('.teach-back-evidence').text()).not.toContain('我估计快一倍。');
  await card.setProps({ currentSourceRevision: 'new-edition' });
  expect(card.get('.teach-back-evidence').text()).toContain('原书版本已变');
  expect(card.find('[data-action]').exists()).toBe(false);
});
