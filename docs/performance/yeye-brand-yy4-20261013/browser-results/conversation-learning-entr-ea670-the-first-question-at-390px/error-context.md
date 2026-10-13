# Instructions

- Following Playwright test failed.
- Explain why, be concise, respect Playwright best practices.
- Provide a snippet of code with the fix, if possible.

# Test info

- Name: conversation-learning-entry.spec.ts >> YY4 welcome becomes a compact identity after the first question at 390px
- Location: playwright\conversation-learning-entry.spec.ts:218:3

# Error details

```
Error: expect(locator).toBeFocused() failed

Locator:  locator('.agent-input textarea')
Expected: focused
Received: inactive
Timeout:  5000ms

Call log:
  - Expect "toBeFocused" with timeout 5000ms
  - waiting for locator('.agent-input textarea')
    14 × locator resolved to <textarea rows="1" value="" data-v-1eec1374="" aria-label="围绕当前阅读内容提问" placeholder="说说你的想法，或继续提问…" data-workspace-input="agent"></textarea>
       - unexpected value "inactive"

```

```yaml
- textbox "围绕当前阅读内容提问":
  - /placeholder: 说说你的想法，或继续提问…
```

# Test source

```ts
  150 |   await page.locator('.tutor-control').getByRole('button',{name:'重试',exact:true}).click();
  151 |   await expect(toggle).toHaveAttribute('aria-checked','false');
  152 |   expect(fixture.mutations[0]).toEqual(fixture.mutations[1]);
  153 |   await page.getByRole('button',{name:'开启并继续这次学习'}).click();
  154 |   await expect(toggle).toHaveAttribute('aria-checked','true');
  155 |   await expect(page.locator('.goal-resume')).toHaveCount(0);
  156 |   await expect.poll(()=>fixture.starts()).toBe(1);
  157 |   await expect(page.locator('.a-msg')).toBeVisible();
  158 |   await page.getByRole('button',{name:'调整目标',exact:true}).click();
  159 |   await page.getByRole('textbox',{name:'学习目标',exact:true}).fill('比较计算量与显存占用');
  160 |   fixture.failNext(); await page.getByRole('button',{name:'保存目标',exact:true}).click();
  161 |   await expect(page.getByRole('textbox',{name:'学习目标',exact:true})).toHaveValue('比较计算量与显存占用');
  162 |   await expect(page.locator('.goal-text')).toHaveText('弄懂 KV Cache 的计算收益与显存代价');
  163 |   await page.getByRole('dialog',{name:'调整目标',exact:true}).getByRole('button',{name:'重试',exact:true}).click();
  164 |   await expect(page.locator('.goal-text')).toHaveText('比较计算量与显存占用');
  165 |   await expect(page.getByRole('textbox',{name:'学习目标',exact:true})).toHaveCount(0);
  166 |   expect(fixture.mutations.at(-1)).toEqual(fixture.mutations.at(-2));
  167 |   expect(fixture.errors).toEqual([]);
  168 | });
  169 | 
  170 | for(const width of [390,320]) test(`phone learning sheet, keyboard-sized viewport and direct understanding at ${width}px`,async({page})=>{
  171 |   await page.setViewportSize({width,height:844});
  172 |   const fixture=await setup(page,makeSession('book','paused'),true);
  173 |   const input=page.locator('.agent-input textarea');
  174 |   await input.fill('保留这份草稿');
  175 |   expect(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth)).toBe(true);
  176 |   await expect(input).toBeInViewport();
  177 |   await page.getByRole('button',{name:'学习与会话',exact:true}).click();
  178 |   await expect(page.getByRole('button',{name:'继续这次学习',exact:false})).toBeInViewport();
  179 |   mkdirSync(output,{recursive:true}); await page.screenshot({path:resolve(output,`paused-${width}.png`)});
  180 |   await page.getByRole('button',{name:'关闭学习与会话',exact:true}).click();
  181 |   await page.getByRole('button',{name:'我的理解',exact:true}).click();
  182 |   const panel=page.getByRole('dialog',{name:'学习详情',exact:true});
  183 |   await expect(panel.getByRole('tab',{name:'我的理解',exact:true})).toHaveAttribute('aria-selected','true');
  184 |   await panel.getByRole('button',{name:'关闭',exact:true}).click();
  185 |   await expect(input).toHaveValue('保留这份草稿');
  186 |   await page.getByRole('button',{name:'学习与会话',exact:true}).click();
  187 |   await expect(page.getByRole('switch',{name:'连续教学'})).toHaveCount(1);
  188 |   await page.getByRole('button',{name:'学习详情',exact:true}).click();
  189 |   await expect(panel).toBeVisible();
  190 |   await expect(panel.getByRole('button',{name:'关闭',exact:true})).toBeFocused();
  191 |   await panel.getByRole('button',{name:'关闭',exact:true}).click();
  192 |   await expect(page.getByRole('button',{name:'学习与会话',exact:true})).toBeFocused();
  193 |   await page.setViewportSize({width,height:500});
  194 |   await expect(input).toBeInViewport();
  195 |   await expect(page.locator('.agent-input').getByRole('button',{name:'发送',exact:true})).toBeInViewport();
  196 |   expect(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth)).toBe(true);
  197 |   expect(fixture.errors).toEqual([]);
  198 | });
  199 | 
  200 | test('a different book offers an explicit new goal without resuming or overwriting the old session',async({page})=>{
  201 |   const old=makeSession('another-book','paused');
  202 |   const fixture=await setup(page,old,true);
  203 |   await expect(page.locator('.conversation-learning-goal')).toContainText('当前学习属于其他书籍');
  204 |   await expect(page.locator('.goal-text')).toHaveCount(0);
  205 |   await expect(page.locator('.goal-resume')).toHaveCount(0);
  206 |   expect(fixture.mutations).toHaveLength(0);
  207 |   await page.getByRole('button',{name:'为这本书设定目标'}).click();
  208 |   await page.getByRole('textbox',{name:'学习目标',exact:true}).fill('读懂当前书籍');
  209 |   await page.getByRole('button',{name:'保存目标',exact:true}).click();
  210 |   await expect(page.locator('.goal-text')).toHaveText('读懂当前书籍');
  211 |   expect(fixture.state.sessions.learning?.material_scope[0].source_id).toBe('another-book');
  212 |   expect(fixture.state.sessions.learning?.status).toBe('paused');
  213 |   expect(fixture.questions).toHaveLength(0);
  214 |   expect(fixture.errors).toEqual([]);
  215 | });
  216 | 
  217 | for (const viewport of [{ width: 1440, height: 900 }, { width: 390, height: 669 }, { width: 320, height: 568 }]) {
  218 |   test(`YY4 welcome becomes a compact identity after the first question at ${viewport.width}px`, async ({ page }, info) => {
  219 |     await page.setViewportSize(viewport);
  220 |     const fixture = await setup(page);
  221 |     const evidence = resolve('../../docs/performance/yeye-brand-yy4-20261013');
  222 |     mkdirSync(evidence, { recursive: true });
  223 |     const welcome = page.locator('.conversation-welcome');
  224 |     const input = page.locator('.agent-input textarea');
  225 |     const identity = page.locator(viewport.width < 1024 ? '.mobile-learning-title' : '#reader-tab-agent');
  226 |     await expect(welcome.getByRole('heading')).toHaveText('这一节，想先弄明白什么？');
  227 |     await expect(welcome.locator('img')).toBeInViewport();
  228 |     await expect(identity.locator('img')).toBeVisible();
  229 |     for (const image of [welcome.locator('img'), identity.locator('img')]) {
  230 |       await expect.poll(() => image.evaluate((el: HTMLImageElement) => el.complete && el.naturalWidth > 0)).toBe(true);
  231 |     }
  232 |     await expect(welcome.locator('.yeye-mascot')).toHaveAttribute('aria-hidden', 'true');
  233 |     expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
  234 |     await expect(input).toBeInViewport();
  235 |     await page.screenshot({ path: resolve(evidence, `welcome-${viewport.width}.png`) });
  236 |     if (viewport.width >= 1024) {
  237 |       await page.getByRole('button', { name: '问答全屏', exact: true }).click();
  238 |       await expect(page.locator('.rail-title img')).toBeVisible();
  239 |       await expect(welcome).toHaveCSS('max-width', '360px');
  240 |       await page.screenshot({ path: resolve(evidence, 'welcome-fullscreen.png') });
  241 |     }
  242 |     await page.getByRole('button', { name: '帮我讲清当前这一节' }).click();
  243 |     await expect(input).toBeFocused();
  244 |     await expect(input).toHaveValue('帮我梳理当前这一节的核心概念。');
  245 |     expect(fixture.questions).toEqual([]);
  246 |     await input.press('Control+Enter');
  247 |     await expect(welcome).toHaveCount(0);
  248 |     await expect(page.locator('.a-msg')).toBeVisible();
  249 |     expect(fixture.questions).toEqual(['帮我梳理当前这一节的核心概念。']);
> 250 |     await expect(input).toBeFocused();
      |                         ^ Error: expect(locator).toBeFocused() failed
  251 |     await input.fill('下一条问题先留在这里');
  252 |     await expect(input).toBeInViewport();
  253 |     await expect(page.locator('.transcript .yeye-mascot')).toHaveCount(0);
  254 |     await page.screenshot({ path: resolve(evidence, `conversation-${viewport.width}.png`) });
  255 |     await info.attach('YY4-layout', { body: JSON.stringify({ viewport, transcript: await page.locator('.transcript').boundingBox(), input: await input.boundingBox() }), contentType: 'application/json' });
  256 |     if (viewport.width < 1024) {
  257 |       await page.getByRole('button', { name: '返回阅读', exact: true }).click();
  258 |       await page.locator('.workspace-mobile-nav').getByRole('button', { name: '问答', exact: true }).click();
  259 |     } else {
  260 |       await page.getByRole('button', { name: '退出问答全屏', exact: true }).click();
  261 |     }
  262 |     await expect(input).toHaveValue('下一条问题先留在这里');
  263 |     await expect(identity.locator('img')).toBeVisible();
  264 |     expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
  265 |     expect(fixture.errors).toEqual([]);
  266 |   });
  267 | }
  268 | 
```