import { test, expect, type Page } from '@playwright/test';
import { mkdirSync } from 'node:fs';
import { resolve } from 'node:path';

const output = resolve('../../docs/performance/reading-share-rs8-20261010');
const longExplanation = '平均速度把整个过程的总距离与总时间对应起来，不能在耗时不同的路段直接平均速度。\n\n' + '先比较每一段所用的时间，再说明为什么分母应当是总时间。'.repeat(18) + '\n解释末尾：还要考虑总时间为零时的适用边界。';
const longQuote = '我已经理解总时间是分母，我的问题是两个路段的时间不同，能否直接平均它们的速度？\n' + '我尝试用一小时和两小时的行程比较结果。'.repeat(12) + '\n原话末尾：请用整个过程说明。';
async function setup(page: Page) {
  const requests: { path: string; body: any }[] = [];
  const record = (id: string, extra = {}) => ({ evidence_id: id, nature: 'performance', label: '平均速度', capability: 'explanation',
    prompt: '比较不同用时的两段路程，并解释总距离与总时间的关系。', learner_quote: '总距离除以总时间', interpretation: '这次作答说明了整个过程的比例关系。',
    correction: null, status: 'correct', assistance_count: 0, attempt: 1, feedback_hidden: false, assessment_ref: 'action-' + id,
    source_quotes: [{ reason: '回答引用了整个过程的量。', response_quote: '总距离除以总时间', sources: [{ quote: '平均速度是总路程与总时间之比。' }] }], ...extra });
  const records: Record<string, any> = {
    hypothesis: record('hypothesis', { nature: 'hypothesis', attempt: 0, status: 'unassessed', assessment_ref: null, interpretation: longExplanation, learner_quote: longQuote, source_quotes: [] }),
    independent: record('independent'), assisted: record('assisted', { assistance_count: 1 }),
    revised: record('revised', { attempt: 2, status: 'partial' }),
    hidden: record('hidden', { feedback_hidden: true, source_quotes: null }),
  };
  let current = 'hypothesis';
  const object = { object_id: 'speed', object_revision: 1, label: '平均速度', capability: 'explanation', state: 'supported', historical: false,
    independent_support: 1, assisted_support: 1, revised_support: 1, partial: 1, difficulty: 0, uncertain: 0, evidence_count: 4, evidence_refs: ['independent', 'assisted', 'revised', 'hidden'] };
  await page.route('**/api/**', async route => {
    const path = new URL(route.request().url()).pathname.split('/').pop()!;
    const body = route.request().postDataJSON(); requests.push({ path, body });
    let data: any;
    if (path === 'understanding') data = { rows: [object, { ...object, object_id: 'acceleration', label: '加速度', state: 'unknown', evidence_count: 0, evidence_refs: [], capability: '' }], interpretations: [records[current]], next: null, stale: false };
    else if (path === 'evidence') data = records[body.evidence_ref];
    else if (path === 'correct') { records.corrected = { ...records[body.evidence_ref], evidence_id: 'corrected', correction: body.text, interpretation: '原解释已被用户纠正，等待新的表现依据', status: 'unassessed' }; current = 'corrected'; data = { evidence_ref: current }; }
    else if (path === 'evidence-list') data = body.before ? { refs: ['revised'], next: null } : { refs: ['independent', 'assisted'], next: 10 };
    else if (path === 'feedback-displayed') data = {};
    else throw new Error('Unexpected request: ' + path);
    await route.fulfill({ json: data });
  });
  await page.route('**/rs8-test', route => route.fulfill({ contentType: 'text/html', body: `<!doctype html><html><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"></head><body><div id="app"></div><script type="module">
  import {createApp,defineComponent,h,ref} from '/node_modules/.vite/deps/vue.js';
  import TutorPanel from '/src/components/TutorPanel.vue';import '/src/style.css';
  createApp(defineComponent({setup(){const open=ref(false),draft=ref('未发送的问题');
  return()=>h('main',{style:'padding:20px'},[h('h1','原阅读现场'),h('textarea',{'aria-label':'未发送问题',value:draft.value,onInput:e=>draft.value=e.target.value}),h('button',{onClick:()=>open.value=true},'打开学习会话'),h('div',{style:'height:1600px'},'正文位置保持'),open.value?h(TutorPanel,{state:{control:{enabled:true,current_tutor_session_id:null,revision:1},sessions:{}},busy:false,error:'',pending:false,sourceId:'book',label:'Tutor 已开启',readiness:{status:'ready',reason:'可用',limitations:[]},onClose:()=>open.value=false}):null]);}})).mount('#app');
  </script></body></html>` }));
  await page.goto('/rs8-test');
  await page.getByRole('button', { name: '打开学习会话' }).click();
  await expect(page.locator('.concept')).toHaveCount(3);
  return requests;
}

