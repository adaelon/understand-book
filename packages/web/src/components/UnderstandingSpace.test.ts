// @vitest-environment happy-dom
import { mount, flushPromises, type VueWrapper } from '@vue/test-utils';
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import UnderstandingSpace from './UnderstandingSpace.vue';
import type { LearningEvidenceView, UnderstandingRow } from '../api';

let intersection: IntersectionObserverCallback;
let observed: Element[];
const button = (w: VueWrapper, name: string) => w.findAll('button').find(b => b.text() === name)!;
const openFacts = async (w: VueWrapper) => {
  (w.get('details.facts').element as HTMLDetailsElement).open = true;
  await w.get('details.facts').trigger('toggle');
  await flushPromises();
};
const evidence = (id = 'hypothesis', patch: Partial<LearningEvidenceView> = {}): LearningEvidenceView => ({
  evidence_id: id, nature: 'hypothesis', label: '平均速度', capability: 'explanation', prompt: '理解总时间的含义',
  interpretation: '可能需要理解分母', teaching_implication: '从整个过程解释', learner_quote: '为什么用总时间？',
  correction: null, status: 'unassessed', attempt: 0, assistance_count: 0, feedback_hidden: false, source_quotes: [], ...patch,
});
const row = (patch: Partial<UnderstandingRow> = {}): UnderstandingRow => ({
  object_id: 'speed', object_revision: 1, label: '平均速度', capability: 'explanation', historical: false, state: 'supported',
  independent_support: 1, assisted_support: 1, revised_support: 1, partial: 1, difficulty: 1, uncertain: 1,
  evidence_count: 3, evidence_refs: ['independent', 'assisted', 'revised'], ...patch,
});
function setup(options: { rows?: UnderstandingRow[]; interpretations?: LearningEvidenceView[];
  records?: Record<string, LearningEvidenceView>; handler?: (path: string, body: any) => Response | undefined | Promise<Response | undefined> } = {}) {
  let interpretations = options.interpretations ?? [evidence()];
  const records: Record<string, LearningEvidenceView> = { hypothesis: evidence(), ...options.records };
  const fetch = vi.fn(async (url: string, init: RequestInit) => {
    const path = url.split('/').pop()!; const body = JSON.parse(init.body as string);
    const response = await options.handler?.(path, body); if (response) return response;
    if (path === 'understanding') return Response.json({ rows: options.rows ?? [], interpretations, next: null, stale: false });
    if (path === 'evidence') return Response.json(records[body.evidence_ref]);
    if (path === 'correct') {
      records.corrected = evidence('corrected', { ...records[body.evidence_ref], evidence_id: 'corrected', interpretation: '原解释已被用户纠正，等待新的表现依据', correction: body.text });
      interpretations = [records.corrected]; return Response.json({ evidence_ref: 'corrected' });
    }
    if (path === 'feedback-displayed') return Response.json({});
    throw new Error(`Unexpected request ${path}`);
  });
  vi.stubGlobal('fetch', fetch);
  return { w: mount(UnderstandingSpace, { attachTo: document.body }), fetch };
}
beforeEach(() => {
  observed = [];
  vi.stubGlobal('IntersectionObserver', class {
    constructor(callback: IntersectionObserverCallback) { intersection = callback; }
    observe(target: Element) { observed.push(target); }
    unobserve() {}
    disconnect() {}
  });
});
afterEach(() => { vi.unstubAllGlobals(); document.body.innerHTML = ''; });

it('opens one hypothesis with real words and corrects it without inventing a performance', async () => {
  const { w, fetch } = setup();
  try {
    await flushPromises();
    expect(w.find('[aria-label="单项理解"]').exists()).toBe(false);
    await w.get('.concept').trigger('click'); await flushPromises();
    expect(w.get('[aria-label="单项理解"]').text()).toContain('为什么用总时间？');
    expect(w.text()).not.toContain('第 0 次作答');
    expect(w.get('.listing').isVisible()).toBe(false);
    await w.get('textarea').setValue('我已理解分母');
    await w.get('form').trigger('submit'); await flushPromises();
    expect(w.get('[aria-label="表现依据"]').text()).toContain('你的纠正：我已理解分母');
    expect(w.text()).not.toContain('可能需要理解分母');
    const request = fetch.mock.calls.find(([url]) => url.endsWith('/correct'))!;
    expect(JSON.parse(request[1].body as string)).toMatchObject({ evidence_ref: 'hypothesis', text: '我已理解分母' });
    await button(w, '← 返回概念列表').trigger('click');
    expect(w.get('.listing').isVisible()).toBe(true);
    expect(document.activeElement).toBe(w.get('.listing').element);
  } finally { w.unmount(); }
});

