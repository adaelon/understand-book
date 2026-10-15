import { expect, test } from '@playwright/test';
import { readFileSync } from 'node:fs';

for (const viewport of [{ width: 390, height: 844 }, { width: 1440, height: 900 }]) {
  test(`JL9 recap is readable and returns to chat at ${viewport.width}px`, async ({ page }) => {
    await page.setViewportSize(viewport);
    const evidence = [{ turn_id: 'turn-1', event_seq: 2 }];
    let seq = 9;
    const calls: string[] = [];
    await page.route('**/api/**', async route => {
      const request = route.request(); calls.push(`${request.method()} ${new URL(request.url()).pathname}`);
      if (request.url().includes('/agent/history/recap?')) {
        await route.fulfill({ json: { session_id: 'fixture-chat', through_seq: seq, through_at: '2026-10-02T09:00:00+08:00', generated_at: '2026-10-02T09:01:00+08:00',
          questions: [{ text: seq === 9 ? '为什么读过一遍，还不能独立解释这个结论？' : '新记录已加入回顾', status: 'running', evidence }],
          sources: [{ source_ref_id: 'source-1', label: '第一章 · 理解与证据', quote: '解释依赖可以检查的前提。', published_book_ref: null, evidence, unavailable_reason: '原发布当前不可用' }],
          effects: [{ effect_id: 'note-1', label: '笔记：区分阅读活动与理解证据', status: 'kept', effect: { kind: 'reader', effect: { kind: 'Note', mem_id: 'note-1', lid: '1.1', text: '证据' } }, object_id: 'note-1', published_book_ref: null, evidence, unavailable_reason: null }],
          continuations: [{ goal_id: 'goal-1', text: '用一个新的例子检验适用条件', status: 'open', evidence }] } });
      } else await route.fulfill({ json: {} });
    });
    await page.goto('/mobile-workspace-visual.html');
    if (viewport.width < 1024) await page.getByRole('button', { name: '问答', exact: true }).click();
    const input = page.locator('.agent-input textarea');
    await expect(input).toHaveValue('未发送草稿');
    const before = calls.length;
    if (await page.getByRole('button', { name: '问答操作', exact: true }).isVisible()) await page.getByRole('button', { name: '问答操作', exact: true }).click();
    await page.getByRole('button', { name: '本次阅读回顾', exact: true }).click();
    const panel = page.getByRole('dialog', { name: '本次阅读回顾' });
    await expect(panel.getByText('已保留', { exact: true })).toBeVisible();
    await expect(panel.getByRole('button', { name: '查看原文', exact: true })).toBeDisabled();
    expect(calls.slice(before)).toEqual(['GET /api/agent/history/recap']);
    expect(await panel.evaluate(el => el.scrollWidth - el.clientWidth)).toBeLessThanOrEqual(1);
    await panel.getByRole('heading', { name: '待继续事项 1' }).scrollIntoViewIfNeeded();
    await expect(panel.getByText('用一个新的例子检验适用条件')).toBeVisible();
    await panel.evaluate(el => { el.scrollTop = 0; });
    await page.screenshot({ path: `../../tmp/jl89-recap-${viewport.width}.png`, fullPage: true });
    seq = 12;
    await expect(panel.getByText('新记录已加入回顾')).toHaveCount(0);
    await panel.getByRole('button', { name: '刷新回顾' }).click();
    await expect(panel.locator('summary')).toHaveText('新记录已加入回顾');
    await panel.getByRole('button', { name: '返回聊天' }).click();
    await expect(panel).toHaveCount(0);
    await expect(input).toHaveValue('未发送草稿');
    await expect(page.getByRole('button', { name: '问答操作', exact: true })).toBeFocused();
  });
}

