import { expect, test } from '@playwright/test';
import { setup } from './fixtures/tutor-continuation';

for (const width of [1366, 390]) {
  test(`a Socratic question leaves the reader room to answer at ${width}px`, async ({ page }, testInfo) => {
    await page.addInitScript(() => localStorage.setItem('understand-book.product-tour.v1.local.reader', 'done'));
    const fixture = await setup(page, 'markdown', true, true);
    const question = '车停下来时，里程没有增加。你会把这段时间算进全程吗？';
    const explanation = '去程和回程各走 30 公里。算整趟的平均速度，先看出发到抵达一共过了多久。';
    fixture.turns[0].user = '这两段路一样长，为什么不能直接平均两个速度？';
    fixture.turns[0].outcome = { ...fixture.turns[0].outcome as object,
      answer: explanation + '\n\n' + question,
      answer_view: { parts: [{ kind: 'markdown', text: explanation + '\n\n' }, { kind: 'socratic_question', text: question, purpose: { focus: 'private-focus', reason: 'private-reason', learner_inference: 'private-inference', evidence_to_notice: 'private-evidence', basis_refs: ['private-ref'] } }], sources: [] } };
    const suggestions: string[] = [];
    await page.route('**/api/reader/guidance', route => {
      suggestions.push(route.request().postData() ?? '');
      return route.fulfill({ json: null });
    });
    await page.setViewportSize({ width, height: width > 1000 ? 900 : 844 });
    await page.reload();
    if (width < 1024) await page.getByRole('navigation', { name: '阅读工作区' }).getByRole('button', { name: '问答', exact: true }).click();
    const card = page.locator('.socratic-question');
    const input = page.locator('.agent-input textarea');
    await expect(card).toContainText('苏格拉底式问答');
    await expect(page.getByText(question, { exact: true })).toHaveCount(1);
    await expect(page.locator('body')).not.toContainText('private-');
    await expect(input).toHaveAttribute('placeholder', '说说你的想法…');
    await expect(input).toHaveValue('');
    await card.scrollIntoViewIfNeeded();
    await expect(card).toBeInViewport();
    await page.screenshot({ path: testInfo.outputPath(`socratic-${width}.png`) });
    // The fixture initially loaded a different ordinary answer. Only the
    // restored Socratic turn must not generate another invitation or draft.
    expect(suggestions.filter(request => request.includes(question))).toEqual([]);
    const words = width > 1000 ? '我会算进去，因为这时钟还在走。' : '为什么时间还要算进去？';
    await input.fill(words);
    await page.locator('.send-agent').click();
    await expect.poll(() => fixture.requests.length).toBe(1);
    expect(fixture.requests[0]).toMatchObject({ message: words });
    expect(fixture.requests[0].presentation_requested).toBeUndefined();
    await expect(input).not.toHaveAttribute('placeholder', '说说你的想法…');
    if (width > 1000) {
      fixture.turns.at(-1)!.outcome = fixture.turns[0].outcome;
      await page.reload();
      const unfinished = '我还想留着这句没写完的话';
      await input.fill(unfinished);
      await page.getByRole('button', { name: '先到这里', exact: true }).click();
      await expect.poll(() => fixture.requests.length).toBe(2);
      expect(fixture.requests[1]).toMatchObject({ message: '先到这里', teaching_response_origin: { kind: 'interface_request' } });
      await expect(input).toHaveValue(unfinished);
      await expect(page.getByRole('button', { name: '先到这里', exact: true })).toHaveCount(0);
    }
    expect(fixture.errors).toEqual([]);
  });
}
