import { expect, test } from '@playwright/test';
import { scenario, setup } from './fixtures/tutor-continuation';

// A long teaching turn exercises the same scroll/response layout as the reported screen.
const lesson = `## 1.4.2 SmartNIC：这一站没有「余量」可等

先把上一站的触发条件放到桌上：

> 请求量增加，现有设备的空闲能力最终会用尽；此时可以增加通用服务器，也可以开发专用处理器。

请盯住「**空闲**」这个词。上一节的节奏是两拍：先有一份闲着的能力，被每天的请求慢慢吃掉；吃光了，才轮到「加通用服务器 / 做专用处理器」的取舍。

它有一个前半场——**等余量见底**。

### 这一站，变大的是什么？

SmartNIC 面对的是另一类增长：网络带宽提高，虚拟网络策略也日趋复杂。先把两个变化分开看，再看它们如何一起影响处理工作。

> 网络处理既要跟上数据到达的速度，也要完成每个数据包所需的策略处理。

同样是需求增长，这里的问题换成了：**网络数据处理占用了谁的资源？**

### 把工作放到哪里

SmartNIC 是能够在网卡上执行部分数据处理的设备。它让一部分原本由 CPU 完成的网络工作，移动到网卡上。

| 要看清的关系 | 这一站的观察 |
| --- | --- |
| 变大的量 | 网络带宽与策略处理工作 |
| 被占用的资源 | CPU 的处理能力 |
| 工作的去向 | 可以承担部分处理的网卡 |

**把这三个位置连起来，再解释架构为什么发生变化。** 不急着记设备名称，先说清需求、资源和工作之间的关系。

### 再和上一站比较

上一站先观察已有能力什么时候被用尽；这一站要追踪网络工作如何挤占处理资源。两者都从需求出发，但判断的起点不同。

回到那三个空：① 变大的量，② 被占用的资源，③ 凭什么搬。试着用自己的话把它们连成一句解释。
`;

