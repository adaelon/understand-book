import { expect, test } from '@playwright/test';
import { mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { resolve } from 'node:path';

// Real Rust service, real AI Infra source, real persisted private progression.
// cargo test -p server --lib inquiry_course_browser_host --config profile.test.debug=0 -- --ignored --nocapture
const output = resolve('../../docs/evidence/inquiry-course-rounds-20261110');
test('course entry survives an opening retry and refresh at compact widths', async ({ page }) => {
  page.setDefaultTimeout(15_000);
  await page.setViewportSize({ width: 893, height: 780 });
  await page.addInitScript(() => {
    for (const context of ['library', 'reader']) localStorage.setItem(`understand-book.product-tour.v1.A.${context}`, 'skipped');
  });
  let rejectOpen = true;
  await page.route('**/api/workspaces', async route => {
    if (route.request().method() === 'POST' && rejectOpen) {
      rejectOpen = false;
      await route.fulfill({ status: 503, contentType: 'application/json', body: JSON.stringify({
        error_code: 'WORKSPACE_CAPACITY', category: 'unavailable', message: 'The operation could not be completed',
      }) });
    } else await route.continue();
  });
  const errors: string[] = []; page.on('pageerror', error => errors.push(error.message));
  const entryOutput = resolve(output, 'entry'); mkdirSync(entryOutput, { recursive: true });
  const course = page.locator('.course-reader .inquiry-course');
  const shell = page.locator('.course-workspace');
  const navigation = page.getByRole('navigation', { name: '书内学习', exact: true });
  try {
    await page.goto('/start');
    await page.locator('.inquiry-course').getByRole('button', { name: '继续学习', exact: true }).click();
    await expect(page.locator('.book-opening')).toContainText('当前阅读窗口已达到上限');
    await page.getByRole('button', { name: '重试打开', exact: true }).click();
    await expect(course.locator('.lesson-top')).toBeVisible();
    await expect(navigation.getByRole('button', { name: '课程', exact: true })).toHaveAttribute('aria-pressed', 'true');
    await expect(shell).toHaveAttribute('data-mode', 'compare');
    await expect(page.locator('.course-dialogue-context')).toBeVisible();
    await expect(page.locator('.agent-input textarea')).toBeVisible();
    await expect(page.locator('.book-opening')).toHaveCount(0);
    await page.screenshot({ path: resolve(entryOutput, 'compact-after-retry.png') });

    await page.reload();
    await expect(course.locator('.lesson-top')).toBeVisible();
    await expect(shell).toHaveAttribute('data-mode', 'compare');
    await expect(page.locator('.course-dialogue-context')).toBeVisible();
    await page.screenshot({ path: resolve(entryOutput, 'compact-after-refresh.png') });
    expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);

    await navigation.getByRole('button', { name: '继续自由阅读', exact: true }).click();
    await page.reload();
    await expect(navigation).toHaveCount(0);
    await page.getByRole('button', { name: '更多阅读工具', exact: true }).click();
    await expect(page.locator('.course-workspace')).toHaveCount(0);
    await page.getByRole('button', { name: '本书课程 →', exact: true }).click();
    await expect(course.locator('.lesson-top')).toBeVisible();

    await page.setViewportSize({ width: 390, height: 844 });
    await expect(shell).toHaveAttribute('data-mode', 'single');
    await expect(course.locator('.lesson-top')).toBeVisible();
    await page.screenshot({ path: resolve(entryOutput, 'mobile-course.png') });
    await page.locator('.workspace-mobile-nav').getByRole('button', { name: '问答', exact: true }).click();
    await expect(page.locator('.agent-input textarea')).toBeVisible();
    await expect(page.locator('.course-dialogue-context')).toBeVisible();
    await page.getByRole('button', { name: '返回学习', exact: true }).click();
    await expect(course.locator('.lesson-top')).toBeVisible();
    expect(errors).toEqual([]);
    writeFileSync(resolve(entryOutput, 'acceptance.json'), JSON.stringify({
      opening_retry_retains_course: true, refresh_retains_course: true, explicit_free_reading_retained: true,
      compact_width: 893, compact_layout: 'compare', mobile_width: 390, mobile_switch_roundtrip: true, errors,
    }, null, 2));
  } catch (error) {
    await page.screenshot({ path: resolve(entryOutput, 'failure.png') });
    writeFileSync(resolve(entryOutput, 'failure.txt'), String(error) + '\n' + await page.locator('body').innerText());
    throw error;
  }
});

