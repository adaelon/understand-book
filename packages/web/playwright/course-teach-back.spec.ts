import { expect, test } from '@playwright/test';
import { writeFile } from 'node:fs/promises';
import type { CourseLessonView } from '../src/generated/CourseLessonView';
import type { ClosedBookPractice } from '../src/generated/ClosedBookPractice';
import { averageSpeedComparison, setClarifiedStop } from './fixtures/teach-back-rounds';
import { setup, scenario } from './fixtures/tutor-continuation';

test('course reflection and clarification survive reopening and remain inaccessible during closed-book practice', async ({ page }, info) => {
  test.setTimeout(90_000);
  const fixture = await setup(page, 'markdown', true, true);
  const chat = 'tc0-chat';
  const lesson: CourseLessonView = {
    summary: { id: 'journey', unit_id: 'motion', question: '停车会怎样影响全程平均速度？', objective: '根据总路程和总时间解释平均速度',
      prerequisites: '能读懂一段行程记录', estimated_minutes: 8, duration_basis: '两轮比较',
      source: { publication: { book_id: scenario.book_id, publication_id: 'local' }, source_revision: 'tc0-source',
        start_lid: '1.1', end_lid: '1.2', label: '平均速度', structure_refs: ['1'] } },
    progress: { run_id: 'journey-run', release_id: 'motion-course', release_version: 1,
      lesson_id: 'journey', revision: 4, goal_revision: 1, stage: 'explanation', status: 'active' },
    introduction: '比较行程相同的两辆车。', prediction: { question: '停车会改变结果吗？', options: [] },
    prediction_response: null, experiment: null, preset: null, observed: [], explanation: scenario.quote,
    task: null, answer: null, hint: null, solution: null, assessment_status: null, support_delivered: [],
    source_excerpt: null, concepts: [], reflection_prompt: '用自己的话说明，路上的停留时间要不要算。', lab: null,
    exploration: null, expressions: [], reviews: [], reading: null, round_index: 0, round_titles: ['计入停留', '比较速度'],
    round_history: [], decision: null, activity_response: null, inquiry_kind: 'reasoning', resolved_questions: [], math_exploration: null,
  };
  const courseWrites: string[] = [];
  await page.route('**/api/book/course/**', route => {
    const post = route.request().method() === 'POST';
    if (post) courseWrites.push(route.request().postDataJSON().action.kind);
    return route.fulfill({ json: post || new URL(route.request().url()).searchParams.has('run_id') ? lesson : {
      release_id: lesson.progress.release_id, release_version: 1, progress: [lesson.progress],
      course: { language: 'zh', units: [{ id: 'motion', title: '行程比较' }], routes: [], lessons: [lesson.summary] },
    } });
  });
  const initial = 'I divide total distance by total time. Stopped time can be removed.';
  const clarification = 'From departure to arrival the clock keeps ticking, so stopped time counts.';
  const variant = '两辆车都经过了 10 公里，甲在途中停了 5 分钟，乙在终点多等了 5 分钟。比较从出发到抵达的时间，该把哪次停留计入？';
  const first = averageSpeedComparison('course-turn-1');
  first.follow_up = { row_id: 'r2', question: 'Does elapsed time include the stop?' };
  first.support_delivery_refs = ['course:explanation'];
  const second = averageSpeedComparison('course-turn-2');
  second.continuation = { kind: 'follow_up', parent_round_id: first.round_id, first_round_id: first.round_id, row_id: 'r2' };
  second.support_delivery_refs = [...first.support_delivery_refs, first.round_id];
  setClarifiedStop(second, 'course-turn-2', clarification);
  const cards = [first, second];
  let practice: ClosedBookPractice | null = null;
  const practiceWrites: Record<string, unknown>[] = [];
  await page.route('**/api/tutor/closed-book', route => {
    const request = route.request().postDataJSON();
    if (request.action !== 'state') practiceWrites.push(request);
    if (request.action === 'start') practice = {
      id: 'journey-practice', owner_id: request.course_run_id, course: true, chat_session_id: chat, source_id: scenario.book_id,
      source_revision: 'tc0-source', goal_revision: 1, revision: 1, status: 'active', can_resume: true, target: lesson.summary.question,
      criteria: '说清起止条件', attempt: 1, assisted: false, needs_retry: false, visible_turn_ids: [], draft: '',
      completion_reason: '', learner_quote: '', course_origin: { round_index: 0, stage: 'explanation', decision_phase: null, question: lesson.reflection_prompt! },
    };
    else if (practice && request.action !== 'state') {
      practice.revision++;
      practice.draft = request.draft ?? practice.draft;
      if (request.action === 'pause') practice.status = 'paused';
      if (request.action === 'resume') practice.status = 'active';
      if (request.action === 'exit') practice.status = 'ended';
    }
    return route.fulfill({ json: practice });
  });
  await page.route('**/api/tutor/display', route => {
    const body = route.request().postDataJSON();
    fixture.displayed.push(body);
    const review = lesson.reviews.find(review => review.expression.job_ref === body.round_id);
    if (review) review.displayed = true;
    return route.fulfill({ json: { event_id: 'display-' + body.round_id } });
  });
  const outcome = fixture.turns[0].outcome as object;
  await page.route('**/api/agent/runs', route => {
    const body = route.request().postDataJSON();
    fixture.requests.push(body);
    const id = 'course-turn-' + fixture.requests.length;
    const card = cards[fixture.requests.length - 1];
    if (card) {
      const expression = { fact_ref: id, job_ref: card.round_id, activity: 'reflection', response: body.message,
        teach_back: { session_id: chat, turn_id: id, round_id: card.round_id }, round_index: 0, round_title: lesson.round_titles[0] };
      lesson.expressions.push(expression);
      lesson.reviews.push({ expression, status: 'ready', displayed: false, feedback: null, comparison: card, understanding: [], error: null });
      lesson.progress.revision++;
      fixture.turns.push({ turn_id: id, user: body.message, user_turn_ordinal: fixture.turns.length + 1, status: 'completed', effect_labels: [],
        domain: { interaction: { entry_action: 'explain_back', course: { run_id: lesson.progress.run_id,
          lesson_id: lesson.progress.lesson_id, round_index: 0, stage: 'explanation', delivery_ref: first.support_delivery_refs![0] }, teach_back_reply: body.teach_back_reply },
          effects: [], teach_back_attempts: [{ operation_id: body.operation_id, expression_id: body.expression_id }] },
        outcome: { ...outcome, answer: '', answer_view: { parts: [{ kind: 'teach_back_comparison', comparison: card }], sources: [] } } });
    } else {
      practice!.visible_turn_ids.push(id);
      fixture.turns.push({ turn_id: id, user_turn_ordinal: fixture.turns.length + 1, user: body.message, status: 'completed', effect_labels: [],
        outcome: { ...outcome, answer: variant, answer_view: { parts: [{ kind: 'markdown', text: variant }], sources: [] } } });
    }
    return route.fulfill({ json: { book_id: scenario.book_id, session_id: chat, turn_id: id } });
  });

  await page.reload();
  await page.getByRole('button', { name: '本书课程 →' }).click();
  await page.getByRole('button', { name: '继续学习', exact: true }).click();
  const course = page.locator('.lesson-player');
  const actions = course.locator('.contextual-practice');
  const start = actions.getByRole('button', { name: '练到会', exact: true });
  await expect(start).toBeVisible();
  await expect(actions.getByRole('button', { name: '给我一道应用题', exact: true })).toBeVisible();
  await expect(actions.getByRole('button', { name: '我来解释一下' })).toHaveCount(0);
  const tutor = course.locator('.activity-tutor');
  const card = tutor.locator('.teach-back-comparison');
  await expect(card).toHaveCount(0);
  await course.locator('#course-reflection').fill(initial);
  await course.getByRole('button', { name: '提交我的解释' }).click();
  await expect(card).toBeVisible();
  await expect(tutor.locator('.guidance')).toHaveCount(0);
  await expect(course.locator('#course-reflection')).toHaveCount(0);
  await card.locator('[data-row-id="r2"]').click();
  await expect(card.locator('.teach-back-evidence')).toContainText('Stopped time can be removed.');
  await card.locator('[data-row-id="r2"]').evaluate(node => node.scrollIntoView({ block: 'start' }));
  await page.screenshot({ path: info.outputPath('initial-course-feedback.png') });
  await tutor.locator('#activity-reply').fill(clarification);
  await tutor.getByRole('button', { name: '回应当前问题' }).click();
  await expect(card.locator('.comparison-status[data-status="clarified"]')).toBeVisible();
  await expect(card).toContainText(clarification);
  await expect.poll(() => fixture.displayed.some(r => r.round_id === second.round_id)).toBe(true);
  await expect(page.locator('.transcript [data-turn-id="course-turn-2"] .teach-back-comparison:visible')).toHaveCount(0);
  await page.reload();
  await expect(card).toBeVisible();
  await expect(card).toContainText(clarification);
  await card.locator('button[data-round-id="' + first.round_id + '"]').click();
  await card.locator('[data-row-id="r2"]').click();
  await expect(card.locator('.teach-back-evidence')).toContainText('Stopped time can be removed.');
  await card.locator('button[data-round-id="' + second.round_id + '"]').click();
  await expect(card.locator('.teach-back-support')).toContainText('1 轮三图');
  await card.locator('.teach-back-rounds').evaluate(node => node.scrollIntoView({ block: 'start' }));
  await page.screenshot({ path: info.outputPath('reopened-course-feedback.png') });
  await page.setViewportSize({ width: 320, height: 740 });
  await page.getByRole('navigation', { name: '书内学习' }).getByRole('button', { name: '课程', exact: true }).click();
  await card.locator('[data-row-id="clarified-stop"]').click();
  await expect(card.locator('.teach-back-evidence')).toContainText(clarification);
  expect(await course.evaluate(node => node.scrollWidth - node.clientWidth)).toBeLessThanOrEqual(1);
  await card.locator('[data-row-id="clarified-stop"]').evaluate(node => node.scrollIntoView({ block: 'start' }));
  await page.screenshot({ path: info.outputPath('reopened-course-feedback-320.png') });
  const navigation = page.getByRole('navigation', { name: '阅读工作区' });
  await navigation.getByRole('button', { name: '问答', exact: true }).click();
  const savedInChat = page.locator('.transcript [data-turn-id="course-turn-2"] .teach-back-comparison');
  await expect(savedInChat).toContainText(clarification);
  await page.getByRole('button', { name: '返回学习', exact: true }).click();
  await expect(card).toContainText(clarification);
  await page.setViewportSize({ width: 1440, height: 960 });
  expect(fixture.requests).toHaveLength(2);
  expect(fixture.requests[0]).toMatchObject({ entry_action: 'explain_back', message: initial,
    course_context: { run_id: lesson.progress.run_id, revision: 4, surface: 'course' } });
  expect(fixture.requests[1]).toMatchObject({ message: clarification, teach_back_reply: { round_id: first.round_id, row_id: 'r2' } });
  expect(courseWrites).not.toContain('reflect');
  expect(courseWrites).not.toContain('advance');

  await start.click();
  const dialog = page.getByRole('dialog', { name: '练到会' });
  async function covered() {
    await expect(dialog).toBeVisible();
    await expect(page.locator('.app')).toHaveAttribute('inert', '');
    await expect(dialog).not.toContainText(scenario.quote);
    await expect(dialog).not.toContainText(initial);
    await expect(dialog).not.toContainText(clarification);
    await expect(page.getByRole('region', { name: '讲后对照' })).toHaveCount(0);
    await expect(page.getByRole('button', { name: '追问路径' })).toHaveCount(0);
  }
  await expect(dialog).toContainText(variant);
  await covered();
  const input = dialog.getByRole('textbox', { name: '你的回答' });
  const draft = '车开到终点的那一刻就是这次行程的终点。';
  await input.fill(draft);
  await page.screenshot({ path: info.outputPath('closed-book-active.png') });
  await dialog.getByRole('button', { name: '暂停', exact: true }).click();
  await expect(dialog).toContainText('已暂停');
  await covered();
  await page.reload();
  await expect(dialog).toContainText('已暂停');
  await covered();
  await page.screenshot({ path: info.outputPath('closed-book-paused-reopened.png') });
  await dialog.getByRole('button', { name: '继续练习' }).click();
  await expect(input).toHaveValue(draft);
  await covered();
  await dialog.getByRole('button', { name: '返回看资料' }).click();
  await expect(dialog).toHaveCount(0);
  await expect(card).toBeVisible();
  await expect(card).toContainText(clarification);
  expect(practiceWrites.filter(r => r.action === 'start')).toHaveLength(1);
  expect(practiceWrites.find(r => r.action === 'start')).toMatchObject({ chat_session_id: chat, course_run_id: lesson.progress.run_id });
  expect(fixture.requests).toHaveLength(3);
  expect(fixture.errors).toEqual([]);
  await writeFile(info.outputPath('browser-actions.json'), JSON.stringify({
    runs: fixture.requests, displays: fixture.displayed, courseWrites, practiceWrites,
  }, null, 2));
});
