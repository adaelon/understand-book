import { createRequire } from 'node:module';
import { resolve } from 'node:path';
import { readFile, writeFile } from 'node:fs/promises';
const require = createRequire(resolve('packages/web/package.json'));
const { chromium, expect } = require('@playwright/test');
const output = resolve('docs/performance/workspace-library-20261021');
const video = { book_id: '19ac880d-a788-43f4-9725-6c56f4980189', publication_id: 'lecture-v1' };
const infra = { book_id: 'ai-infra', publication_id: 'infra-v1' };
const rust = { book_id: 'mastering-rust', publication_id: 'rust-v1' };
const books = [
  { published_book_ref: video, title: 'CS336 2026 · 第 2 讲：PyTorch 与资源核算', media_kind: 'video', cover: { kind: 'image', url: '/api/preview/course.jpg' } },
  { published_book_ref: infra, title: '深入理解 AI Infra：量化分析与系统设计', media_kind: 'book', cover: null },
  { published_book_ref: rust, title: 'Mastering Rust', media_kind: 'book', cover: { kind: 'image', url: '/api/preview/rust.jpg' } },
];
const goal = '结合课程录像、中英转写和画面索引，理解 PyTorch 张量运算，并掌握显存占用与 FLOPs 的基本核算方法。';
const space = { learning_workspace_id: 'course', goal: { text: goal, constraints: [], revision: 1, created_at: 1 }, goal_history: [],
  members: [video, infra].map(published_book_ref => ({ published_book_ref, readable: true, added_at: 1 })), archived: false, created_at: 1, updated_at: 1 };
const empty = { ...space, learning_workspace_id: 'empty', goal: { ...space.goal, text: '学习 CS336' }, members: [] };
const other = { ...space, learning_workspace_id: 'rust', goal: { ...space.goal, text: '从所有权与借用开始，理解 Rust 的内存模型。' }, members: [{ published_book_ref: rust, readable: true, added_at: 1 }] };
const release = { release_id: 'cs336', version: 1, latest_version: 1, archived: false, created_at: 1,
  manifest: { name: 'CS336 2026 伴学 · PyTorch 与资源核算', publisher: 'Understand Book', public_goal: goal,
    applicability: '准备阅读与实现模型训练代码的读者。', prerequisites: '具备 Python 基础，了解矩阵乘法与神经网络。',
    materials: [{ published_book_ref: video, required_capabilities: ['transcript', 'translation', 'text', 'visual_index', 'graph', 'discourse', 'audio_review'] }], topic_links: [],
    suggested_route: [
      { title: '观看课程并对照中英转写', description: '顺序观看录像，在不熟悉的术语和代码处暂停，对照中英转写回看原声。', materials: [video] },
      { title: '理解张量运算与资源开销', description: '围绕张量、数据类型、矩阵运算和 FLOPs，结合对应画面及讲解梳理概念关系。', materials: [video, infra] },
      { title: '带着代码和计算问题复习', description: '定位课程中的代码或公式画面，选择片段提问，并回到原视频核对解释。', materials: [video] },
    ] }, availability: { status: 'ready', materials: [], audience: [] } };
