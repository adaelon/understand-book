import { expect, test } from '@playwright/test';
import type { CourseLessonView } from '../src/generated/CourseLessonView';
import type { ClosedBookPractice } from '../src/generated/ClosedBookPractice';
import type { CourseCatalog } from '../src/inquiry-course';
import { openFollowUpBook } from './fixtures/follow-up-conversations';
import { scenario } from './fixtures/tutor-continuation';

test('课程预测中进入支线求助，回来仍保留本步作答草稿', async ({ page }) => {
  const fixture = await openFollowUpBook(page, 1440, 900);
  const lesson: CourseLessonView = {
    summary: {
      id: 'group-comparison', unit_id: 'experiment', question: '八分差能说明软件有效吗？',
      objective: '辨别分组和测验之间的因果关系', prerequisites: '会比较平均分', estimated_minutes: 8, duration_basis: '一轮预测',
      source: { publication: { book_id: scenario.book_id, publication_id: 'local' }, source_revision: 'tc0-source',
        start_lid: '1.1', end_lid: '1.3.1', label: '分组之前', structure_refs: ['1'] },
    },
    progress: { run_id: 'comparison-run', release_id: 'comparison-course', release_version: 1,
      lesson_id: 'group-comparison', revision: 3, goal_revision: 1, stage: 'prediction', status: 'active' },
    introduction: '两组学生考了同一份试卷。', prediction: { question: '两组学生起点相同吗？', options: [
      { id: 'unknown', text: '还得看分组方式' }, { id: 'same', text: '起点相同' },
    ] }, prediction_response: null, experiment: null, preset: null, observed: [], explanation: null, task: null,
    answer: null, hint: null, solution: null, assessment_status: null, support_delivered: [], source_excerpt: null,
    concepts: [], reflection_prompt: null, lab: null, exploration: null, expressions: [], reviews: [],
    reading: null, round_index: 0, round_titles: ['比较两组学生'], round_history: [], decision: null,
    activity_response: null, inquiry_kind: null, resolved_questions: [], math_exploration: null,
  };
  const catalog: CourseCatalog = { release_id: lesson.progress.release_id, release_version: 1,
    course: { language: 'zh', units: [{ id: 'experiment', title: '比较实验' }], routes: [], lessons: [lesson.summary] },
    progress: [lesson.progress] };
  const mutations: unknown[] = [];
  await page.route('**/api/book/course/**', route => {
    if (route.request().method() === 'POST') mutations.push(route.request().postDataJSON());
    const run = new URL(route.request().url()).searchParams.get('run_id');
    return route.fulfill({ json: run ? lesson : catalog });
  });
  await page.reload();
  await page.getByRole('button', { name: '本书课程 →' }).click();
  await page.getByRole('button', { name: '继续学习', exact: true }).click();
  const course = page.locator('.lesson-player');
  const choice = course.getByRole('radio', { name: '还得看分组方式' });
  const reason = course.locator('#course-reason');
  const draft = '也许参加软件组的学生本来成绩就较好。';
  await choice.check();
  await reason.fill(draft);
  await page.getByRole('button', { name: '追问路径', exact: true }).click();
  await page.locator('.follow-up-path-panel:visible button[data-session-id="control"]').click();
  await expect(page.locator('.follow-up-preview:visible')).toContainText('对照组也上课吗？');
  await expect(choice).toBeChecked();
  expect(fixture.runs).toHaveLength(0);
  expect(fixture.selections).toEqual([]);
  await page.getByRole('button', { name: '进入这条对话' }).click();
  await expect(page.locator('[data-turn-id="control-answer"]')).toContainText('对照组照常上课');
  const composer = page.locator('.agent-input textarea');
  await composer.fill('两组同样上课，为什么也可能有分差？');
  await composer.press('Control+Enter');
  await expect(page.locator('[data-turn-id="sent-1"]')).toBeVisible();
  expect(fixture.runs).toHaveLength(1);
  expect(fixture.runs[0].course_context).toEqual({ run_id: 'comparison-run', revision: 3, surface: 'course' });
  await page.getByRole('button', { name: '回到原回答' }).click();
  await expect(page.locator('[data-turn-id="root-answer"]')).toBeVisible();
  await expect(course.getByRole('heading', { name: lesson.summary.question })).toBeVisible();
  await expect(choice).toBeChecked();
  await expect(reason).toHaveValue(draft);
  expect(fixture.selections).toEqual(['control', 'root']);
  expect(mutations).toEqual([]);
  expect(fixture.errors).toEqual([]);
});

test('恢复闭卷练习时屏蔽追问路径，退出后才读取原回答的支线', async ({ page }) => {
  const fixture = await openFollowUpBook(page, 1440, 900);
  const path = page.getByRole('button', { name: '追问路径', exact: true });
  await path.click();
  await expect(page.locator('.follow-up-path-panel:visible button[data-session-id="control"]')).toBeVisible();
  const practice: ClosedBookPractice = {
    id: 'comparison-practice', owner_id: 'root', course: false, chat_session_id: 'root',
    source_id: scenario.book_id, source_revision: 'tc0-source', goal_revision: null,
    revision: 3, status: 'paused', can_resume: true, target: '解释两组分差', criteria: '独立说明原因',
    attempt: 1, assisted: false, needs_retry: false, visible_turn_ids: [], draft: '我觉得',
    completion_reason: '', learner_quote: '',
  };
  await page.route('**/api/tutor/closed-book', route => {
    if (route.request().postDataJSON().action === 'exit') {
      practice.status = 'ended';
      practice.revision++;
    }
    return route.fulfill({ json: practice });
  });
  const pathReads: string[] = [];
  await page.route(/\/api\/agent\/follow-ups(?:\?|$)/, route => {
    pathReads.push(route.request().method());
    return route.fallback();
  });
  await page.reload();
  const cover = page.getByRole('dialog', { name: '练到会' });
  await expect(cover).toContainText('已暂停');
  await expect(path).toHaveCount(0);
  await expect(page.locator('.follow-up-path-panel:visible')).toHaveCount(0);
  expect(pathReads).toEqual([]);
  await cover.getByRole('button', { name: '返回看资料' }).click();
  await expect(cover).toHaveCount(0);
  await path.click();
  await page.locator('.follow-up-path-panel:visible button[data-session-id="control"]').click();
  await expect(page.locator('.follow-up-preview:visible')).toContainText('对照组也上课吗？');
  expect(pathReads).toContain('GET');
  expect(fixture.selections).toEqual([]);
  expect(fixture.runs).toEqual([]);
  expect(fixture.errors).toEqual([]);
});
