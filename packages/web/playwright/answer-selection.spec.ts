import { expect, test, type Page } from '@playwright/test';

test.use({ viewport: { width: 1440, height: 900 } });

test.beforeEach(async ({ page }) => {
  await page.goto('/mobile-workspace-visual.html?long-chat&answer-selection');
  await expect(page.locator('.answer-markdown li')).toHaveCount(2);
  await page.locator('.transcript').evaluate(el => { el.scrollTop = 0; });
});

async function geometry(page: Page) {
  return page.locator('.transcript').evaluate(el => {
    const bounds = el.getBoundingClientRect();
    return { top: bounds.top, height: bounds.height, scroll: el.scrollTop, textTop: el.querySelector('li')!.getBoundingClientRect().top };
  });
}

test('dragging through partial bold and list boundaries keeps the transcript still and saves the excerpt', async ({ page }) => {
  const before = await geometry(page);
  const start = await page.locator('.answer-markdown li').first().evaluate(el => {
    const range = document.createRange(); range.setStart(el.firstChild!, 0); range.setEnd(el.firstChild!, 1);
    const rect = range.getBoundingClientRect(); return { x: rect.left + 1, y: rect.top + rect.height / 2 };
  });
  await page.mouse.move(start.x, start.y); await page.mouse.down();
  for (const [index, offset] of [[0, 2], [1, 3], [0, 1], [1, 3]]) {
    // Exercise intermediate native ranges without depending on font-specific hit testing.
    await page.locator('.answer-markdown').evaluate((root, { index, offset }) => {
      const range = document.createRange(); range.setStart(root.querySelector('li')!.firstChild!, 0);
      range.setEnd(root.querySelectorAll('strong')[index].firstChild!, offset);
      const selection = window.getSelection()!; selection.removeAllRanges(); selection.addRange(range);
      document.dispatchEvent(new Event('selectionchange'));
    }, { index, offset });
    await expect(page.locator('.answer-popover')).toHaveCount(0);
    await expect(page.getByText(/这段选区无法对应/)).toHaveCount(0);
    expect(await geometry(page)).toEqual(before);
  }
  await page.mouse.up();
  const menu = page.getByRole('group', { name: '选中文字的操作' });
  await expect(menu).toBeVisible();
  expect(await geometry(page)).toEqual(before);
  await menu.getByRole('button', { name: '记笔记', exact: true }).click();
  await expect(page.locator('.fixture-answer-note')).toHaveText('给了第二个名字 → 终点取**那个名字**对应节点的末尾；\n- 没给 → 终点取**第一个');
  await expect(menu).toHaveCount(0);
  expect(await geometry(page)).toEqual(before);
});

test('a selection made without a pointer can save part of bold text', async ({ page }) => {
  await page.locator('.answer-markdown strong').first().evaluate(el => {
    const range = document.createRange(); range.setStart(el.firstChild!, 1); range.setEnd(el.firstChild!, 3);
    const selection = window.getSelection()!; selection.removeAllRanges(); selection.addRange(range);
    document.dispatchEvent(new Event('selectionchange'));
  });
  const button = page.locator('.answer-popover').getByRole('button', { name: '记笔记', exact: true });
  await button.focus(); await page.keyboard.press('Enter');
  await expect(page.locator('.fixture-answer-note')).toHaveText('个名');
});

test('native mouse dragging across list items saves exactly the selected source', async ({ page }) => {
  const before = await geometry(page);
  const points = await page.locator('.answer-markdown').evaluate(root => {
    const start = document.createRange(); start.setStart(root.querySelector('li')!.firstChild!, 0); start.collapse(true);
    const end = document.createRange(); end.setStart(root.querySelectorAll('strong')[1].firstChild!, 3); end.collapse(true);
    return [start, end].map(range => { const r = range.getBoundingClientRect(); return { x: r.x, y: r.y + r.height / 2 }; });
  });
  await page.mouse.move(points[0].x, points[0].y); await page.mouse.down();
  await page.mouse.move(points[1].x, points[1].y, { steps: 20 });
  await expect(page.locator('.answer-popover')).toHaveCount(0);
  expect(await geometry(page)).toEqual(before);
  await page.mouse.up();
  await page.locator('.answer-popover').getByRole('button', { name: '记笔记', exact: true }).click();
  await expect(page.locator('.fixture-answer-note')).toHaveText('给了第二个名字 → 终点取**那个名字**对应节点的末尾；\n- 没给 → 终点取**第一个');
  expect(await geometry(page)).toEqual(before);
});

test('an unsupported cross-part selection reports in a floating surface without moving the transcript', async ({ page }) => {
  await page.locator('.transcript').evaluate(el => { el.scrollTop = el.scrollHeight; });
  const before = await geometry(page);
  await page.locator('.answer-markdown').evaluate(root => {
    const paragraphs = root.querySelectorAll('p');
    const range = document.createRange();
    range.setStart(paragraphs[paragraphs.length - 3].firstChild!, 0);
    range.setEnd(paragraphs[paragraphs.length - 1].firstChild!, 5);
    const selection = window.getSelection()!; selection.removeAllRanges(); selection.addRange(range);
    document.dispatchEvent(new Event('selectionchange'));
  });
  await expect(page.getByText(/这段选区无法对应/)).toBeVisible();
  expect(await geometry(page)).toEqual(before);
  await expect(page.locator('.answer-popover [role="status"]')).toBeVisible();
  await expect(page.locator('.answer-popover').getByRole('button', { name: '记笔记', exact: true })).toHaveCount(0);
  await page.getByRole('button', { name: '取消选区' }).click();
  await expect(page.getByText(/这段选区无法对应/)).toHaveCount(0);
  expect(await geometry(page)).toEqual(before);
});
