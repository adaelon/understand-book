import { expect, test } from '@playwright/test';

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
    if (viewport.width < 1024) await page.getByRole('button', { name: '问答操作', exact: true }).click();
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
    await expect(page.getByRole('button', { name: viewport.width < 1024 ? '问答操作' : '本次阅读回顾', exact: true })).toBeFocused();
  });
}