const browser = await chromium.launch({ headless: true });
const context = await browser.newContext({ viewport: { width: 1440, height: 1000 }, deviceScaleFactor: 1 });
await context.addInitScript(() => sessionStorage.setItem('understand-book:learning:adaelon:selected', 'course'));
const page = await context.newPage();
const errors = [], unknown = [], measurements = [];
page.on('pageerror', error => errors.push(error.message));
await context.route('**/api/**', async route => {
  const path = new URL(route.request().url()).pathname;
  const json = body => route.fulfill({ json: body });
  if (path === '/api/auth/me') return json({ user_id: 'adaelon', csrf_token: 'preview-only' });
  if (path === '/api/library') return json({ books });
  if (path === '/api/videos') return json({ videos: [] });
  if (path === '/api/learning-workspaces') return json({ workspaces: [space, empty, other] });
  if (path === '/api/me/reading-resumption') return json(null);
  if (path === '/api/account/storage') return json({ used_bytes: 0, reserved_bytes: 0, total_bytes: 0, limit_bytes: 1e9, max_file_bytes: null, available_bytes: 1e9, can_upload: true, over_limit: false, expires_at: null, retained_until: null, items: [] });
  if (path.endsWith('/preparations')) return json({ items: [] });
  if (path.endsWith('/exam-sprint')) return json({ sprint: null });
  if (path.endsWith('/shared-release')) return json({ instance: path.includes('/course/') ? { release_id: 'cs336', version: 1, latest_version: 1, release, history: [] } : null });
  if (path === '/api/workspace-releases') return json({ items: [release], total: 1, limit: 12, offset: 0 });
  if (path === '/api/preview/course.jpg') return route.fulfill({ contentType: 'image/jpeg', body: await readFile(resolve(output, 'course-frame.jpg')) });
  if (path === '/api/preview/rust.jpg') return route.fulfill({ contentType: 'image/jpeg', body: await readFile(resolve('.understand-book/mastering-rust/assets/images/1_2-6ae1803c33bcc4b2.jpg')) });
  unknown.push(path); return route.fulfill({ status: 404, json: { message: `Missing preview route: ${path}` } });
});
try {
  await page.goto('http://127.0.0.1:4186/index.html');
  await expect(page.locator('.learning-member')).toHaveCount(2);
  await expect(page.locator('.materials-add-panel')).not.toBeVisible();
  await page.getByRole('button', { name: '添加资料', exact: true }).click();
  await expect(page.locator('.materials-add-panel')).toBeVisible();
  await page.getByRole('button', { name: '添加资料', exact: true }).click();
  await expect(page.locator('.materials-add-panel')).not.toBeVisible();
  await page.evaluate(() => document.fonts.ready);
  await expect(page.locator('.learning-member img').first()).toBeVisible();
  await page.locator('.learning-member img').first().evaluate(img => img.decode());
  for (const [width, height] of [[1440, 1000], [1024, 900], [390, 844]]) {
    await page.setViewportSize({ width, height });
    await page.locator('.network-library').evaluate(el => { el.scrollTop = 0; });
    await page.screenshot({ path: resolve(output, `workspace-${width}.png`) });
    measurements.push(await page.evaluate(() => ({ width: innerWidth, documentWidth: document.documentElement.scrollWidth,
      libraryWidth: document.querySelector('.network-library').scrollWidth, font: getComputedStyle(document.querySelector('.network-library')).fontFamily,
      cards: [...document.querySelectorAll('.learning-member')].map(el => { const r=el.getBoundingClientRect(); return { left:r.left, right:r.right, top:r.top, width:r.width }; }) })));
    expect(measurements.at(-1).documentWidth).toBe(width);
    expect(measurements.at(-1).libraryWidth).toBeLessThanOrEqual(width);
    for (const card of measurements.at(-1).cards) { expect(card.left).toBeGreaterThanOrEqual(0); expect(card.right).toBeLessThanOrEqual(width); }
  }
  await page.setViewportSize({ width: 1440, height: 1000 });
  await page.locator('.original-route summary').click();
  await page.locator('.shared-instance').screenshot({ path: resolve(output, 'shared-route.png') });
  await expect(page.locator('.shared-manifest')).not.toContainText(video.book_id);
  await expect(page.locator('.shared-manifest')).toContainText('双语字幕');
  await page.locator('.learning-space-select').filter({ hasText: '学习 CS336' }).click();
  await expect(page.locator('.learning-empty')).toBeVisible();
  await expect(page.locator('.materials-add-panel')).toBeVisible();
  await page.locator('.learning-space-select').filter({ hasText: '从所有权' }).click();
  await expect(page.locator('.learning-member')).toHaveCount(1);
  await page.locator('.learning-member img').evaluate(img => img.decode());
  await page.locator('.network-library').evaluate(el => { el.scrollTop = 0; });
  await page.screenshot({ path: resolve(output, 'workspace-book.png') });
  await page.getByRole('button', { name: '全部资料', exact: true }).click();
  await expect(page.locator('.network-book-card')).toHaveCount(3);
  await page.locator('.network-library').evaluate(el => { el.scrollTop = 0; });
  await page.screenshot({ path: resolve(output, 'library-all.png') });
  await page.getByRole('button', { name: '新建工作区', exact: true }).click();
  await expect(page.locator('.learning-create')).toBeVisible();
  expect(errors).toEqual([]); expect(unknown).toEqual([]);
  const result = { measurements, errors, unknown, interactions: 'workspace selection, empty workspace, real book cover, shelf, creation form, shared route' };
  await writeFile(resolve(output, 'browser.json'), JSON.stringify(result, null, 2));
  console.log(JSON.stringify(result, null, 2));
} finally { await browser.close(); }
