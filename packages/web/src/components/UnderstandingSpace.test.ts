// @vitest-environment happy-dom
import { mount, flushPromises } from '@vue/test-utils';
import { afterEach, expect, it, vi } from 'vitest';
import UnderstandingSpace from './UnderstandingSpace.vue';

afterEach(() => vi.unstubAllGlobals());

it('shows source-only hypotheses and corrections without inventing an attempt or object', async () => {
  vi.stubGlobal('IntersectionObserver', class { observe() {} disconnect() {} });
  let corrected = false;
  const evidence = () => ({ evidence_id: corrected ? 'new' : 'old', nature: 'hypothesis', label: '平均速度',
    interpretation: corrected ? '原解释已被用户纠正' : '可能需要理解分母', teaching_implication: '从整个过程解释',
    learner_quote: '为什么用总时间？', correction: corrected ? '我已理解分母' : null, attempt: 0, assistance_count: 0, source_quotes: [] });
  const fetch = vi.fn(async (url: string, init: RequestInit) => {
    if (url.endsWith('/tutor/correct')) { expect(JSON.parse(init.body as string).text).toBe('我已理解分母'); corrected = true; return Response.json({ evidence_ref: 'new' }); }
    if (url.endsWith('/tutor/evidence')) return Response.json(evidence());
    if (url.endsWith('/tutor/understanding')) return Response.json({ rows: [], interpretations: [evidence()], stale: false, next: null });
    throw new Error(`Unexpected request ${url}`);
  });
  vi.stubGlobal('fetch', fetch);
  const w = mount(UnderstandingSpace);
  try {
    await flushPromises();
    expect(w.text()).toContain('暂定理解');
    expect(w.text()).not.toContain('尚无理解记录');
    await w.findAll('button').find(b => b.text() === '查看事实与修订')!.trigger('click');
    await flushPromises();
    expect(w.text()).toContain('为什么用总时间？');
    expect(w.text()).not.toContain('第 0 次作答');
    await w.get('textarea').setValue('我已理解分母');
    await w.findAll('button').find(b => b.text() === '保存纠正')!.trigger('click');
    await flushPromises();
    expect(w.text()).toContain('你的纠正：我已理解分母');
    expect(w.text()).not.toContain('可能需要理解分母');
  } finally { w.unmount(); }
});