// 1526 × 703 also covers the effective viewport of a 1907 × 879 window at 125% zoom.
for (const { width, height } of [{ width: 1907, height: 879 }, { width: 1526, height: 703 }, { width: 1440, height: 900 }, { width: 390, height: 844 }]) test(`long teaching keeps help and the real answer reachable at ${width}px`, async ({ page }, info) => {
  const fixture = await setup(page, 'markdown', true, true, true);
  const answer = width === 1526 ? '接着上一站，看看网络需求如何改变架构选择。' : lesson;
  const original = fixture.turns[0].outcome as Record<string, unknown>;
  fixture.turns[0] = { ...fixture.turns[0], user: '带我理解：需求如何推动架构设计', question_quote: null,
    domain: { teaching: ['question'] },
    outcome: { ...original, trace: [], answer, answer_view: { parts: [{ kind: 'markdown', text: answer }], sources: [] } } };
  const actions: Record<string, unknown>[] = [];
  let attempted = false;
  const session = { id: 'learning', revision: 1, status: 'active', user_intent: '学懂 1.4 需求如何推动架构设计', explicit_constraints: [],
    current_focus: { interpretation: '', target_object_refs: [], capability_targets: [] },
    material_scope: [{ source_id: scenario.book_id, scope_refs: [], role: 'primary' }], default_teaching_intent: null, path_instance_ref: null, progress_ref: null };
  await page.route('**/api/tutor/state', route => route.fulfill({ json: { control: { enabled: true, revision: 1, current_tutor_session_id: 'learning' }, sessions: { learning: session } } }));
  await page.route('**/api/tutor/activities', route => route.fulfill({ json: { activities: route.request().postDataJSON().turn_id === 'tc0-answer' ? [
    { delivery_ref: 'question', status: 'active', attempted, help_seen: [],
      move: { move_id: 'question', prompt: (width === 1526 ? lesson + '\n\n' : '') + '如果网络带宽提高、策略也更复杂，为什么把部分工作交给 SmartNIC？\n\n试着把「需求 → 资源 → 工作去向」连起来。', actions: ['submit', 'revise', 'request_hint', 'skip'] } },
  ] : [] } }));
  await page.route('**/api/tutor/action', route => {
    actions.push(route.request().postDataJSON()); attempted = true;
    return route.fulfill({ json: { event_id: 'answer-recorded' } });
  });
  await page.setViewportSize({ width, height });
  await page.reload();
  if (width < 1024) await page.locator('.workspace-mobile-nav').getByRole('button', { name: '问答', exact: true }).click();
  await page.getByRole('button', { name: '问答全屏', exact: true }).click();
  const input = page.locator('.agent-input textarea');
  await input.fill('保留这份尚未发送的想法');
  await input.blur();
  const dock = page.getByRole('group', { name: '当前回答的学习操作' });
  await expect(dock.getByRole('button', { name: '回答问题', exact: true })).toBeVisible();
  await page.locator('.transcript').evaluate(node => { node.scrollTop = 0; });
  await expect(dock.getByRole('button', { name: '这一步没跟上', exact: true })).toBeInViewport();
  await expect(dock.getByRole('button', { name: '回答问题', exact: true })).toBeInViewport();
  await expect(page.locator('.turn .clarify-answer')).not.toBeInViewport();
  const transcript = await page.locator('.transcript').boundingBox();
  const goal = await page.locator('.conversation-learning-goal').boundingBox();
  const responseDock = await dock.boundingBox();
  const reading = await page.locator('.turn').boundingBox();
  const composer = await page.locator('.agent-input').boundingBox();
  if (width >= 1280) {
    expect(goal!.x + goal!.width).toBeLessThanOrEqual(transcript!.x);
    expect(responseDock!.x).toBeGreaterThanOrEqual(transcript!.x + transcript!.width);
    expect(reading!.width).toBeGreaterThan(width * .63);
    expect(transcript!.height).toBeGreaterThan(height * .8);
    expect(composer!.height).toBeLessThan(90);
    await expect(page.getByRole('complementary', { name: '学习与会话', exact: true }).getByRole('switch', { name: '连续教学' })).toBeVisible();
    await expect(page.getByRole('complementary', { name: '学习助手', exact: true }).getByRole('button', { name: '我的理解', exact: true })).toBeVisible();
  } else {
    await expect(page.getByRole('complementary', { name: '学习与会话', exact: true })).not.toBeVisible();
  }
  await info.attach('reading-space', { body: JSON.stringify({ width, height, transcript, goal, responseDock, reading, composer }, null, 2), contentType: 'application/json' });
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
  await page.screenshot({ path: info.outputPath(`reading-${width}.png`) });
  await dock.getByRole('button', { name: '回答问题', exact: true }).click();
  const response = page.getByRole('textbox', { name: '活动回答', exact: true });
  await expect(response).toBeFocused();
  await expect(response).toBeInViewport();
  expect(actions).toHaveLength(0);
  expect(fixture.requests).toHaveLength(0);
  await expect(input).toHaveValue('保留这份尚未发送的想法');
  await response.fill('网络处理挤占了 CPU 的能力，因此把适合的工作交给网卡。');
  await page.screenshot({ path: info.outputPath(`responding-${width}.png`) });
  if (width === 1440) {
    // Moving controls into or out of the sidebars must retain the same editors.
    await page.getByRole('button', { name: '调整目标', exact: true }).click();
    const goalDialog = page.getByRole('dialog', { name: '调整目标', exact: true });
    const goalInput = goalDialog.getByRole('textbox', { name: '学习目标', exact: true });
    await goalInput.fill('尚未保存的学习目标');
    await page.setViewportSize({ width: 1024, height });
    await expect(goalInput).toHaveValue('尚未保存的学习目标');
    await expect(input).toHaveValue('保留这份尚未发送的想法');
    await expect(response).toHaveValue('网络处理挤占了 CPU 的能力，因此把适合的工作交给网卡。');
    await page.setViewportSize({ width, height });
    await expect(goalInput).toHaveValue('尚未保存的学习目标');
    await goalDialog.getByRole('button', { name: '取消', exact: true }).click();
    const history = page.getByRole('navigation', { name: '本书会话', exact: true }).getByRole('button', { name: /^本书历史/ });
    await history.click();
    await page.getByRole('dialog', { name: '对话历史', exact: true }).getByRole('button', { name: '关闭历史', exact: true }).click();
    await expect(history).toBeFocused();
  }
  await page.getByRole('button', { name: '提交回答', exact: true }).click();
  await expect.poll(() => actions.length).toBe(1);
  expect(actions[0]).toMatchObject({ delivery_ref: 'question', action: 'submit', response: '网络处理挤占了 CPU 的能力，因此把适合的工作交给网卡。' });
  await expect(input).toHaveValue('保留这份尚未发送的想法');
  expect(fixture.errors).toEqual([]);
});
