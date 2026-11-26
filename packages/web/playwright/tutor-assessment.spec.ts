import { expect, test } from '@playwright/test';
import { scenario, setup } from './fixtures/tutor-continuation';

test('a restored open question refreshes its assessment before the answer turn ends', async ({ page }) => {
  const fixture = await setup(page, 'markdown', true, true, true);
  const action = 'action:grain-answer';
  const turn = 'pending-assessment';
  fixture.turns[0].domain = { teaching: ['question'] };
  fixture.turns.push({ turn_id: turn, user: '因为路越远，途中需要的粮食越多。', user_turn_ordinal: 2,
    status: 'pending_assistant', teaching_ref: action, effect_labels: [] });
  let graded = false;
  let releaseGrade!: () => void;
  const grade = new Promise<void>(resolve => { releaseGrade = resolve; });
  const step = { step_id: 1, kind: 'tool', name: 'tutor.step', label: '评定此次作答', status: 'succeeded' };
  await page.route('**/api/tutor/activities', route => route.fulfill({ json: { activities:
    route.request().postDataJSON().turn_id === 'tc0-answer' ? [{
      delivery_ref: 'grain', attempted: true, status: 'active', help_seen: [],
      move: { move_id: 'grain', prompt: '为什么长途需要提前备粮？', actions: ['revise'] },
      assessment: { status: graded ? 'correct' : 'unassessed', action_ref: action, attempt: 1, assistance_count: 1 },
    }] : [] } }));
  await page.route('**/api/agent/runs/' + turn, route => route.fulfill({ json: {
    descriptor: { book_id: scenario.book_id, session_id: 'tc0-chat', turn_id: turn },
    execution_state: 'running', persistence_state: 'pending', last_seq: 0, activities: [],
  } }));
  await page.route('**/api/agent/runs/' + turn + '/events*', async route => {
    await grade;
    const event = { type: 'tool.finished', turn_id: turn, seq: 1, elapsed_ms: 100, payload: step };
    await route.fulfill({ contentType: 'text/event-stream', body: 'event: tool.finished\ndata: ' + JSON.stringify(event) + '\n\n' });
  });
  try {
    await page.reload();
    const card = page.locator('[data-delivery-ref="grain"]').first();
    await expect(card).toContainText('回答已记录，尚未评分。');
    graded = true;
    releaseGrade();
    await expect(card).toContainText('本次回答符合标准');
    await expect(card).toContainText('本次回答前已展示帮助。');
    expect(fixture.turns[1].status).toBe('pending_assistant');
  } finally { releaseGrade(); }
});

test('frozen grading, assistance and corrected understanding survive reopening', async ({ page }) => {
  let failCorrection = true;
  await page.route('**/api/**', async route => {
    const request = route.request();
    if (request.url().endsWith('/tutor/correct') && failCorrection) {
      failCorrection = false;
      await route.fulfill({ status:503, json:{ error_code:'TEST_UNAVAILABLE', category:'unavailable', message:'保存暂不可用' } }); return;
    }
    const response = await route.fetch({ url:request.url().replace(/^.*\/api/,'http://127.0.0.1:4175') });
    await route.fulfill({ response });
  });
  await page.goto('/agent-presentation-visual.html?tutor');
  const activity = page.locator('.agent-presentation .tutor-activities article').filter({ hasText:'Observe one:' });
  await activity.scrollIntoViewIfNeeded();
  const facts = async () => (await page.request.get('http://127.0.0.1:4175/facts').then(r => r.json())).map((row:any) => row[1]);
  await expect.poll(async () => (await facts()).filter((f:any) => f.kind === 'displayed').length).toBeGreaterThan(0);
  await activity.getByRole('textbox').fill('A');
  await activity.getByRole('button',{ name:'提交回答',exact:true }).click();
  await expect(activity.getByText('本次回答部分符合标准')).toBeVisible();
  await activity.getByRole('button',{ name:'直接讲解',exact:true }).click();
  await expect(activity.getByText('Use total distance divided by total time.')).toBeVisible();
  await expect.poll(async () => (await facts()).filter((f:any) => f.kind === 'help_displayed').length).toBe(1);
  await activity.getByRole('textbox').fill('["A","B"]');
  await activity.getByRole('button',{ name:'改答',exact:true }).click();
  await expect(activity.getByText('本次回答符合标准',{ exact:true })).toBeVisible();
  await expect(activity.getByText('本次回答前已展示帮助。')).toBeVisible();
  await page.getByRole('button',{ name:'学习详情' }).first().click();
  await page.getByRole('tab', { name: '我的理解', exact: true }).click();
  const space = page.getByRole('region',{ name:'我的理解' });
  await space.locator('[aria-label="概念与能力"] .concept').first().click();
  await expect(space).toContainText('帮助后符合 1 次');
  await space.getByText('依据与解释历史', { exact: true }).click();
  await space.getByRole('button',{ name:'查看依据 1',exact:true }).click();
  await expect(space.getByRole('blockquote')).toHaveText('["A","B"]');
  await space.locator('.correction-editor > summary').click();
  await space.getByLabel('纠正这条解释').fill('我照着刚才的讲解复述，还不能独立解释。');
  await space.getByRole('button',{ name:'保存纠正' }).click();
  await expect(space.getByRole('alert')).toContainText('保存暂不可用');
  await space.getByRole('alert').getByRole('button',{ name:'重试' }).click();
  await expect(space).toContainText('你的纠正：我照着刚才的讲解复述，还不能独立解释。');
  await expect(space).toContainText('帮助后符合 0 次');
  await page.request.post('http://127.0.0.1:4175/reopen');
  await page.setViewportSize({ width:390,height:844 });
  await page.reload();
  await page.getByRole('button',{ name:'学习详情' }).first().click();
  await page.getByRole('tab', { name: '我的理解', exact: true }).click();
  await space.locator('[aria-label="概念与能力"] .concept').first().click();
  await expect(space).toContainText('帮助后符合 0 次');
  await space.getByText('依据与解释历史', { exact: true }).click();
  await space.getByRole('button',{ name:'查看依据 1',exact:true }).click();
  await expect(space).toContainText('你的纠正：我照着刚才的讲解复述，还不能独立解释。');
  expect(await page.getByRole('dialog', { name:'学习详情', exact:true }).evaluate(el => el.scrollWidth <= el.clientWidth + 1)).toBe(true);
  await page.getByRole('dialog', { name:'学习详情', exact:true }).getByRole('button',{ name:'关闭',exact:true }).click();
  const openActivity = page.locator('.agent-presentation .tutor-activities article').filter({ hasText:'Observe two:' });
  await openActivity.getByRole('textbox').fill('all distance over all time');
  await openActivity.getByRole('button',{ name:'提交回答',exact:true }).click();
  await expect(openActivity.getByText('回答已记录，尚未评分。')).toBeVisible();
  const openSubmission = (await facts()).find((f:any) => f.kind === 'learner_action' && f.payload.delivery_ref.endsWith(':two'));
  await page.request.post('http://127.0.0.1:4175/evaluate',{ data:{ action_ref:openSubmission.event_id } });
  await page.evaluate(() => window.dispatchEvent(new Event('tutor-state-changed')));
  await expect(openActivity.getByText('The response names the total distance.')).toBeVisible();
  await expect.poll(async () => (await facts()).filter((f:any) => f.kind === 'help_displayed' && f.payload.action === 'feedback').length).toBe(1);
  await openActivity.getByRole('button',{ name:'改答',exact:true }).click();
  await expect(openActivity.getByText('本次回答前已展示帮助。')).toBeVisible();
  await page.unrouteAll({ behavior:'wait' });
});
