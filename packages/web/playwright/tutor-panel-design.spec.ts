import { test, expect, type Page } from '@playwright/test';
import { mkdirSync } from 'node:fs';
import { resolve } from 'node:path';
import type { TutorSession } from '../src/generated/TutorSession';
import type { TutorState } from '../src/generated/TutorState';
import type { TutorMutation } from '../src/generated/TutorMutation';

const output = resolve('../../docs/performance/tutor-session-design-20261010');
const goal = '学懂 1.3 用几个数字估算一次模型执行';
const makeSession = (id: string, intent: string, status: TutorSession['status'] = 'active'): TutorSession => ({
  id, revision: 1, user_intent: intent, status, explicit_constraints: [],
  current_focus: { interpretation: '', target_object_refs: [], capability_targets: [] },
  material_scope: [{ source_id: 'book', scope_refs: [], role: 'primary' }], default_teaching_intent: null, path_instance_ref: null, progress_ref: null,
});
async function setup(page: Page) {
  const mutations: TutorMutation[] = [];
  const state: TutorState = { control: { enabled: true, revision: 1, current_tutor_session_id: 'current' }, sessions: {
    current: makeSession('current', goal), old: makeSession('old', '理解计算与数据搬运的关系', 'paused'),
  } };
  let failNext = false;
  let record = { evidence_id: 'e1', nature: 'hypothesis', label: '估算模型执行', capability: 'explanation',
    interpretation: '你开始把计算量、内存带宽和执行耗时联系起来。接下来，可以试着解释：什么时候由计算主导，什么时候由数据搬运主导。',
    learner_quote: '估算一次执行，是不是要同时看计算量和内存带宽？',
    teaching_implication: '用一个具体的数字例子，把两种耗时放在一起比较。',
    correction: null as string | null, prompt: goal, attempt: 0, assistance_count: 0, status: 'unassessed', feedback_hidden: false, source_quotes: [] };
  await page.route('**/api/**', async route => {
    const path = new URL(route.request().url()).pathname.split('/').pop();
    const body = route.request().postDataJSON();
    let result: unknown;
    if (path === 'state') result = state;
    else if (path === 'readiness') result = { status: 'ready', source_id: 'book', reason: '可以学习', limitations: [] };
    else if (path === 'understanding') result = { rows: [], interpretations: [record], stale: false, next: null };
    else if (path === 'evidence') result = record;
    else if (path === 'correct') { record = { ...record, evidence_id: 'e2', correction: body.text }; result = { evidence_ref: 'e2' }; }
    else if (path === 'mutate') {
      mutations.push(body);
      if (failNext) { failNext = false; await route.fulfill({ status: 503, json: { error_code: 'TEST_UNAVAILABLE', category: 'unavailable', message: '保存暂不可用' } }); return; }
      const action = (body as TutorMutation).action;
      if (action.kind === 'set_mode') state.sessions[action.session_id]!.default_teaching_intent = action.mode;
      if (action.kind === 'pause' || action.kind === 'end') state.sessions[action.session_id]!.status = action.kind === 'pause' ? 'paused' : 'ended';
      if (action.kind === 'resume' || action.kind === 'start') {
        const previous = state.sessions[state.control.current_tutor_session_id ?? ''];
        if (previous && previous.status === 'active') previous.status = 'paused';
        const id = action.kind === 'start' ? 'created-' + state.control.revision : action.session_id;
        if (action.kind === 'start') state.sessions[id] = makeSession(id, action.user_intent);
        state.sessions[id]!.status = 'active'; state.control.current_tutor_session_id = id;
      }
      state.control.revision++; result = state;
    } else throw new Error('Unexpected fixture request: ' + path);
    await route.fulfill({ json: result });
  });
  await page.route('**/tutor-design', route => route.fulfill({ contentType: 'text/html', body: `<!doctype html><html lang="zh-CN"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"></head><body><div id="app"></div><script type="module">
    import {createApp,defineComponent,h,ref,onMounted} from '/node_modules/.vite/deps/vue.js';
    import TutorPanel from '/src/components/TutorPanel.vue';
    import {useTutorControl} from '/src/useTutorControl.ts';
    import '/src/style.css';
    createApp(defineComponent({setup(){
      const tutor=useTutorControl(), open=ref(false), draft=ref('未发送的阅读问题');
      onMounted(tutor.load);
      return()=>h('main',{style:'padding:32px;max-width:900px;margin:auto'},[
        h('h1','AI Infra · 阅读现场'),h('p','第 1.3 节 · 用几个数字估算一次模型执行'),
        h('textarea',{'aria-label':'未发送问题',value:draft.value,onInput:e=>draft.value=e.target.value}),
        h('button',{onClick:()=>open.value=true},'打开学习会话'),
        open.value?h(TutorPanel,{state:tutor.state.value,busy:tutor.busy.value,error:tutor.error.value,pending:!!tutor.pending.value,
          sourceId:tutor.readiness.value?.source_id,label:tutor.label.value,readiness:tutor.readiness.value,readinessError:tutor.readinessError.value,
          onAction:tutor.act,onRetry:tutor.retry,onRefreshReadiness:tutor.loadReadiness,onClose:()=>open.value=false}):null
      ]);
    }})).mount('#app');
  </script></body></html>` }));
  await page.goto('/tutor-design');
  await page.getByRole('button', { name: '打开学习会话' }).click();
  await expect(page.locator('.session-goal h3')).toHaveText(goal);
  return { mutations, failNextMutation: () => { failNext = true; } };
}

