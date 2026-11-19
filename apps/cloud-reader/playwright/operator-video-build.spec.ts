import { expect, test } from '@playwright/test';
import { mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { resolve } from 'node:path';

// Uses s2_b1_browser_host plus operator-video-http.acceptance.ts's completed real pipeline.
// Intermediate UI counts below are controlled host observations, not an additional model run.
const backend='http://127.0.0.1:4190';
const output=resolve('../../docs/performance/video-operator-20261018');
test.afterEach(async({page},info)=>{if(info.status!==info.expectedStatus){await page.screenshot({path:info.outputPath('failure.png'),fullPage:true});writeFileSync(info.outputPath('body.txt'),await page.locator('body').innerText());}});

test('operator starts video once, sees actual counts, recovers budget and publishes the completed video',async({page,context})=>{
  test.setTimeout(150_000);page.setDefaultTimeout(20_000);
  const fixture=await page.request.get('http://127.0.0.1:4191/fixture').then(r=>r.json());
  const built=JSON.parse(readFileSync(resolve('../../target/video-operator-20261018/http-acceptance-result.json'),'utf8'));
  const errors:string[]=[];page.on('pageerror',error=>errors.push(error.message));
  const writes:{path:string;body:any}[]=[];
  await context.route(/\/api\/|\/books\//,async route=>{
    const request=route.request(),url=new URL(request.url());
    if(request.method()==='POST')writes.push({path:url.pathname,body:request.postDataJSON()});
    const response=await route.fetch({url:`${backend}${url.pathname}${url.search}`,headers:{...await request.allHeaders(),host:'127.0.0.1:4190',origin:fixture.origin,cookie:fixture.admin_cookie}});
    await route.fulfill({response});
  });
  const api=async(path:string,body?:unknown)=>{
    const response=await page.request.fetch(`${backend}${path}`,{method:body===undefined?'GET':'POST',headers:{origin:fixture.origin,cookie:fixture.admin_cookie,'X-CSRF-Token':fixture.admin_csrf},...(body===undefined?{}:{data:body})});
    expect(response.ok(),`${path}: ${await response.text()}`).toBe(true);return response.json();
  };
  const suffix=Date.now(),name=`视频页面验收 ${suffix}`,host=`video-ui-${suffix}`;
  await page.setViewportSize({width:1440,height:1000});await page.goto('http://127.0.0.1:4184/admin/operator-builds');
  await page.getByRole('button',{name:'创建准备任务',exact:true}).click();
  const create=page.getByRole('dialog',{name:'创建准备任务',exact:true});
  await create.getByLabel('准备任务名称',{exact:true}).fill(name);
  await create.getByLabel('平台预算上限（元）',{exact:true}).fill('0.03');
  await create.getByLabel('每次执行预计占用（元）',{exact:true}).fill('0.01');
  await create.getByLabel('成果授权账号（空格或逗号分隔）',{exact:true}).fill('A, B');
  await create.getByLabel('资料名称',{exact:true}).fill('界面进度课程视频');
  await create.getByLabel('来源文件或工作区位置',{exact:true}).fill(built.source);
  await create.getByLabel('成果根目录',{exact:true}).fill(`${fixture.root}/video-ui-${suffix}`);
  await expect(create.getByLabel('资料类型',{exact:true})).toHaveValue('video');
  await expect(create).toContainText('不预生成 Tutor 教学材料');
  await create.getByRole('button',{name:'创建并开始构建',exact:true}).click();
  await expect(create).not.toBeVisible();
  const detail=page.getByRole('region',{name:'运营构建任务详情',exact:true});
  await expect(detail).toContainText('等待构建宿主执行');
  expect(writes.filter(write=>write.path==='/api/admin/operator-builds')).toHaveLength(1);
  const submitted=writes.find(write=>write.path==='/api/admin/operator-builds')!.body;
  const taskId=submitted.task_id,path=`/api/admin/operator-builds/${taskId}`;
  expect(writes.filter(write=>write.path===`${path}/start`)).toHaveLength(1);
  expect(writes.some(write=>write.path===`${path}/prepare`)).toBe(false);
  expect(submitted.materials[0]).toMatchObject({material_kind:'video',build_tutor:false,required_capabilities:['text','video','translation','visual_index','graph','discourse']});
  const claimed=await api('/api/admin/operator-builds/next',{host_session_id:host});expect(claimed.task.task_id).toBe(taskId);
  const workspace=`${fixture.root}/video-ui-${suffix}/workspace`;
  const progressPath=`${path}/materials/source-1/progress`;
  await api(progressPath,{host_session_id:host,state:'running',workspace_dir:workspace,invocation_ref:`video:${taskId}:source-1`,plan:{version:'operator_video_plan.v1',workspace_dir:workspace,review_markdown:'准备字幕、画面、知识关系和讲解结构；不预生成 Tutor 教学材料。',build_tutor:false},observation:{progress:{stage:'translation',completed:2,total:5,unit:'batches'}}});
  const progress=detail.getByRole('status',{name:'界面进度课程视频构建进度',exact:true});
  await expect(progress).toContainText('本阶段 2 / 5 批（40%）');
  await expect(progress.getByRole('progressbar')).toHaveAttribute('value','2');
  mkdirSync(output,{recursive:true});await detail.scrollIntoViewIfNeeded();await page.screenshot({path:resolve(output,'video-progress-desktop.png'),fullPage:true});
  await page.reload();await page.getByRole('row').filter({hasText:name}).getByRole('button',{name:'查看构建任务'}).click();
  await expect(progress).toContainText('本阶段 2 / 5 批（40%）');
  await api(progressPath,{host_session_id:host,state:'running',observation:{progress:{stage:'transcript',completed:0,total:null,unit:'chunks'}}});
  await expect(progress).toContainText('暂不估算百分比');await expect(progress.getByRole('progressbar')).toHaveCount(0);
  await expect(progress).not.toContainText('100%');
  const costPath=`/api/admin/build-tasks/${taskId}`;
  for(let index=0;index<3;index++){
    const call=`video-ui-call-${suffix}-${index}`;
    await api(`${costPath}/admit`,{execution_id:call,call_id:call,logical_call_id:call,attempt:1,granularity:'execution_total',reserved_micro_cny:10_000,provider:'ui-fixture',model:'controlled-host',purpose:'external_build',budget_enforced:false,receipt_capability:'partial'});
    await api(`${costPath}/receipts`,{call_id:call,status:'unknown',evidence:{source:'browser_controlled_host'},succeeded:true});
  }
  const denied=await page.request.post(`${backend}${costPath}/admit`,{headers:{origin:fixture.origin,cookie:fixture.admin_cookie,'X-CSRF-Token':fixture.admin_csrf},data:{execution_id:`denied-${suffix}`,call_id:`denied-${suffix}`,logical_call_id:`denied-${suffix}`,attempt:1,granularity:'execution_total',reserved_micro_cny:10_000,provider:'ui-fixture',model:'controlled-host',purpose:'external_build',budget_enforced:false,receipt_capability:'partial'}});
  expect(denied.status()).toBe(409);expect((await denied.json()).error_code).toBe('BUILD_BUDGET_EXHAUSTED');
  await detail.getByRole('button',{name:'暂停构建',exact:true}).click();
  const pause=page.getByRole('dialog',{name:'暂停构建',exact:true});await pause.getByRole('button',{name:'确认提交',exact:true}).click();await expect(pause).not.toBeVisible();
  await api(`${path}/finish`,{host_session_id:host,state:'interrupted',error:'BUILD_BUDGET_EXHAUSTED'});
  await expect(detail).toContainText('本任务剩余预算不足');
  await detail.getByRole('button',{name:'查看费用回执',exact:true}).click();
  const receipts=detail.getByRole('region',{name:'构建费用回执',exact:true});
  await expect(receipts.getByRole('row').filter({hasText:'待核算'})).toHaveCount(3);
  await detail.getByRole('button',{name:'继续构建',exact:true}).click();
  const resume=page.getByRole('dialog',{name:'继续构建',exact:true});await resume.getByLabel('确认平台预算上限（元）',{exact:true}).fill('0.05');await resume.getByRole('button',{name:'确认提交',exact:true}).click();await expect(resume).not.toBeVisible();
  const resumed=await api('/api/admin/operator-builds/next',{host_session_id:host});expect(resumed.task.task_id).toBe(taskId);expect(resumed.task.materials[0].invocation_ref).toBe(`video:${taskId}:source-1`);expect(resumed.task.budget.pending_micro_cny).toBe(30_000);expect(resumed.task.budget.remaining_micro_cny).toBe(20_000);
  await api(`${path}/finish`,{host_session_id:host,state:'interrupted',error:'界面状态验收完成'});
  // Inspect the separately executed real import/transcript/model-stage/publisher path.
  const actual=await api(`/api/admin/operator-builds/${built.task_id}`);
  await detail.getByRole('button',{name:'关闭构建详情',exact:true}).click();
  const listed=await api('/api/admin/operator-builds?limit=20&offset=0');
  const actualIndex=listed.items.findIndex((task:{task_id:string})=>task.task_id===built.task_id);expect(actualIndex).toBeGreaterThanOrEqual(0);
  await page.getByRole('row').nth(actualIndex+1).getByRole('button',{name:'查看构建任务'}).click();
  await expect(detail).toContainText(actual.name);await expect(detail).toContainText(`${actual.budget.unknown_count} 笔费用未完整确认`);
  await detail.getByRole('button',{name:'查看费用回执',exact:true}).click();
  await expect(receipts.getByRole('row').filter({hasText:'待核算'})).toHaveCount(actual.budget.unknown_count);
  await detail.getByRole('button',{name:'选择成果并发布',exact:true}).click();
  const release=page.getByRole('dialog',{name:'发布新工作区',exact:true});
  const releaseId=`video-browser-${suffix}`;
  for(const [field,value] of Object.entries({release_id:releaseId,name:'视频预构建课程',publisher:'课程运营',public_goal:'学习视频中的课程知识',applicability:'课程复习',prerequisites:'无需先修知识'}))await release.locator(`[name="${field}"]`).fill(value);
  await expect(release.getByLabel('视频伴读',{exact:true})).toBeChecked();await expect(release.getByLabel('资料理解',{exact:true})).not.toBeChecked();
  await release.getByRole('button',{name:'确认提交',exact:true}).click();await expect(release).not.toBeVisible();
  const published=await api(`/api/workspace-releases/${releaseId}/versions/1`);expect(published.availability.status).toBe('ready');expect([...published.manifest.materials[0].required_capabilities].sort()).toEqual(['text','video','translation','visual_index','graph','discourse'].sort());
  await page.setViewportSize({width:390,height:844});await detail.scrollIntoViewIfNeeded();expect(await page.locator('body').evaluate(element=>element.scrollWidth<=window.innerWidth)).toBe(true);await page.screenshot({path:resolve(output,'video-receipts-mobile.png'),fullPage:true});
  expect(errors).toEqual([]);
  writeFileSync(resolve(output,'browser-verification.json'),JSON.stringify({status:'passed',controlled_ui_task:taskId,real_pipeline_task:built.task_id,create_requests:1,start_requests:1,plan_confirmation_requests:0,controlled_stage:{completed:2,total:5,percent:40},unknown_total_has_no_percentage:true,budget_admission_rejected:true,resumed_pending_micro_cny:resumed.task.budget.pending_micro_cny,release_id:releaseId,capabilities:published.manifest.materials[0].required_capabilities,errors},null,2));
});