for (const width of [320, 390, 768, 1440]) test(`RS4 selects facts and downloads the frozen recap at ${width}px`, async ({ page }, info) => {
  test.setTimeout(120000);
  await page.setViewportSize({ width, height: 900 });
  const evidence = [{ turn_id: 'long-turn', event_seq: 2 }];
  let refreshed = false;
  const calls: string[] = [];
  const question = '为何需要检查结论的适用前提？';
  const quote = '只有在相同条件下比较，才能区分预测与实测。';
  await page.route('**/api/**', async route => {
    const request = route.request(); calls.push(`${request.method()} ${new URL(request.url()).pathname}`);
    if (request.url().includes('/agent/history/recap?')) await route.fulfill({ json: {
      session_id: 'fixture-chat', through_seq: refreshed ? 19 : 9,
      through_at: refreshed ? '2026-10-10T10:00:00+08:00' : '2026-10-10T09:00:00+08:00', generated_at: '2026-10-10T10:01:00+08:00',
      questions: [{ text: refreshed ? '刷新后的问题' : question, status: 'answered', evidence },
        { text: '未选择的私人问题', status: 'running', evidence: [{ turn_id: 'private-turn', event_seq: 4 }] }],
      sources: [{ source_ref_id: 'private-source', label: '《比较的方法》· 第一章', quote, published_book_ref: { book_id: 'private-book', publication_id: 'private-publication' }, evidence, unavailable_reason: '原发布当前不可用' }],
      effects: [{ effect_id: 'private-effect', label: '不分享的演示成果', status: 'delivered', effect: { kind: 'presentation', reference: { presentation_id: 'private-presentation', revision: 1 } }, object_id: null, published_book_ref: null, evidence, unavailable_reason: null }],
      continuations: [{ text: '下次用新例子检查边界。', status: 'open', goal_id: 'private-goal', evidence }],
    } });
    else await route.fulfill({ json: {} });
  });
  await page.goto('/mobile-workspace-visual.html?long-chat');
  if (width < 1024) {
    await page.getByRole('button', { name: '问答', exact: true }).click();
  }
  await page.getByRole('button', { name: '问答操作', exact: true }).click();
  const before = calls.length;
  await page.getByRole('button', { name: '本次阅读回顾', exact: true }).click();
  const recap = page.locator('.session-recap');
  const shareButton = recap.getByRole('button', { name: '生成阅读回顾卡' });
  await expect(shareButton).toBeDisabled();
  const sourceEntry = recap.locator('article').filter({ hasText: quote });
  await sourceEntry.getByRole('button', { name: '重点查看' }).click();
  await expect(recap.getByRole('region', { name: '当前查看重点' })).toContainText(quote);
  await expect(shareButton).toBeDisabled();
  await expect(sourceEntry.getByRole('button', { name: '查看原文', exact: true })).toBeDisabled();
  for (const label of ['分享讨论过的问题第1项', '分享引用的原文第1项', '分享待继续事项第1项']) await recap.getByLabel(label).check();
  expect(await recap.evaluate(el => el.scrollWidth - el.clientWidth)).toBeLessThanOrEqual(1);
  await recap.evaluate(el => { el.scrollTop = 0; });
  await page.screenshot({ path: `../../docs/performance/reading-share-rs4-20261010/recap-${width}.png` });
  await shareButton.click();
  const panel = page.getByRole('dialog', { name: '生成分享图' });
  await expect(panel.getByRole('combobox', { name: '排版', exact: true })).toHaveValue('recap');
  await panel.getByLabel('标题', { exact: true }).fill('前提与证据');
  await panel.getByLabel('我的感想', { exact: true }).fill('下次从一个新例子开始。');
  refreshed = true;
  // Exercise a recap response while a draft is open; the modal intentionally makes the background inert.
  await recap.locator('.recap-refresh').evaluate((el: HTMLButtonElement) => el.click());
  await expect(recap.locator('summary').first()).toHaveText('刷新后的问题');
  await expect(panel.getByLabel('标题', { exact: true })).toHaveValue('前提与证据');
  await expect(panel.locator('.share-original pre').first()).toHaveText(question);
  await expect(panel.locator('.share-source')).toContainText('09:00:00');
  await panel.getByRole('button', { name: '预览图片', exact: true }).click();
  await expect(panel.getByRole('img')).toBeVisible({ timeout: 60000 });
  expect(await panel.evaluate(el => el.scrollWidth - el.clientWidth)).toBeLessThanOrEqual(1);
  const image = panel.getByRole('img');
  const count = await panel.locator('nav span').count() ? Number((await panel.locator('nav span').innerText()).split('/')[1]) : 1;
  for (let index = 1; index <= count; index++) {
    const bytes = await image.evaluate(async (el: HTMLImageElement) => Array.from(new Uint8Array(await (await fetch(el.src)).arrayBuffer())));
    const downloading = page.waitForEvent('download');
    await panel.getByRole('button', { name: '下载 PNG', exact: true }).click();
    const download = await downloading, path = info.outputPath(`recap-${width}-${index}.png`);
    await download.saveAs(path); expect(readFileSync(path).equals(Buffer.from(bytes))).toBe(true);
    if (width === 1440) await download.saveAs(`../../docs/performance/reading-share-rs4-20261010/recap-page-${index}.png`);
    if (index < count) await panel.getByRole('button', { name: '下一页' }).click();
  }
  await page.screenshot({ path: `../../docs/performance/reading-share-rs4-20261010/share-${width}.png` });
  await page.keyboard.press('Escape');
  await expect(panel).toHaveCount(0); await expect(recap).toBeVisible(); await expect(shareButton).toBeFocused();
  await shareButton.click();
  await expect(panel.locator('.share-original pre').first()).toHaveText('刷新后的问题');
  await panel.getByRole('button', { name: '重新选材' }).click();
  await expect(panel).toHaveCount(0); await expect(shareButton).toBeFocused();
  await recap.getByRole('button', { name: '回到问题', exact: true }).first().click();
  await expect(recap).toHaveCount(0);
  await expect(page.locator('.agent-input textarea')).toHaveValue('未发送草稿');
  expect(calls.slice(before)).toEqual(['GET /api/agent/history/recap', 'GET /api/agent/history/recap']);
});