for (const width of [1280, 390, 320]) {
  test(`learning dialog layout and navigation at ${width}px`, async ({ page }) => {
    await page.setViewportSize({ width, height: 850 });
    await setup(page);
    mkdirSync(output, { recursive: true });
    const panel = page.getByRole('dialog', { name: '学习详情', exact: true });
    const checkLayout = async () => {
      const bounds = await panel.evaluate(el => {
        const box = el.getBoundingClientRect();
        return { left: box.left, right: box.right, bottom: box.bottom, width: el.clientWidth, scroll: el.scrollWidth,
          bodyWidth: el.querySelector('.tutor-body')!.clientWidth, bodyScroll: el.querySelector('.tutor-body')!.scrollWidth };
      });
      expect(bounds.left).toBeGreaterThanOrEqual(0); expect(bounds.right).toBeLessThanOrEqual(width);
      expect(bounds.bottom).toBeLessThanOrEqual(page.viewportSize()!.height);
      expect(bounds.scroll).toBeLessThanOrEqual(bounds.width + 1);
      expect(bounds.bodyScroll).toBeLessThanOrEqual(bounds.bodyWidth + 1);
    };
    await checkLayout();
    await page.screenshot({ path: resolve(output, `current-${width}.png`) });
    await panel.getByRole('button', { name: '新学习', exact: true }).click();
    await panel.getByRole('button', { name: '带我推导一遍' }).click();
    await expect(panel.getByRole('button', { name: '开始学习', exact: true })).toBeEnabled();
    await checkLayout();
    await page.screenshot({ path: resolve(output, `new-${width}.png`) });
    await panel.getByRole('tab', { name: '我的理解', exact: true }).click();
    await panel.locator('.concept').click();
    await checkLayout();
    await page.screenshot({ path: resolve(output, `understanding-${width}.png`) });
    await panel.locator('.correction-editor > summary').click();
    await panel.getByRole('textbox', { name: '纠正这条解释' }).fill('这里是尚未提交的纠正。');
    await panel.getByRole('button', { name: '新学习', exact: true }).click();
    await expect(panel.getByRole('textbox', { name: '你的学习目标' })).toHaveValue(/一步步推导/);
    await panel.getByRole('tab', { name: '我的理解', exact: true }).click();
    await expect(panel.getByRole('textbox', { name: '纠正这条解释' })).toHaveValue('这里是尚未提交的纠正。');
    await panel.getByRole('tab', { name: '当前学习', exact: true }).click();
    await page.setViewportSize({ width, height: 500 });
    await checkLayout();
    await expect(panel.getByRole('button', { name: '继续学习', exact: true })).toBeInViewport();
    await panel.getByRole('button', { name: '结束这次学习' }).click();
    await expect(panel.getByRole('group', { name: '确认结束学习' })).toBeInViewport();
    await panel.getByRole('button', { name: '取消', exact: true }).click();
    await panel.getByRole('button', { name: '刷新学习状态' }).focus();
    await page.keyboard.press('Shift+Tab');
    await expect(panel.getByRole('button', { name: '继续学习', exact: true })).toBeFocused();
    await page.keyboard.press('Tab');
    await expect(panel.getByRole('button', { name: '刷新学习状态' })).toBeFocused();
    await page.keyboard.press('Escape');
    await expect(panel).toHaveCount(0);
    await expect(page.getByRole('button', { name: '打开学习会话' })).toBeFocused();
    await expect(page.getByRole('textbox', { name: '未发送问题' })).toHaveValue('未发送的阅读问题');
  });
}

test('real control hook retries unchanged mutations, restores history and saves new goals', async ({ page }) => {
  const fixture = await setup(page);
  const panel = page.getByRole('dialog', { name: '学习详情', exact: true });
  fixture.failNextMutation();
  const direct = panel.getByRole('button', { name: /直接讲解/ });
  await direct.click();
  await expect(panel.getByRole('alert')).toContainText('保存暂不可用');
  await expect(direct).toHaveAttribute('aria-pressed', 'false');
  await expect(direct).toBeDisabled();
  await panel.getByRole('button', { name: '重试', exact: true }).click();
  await expect(direct).toHaveAttribute('aria-pressed', 'true');
  expect(fixture.mutations[0]).toEqual(fixture.mutations[1]);
  await panel.getByRole('button', { name: '暂停', exact: true }).click();
  await expect(panel.locator('.session-status')).toHaveText('已暂停');
  await panel.getByRole('tab', { name: '历史会话' }).click();
  await expect(panel.locator('.history-session')).toHaveCount(2);
  await panel.getByRole('button', { name: /理解计算与数据搬运的关系/ }).click();
  const before = fixture.mutations.length;
  await expect(panel.locator('.session-goal h3')).toHaveText('理解计算与数据搬运的关系');
  expect(fixture.mutations).toHaveLength(before);
  await panel.getByRole('button', { name: '继续学习', exact: true }).click();
  await expect(panel.locator('.session-status')).toHaveText('进行中');
  expect(fixture.mutations.at(-1)?.action).toEqual({ kind: 'resume', session_id: 'old' });
  await panel.getByRole('button', { name: '新学习', exact: true }).click();
  await panel.getByRole('textbox', { name: '你的学习目标' }).fill('比较两种执行耗时');
  await panel.getByRole('button', { name: '开始学习', exact: true }).click();
  await expect(panel.locator('.session-goal h3')).toHaveText('比较两种执行耗时');
  await panel.getByRole('tab', { name: '我的理解', exact: true }).click();
  await panel.locator('.concept').click();
  await panel.locator('.correction-editor > summary').click();
  await panel.getByRole('textbox', { name: '纠正这条解释' }).fill('我还不能独立估算，想再做一个例子。');
  await panel.getByRole('button', { name: '保存纠正' }).click();
  await expect(panel.locator('.correction')).toContainText('我还不能独立估算');
  await expect(panel.locator('.quote blockquote')).toHaveText('估算一次执行，是不是要同时看计算量和内存带宽？');
});

