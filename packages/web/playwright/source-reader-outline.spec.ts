import { expect, test, type Page } from '@playwright/test';

const quote = '关于货币需求理论的微观基础研究，经济学家主要提出了两种理论。一是资产组合理论，二是交易理论。';
const paragraph = '阅读这一段时，目录应指向当前章节。连续上下文保留理论的前提、比较对象和结论，读者可以继续向前或向后阅读。';
const outline = Array.from({ length: 80 }, (_, index) => [
  { lid: `1.${index + 1}`, title: `第${index + 1}章 宏观经济学的理论与应用`, depth: 1 },
  ...Array.from({ length: 4 }, (_, section) => ({ lid: `1.${index + 1}.${section + 1}`, title: `第${section + 1}节 ${section === 3 ? '结束语' : '理论与分析'}`, depth: 2 })),
]).flat();
const sourceLid = '1.60.4.2';
const segment = (lid: string, text: string, cited = false) => ({ lid, text, kind: 'paragraph', highlights: cited ? [{ start: 0, end: text.length }] : [] });

async function installBook(page: Page) {
  const reads: { anchor_lid?: string; direction: string }[] = [];
  await page.route('**/api/agent/source.resolve', route => route.fulfill({ json: {
    source_ref_id: 'source_ref_methods', label: '正文 · 第四节 结束语', material_title: '西方经济学（宏观部分·第八版）',
    highlighted_quote: quote, context_before: '', context_after: '', stale: false, can_open_in_reader: true,
  } }));
  await page.route('**/api/agent/source.read', route => {
    const request = route.request().postDataJSON();
    reads.push(request);
    const anchor = request.anchor_lid || sourceLid;
    const section = anchor === sourceLid ? '1.60.4' : anchor;
    const initial = !request.anchor_lid;
    const title = outline.find(entry => entry.lid === section)!.title;
    return route.fulfill({ json: {
      source_ref_id: 'source_ref_methods', source_lid: sourceLid, focus_lid: initial ? sourceLid : `${section}.1`,
      segments: [
        segment(`${section}.1`, `### ${title}`),
        segment(`${section}.2`, initial ? quote.repeat(6) : paragraph.repeat(8), initial),
        segment(`${section}.3`, paragraph.repeat(8)),
        ...(initial ? [segment('1.61.1.1', '### 第1节 理论与分析'), segment('1.61.1.2', paragraph.repeat(24))] : []),
      ],
      previous_lid: null, next_lid: null,
      ...(request.include_outline ? { outline } : {}),
    } });
  });
  return reads;
}

test('long contents follow the citation, scrolling, chapter jumps and reopening without moving the reading position', async ({ page }, info) => {
  await page.setViewportSize({ width: 1440, height: 900 });
  const reads = await installBook(page);
  await page.goto('/agent-source-visual.html');
  await page.locator('.agent-source-button').first().click();
  const dialog = page.getByRole('dialog', { name: '回答来源' });
  const nav = dialog.getByRole('navigation', { name: '原文目录' });
  const scroll = dialog.getByLabel('原文阅读区');
  const current = nav.locator('[aria-current="location"]');
  await expect(dialog.locator('mark')).toBeInViewport();
  await expect(current).toHaveAttribute('data-outline-lid', '1.60.4');
  await expect(current).toBeInViewport();
  expect(await nav.evaluate(node => node.scrollTop)).toBeGreaterThan(0);

  // Browsing the contents independently must not leave it disconnected when
  // returning to a citation within the same chapter.
  await nav.evaluate(node => { node.scrollTop = 0; });
  await expect(current).not.toBeInViewport();
  await dialog.getByRole('button', { name: '回到引用处' }).click();
  await expect(current).toBeInViewport();
  expect(reads).toHaveLength(1);

  const beforeToggle = await scroll.evaluate(node => node.scrollTop);
  await dialog.getByRole('button', { name: '目录', exact: true }).click();
  await dialog.getByRole('button', { name: '目录', exact: true }).click();
  await expect(current).toBeInViewport();
  expect(await scroll.evaluate(node => node.scrollTop)).toBeCloseTo(beforeToggle, 0);

  await scroll.evaluate(node => {
    const target = node.querySelector('[data-source-lid="1.61.1.1"]')!;
    node.scrollTop += target.getBoundingClientRect().top - node.getBoundingClientRect().top - 24;
  });
  await expect(current).toHaveAttribute('data-outline-lid', '1.61.1');
  await expect(current).toBeInViewport();
  await expect(dialog.locator('.source-reader-location')).toHaveText('第1节 理论与分析');

  await nav.locator('[data-outline-lid="1.59.2"]').click();
  await expect(scroll.locator('[data-source-lid="1.59.2.1"]')).toBeInViewport();
  await expect(current).toHaveAttribute('data-outline-lid', '1.59.2');
  await dialog.getByRole('button', { name: '回到引用处' }).click();
  await expect(current).toHaveAttribute('data-outline-lid', '1.60.4');
  await expect(current).toBeInViewport();
  await expect(dialog.locator('mark')).toBeInViewport();
  expect(reads.filter(request => request.direction === 'around')).toHaveLength(3);
  await expect(page.getByTestId('reader-status')).toHaveText('保持当前阅读位置');

  const content = (await dialog.locator('.board-source-segment.has-citation').boundingBox())!;
  const readingArea = (await scroll.boundingBox())!;
  expect(content.width).toBeGreaterThan(1000);
  expect(readingArea.height).toBeGreaterThan(720);
  await page.screenshot({ path: info.outputPath('source-reader-desktop.png') });
});

test('mobile contents reveal the current chapter and close after navigation', async ({ page }, info) => {
  await page.setViewportSize({ width: 390, height: 844 });
  await installBook(page);
  await page.goto('/agent-source-visual.html');
  await page.locator('.agent-source-button').first().click();
  const dialog = page.getByRole('dialog', { name: '回答来源' });
  const nav = dialog.getByRole('navigation', { name: '原文目录' });
  const toggle = dialog.getByRole('button', { name: '目录', exact: true });
  await expect(dialog.locator('mark')).toBeInViewport();
  await expect(nav).toBeHidden();
  await toggle.click();
  await expect(nav.locator('[aria-current="location"]')).toBeInViewport();
  await page.screenshot({ path: info.outputPath('source-reader-mobile-outline.png') });
  await nav.locator('[data-outline-lid="1.60.3"]').click();
  await expect(nav).toBeHidden();
  await expect(dialog.locator('[data-source-lid="1.60.3.1"]')).toBeInViewport();
  await toggle.click();
  await expect(nav.locator('[aria-current="location"]')).toHaveAttribute('data-outline-lid', '1.60.3');
  await expect(nav.locator('[aria-current="location"]')).toBeInViewport();
  await toggle.click();
  await dialog.getByRole('button', { name: '回到引用处' }).click();
  await expect(dialog.locator('mark')).toBeInViewport();
  expect(await dialog.evaluate(node => node.scrollWidth <= node.clientWidth)).toBe(true);
  expect((await dialog.getByLabel('原文阅读区').boundingBox())!.height).toBeGreaterThan(670);
  await page.screenshot({ path: info.outputPath('source-reader-mobile.png') });
});