test('RS4 pagination preserves selected facts, status and original cutoff on every page', async ({ page }) => {
  await page.route('**/api/**', route => route.fulfill({ json: {} }));
  await page.goto('/mobile-workspace-visual.html');
  const result = await page.evaluate(async () => {
    // @ts-ignore Vite serves this browser module.
    const { recapEntries, recapShareSource } = await import('/src/session-recap.ts');
    // @ts-ignore Vite serves this browser module.
    const { paginateShare } = await import('/src/reading-share.ts');
    const evidence = [{ turn_id: 'private-turn', event_seq: 1 }];
    const text = '检查前提，才能判断结论的适用范围。'.repeat(24) + 'literal * 2 与 $price';
    const recap = { session_id: 'private-chat', through_seq: 9, through_at: '2026-10-10T09:00:00+08:00', generated_at: '2026-10-10T10:00:00+08:00',
      questions: [{ text, status: 'answered', evidence }, { text: '未选内容', status: 'running', evidence: [{ turn_id: 'unselected', event_seq: 2 }] }],
      sources: [], effects: [], continuations: [] };
    const source = recapShareSource(recap, [recapEntries(recap)[0].id], '检查前提');
    const output = await paginateShare({ source, selected: source.parts.map((part: { id: string }) => part.id), title: '阅读回顾', reflection: '', layout: 'recap', palette: 'blue' });
    try {
      return { expected: text, body: output.pages.map((el: HTMLElement) => [...el.querySelectorAll('.paper .share-section > :not(.identity)')].map(node => node.textContent).join('')).join(''),
        count: output.pages.length, pages: output.pages.map((el: HTMLElement) => ({ text: el.textContent, footer: el.querySelector('footer')!.textContent })) };
    } finally { output.dispose(); }
  });
  expect(result.count).toBeGreaterThan(1); expect(result.body).toBe(result.expected);
  for (const page of result.pages) {
    expect(page.text).toContain('已回答'); expect(page.text).not.toMatch(/已懂|未选内容|private-/);
    expect(page.footer).toContain('对话「检查前提」'); expect(page.footer).toContain('09:00:00');
  }
});
