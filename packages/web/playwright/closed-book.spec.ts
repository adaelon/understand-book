import { expect, test } from '@playwright/test';
import { setup, scenario } from './fixtures/tutor-continuation';
import type { ClosedBookPractice } from '../src/generated/ClosedBookPractice';
import type { LearningTaskView } from '../src/learning-tasks';

for (const width of [1366, 390, 320]) test(`closed-book dialogue hides sources and resumes independent attempts at ${width}px`, async ({ page }, info) => {
  await page.addInitScript(() => localStorage.setItem('understand-book.product-tour.v1.local.reader', 'done'));
  const fixture=await setup(page,'markdown',true,true);
  let state: ClosedBookPractice | null=null;
  let stateUnavailable=false;
  const closedRequests: Record<string, unknown>[]=[];
  await page.route('**/api/learning-task',async route => {
    const task: LearningTaskView | null=state ? {
      learning_workspace_id:null,published_book_ref:{book_id:scenario.book_id,publication_id:'local'},
      course_run_id:null,chat_session_id:'tc0-chat',selected_at:1,kind:'closed_book',title:state.target,
      status:state.status==='active'?'active':'paused',lesson_id:null,stage:null,closed_book_id:state.id,
      question:state.target,message:null,
    } : null;
    await route.fulfill({json:route.request().method()==='GET' ? {tasks:task?[task]:[]} : {}});
  });
  await page.route('**/api/tutor/closed-book',async route => {
    const request=route.request().postDataJSON();
    closedRequests.push(request);
    if (request.action==='state' && stateUnavailable) {
      await route.fulfill({status:503,json:{error_code:'TEMPORARILY_UNAVAILABLE',category:'unavailable',message:'练习状态暂时无法读取'}});return;
    }
    if (request.action==='start') state={id:'closed',owner_id:'tc0-chat',course:false,chat_session_id:'tc0-chat',source_id:scenario.book_id,
      source_revision:'tc0-source',goal_revision:null,revision:1,status:'active',can_resume:true,target:'解释全程平均速度，并用总时间比较两段旅程',criteria:'计入停留时间',
      answer_origin:{turn_id:request.answer_turn_id,question:'Explain average speed',answer:scenario.answer},
      attempt:1,assisted:false,needs_retry:false,visible_turn_ids:[],draft:'',completion_reason:'',learner_quote:''};
    else if (state && request.action!=='state') {
      state.revision++; state.draft=request.draft ?? state.draft;
      if (request.action==='pause' || request.action==='exit') {state.status=request.action==='pause'?'paused':'ended';state.needs_retry=request.action==='exit';}
      if (request.action==='resume') state.status='active';
      if (request.action==='help') state.assisted=true;
    }
    await route.fulfill({json:state});
  });
  await page.route('**/api/agent/runs',async route => {
    const request=route.request().postDataJSON();fixture.requests.push(request);
    const id=`cb-${fixture.requests.length}`;
    let text='一辆车行驶了 30 公里，中途停留了 10 分钟。计算全程平均速度时，应使用哪一段时间？请说说理由。';
    if (request.message.includes('提示')) {text='停留期间，路程没有增加，但从出发到抵达的时间仍在增加。';state!.assisted=true;}
    if (state!.needs_retry || request.message.includes('新问题')) {
      state!.attempt++;state!.visible_turn_ids=[];state!.assisted=false;state!.needs_retry=false;
      text='两辆车走过相同距离。甲连同停留共用了 12 分钟，乙用了 15 分钟。谁的全程平均速度更高？为什么？';
    }
    if (request.message.includes('甲的速度更高')) {
      text='你把相同路程与全部经过的时间联系起来，独立完成了这次比较。';state!.status='complete';
      state!.completion_reason='能独立用总路程和总时间解释并比较平均速度。';state!.learner_quote=request.message;
    }
    state!.visible_turn_ids.push(id);state!.revision++;state!.draft='';
    const previous=fixture.turns[0].outcome as object;
    fixture.turns.push({turn_id:id,user_turn_ordinal:fixture.turns.length+1,user:request.display_user??request.message,status:'completed',
      outcome:{...previous,answer:text,answer_view:{parts:[{kind:'markdown',text}],sources:[]}},question_source_label:null,question_quote:null,effect_labels:[]});
    await route.fulfill({json:{book_id:scenario.book_id,session_id:'tc0-chat',turn_id:id}});
  });
  await page.setViewportSize({width,height:width<600?844:900});await page.reload();
  if (width<600) await page.getByRole('navigation',{name:'阅读工作区'}).getByRole('button',{name:'问答',exact:true}).click();
  const entry=page.getByRole('button',{name:'我来解释一下',exact:true});
  await expect(entry).toBeVisible();await expect(entry).toBeEnabled();
  await expect(page.getByRole('group',{name:'当前回答的学习操作'})).toContainText('解释练习会收起资料');
  await entry.scrollIntoViewIfNeeded();await page.screenshot({path:info.outputPath(`closed-book-entry-${width}.png`)});
  await entry.click();
  const dialog=page.getByRole('dialog',{name:'练到会',exact:true});
  await expect(dialog).toBeVisible();await expect(dialog).toContainText('中途停留了 10 分钟');
  expect(closedRequests.find(request=>request.action==='start')).toMatchObject({answer_turn_id:'tc0-answer',chat_session_id:'tc0-chat'});
  await expect(page.locator('.app')).toHaveAttribute('inert','');
  await expect(dialog.locator('a,iframe,img')).toHaveCount(0);
  await expect(dialog).not.toContainText(scenario.answer);
  const input=dialog.getByRole('textbox',{name:'你的回答'});
  await input.fill('我想把全部经过的时间算进去。');await page.reload();
  await expect(dialog).toBeVisible();await expect(input).toHaveValue('我想把全部经过的时间算进去。');
  await expect(dialog).toContainText('中途停留了 10 分钟');
  stateUnavailable=true;await page.reload();
  const recovery=page.getByRole('dialog',{name:'正在加载',exact:true});
  await expect(recovery).toContainText('暂时无法恢复学习，请重试。');
  await expect(dialog).not.toBeVisible();
  await expect(page.locator('.app')).toHaveAttribute('inert','');
  stateUnavailable=false;await recovery.getByRole('button',{name:'重试',exact:true}).click();
  await expect(input).toHaveValue('我想把全部经过的时间算进去。');
  expect(fixture.requests).toHaveLength(1);
  await dialog.getByRole('button',{name:'给一点提示'}).click();
  await expect(dialog).toContainText('但从出发到抵达的时间仍在增加');
  await expect(dialog).toContainText('这次已获得帮助');
  await dialog.getByRole('button',{name:'独立试一次'}).click();
  await expect(dialog).toContainText('甲连同停留共用了 12 分钟');
  await expect(dialog).not.toContainText('但从出发到抵达的时间仍在增加');
  await input.fill('先比较两段路所用的总时间。');
  await dialog.getByRole('button',{name:'暂停',exact:true}).click();await expect(dialog).toContainText('已暂停');
  await expect(page.locator('.app')).toHaveAttribute('inert','');
  await page.reload();await expect(dialog).toContainText('已暂停');
  await dialog.getByRole('button',{name:'继续练习',exact:true}).click();
  await expect(input).toHaveValue('先比较两段路所用的总时间。');
  await dialog.getByRole('button',{name:'返回看资料',exact:true}).click();await expect(dialog).not.toBeVisible();
  await expect(page.locator('.app')).not.toHaveAttribute('inert','');
  const resume=page.getByRole('button',{name:'继续学习',exact:true});
  await expect(resume).toBeVisible();
  await resume.click();
  await expect(dialog).toBeVisible();await expect(dialog).toContainText('甲连同停留共用了 12 分钟');
  expect(closedRequests.filter(request=>request.action==='start')).toHaveLength(1);
  expect(closedRequests.filter(request=>request.action==='resume')).toHaveLength(2);
  expect(state!.answer_origin?.turn_id).toBe('tc0-answer');expect(state!.attempt).toBe(3);
  await page.screenshot({path:info.outputPath(`closed-book-active-${width}.png`)});
  await input.fill('甲的速度更高，因为相同距离除以较少的总时间，得到的速度更大。');
  await dialog.getByRole('button',{name:'发送回答'}).click();
  await expect(dialog).toContainText('这一轮，已经能独立完成');
  await page.screenshot({path:info.outputPath(`closed-book-${width}.png`)});
  await dialog.getByRole('button',{name:'回到学习'}).click();await expect(dialog).not.toBeVisible();
  expect(fixture.errors).toEqual([]);
});

