import { expect, test } from '@playwright/test';
import { mkdirSync, writeFileSync } from 'node:fs';
import { resolve } from 'node:path';

const host = 'http://127.0.0.1:4187';
const output = resolve('../../docs/performance/tutor-building-blocks-tb5/browser');

test('TB4 understanding shows support, stays ungraded, and preserves corrections after reopening', async ({ page }) => {
  mkdirSync(output, { recursive: true });
  const initial = await page.request.get(`${host}/fixture`).then(response => response.json());
  const calls: string[] = [];
  await page.route('**/api/**', async route => {
    const path = route.request().url().replace(/^.*\/api/, '');
    calls.push(path);
    const response = await route.fetch({ url: host + path });
    await route.fulfill({ response });
  });
  // Only the mounting shell is a fixture. TutorPanel, its control hook, API,
  // UnderstandingSpace, all responses and mutations use production paths.
  await page.route('**/tutor-tb5', route => route.fulfill({ contentType: 'text/html', body: `<!doctype html>
    <html lang="zh-CN"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"></head>
    <body><div id="app"></div><script type="module">
    import {createApp,defineComponent,h,ref,onMounted} from '/node_modules/.vite/deps/vue.js';
    import TutorPanel from '/src/components/TutorPanel.vue';
    import {useTutorControl} from '/src/useTutorControl.ts';
    import '/src/style.css';
    createApp(defineComponent({setup(){
      const tutor=useTutorControl(),open=ref(true);
      onMounted(tutor.load);
      return()=>h('main',[
        h('button',{onClick:()=>open.value=true},'打开学习详情'),
        open.value?h(TutorPanel,{state:tutor.state.value,busy:tutor.busy.value,error:tutor.error.value,pending:!!tutor.pending.value,
          sourceId:tutor.readiness.value?.source_id,label:tutor.label.value,readiness:tutor.readiness.value,readinessError:tutor.readinessError.value,
          onAction:tutor.act,onRetry:tutor.retry,onRefreshReadiness:tutor.loadReadiness,onClose:()=>open.value=false}):null
      ]);
    }})).mount('#app');
    </script></body></html>` }));
  await page.setViewportSize({ width: 1280, height: 900 });
  await page.goto('/tutor-tb5');
  const panel = page.getByRole('dialog', { name: '学习详情', exact: true });
  const guided = panel.getByRole('button', { name: /引导思考/ });
  const direct = panel.getByRole('button', { name: /直接讲解/ });
  await expect(guided).toHaveAttribute('aria-pressed', 'true');
  await expect(guided).toContainText('尝试、示范与按需帮助');
  await expect(panel.locator('.teaching-note')).toContainText('只调整本轮');
  await page.screenshot({ path: resolve(output, 'modes-desktop.png') });
  await direct.click();
  await expect(direct).toHaveAttribute('aria-pressed', 'true');
  await guided.click();
  await expect(guided).toHaveAttribute('aria-pressed', 'true');

  const checkLayout = async () => {
    const bounds = await panel.evaluate(el => {
      const rect = el.getBoundingClientRect(), body = el.querySelector('.tutor-body')!;
      return { left: rect.left, right: rect.right, width: body.clientWidth, scroll: body.scrollWidth };
    });
    expect(bounds.left).toBeGreaterThanOrEqual(0);
    expect(bounds.right).toBeLessThanOrEqual(page.viewportSize()!.width);
    expect(bounds.scroll).toBeLessThanOrEqual(bounds.width + 1);
  };
  await checkLayout();
  await page.setViewportSize({ width: 320, height: 700 });
  await checkLayout();
  await guided.scrollIntoViewIfNeeded();
  await page.screenshot({ path: resolve(output, 'modes-narrow.png') });

  await panel.getByRole('tab', { name: '我的理解', exact: true }).click();
  await panel.locator('.concept').click();
  await expect(panel.locator('.explanation p')).toHaveText(initial.interpretation);
  await expect(panel.locator('.quote blockquote')).toHaveText(initial.learner_quote);
  const conditions = panel.getByRole('region', { name: '适用目标与帮助条件' });
  await expect(conditions).toContainText(initial.prompt);
  await expect(panel.locator('.evidence')).toContainText('普通交流中的暂定理解，未评分');
  await expect(panel.locator('.evidence')).not.toContainText('第 0 次作答');
  await expect(panel.locator('.summary')).toHaveCount(0);
  await checkLayout();
  await conditions.scrollIntoViewIfNeeded();
  await page.screenshot({ path: resolve(output, 'conditions-narrow.png') });
  await page.setViewportSize({ width: 1280, height: 900 });
  await conditions.scrollIntoViewIfNeeded();
  await checkLayout();
  await page.screenshot({ path: resolve(output, 'conditions-desktop.png') });
  await panel.locator('.facts > summary').click();
  await expect(panel.locator('.facts')).toContainText('此条记录未附详细引用');

  const correction = '我只能说明这两组的结果，还需要帮助把理由接起来。';
  await panel.locator('.correction-editor > summary').click();
  await panel.getByRole('textbox', { name: '纠正这条解释' }).fill(correction);
  await panel.getByRole('button', { name: '保存纠正', exact: true }).click();
  await expect(panel.locator('.correction')).toHaveText(`你的纠正：${correction}`);
  await expect(panel.locator('.quote blockquote')).toHaveText(initial.learner_quote);
  await expect(conditions).toContainText(initial.prompt);

  expect((await page.request.post(`${host}/reopen`)).ok()).toBe(true);
  await page.reload();
  await expect(guided).toHaveAttribute('aria-pressed', 'true');
  await panel.getByRole('tab', { name: '我的理解', exact: true }).click();
  await expect(panel.locator('.concept')).toHaveCount(1);
  await panel.locator('.concept').click();
  await expect(panel.locator('.correction')).toHaveText(`你的纠正：${correction}`);
  await expect(conditions).toContainText(initial.prompt);
  await expect(panel.locator('.quote blockquote')).toHaveText(initial.learner_quote);
  await expect(panel.locator('.evidence')).toContainText('未评分');
  await expect(panel.locator('.explanation')).not.toContainText(initial.interpretation);
  await page.setViewportSize({ width: 390, height: 760 });
  await panel.locator('.correction').scrollIntoViewIfNeeded();
  await checkLayout();
  await page.screenshot({ path: resolve(output, 'corrected-reopened.png') });
  expect(calls.filter(path => path === '/tutor/mutate')).toHaveLength(2);
  expect(calls.filter(path => path === '/tutor/correct')).toHaveLength(1);
  expect(calls).not.toContain('/tutor/feedback-displayed');
  writeFileSync(resolve(output, 'checks.json'), JSON.stringify({
    evidence_id: initial.evidence_id, fixture: 'TB4 captured facts and hypothesis; scripted learner',
    viewports: [1280, 320, 390], modes_saved: true, conditions_visible: true,
    correction_reopened: true, learner_quote_preserved: true, status: 'unassessed', calls,
    model_calls: 0, human_readers: 0,
  }, null, 2));
});
