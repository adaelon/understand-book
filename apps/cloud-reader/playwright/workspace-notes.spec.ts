import { expect, test, type Page } from '@playwright/test';
import type { BoardDocument } from '@understand-book/web/reader/generated/BoardDocument';
import type { WorkspaceNotebook } from '@understand-book/web/reader/generated/WorkspaceNotebook';

const backend = process.env.WORKSPACE_NOTES_BACKEND_URL ?? 'http://127.0.0.1:4220';
const control = process.env.WORKSPACE_NOTES_CONTROL_URL ?? 'http://127.0.0.1:4221';
interface Fixture {
  cookie: string; learning_workspace_id: string;
  publication: { book_id: string; publication_id: string };
  board_a: BoardDocument; board_b: BoardDocument; seed_turn_id: string;
}
async function notebook(page: Page, scope: string): Promise<WorkspaceNotebook> {
  return page.evaluate(async id => { const path = '/reader-src/api'; return (await import(path)).api.workspaceNotes(id); }, scope);
}
async function readBoard(page: Page, id: string): Promise<BoardDocument> {
  return page.evaluate(async id => { const path = '/reader-src/api'; return (await (await import(path)).api.boardRead(id)).document; }, id);
}
function boardUrl(seed: Fixture, id: string) {
  return `/?${new URLSearchParams({ board_id: id, board_book_id: seed.publication.book_id,
    board_publication_id: seed.publication.publication_id, board_learning_workspace_id: seed.learning_workspace_id })}`;
}

