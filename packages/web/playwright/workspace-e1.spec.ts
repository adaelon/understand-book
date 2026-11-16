import {test,expect,type Page,type BrowserContext} from '@playwright/test';
import {mkdirSync,writeFileSync} from 'node:fs';
import path from 'node:path';

const output=path.resolve('../../docs/performance/workspace-e1-20261018');
test.afterEach(async({page,context},info)=>{
  if(info.status!==info.expectedStatus){await page.screenshot({path:info.outputPath('failure.png'),fullPage:true});writeFileSync(info.outputPath('body.txt'),await page.locator('body').innerText());}
  await context.unrouteAll({behavior:'ignoreErrors'});
});
const scene=(page:Page)=>page.evaluate(async()=>{const module='/src/network-context.ts';return(await import(module)).network.value.workspace;});

test('E1 shared learning, personal preparation, video, history and allowance remain connected',async({page,context,browser})=>{
  test.setTimeout(180000);page.setDefaultTimeout(15000);mkdirSync(output,{recursive:true});
  const fixture=await(await page.request.get('http://127.0.0.1:4191/fixture')).json();
  const errors:string[]=[],writes:any[]=[],navigation:any[]=[];page.on('pageerror',e=>errors.push(e.message));
  let releaseUpload=()=>{},uploadHeld=false;
  const uploadGate=new Promise<void>(resolve=>{releaseUpload=resolve;});
  const proxy=async(target:BrowserContext,cookie:string)=>target.route(/\/api\/|\/books\//,async route=>{
    const req=route.request(),url=new URL(req.url()),body=req.method()==='POST'?req.postDataJSON():null;
    if(body)writes.push({path:url.pathname,body});
    if(body?.action==='upload'&&body.offset>0&&!uploadHeld){uploadHeld=true;await uploadGate;}
    const response=await route.fetch({url:`http://127.0.0.1:4190${url.pathname}${url.search}`,headers:{...await req.allHeaders(),host:'127.0.0.1:4190',origin:fixture.origin,cookie}});
    if(req.method()==='POST'&&/\/workspaces\//.test(url.pathname)&&/\/(book\/open|chat\/select|attach)$/.test(url.pathname)){
      const value=await response.json();navigation.push({path:url.pathname,input:body,published_book_ref:value.published_book_ref,learning_workspace_id:value.learning_workspace_id,generation:value.generation,revision:value.revision});
    }
    await route.fulfill({response});
  });
  await proxy(context,fixture.reader_cookie);
  const secondContext=await browser.newContext({baseURL:'http://127.0.0.1:4174'});await proxy(secondContext,fixture.admin_cookie);const other=await secondContext.newPage();other.on('pageerror',e=>errors.push(e.message));
  const api=async(route:string,body?:any,admin=false)=>{
    const r=await page.request.fetch(`http://127.0.0.1:4190/api${route}`,{method:body===undefined?'GET':'POST',headers:{origin:fixture.origin,cookie:admin?fixture.admin_cookie:fixture.reader_cookie,'X-CSRF-Token':admin?fixture.admin_csrf:fixture.reader_csrf},...(body===undefined?{}:{data:body})});
    expect(r.ok(),`${route}: ${await r.text()}`).toBeTruthy();return r.json();
  };
  try{
    let legacy=await api('/workspaces',{published_book_ref:fixture.first,attachment_id:'e1-legacy'});
    legacy=await api(`/workspaces/${legacy.workspace_id}/chat/new`,{generation:legacy.generation,expected_revision:legacy.revision,attachment_id:'e1-legacy'});
    await api('/agent/chat',{workspace_id:legacy.workspace_id,generation:legacy.generation,expected_revision:legacy.revision,attachment_id:'e1-legacy',session_id:legacy.selected_chat,client_request_id:crypto.randomUUID(),message:'旧记录：我想分清速度与变化率'});
    await expect.poll(async()=>(await api(`/me/learning-records/${legacy.selected_chat}`)).turns.at(-1)?.status).toBe('completed');
    const legacyRecord=(await api('/me/learning-records')).records.find((r:any)=>r.session_id===legacy.selected_chat);
    const oldDetail=await api(`/me/learning-records/${legacy.selected_chat}?through_seq=${legacyRecord.through_seq}`);delete oldDetail.recap.generated_at;
    const initialCosts=await api('/admin/cost-summary',undefined,true);
    const use=async(target:Page,goal:string)=>{
      await target.goto('/');await target.getByRole('button',{name:'浏览共享预构建工作区',exact:true}).click();
      await target.locator('.shared-card').filter({hasText:'深度学习 · 书与视频'}).getByRole('button',{name:'查看目标与路线',exact:true}).click();
      const detail=target.getByRole('article',{name:'共享工作区详情',exact:true});await detail.getByLabel('确认你的学习目标').fill(goal);await detail.getByRole('button',{name:'确认目标并开始学习',exact:true}).click();
      await expect.poll(async () => (await scene(target))?.learning_workspace_goal?.text).toBe(goal);return scene(target);
    };
    const goal='用书籍与视频理解深度学习';const first=await use(page,goal),second=await use(other,'保持原版，按自己的节奏学习');
    expect(first.learning_workspace_id).not.toBe(second.learning_workspace_id);
    const manage=async()=>{await page.getByRole('button',{name:'学习资料',exact:true}).click();await expect(page.getByRole('region',{name:'当前工作区',exact:true})).toBeVisible();};
    await manage();const panel=page.getByRole('region',{name:'个人资料准备',exact:true}),zone=panel.getByRole('group',{name:'资料上传区'});
    await page.setViewportSize({width:1440,height:1000});await panel.screenshot({path:path.join(output,'upload-desktop.png')});
    await page.setViewportSize({width:390,height:844});await panel.scrollIntoViewIfNeeded();expect(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth+1)).toBe(true);await panel.screenshot({path:path.join(output,'upload-mobile.png')});
    await page.setViewportSize({width:1440,height:1000});
    const drop=async(name:string)=>{const transfer=await page.evaluateHandle(name=>{const value=new DataTransfer();value.items.add(new File(['# Notes\nLearning notes.'],name,{type:'text/plain'}));return value;},name);await zone.dispatchEvent('drop',{dataTransfer:transfer});await transfer.dispose();};
    await drop('unsupported.pdf');await expect(panel.getByRole('alert')).toContainText('请选择 EPUB');expect(writes.filter(v=>v.body.action==='upload')).toHaveLength(0);
    const content=Buffer.from('# Transactions\n\n'+'Transactions group operations into one atomic change.\n'.repeat(1500));
    const chooserEvent=page.waitForEvent('filechooser');await panel.getByRole('button',{name:'选择文件',exact:true}).focus();await page.keyboard.press('Enter');const chooser=await chooserEvent;
    await chooser.setFiles({name:'期末复习笔记.md',mimeType:'text/markdown',buffer:content});
    await expect.poll(()=>uploadHeld).toBe(true);await expect(panel.getByRole('progressbar',{name:'资料上传进度'})).toHaveAttribute('value','40');await expect(panel).toContainText('79.1 KB');
    await panel.screenshot({path:path.join(output,'upload-progress.png')});releaseUpload();await expect(panel).toContainText('等待准备');
    const created=writes.find(v=>v.body.action==='create'&&v.body.upload_id),id=created.body.task_id,scope=first.learning_workspace_id;
    expect(created.path).toBe(`/api/learning-workspaces/${scope}/preparations`);
    const task=panel.locator('article').filter({hasText:'期末复习笔记.md'}),endpoint=`/admin/operator-builds/${id}`,host='e1-browser-host';
    await task.getByRole('button',{name:'生成准备方案',exact:true}).click();await api('/admin/operator-builds/next',{host_session_id:host},true);
    const directory=`${fixture.root}/library/published/${fixture.second.book_id}/${fixture.second.publication_id}`;
    await api(`${endpoint}/materials/source/progress`,{host_session_id:host,state:'planned',workspace_dir:directory,plan:{build_plan_path:`${directory}/plan.json`,workspace_dir:directory,plan:{private_artifacts:[],estimate:{input_tokens:{lower:20,upper:100},output_tokens:{lower:10,upper:50},unknown_stages:['structure']}},model_runtime:{provider:'probe',model:'scripted',max_output_tokens:100,context_window_tokens:1000},review_markdown:'原文与基础理解'}},true);
    await api(`${endpoint}/finish`,{host_session_id:host,state:'awaiting_confirmation'},true);
    await expect(task).toContainText('请确认额度上限');await task.getByLabel('期末复习笔记.md额度上限').fill('1.25');await task.getByRole('button',{name:'确认上限并开始',exact:true}).click();await expect(task).toContainText('等待开始');
    await task.getByRole('button',{name:'暂停准备',exact:true}).click();await expect(task).toContainText('已暂停');
    await page.getByLabel('这次想学会什么？').fill('另一个独立工作区');await page.getByRole('button',{name:'创建工作区',exact:true}).click();await expect(panel).not.toContainText('期末复习笔记.md');
    await page.locator('.learning-space-card').filter({hasText:goal}).getByRole('button',{name:'管理工作区',exact:true}).click();await expect(task).toContainText('已暂停');
    await task.getByRole('button',{name:'确认当前额度并继续',exact:true}).click();await expect(task).toContainText('等待开始');await api('/admin/operator-builds/next',{host_session_id:host},true);
    const call=crypto.randomUUID();await api(`/admin/build-tasks/${id}/metered-admit`,{execution_id:'e1-browser',call_id:call,logical_call_id:call,attempt:1,granularity:'calls',provider:'probe',model:'scripted',purpose:'external_build',budget_enforced:true,receipt_capability:'complete',request:{model:'scripted',input_tokens_estimate:25,max_tokens:25}},true);
    await api(`/admin/build-tasks/${id}/metered-receipts`,{call_id:call,status:'confirmed',usage:{input_tokens:14,cached_input_tokens:4,output_tokens:2,total_tokens:16},evidence:{source:'controlled UI host'},succeeded:true},true);
    await api(`${endpoint}/materials/source/progress`,{host_session_id:host,state:'complete',workspace_dir:directory,invocation_ref:'e1-controlled-ui'},true);
    await api(`${endpoint}/materials/source/import`,{host_session_id:host,workspace_dir:directory,invocation_ref:'e1-controlled-ui'},true);await api(`${endpoint}/finish`,{host_session_id:host,state:'ready'},true);
    await expect(task).toContainText('已加入原工作区');await expect(task).toContainText('已用 低于 0.1%');
    const preparedReference=(await api(`/learning-workspaces/${scope}/preparations`)).items.find((item:any)=>item.task_id===id).materials[0].published_book_ref;
    await task.getByRole('button',{name:'继续学习',exact:true}).click();await expect.poll(async()=>(await scene(page)).published_book_ref).toEqual(preparedReference);await expect(page.locator('.reader-pane')).toBeVisible();await manage();
    await drop('课堂补充.txt');await expect(panel.locator('article').filter({hasText:'课堂补充.txt'})).toContainText('等待准备');
    await page.getByRole('button',{name:'带入已有学习记录',exact:true}).click();const records=page.getByRole('region',{name:'带入已有学习记录',exact:true});const choice=records.locator('.record-choice').filter({hasText:legacyRecord.title});
    await choice.getByRole('checkbox').check();await records.getByRole('button',{name:'确认带入 1 份记录',exact:true}).click();await expect(choice.getByRole('checkbox')).toBeDisabled();
    const kept=await api(`/me/learning-records/${legacy.selected_chat}?through_seq=${legacyRecord.through_seq}`);delete kept.recap.generated_at;expect(kept).toEqual(oldDetail);
    await page.getByRole('button',{name:'使用额度',exact:true}).click();const allowance=page.getByRole('dialog',{name:'AI 使用额度'});await expect(allowance).toContainText('本期已用');await expect(allowance).toContainText('%');await allowance.getByRole('button',{name:'按任务与工作区查看',exact:true}).click();await expect(allowance).toContainText('任务');await allowance.getByRole('button',{name:'关闭使用额度'}).click();
    const catalog=(await api('/library')).books;
    const videoTitle=catalog.find((book:any)=>book.published_book_ref.book_id===fixture.video.book_id&&book.published_book_ref.publication_id===fixture.video.publication_id).title||fixture.video.book_id.replace(/[-_]+/g,' ');
    await page.locator('.learning-member').filter({hasText:videoTitle}).getByRole('button',{name:/^(开始|继续)阅读$/}).click();
    const video=page.locator('.video-reader video');await expect(video).toBeVisible();await page.waitForFunction(()=>(document.querySelector('.video-reader video') as HTMLVideoElement)?.readyState>=2);
    expect((await scene(page)).published_book_ref).toEqual(fixture.video);await video.evaluate((v:HTMLVideoElement)=>{v.currentTime=2;});
    await page.getByRole('button',{name:'选择时间范围',exact:true}).click();const range=page.getByRole('form',{name:'时间范围编辑'});await range.getByLabel('起点（秒）').fill('1');await range.getByLabel('终点（秒）').fill('4');await range.getByRole('button',{name:'保存时间范围'}).click();
    await expect(page.getByLabel('已选时间范围')).toContainText('0:01');await page.getByRole('button',{name:'取消草稿',exact:true}).click();
    await page.request.get('http://127.0.0.1:4191/publish-next');await manage();const instance=page.getByRole('region',{name:'共享工作区个人实例',exact:true});
    await instance.getByRole('button',{name:'检查共享版本',exact:true}).click();await instance.getByRole('button',{name:'查看新版变化',exact:true}).click();const preview=page.getByRole('region',{name:'共享工作区更新预览',exact:true});await expect(preview).toContainText('保留私人追加资料');await preview.getByRole('button',{name:'选择更新到此版本',exact:true}).click();await expect(instance).toContainText('当前使用第 2 版');
    expect((await api(`/learning-workspaces/${second.learning_workspace_id}/shared-release`,undefined,true)).instance.version).toBe(1);
    const members=(await api(`/learning-workspaces/${scope}`)).members;expect(members.some((m:any)=>m.published_book_ref.publication_id===preparedReference.publication_id)).toBe(true);
    await page.reload();await page.locator('.learning-space-card').filter({hasText:goal}).getByRole('button',{name:'管理工作区',exact:true}).click();await expect(task).toContainText('已加入原工作区');await expect(instance).toContainText('当前使用第 2 版');
    await page.setViewportSize({width:390,height:844});await panel.scrollIntoViewIfNeeded();expect(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth+1)).toBe(true);
    await page.setViewportSize({width:390,height:Math.max(844,Math.ceil((await panel.boundingBox())!.height)+300)});await panel.scrollIntoViewIfNeeded();await panel.screenshot({path:path.join(output,'preparation-complete-mobile.png')});
    const finalCosts=await api('/admin/cost-summary',undefined,true);expect(finalCosts.account.request_count-initialCosts.account.request_count).toBe(1);expect(errors).toEqual([]);
    writeFileSync(path.join(output,'browser.json'),JSON.stringify({status:'passed',upload_chunks:writes.filter(v=>v.body.action==='upload'&&v.body.upload_id===created.body.upload_id).length,keyboard_file_chooser:true,drop_upload:true,real_progress:true,original_workspace:scope,shared_instances_independent:true,private_material_survives_update:true,legacy_record_unchanged:true,real_video_playable:true,percent_allowance:true,page_errors:errors,mobile_overflow:false,semantic_progress:'controlled host; actual compiled Engine and DSH covered by HTTP acceptance'},null,2));
  }finally{releaseUpload();writeFileSync(path.resolve('../../target/e1-browser-navigation.json'),JSON.stringify({navigation,scene:await scene(page),errors},null,2));await secondContext.unrouteAll({behavior:'ignoreErrors'});await secondContext.close();}
});