test('three investigations preserve predictions, original reading, experiments and explanations before the final task', async ({ page, context }) => {
  page.setDefaultTimeout(15_000);
  const fixture = await page.request.get('http://127.0.0.1:4261/fixture').then(r => r.json());
  const headers = { origin:fixture.origin,cookie:fixture.reader_cookie,'X-CSRF-Token':fixture.reader_csrf };
  const used = await page.request.post('http://127.0.0.1:4260/api/workspace-releases/exam-course/versions/1/use', {
    headers, data:{operation_id:`rounds-${Date.now()}`,goal:{text:'三轮资源决策 · 页面验收',constraints:[]}},
  });
  expect(used.ok(),await used.text()).toBe(true);
  const workspace=(await used.json()).workspace;
  await page.addInitScript(id => sessionStorage.setItem('understand-book:learning:A:selected', id), workspace.learning_workspace_id);
  const errors:string[]=[];page.on('pageerror',e=>errors.push(e.message));
  await context.route(/\/api\//, async route => {
    const request=route.request(),url=new URL(request.url());
    const response=await route.fetch({url:`http://127.0.0.1:4260${url.pathname}${url.search}`,headers:{...await request.allHeaders(),host:'127.0.0.1:4260',origin:fixture.origin,cookie:fixture.reader_cookie}});
    await route.fulfill({response});
  });
  const course=page.locator('.inquiry-course');
  const click=async(name:string)=>{await course.getByRole('button',{name,exact:true}).click();await expect(course).toHaveAttribute('aria-busy','false');};
  const value=async(key:string,n:number)=>{const field=course.locator('#course-value-'+key);await field.fill(String(n));await field.press('Tab');};
  mkdirSync(output,{recursive:true});
  try {
    await page.goto('/?lang=zh-CN');
    await expect(course.getByRole('heading',{name:'从一个问题开始'})).toBeVisible();
    const skip=page.getByRole('button',{name:'跳过引导',exact:true});await skip.click();
    await click('开始这一课');
    await expect(course.getByRole('heading',{name:'先认识这个问题'})).toBeVisible();
    await page.getByRole('button',{name:'跳过引导',exact:true}).click();
    await expect(course.locator('.lesson-top')).toContainText('27 分钟');
    await expect(course.locator('.course-source')).toContainText('1.3.1「生成一个 token，先检查什么」');
    await expect(course.locator('.round-map li')).toHaveCount(4);
    await page.screenshot({path:resolve(output,'desktop-introduction.png')});
    await click('查看本课来源 →');
    await expect(page.getByRole('dialog',{name:'本课来源',exact:true})).toContainText('充分重叠');
    await page.screenshot({path:resolve(output,'desktop-original.png')});
    await page.getByRole('button',{name:'返回当前活动',exact:true}).click();
    await click('做出我的预测 →');
    const rounds=[
      {choice:'未必，要先比较计算与读取各自需要的时间。',reason:'我需要比较两项时间，单独加快计算可能仍受读取限制。',expected:['20.90','20.90','10.45'],reflection:'原配置计算约 0.1415 ms，读取约 20.90 ms；加算力不改变读取。带宽翻倍或读取量减半得到约 10.45 ms，仍超过 9 ms。'},
      {choice:'第一次升级可能降低下界，但读取会接替成为限制；继续加算力可能不再有效。',reason:'两项分别为 10 和 8 ms，我预测算力升级会在读取项处停止降低下界。',expected:['10.00','8.00','10.00'],reflection:'算力为 60 时是 (10,8)，为 75 时是 (8,8)，为 120 时是 (5,8)。C=75 是 F/C=R/W 的交点，再加算力只沿曲面平移；增加带宽后才可能继续降低。'},
      {choice:'需要重新检查两项约束；如果两项都超时，只改其中一项仍可能不够。',reason:'新工作量使两项都可能超过目标，应同时检查 C 和 W 的门槛。',expected:['12.00','12.00','12.00'],reflection:'保存的 C=200、W=6 对应 7.2 与 8 ms，目标 9 ms，模型余量 1 ms。9 ms 要求 C 至少160、W至少16/3；只加一项仍有另一项12 ms。实测11 ms符合下界但未达标，需要检查利用率、重叠与其他开销。'},
    ];
    for(let i=0;i<rounds.length;i++){
      const round=rounds[i]!;
      await expect(course.locator('.round-caption')).toContainText(`第 ${i+1} / 3 轮`);
      await expect(course.locator('.decision-lab')).toHaveCount(0);
      await course.getByLabel(round.choice,{exact:true}).check();
      await course.locator('#course-reason').fill(round.reason);
      if(i===1){
        await click('查看本课来源 →');await page.getByRole('button',{name:'返回当前活动',exact:true}).click();
        await expect(course.locator('#course-reason')).toHaveValue(round.reason);
      }
      await click('保存预测，开始实验 →');
      for(const [j,name] of ['原配置','算力翻倍','带宽翻倍'].entries()){
        await course.getByRole('button',{name,exact:true}).click();await expect(course).toHaveAttribute('aria-busy','false');
        await expect(course.locator('.bound strong')).toContainText(round.expected[j]!);
      }
      await click('恢复本轮实验起点');
      if(i===0)await value('read_gb',35);
      if(i===1){
        await value('compute_tflops',75);await expect(course.locator('.bound')).toContainText('两项相等');
        await value('compute_tflops',120);await value('bandwidth_tb_s',4);await expect(course.locator('.bound strong')).toContainText('5.00');
      }
      if(i===2){
        await value('compute_tflops',160);await value('bandwidth_tb_s',5.4);await expect(course.locator('.bound strong')).toContainText('9.00');
        await value('compute_tflops',200);await value('bandwidth_tb_s',6);await expect(course.locator('.bound strong')).toContainText('8.00');
        await value('target_ms',6);await expect(course.locator('.target-result')).toContainText('直接排除');
        await value('target_ms',9);
      }
      const surface=course.getByRole('img',{name:'可旋转的三维时间下界曲面',exact:true});
      const before=await surface.locator('polygon').first().getAttribute('points');await surface.press('ArrowRight');
      expect(await surface.locator('polygon').first().getAttribute('points')).not.toBe(before);
      await click('保存这个实验配置');
      if(i===1){
        await click('暂停，稍后继续');await page.reload();
        await page.getByRole('button',{name:'更多阅读工具',exact:true}).click();
        await page.getByRole('button',{name:'本书课程 →',exact:true}).click();
        await expect(course.getByText('停在这里也没关系')).toBeVisible();await click('从这里继续');
        await expect(course.locator('.round-caption')).toContainText('第 2 / 3 轮');
        await expect(course.locator('.bound strong')).toContainText('5.00');
        await expect(course.getByText('已观察 3 / 3 种配置')).toBeVisible();
      }
      if(i===2){
        await course.locator('.plots').scrollIntoViewIfNeeded();await page.screenshot({path:resolve(output,'desktop-third-experiment.png')});
        await page.setViewportSize({width:390,height:844});await course.locator('.plots').scrollIntoViewIfNeeded();
        expect(await course.evaluate(el=>el.scrollWidth<=el.clientWidth+1)).toBe(true);
        await page.screenshot({path:resolve(output,'mobile-third-experiment.png')});
      }
      await click('对照预测，看看原因 →');
      await expect(course.getByRole('button',{name:i<2?'继续下一轮探究 →':'进入综合决策题 →',exact:true})).toBeDisabled();
      await course.locator('#course-reflection').fill(round.reflection);
      if(i===2){
        await click('核对本课来源 →');await page.getByRole('button',{name:'返回当前活动',exact:true}).click();
        await expect(course.locator('#course-reflection')).toHaveValue(round.reflection);
      }
      await click('提交我的解释');await expect(course).toContainText('解释已保存');
      await click(i<2?'继续下一轮探究 →':'进入综合决策题 →');
    }
    await expect(course.locator('.question')).toContainText('1200 GFLOPs');
    await expect(course.locator('.solution')).toHaveCount(0);
    await course.getByLabel('先升级算力并实测；工作量变化后要重新检查两条约束。',{exact:true}).check();
    const answer='算力翻倍下界为 max(6,8)=8 ms，带宽翻倍为 max(12,4)=12 ms，先升算力。新任务沿用升级后配置为 (12,12) ms，两个资源都要检查；下界满足不保证实测满足，需要相同工作量与精度下的延迟和利用率证据。';
    await course.locator('#course-reason').fill(answer);
    await page.getByRole('button',{name:'实验',exact:true}).click();await click('当前算力 ×2');await click('保存这个实验配置');
    await page.getByRole('button',{name:'课程',exact:true}).click();await expect(course.locator('#course-reason')).toHaveValue(answer);
    await click('提交并查看反馈 →');await expect(course.getByRole('heading',{name:'方案选择正确，再看理由是否成立'})).toBeVisible();
    await click('完成这一课 ✓');await expect(course.getByText('本课已完成',{exact:true})).toBeVisible();
    await course.locator('.round-history>summary').click();await expect(course.locator('.round-history>article')).toHaveCount(3);
    await page.screenshot({path:resolve(output,'mobile-completed.png')});
    const path=`http://127.0.0.1:4260/api/learning-workspaces/${workspace.learning_workspace_id}/shared-release/course`;
    const catalog=await page.request.get(path,{headers}).then(r=>r.json());
    const saved=await page.request.get(path+'?run_id='+encodeURIComponent(catalog.progress[0].run_id),{headers}).then(r=>r.json());
    expect(saved.round_history).toHaveLength(3);expect(saved.expressions).toHaveLength(7);expect(saved.progress.status).toBe('completed');
    expect(saved.round_history.map((r:any)=>r.reflection)).toEqual(rounds.map(r=>r.reflection));
    expect(saved.round_history[0].exploration.read_gb).toBe(35);
    expect(saved.round_history[1].exploration.compute_tflops).toBe(120);
    expect(saved.round_history[2].exploration.compute_tflops).toBe(200);
    writeFileSync(resolve(output,'acceptance.json'),JSON.stringify({date:'2026-11-10',source:saved.summary.source,progress:saved.progress,round_history:saved.round_history,expressions:saved.expressions,api_model_calls:'Provider not configured for this acceptance host',errors},null,2)+'\n');
    expect(errors).toEqual([]);
  } catch(e){await page.screenshot({path:resolve(output,'failure.png'),fullPage:true}).catch(()=>{});throw e;}
  finally {await context.unrouteAll({behavior:'ignoreErrors'}).catch(()=>{});}
});