for (const entry of ['open', 'first-question'] as const) test(`ordinary ${entry} waits without flashing practice controls`, async ({page}, info) => {
  const fixture=await setup(page,'markdown',true);
  if (entry==='first-question') {
    await page.route('**/api/agent/history', route => {
      const id=fixture.turns.length ? 'tc0-chat' : '';
      return route.fulfill({json:{active_session_id:id,sessions:[],current:{id,book_id:scenario.book_id,turns:fixture.turns,goals:[]}}});
    });
    await page.reload();
    await expect(page.locator('.agent-input textarea')).toBeVisible();
  }
  let releaseState!: () => void;
  const stateReady=new Promise<void>(resolve=>{releaseState=resolve;});
  const actions:string[]=[];
  await page.route('**/api/tutor/closed-book',async route=>{
    actions.push(route.request().postDataJSON().action);
    await stateReady;
    await route.fulfill({json:null});
  });
  try {
    if(entry==='open') await page.reload({waitUntil:'domcontentloaded'});
    else {
      await page.locator('.agent-input textarea').fill('为什么平均速度要用总时间？');
      await page.locator('.agent-input').getByRole('button',{name:'发送',exact:true}).click();
    }
    const loading=page.getByRole('dialog',{name:'正在加载',exact:true});
    await expect(loading.getByRole('status',{name:'正在加载',exact:true})).toBeVisible();
    await expect(page.getByRole('dialog',{name:'练到会',exact:true})).not.toBeVisible();
    await expect(loading.locator('h1,button,textarea')).toHaveCount(0);
    await expect(page.locator('.app')).toHaveAttribute('inert','');
    await page.screenshot({path:info.outputPath(`neutral-${entry}.png`)});
    if(entry==='open') {
      await page.setViewportSize({width:390,height:844});
      await page.screenshot({path:info.outputPath('neutral-open-mobile.png')});
      await page.setViewportSize({width:1440,height:960});
    }
    releaseState();
    await expect(loading).not.toBeVisible();
    await expect(page.locator('.app')).not.toHaveAttribute('inert','');
    await expect(page.locator('.agent-input textarea')).toBeVisible();
    expect(actions.length).toBeGreaterThan(0);expect(actions.every(action=>action==='state')).toBe(true);
    if(entry==='first-question') expect(fixture.requests).toHaveLength(1);
    expect(fixture.errors).toEqual([]);
  } finally {releaseState();}
});