it('keeps unknown objects unknown and returns keyboard focus without reading invented evidence', async () => {
  const { w, fetch } = setup({ interpretations: [], rows: [row({ evidence_count: 0, evidence_refs: [], state: 'unknown', capability: '' })] });
  try {
    await flushPromises(); const origin = w.get('.concept');
    await origin.trigger('click'); await flushPromises();
    expect(w.get('[aria-label="单项理解"]').text()).toContain('尚未观察能力：未知');
    expect(w.find('textarea').exists()).toBe(false);
    expect(fetch.mock.calls.some(([url]) => url.endsWith('/evidence'))).toBe(false);
    await button(w, '← 返回概念列表').trigger('click');
    expect(document.activeElement).toBe(origin.element);
  } finally { w.unmount(); }
});

it('preserves independent, assisted, revised and historical evidence and pages history by the original object', async () => {
  const historyRequests: any[] = [];
  const { w } = setup({ interpretations: [], rows: [row({ historical: true, object_revision: 2 })], records: {
    independent: evidence('independent', { nature: 'performance', attempt: 1, status: 'correct', learner_quote: '总距离除以总时间' }),
    assisted: evidence('assisted', { nature: 'performance', attempt: 1, assistance_count: 1 }),
    revised: evidence('revised', { nature: 'performance', attempt: 2, status: 'partial' }),
  }, handler(path, body) {
    if (path === 'evidence-list') { historyRequests.push(body); return Response.json(body.before ? { refs: ['revised'], next: null } : { refs: ['independent', 'assisted'], next: 10 }); }
  } });
  try {
    await flushPromises(); await w.get('.concept').trigger('click'); await flushPromises();
    expect(w.text()).toContain('历史版本 · 对象版本 2');
    expect(w.text()).toContain('独立符合标准 1 次 · 帮助后符合 1 次 · 改答后符合 1 次');
    expect(w.text()).toContain('未记录到事先展示的帮助');
    await button(w, '查看依据 2').trigger('click'); await flushPromises();
    expect(w.text()).toContain('帮助后作答，回答前已展示帮助');
    await w.get('textarea').setValue('历史解释的草稿');
    await button(w, '查看依据 3').trigger('click'); await flushPromises();
    expect(w.text()).toContain('第 2 次作答'); expect(w.text()).toContain('改答后记录');
    expect(w.text()).toContain('部分符合当时标准');
    await button(w, '查看完整解释历史').trigger('click'); await flushPromises();
    await button(w, '更早的解释').trigger('click'); await flushPromises();
    expect(historyRequests).toEqual([
      { object_id: 'speed', object_revision: 2, capability: 'explanation' },
      { object_id: 'speed', object_revision: 2, capability: 'explanation', before: 10 },
    ]);
    expect(w.findAll('[aria-label="解释历史"] button')).toHaveLength(3);
    await button(w, '查看历史依据 2').trigger('click'); await flushPromises();
    expect(w.get('textarea').element.value).toBe('历史解释的草稿');
    expect(w.text()).toContain('历史解释 · 系统解释');
  } finally { w.unmount(); }
});

it('records feedback only after details are expanded and visible; hidden feedback stays hidden', async () => {
  const { w, fetch } = setup({ interpretations: [evidence('shown'), evidence('hidden')], records: {
    shown: evidence('shown', { nature: 'performance', attempt: 1, assessment_ref: 'action', source_quotes: [{ reason: '比较总量', response_quote: '总时间', sources: [{ quote: '总距离与总时间之比' }] }] }),
    hidden: evidence('hidden', { nature: 'performance', feedback_hidden: true, assessment_ref: 'private', source_quotes: null }),
  } });
  try {
    await flushPromises(); await w.findAll('.concept')[0].trigger('click'); await flushPromises();
    expect(observed).toHaveLength(0); expect(w.text()).not.toContain('总距离与总时间之比');
    await openFacts(w); expect(observed).toHaveLength(1);
    expect(w.text()).toContain('作答片段：总时间');
    intersection([{ isIntersecting: true, target: observed[0] } as IntersectionObserverEntry], {} as IntersectionObserver);
    await flushPromises();
    expect(fetch.mock.calls.filter(([url]) => url.endsWith('/feedback-displayed'))).toHaveLength(1);
    await button(w, '← 返回概念列表').trigger('click');
    await w.findAll('.concept')[1].trigger('click'); await flushPromises(); await openFacts(w);
    expect(w.text()).toContain('详细判定依据会在本活动请求直接讲解后揭示');
    expect(observed).toHaveLength(1);
  } finally { w.unmount(); }
});

it('retries an evidence read instead of reloading the list', async () => {
  let failed = false;
  const { w, fetch } = setup({ handler(path) { if (path === 'evidence' && !failed) { failed = true; return Response.json({ error: '暂不可用' }, { status: 503 }); } } });
  try {
    await flushPromises(); await w.get('.concept').trigger('click'); await flushPromises();
    expect(w.get('[role="alert"]').text()).toContain('依据读取失败');
    await button(w, '重试').trigger('click'); await flushPromises();
    expect(w.get('[aria-label="表现依据"]').text()).toContain('为什么用总时间？');
    expect(fetch.mock.calls.filter(([url]) => url.endsWith('/understanding'))).toHaveLength(1);
  } finally { w.unmount(); }
});