test('workspace notes survive home and reader navigation and remain shared across two real boards', async ({ page, context, request }, info) => {
  page.setDefaultTimeout(20_000);
  await page.addInitScript(() => {
    if (window.top !== window) return;
    for (const scene of ['library', 'reader', 'board', 'boards']) localStorage.setItem(`understand-book.product-tour.v1.A.${scene}`, 'done');
  });
  const fixture = await request.get(`${control}/fixture`);
  expect(fixture.ok()).toBe(true);
  const seed = await fixture.json() as Fixture;
  const errors: string[] = [], httpFailures: string[] = [];
  page.on('pageerror', error => errors.push(error.message));
  await context.route(/\/api\/|\/books\//, async route => {
    const incoming = route.request(), url = new URL(incoming.url());
    if (url.pathname.endsWith('/reader/guidance')) return route.fulfill({ json: null });
    const response = await route.fetch({ url: `${backend}${url.pathname}${url.search}`,
      headers: { ...await incoming.allHeaders(), host: 'reader.example', origin: 'https://reader.example', cookie: seed.cookie } });
    if (!response.ok() && !(response.status() === 404 && url.pathname.endsWith('/source_manifest'))) {
      httpFailures.push(`${response.status()} ${incoming.method()} ${url.pathname}: ${await response.text()}`);
    }
    await route.fulfill({ response });
  });
  const notes = page.getByRole('region', { name: '学习空间笔记', exact: true });
  const board = page.getByRole('region', { name: '知识画板工作区', exact: true });
  const marker = Date.now().toString().slice(-6);
  const firstTitle = `练习需要明确的反馈 ${marker}`, secondTitle = `一次慢练的观察 ${marker}`;
  const firstBody = '把一次练习中的具体表现记录下来，才能判断下一次应该怎样调整。';
  const revisedTitle = `反馈需要对应具体目标 ${marker}`;
  const revisedBody = '先确定这次练习的目标，再比较两次尝试。一次有效的观察，应当能改变下一步怎么练。';
  let firstBoardId = '', firstBoardUrl = '';
  async function addNote(title: string, content: string) {
    await notes.getByRole('button', { name: '新建笔记', exact: true }).click();
    await notes.getByRole('textbox', { name: '笔记标题', exact: true }).fill(title);
    await notes.getByRole('textbox', { name: '笔记正文', exact: true }).fill(content);
    await notes.getByRole('button', { name: '保存笔记', exact: true }).click();
    await expect(notes.locator('.notes-save-status')).toHaveText('已保存');
  }
  async function addExisting(title: string) {
    await board.getByRole('button', { name: '添加内容', exact: true }).click();
    const picker = page.getByRole('dialog', { name: '添加到画板', exact: true });
    await picker.getByRole('tab', { name: '空间笔记', exact: true }).click();
    await picker.getByRole('textbox', { name: '查找可添加的内容', exact: true }).fill(title);
    await picker.locator('.board-catalog-item').filter({ hasText: title }).click();
    await expect(picker).not.toBeVisible();
  }

  await test.step('create from the workspace home without an open book and reopen persisted content', async () => {
    await page.goto('/');
    await page.locator('.learning-space-select').filter({ hasText: '用完整板书理解平均速度与等时间条件' }).click();
    await page.getByRole('button', { name: '空间笔记', exact: true }).click();
    await expect(notes).toBeVisible();
    await addNote(firstTitle, firstBody);
    await addNote(secondTitle, '这次只留意节奏，发现停顿集中在同一个位置。');
    await notes.getByRole('button', { name: '关闭笔记，回到学习', exact: true }).click();
    await page.reload();
    await page.getByRole('button', { name: '空间笔记', exact: true }).click();
    await notes.locator('.notes-list-item').filter({ hasText: firstTitle }).click();
    await expect(notes.getByRole('textbox', { name: '笔记正文', exact: true })).toHaveValue(firstBody);
    await page.screenshot({ path: info.outputPath('desktop-notebook.png') });
  });
  const saved = await notebook(page, seed.learning_workspace_id);
  const first = saved.notes.find(note => note.title === firstTitle)!, second = saved.notes.find(note => note.title === secondTitle)!;
  expect(first.note_id).toBeTruthy(); expect(second.note_id).toBeTruthy();

  await test.step('home note opens a real reader board and creates an explained shared relationship', async () => {
    await notes.getByRole('button', { name: /放到画板/ }).click();
    const gallery = page.getByRole('dialog', { name: '我的知识画板', exact: true });
    // Home may choose any readable material in this workspace. Create its board
    // here, then reuse the same notes on the other material's seeded board.
    await gallery.getByRole('textbox', { name: '新画板名称', exact: true }).fill(`练习与反馈 ${marker}`);
    await gallery.getByRole('button', { name: '新建画板', exact: true }).click();
    await expect(board.getByRole('textbox', { name: '画板名称', exact: true })).toHaveValue(`练习与反馈 ${marker}`);
    await expect(board.locator('.board-reading-pane:visible')).toContainText(firstBody);
    firstBoardUrl = page.url(); firstBoardId = new URL(firstBoardUrl).searchParams.get('board_id')!;
    await addExisting(secondTitle);
    const relations = board.getByRole('region', { name: '笔记关系', exact: true });
    await relations.getByRole('button', { name: '建立关系', exact: true }).click();
    await relations.getByRole('combobox', { name: '关联到', exact: true }).selectOption(first.note_id);
    await relations.getByLabel('关系名称', { exact: true }).fill('提供练习实例');
    await relations.getByLabel('关联理由', { exact: true }).fill('慢练记录把反馈原则变成了可比较的具体观察。');
    await relations.getByRole('button', { name: '保存关系', exact: true }).click();
    await expect(relations.locator('.note-relation')).toContainText('提供练习实例');
    await expect(board.locator('[data-board-link]').filter({ hasText: '提供练习实例' })).toHaveCount(1);
    await page.screenshot({ path: info.outputPath('desktop-board-relation.png') });
  });

  await test.step('a second board uses the same notes and edits the authoritative relationship', async () => {
    await page.goto(boardUrl(seed, seed.board_b.board_id));
    await expect(board.getByRole('textbox', { name: '画板名称', exact: true })).toHaveValue(seed.board_b.title);
    await addExisting(firstTitle); await addExisting(secondTitle);
    const relations = board.getByRole('region', { name: '笔记关系', exact: true });
    await expect(relations.locator('.note-relation')).toContainText('提供练习实例');
    await relations.getByRole('button', { name: '编辑关系', exact: true }).click();
    await relations.getByLabel('关系名称', { exact: true }).fill('支持这个原则');
    await relations.getByLabel('关联理由', { exact: true }).fill('记录指出了具体停顿位置，因此下一次练习可以针对这个位置。');
    await relations.getByRole('button', { name: '保存关系', exact: true }).click();
    await expect(relations.locator('.note-relation')).toContainText('支持这个原则');
    await expect.poll(async () => (await notebook(page, seed.learning_workspace_id)).relations.find(relation => relation.from_note_id === second.note_id && relation.to_note_id === first.note_id)?.label).toBe('支持这个原则');
    const current = await readBoard(page, seed.board_b.board_id);
    const card = current.items.find(item => item.target.kind === 'note' && item.target.note_id === first.note_id)!;
    await board.locator(`[data-item-id="${card.item_id}"] .board-card-open`).click();
    await board.getByRole('button', { name: '打开并编辑笔记', exact: true }).click();
    await notes.getByRole('textbox', { name: '笔记标题', exact: true }).fill(revisedTitle);
    await notes.getByRole('textbox', { name: '笔记正文', exact: true }).fill(revisedBody);
    await notes.getByRole('button', { name: '保存笔记', exact: true }).click();
    await expect(notes.locator('.notes-save-status')).toHaveText('已保存');
    await page.setViewportSize({ width: 390, height: 844 });
    await expect(notes.getByRole('textbox', { name: '笔记正文', exact: true })).toBeVisible();
    await page.screenshot({ path: info.outputPath('mobile-note-editor.png') });
    expect(await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth + 1)).toBe(true);
    await page.setViewportSize({ width: 1440, height: 1000 });
    await notes.getByRole('button', { name: '关闭笔记，回到学习', exact: true }).click();
    await expect(board.locator('.board-reading-pane:visible')).toContainText(revisedBody);
    await page.reload();
    await expect(board.locator(`[data-item-id="${card.item_id}"]`)).toContainText(revisedTitle);
    await page.goto(firstBoardUrl);
    await expect(board.getByRole('textbox', { name: '画板名称', exact: true })).toHaveValue(`练习与反馈 ${marker}`);
    const other = await readBoard(page, firstBoardId);
    expect(other.items.find(item => item.target.kind === 'note' && item.target.note_id === first.note_id)?.readable_content).toBe(revisedBody);
    expect(other.links.find(link => link.note_relation_id)?.label).toBe('支持这个原则');
  });

  await test.step('a saved note brings its title into the Agent composer', async () => {
    await board.getByRole('button', { name: '关闭画板，回到阅读', exact: true }).click();
    await page.getByRole('button', { name: '更多阅读工具', exact: true }).click();
    await page.getByRole('button', { name: '空间笔记', exact: true }).click();
    await notes.locator('.notes-list-item').filter({ hasText: revisedTitle }).click();
    await notes.getByRole('button', { name: /和 Agent 继续想/ }).click();
    await expect(notes).not.toBeVisible();
    await expect(page.locator('.note-question-context')).toContainText(revisedTitle);
    await expect(page.getByRole('textbox', { name: '围绕当前阅读内容提问', exact: true })).toHaveValue(new RegExp(revisedTitle));
  });

  await test.step('a completed answer becomes an editable note with provenance and a working source', async () => {
    await page.goto(boardUrl(seed, seed.board_a.board_id));
    await expect(board.getByRole('textbox', { name: '画板名称', exact: true })).toHaveValue(seed.board_a.title);
    await board.getByRole('button', { name: '关闭画板，回到阅读', exact: true }).click();
    const answer = page.locator('.answer-actions').first();
    await answer.locator('summary').click();
    await answer.getByRole('button', { name: '保存到空间笔记', exact: true }).click();
    await expect(notes).toBeVisible();
    await expect(notes.locator('.notes-provenance')).toContainText('对话回答');
    await expect(notes.getByRole('textbox', { name: '笔记正文', exact: true })).toHaveValue(/1\.5 m\/s/);
    const imported = (await notebook(page, seed.learning_workspace_id)).notes.find(note => note.origin?.turn_id === seed.seed_turn_id)!;
    expect(imported.author).toBe('assistant');
    expect(imported.source_bindings.length).toBeGreaterThan(0);
    await notes.locator('.notes-source-link').first().click();
    const source = page.getByRole('dialog', { name: '笔记原文', exact: true });
    await expect(source).toBeVisible();
    await expect(source.locator('mark').first()).toBeInViewport();
    await page.screenshot({ path: info.outputPath('answer-note-source.png') });
    await source.getByRole('button', { name: '打开原资料', exact: true }).click();
    await expect(source).not.toBeVisible();
    await expect(notes).not.toBeVisible();
    await expect(page.locator('article.prose')).toContainText('平均速度是总路程除以总时间');
    await page.getByRole('button', { name: '学习资料', exact: true }).click();
    await page.getByRole('button', { name: '空间笔记', exact: true }).click();
    await notes.locator('.notes-list-item').filter({ hasText: imported.title }).click();
    await notes.locator('.notes-source-link').first().click();
    await expect(source.locator('mark').first()).toBeInViewport();
    await expect(source.getByRole('button', { name: '回到引用处', exact: true })).toBeVisible();
    await source.getByRole('button', { name: '打开原资料', exact: true }).click();
    await expect(source).not.toBeVisible();
    await expect(notes).not.toBeVisible();
    await expect(page.locator('article.prose')).toContainText('平均速度是总路程除以总时间');
  });
  expect(errors).toEqual([]);
  expect(httpFailures).toEqual([]);
});