for (const width of [320, 390, 1280]) {
  test(`RS8 ${width}px single reading keeps long text, correction, list and background`, async ({ page }) => {
    mkdirSync(output, { recursive: true });
    await page.setViewportSize({ width, height: 850 });
    const requests = await setup(page);
    await page.locator('.understanding-space').scrollIntoViewIfNeeded();
    await page.screenshot({ path: resolve(output, `list-${width}.png`) });
    await page.locator('[aria-label="当前理解解释"] .concept').click();
    const detail = page.getByRole('region', { name: '单项理解', exact: true });
    await expect(detail).toBeVisible();
    await expect(page.locator('.listing')).toBeHidden();
    await expect(detail.locator('.explanation p')).toHaveText(longExplanation);
    await expect(detail.locator('.quote blockquote')).toHaveText(longQuote);
    await expect(detail).not.toContainText('第 0 次作答');
    await detail.locator('.reading-title').scrollIntoViewIfNeeded();
    await page.screenshot({ path: resolve(output, `reading-${width}.png`) });
    const overflowing = await page.locator('.tutor-panel').evaluate(el => ({ width: el.clientWidth, scroll: el.scrollWidth }));
    expect(overflowing.scroll).toBeLessThanOrEqual(overflowing.width + 1);
    await detail.locator('.quote').scrollIntoViewIfNeeded();
    await expect(detail.locator('.quote')).toBeVisible();
    await detail.getByText('查看详细依据', { exact: true }).click();
    await expect(detail).toContainText('此条记录未附详细引用');
    if (width < 500) await page.setViewportSize({ width, height: 500 });
    await detail.getByRole('textbox', { name: '纠正这条解释' }).fill('我理解分母，请解释适用边界。');
    await detail.getByRole('button', { name: '保存纠正' }).click();
    await expect(detail).toContainText('你的纠正：我理解分母，请解释适用边界。');
    expect(requests.filter(r => r.path === 'correct')).toHaveLength(1);
    expect(requests.filter(r => r.path === 'feedback-displayed')).toHaveLength(0);
    await detail.getByRole('button', { name: '← 返回概念列表' }).click();
    await expect(page.locator('.listing')).toBeVisible();
    await page.getByRole('button', { name: '加速度 尚未观察能力 · 未知' }).click();
    await expect(detail).toContainText('尚未观察能力：未知');
    await expect(detail.locator('textarea')).toHaveCount(0);
    await page.getByRole('button', { name: '关闭', exact: true }).click();
    await expect(page.getByRole('textbox', { name: '未发送问题' })).toHaveValue('未发送的问题');
    expect(requests.every(r => ['understanding', 'evidence', 'correct'].includes(r.path))).toBe(true);
  });
}

test('RS8 facts disclosure and original history preserve help conditions and hidden feedback', async ({ page }) => {
  const requests = await setup(page);
  await page.locator('[aria-label="概念与能力"] .concept').first().click();
  const detail = page.getByRole('region', { name: '单项理解', exact: true });
  await expect(detail).toContainText('独立符合标准 1 次 · 帮助后符合 1 次 · 改答后符合 1 次');
  expect(requests.filter(r => r.path === 'feedback-displayed')).toHaveLength(0);
  await detail.getByText('查看详细依据', { exact: true }).click();
  await detail.getByText('平均速度是总路程与总时间之比。', { exact: true }).scrollIntoViewIfNeeded();
  await expect.poll(() => requests.filter(r => r.path === 'feedback-displayed').length).toBe(1);
  await detail.getByText('依据与解释历史', { exact: true }).click();
  await detail.getByRole('button', { name: '查看依据 2', exact: true }).click();
  await expect(detail).toContainText('帮助后作答，回答前已展示帮助');
  await expect(detail).toContainText('历史解释 · 系统解释');
  await detail.getByRole('button', { name: '查看依据 3', exact: true }).click();
  await expect(detail).toContainText('第 2 次作答');
  await expect(detail).toContainText('改答后记录');
  await detail.getByRole('button', { name: '查看依据 4', exact: true }).click();
  await detail.getByText('查看详细依据', { exact: true }).click();
  await expect(detail).toContainText('详细判定依据会在本活动请求直接讲解后揭示');
  expect(requests.filter(r => r.path === 'feedback-displayed')).toHaveLength(1);
  await detail.getByRole('button', { name: '查看完整解释历史' }).click();
  await detail.getByRole('button', { name: '更早的解释' }).click();
  await detail.getByRole('button', { name: '查看历史依据 3', exact: true }).click();
  await expect(detail).toContainText('历史解释 · 系统解释');
  expect(requests.filter(r => r.path === 'evidence-list').map(r => r.body)).toEqual([
    { object_id: 'speed', object_revision: 1, capability: 'explanation' },
    { object_id: 'speed', object_revision: 1, capability: 'explanation', before: 10 },
  ]);
});