it('reuses a pending correction identity and only rereads after a confirmed write', async () => {
  let commands = 0; let reads = 0;
  const { w, fetch } = setup({ handler(path, body) {
    if (path === 'correct' && ++commands === 1) return Response.json({ error: '连接中断' }, { status: 503 });
    if (path === 'evidence' && body.evidence_ref === 'corrected' && ++reads === 1) return Response.json({ error: '刷新失败' }, { status: 503 });
  } });
  try {
    await flushPromises(); await w.get('.concept').trigger('click'); await flushPromises();
    await w.get('textarea').setValue('我已理解分母'); await w.get('form').trigger('submit'); await flushPromises();
    expect(w.get('textarea').element.value).toBe('我已理解分母');
    expect(button(w, '← 返回概念列表').attributes('disabled')).toBeDefined();
    await button(w, '重试').trigger('click'); await flushPromises();
    await button(w, '重试').trigger('click'); await flushPromises();
    const requests = fetch.mock.calls.filter(([url]) => url.endsWith('/correct')).map(([, init]) => JSON.parse(init.body as string));
    expect(requests).toHaveLength(2); expect(requests[0]).toEqual(requests[1]);
    expect(w.get('[aria-label="表现依据"]').text()).toContain('你的纠正：我已理解分母');
    expect(w.find('[role="alert"]').exists()).toBe(false);
  } finally { w.unmount(); }
});

it('keeps loaded object pages after correction and refreshes their original row', async () => {
  let corrected = false; const afters: number[] = [];
  const { w } = setup({ interpretations: [], records: { independent: evidence('independent', { nature: 'performance', attempt: 1 }) }, handler(path, body) {
    if (path === 'correct') corrected = true;
    if (path === 'understanding') {
      afters.push(body.after);
      return Response.json({ interpretations: [], stale: false, next: body.after ? null : 12, rows: body.after ? [row({ evidence_refs: corrected ? ['corrected'] : ['independent'], uncertain: corrected ? 2 : 1 })] : [row({ object_id: 'unknown', label: '另一概念', state: 'unknown', evidence_refs: [], evidence_count: 0 })] });
    }
  } });
  try {
    await flushPromises(); await button(w, '查看更多对象').trigger('click'); await flushPromises();
    await w.findAll('.concept')[1].trigger('click'); await flushPromises();
    await w.get('textarea').setValue('我借助了例题'); await w.get('form').trigger('submit'); await flushPromises();
    expect(afters).toEqual([0, 12, 0, 12]);
    expect(w.get('[aria-label="能力依据概况"]').text()).toContain('有 2 条');
    await button(w, '← 返回概念列表').trigger('click');
    expect(w.findAll('.concept')).toHaveLength(2);
  } finally { w.unmount(); }
});

it('keeps correction retry available when a displayed-feedback receipt fails later', async () => {
  let failFeedback!: (response: Response) => void;
  let feedbackCalls = 0; let corrections = 0;
  const { w, fetch } = setup({ records: { hypothesis: evidence('hypothesis', {
    nature: 'performance', attempt: 1, assessment_ref: 'action', source_quotes: [{ reason: '依据', quote: '总时间' }],
  }) }, handler(path) {
    if (path === 'feedback-displayed' && ++feedbackCalls === 1) return new Promise(resolve => { failFeedback = resolve; });
    if (path === 'correct' && ++corrections === 1) return Response.json({ error: '纠正断线' }, { status: 503 });
  } });
  try {
    await flushPromises(); await w.get('.concept').trigger('click'); await flushPromises(); await openFacts(w);
    intersection([{ isIntersecting: true, target: observed[0] } as IntersectionObserverEntry], {} as IntersectionObserver);
    await w.get('textarea').setValue('这是我自己的解释'); await w.get('form').trigger('submit'); await flushPromises();
    failFeedback(Response.json({ error: '回执断线' }, { status: 503 })); await flushPromises();
    expect(w.text()).toContain('纠正提交或刷新未完成');
    expect(w.text()).toContain('反馈展示记录未保存');
    await button(w, '重试').trigger('click'); await flushPromises();
    expect(w.get('[aria-label="表现依据"]').text()).toContain('你的纠正：这是我自己的解释');
    await button(w, '重试反馈记录').trigger('click'); await flushPromises();
    expect(w.find('[role="alert"]').exists()).toBe(false);
    expect(fetch.mock.calls.filter(([url]) => url.endsWith('/correct'))).toHaveLength(2);
    expect(feedbackCalls).toBe(2);
  } finally { w.unmount(); }
});