test('a repaired DeepSeek candidate publishes and runs through the same player', async ({ page, context }) => {
  page.setDefaultTimeout(15_000);
  const base = 'http://127.0.0.1:4260';
  const fixture = await page.request.get('http://127.0.0.1:4261/fixture').then(r => r.json());
  const readerHeaders = { origin: fixture.origin, cookie: fixture.reader_cookie, 'X-CSRF-Token': fixture.reader_csrf };
  const login = await page.request.post(`${base}/api/auth/login`, { headers: { origin: fixture.origin }, data: { username: 'B', password: 'fixture-only-password' } });
  expect(login.ok(), await login.text()).toBe(true);
  const adminCookie = login.headers()['set-cookie'].split(';')[0];
  const admin = await page.request.get(`${base}/api/auth/me`, { headers: { cookie: adminCookie } }).then(r => r.json());
  const original = await page.request.get(`${base}/api/workspace-releases/exam-course/versions/1`, { headers: readerHeaders }).then(r => r.json());
  const candidate = JSON.parse(readFileSync(resolve('../../docs/evidence/inquiry-course-20261108/deepseek/candidate.repaired.course.json'), 'utf8'));
  candidate.lessons[0].summary.source = original.manifest.inquiry_course.lessons[0].source;
  const releaseId = `deepseek-trial-${Date.now()}`;
  const published = await page.request.post(`${base}/api/admin/workspace-releases`, {
    headers: { origin: fixture.origin, cookie: adminCookie, 'X-CSRF-Token': admin.csrf_token },
    data: { operation_id: releaseId, release_id: releaseId, expected_version: 0, visible_to_users: ['A', 'B'],
      manifest: { ...original.manifest, name: 'AI Infra · DeepSeek 试制课', inquiry_course: candidate } },
  });
  expect(published.ok(), await published.text()).toBe(true);
  const use = await page.request.post(`${base}/api/workspace-releases/${releaseId}/versions/1/use`, {
    headers: readerHeaders, data: { operation_id: `use-${releaseId}`, goal: { text: '比较改变参数后的资源限制', constraints: [] } },
  });
  expect(use.ok(), await use.text()).toBe(true);
  const workspace = (await use.json()).workspace;
  await page.addInitScript(id => sessionStorage.setItem('understand-book:learning:A:selected', id), workspace.learning_workspace_id);
  await context.route(/\/api\//, async route => {
    const request = route.request(), url = new URL(request.url());
    const response = await route.fetch({ url: `${base}${url.pathname}${url.search}`, headers: { ...await request.allHeaders(), host: '127.0.0.1:4260', origin: fixture.origin, cookie: fixture.reader_cookie } });
    await route.fulfill({ response });
  });
  try {
    await page.goto('/');
    await page.getByRole('button', { name: '跳过引导', exact: true }).click();
    const course = page.getByRole('region', { name: '探究课程', exact: true });
    await course.getByRole('button', { name: '开始这一课', exact: true }).click();
    await course.getByRole('button', { name: '做出我的预测' }).click();
    await course.getByLabel('还不能确定，我想先试一试', { exact: true }).check();
    await course.getByRole('button', { name: '保存预测，开始实验' }).click();
    for (const label of ['原配置', '只把算力翻倍', '只把带宽翻倍']) await course.getByRole('button', { name: label, exact: true }).click();
    await course.getByRole('button', { name: '对照预测，看看原因' }).click();
    await course.getByRole('button', { name: '进入综合决策题' }).click();
    await expect(course.locator('.question')).toContainText('600 GFLOPs');
    await expect(course).toContainText('9 ms 与 12 ms；把计算与读取时间相加');
    await course.getByLabel('5 ms 与 10 ms；计算项较慢，算力翻倍降低计算下界，带宽翻倍后读取仍不是限制', { exact: true }).check();
    await course.getByRole('button', { name: '提交并查看反馈' }).click();
    await expect(course).toContainText('方案选择正确，再看理由是否成立');
    await expect(course).toContainText('600 ÷ 120 = 5 ms');
    await course.getByRole('button', { name: '完成这一课' }).click();
    await expect(course).toContainText('本课已完成');
    await page.screenshot({ path: resolve(output, 'deepseek-course-complete-desktop.png'), animations: 'disabled' });
    const withoutCourse = await page.request.post(`${base}/api/admin/workspace-releases`, {
      headers: { origin: fixture.origin, cookie: adminCookie, 'X-CSRF-Token': admin.csrf_token },
      data: { operation_id: `${releaseId}-v2`, release_id: releaseId, expected_version: 1, visible_to_users: ['A', 'B'],
        manifest: { ...original.manifest, name: 'AI Infra · DeepSeek 试制课', inquiry_course: null } },
    });
    expect(withoutCourse.ok(), await withoutCourse.text()).toBe(true);
    await page.getByRole('button', { name: '学习资料', exact: true }).click();
    await page.getByRole('button', { name: '检查共享版本', exact: true }).click();
    await page.getByRole('button', { name: '查看新版变化', exact: true }).click();
    await expect(page.getByText('课程内容或学习路线已调整。', { exact: true })).toBeVisible();
    await page.getByRole('button', { name: '选择更新到此版本', exact: true }).click();
    await page.reload();
    await course.locator('summary').filter({ hasText: '以前的学习记录' }).click();
    await course.locator('.history button').click();
    await expect(course).toContainText('本课已完成');
    await expect(course).toContainText('600 ÷ 120 = 5 ms');
    await expect(course.getByRole('button', { name: '从头学习', exact: true })).toHaveCount(0);
    writeFileSync(resolve(output, 'historical-candidate-acceptance.json'), JSON.stringify({
      release_id: releaseId, release_version: 1, source: candidate.lessons[0].summary.source,
      real_publication_accepted: true, same_player_completed: true, result: 'correct', history_read_after_removal: true,
      target_learner_trial: 'pending', scope: 'Isolated local service; no production publication',
    }, null, 2) + '\n');
  } finally { await context.unrouteAll({ behavior: 'ignoreErrors' }).catch(() => {}); }
});
