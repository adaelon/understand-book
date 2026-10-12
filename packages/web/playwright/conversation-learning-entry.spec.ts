import { expect, test, type Page } from '@playwright/test';
import { mkdirSync } from 'node:fs';
import { resolve } from 'node:path';
import type { TutorSession } from '../src/generated/TutorSession';
import type { TutorState } from '../src/generated/TutorState';
import type { TutorMutation } from '../src/generated/TutorMutation';

const output = resolve('../../docs/performance/conversation-learning-entry-20261010');
const makeSession = (source = 'book', status: TutorSession['status'] = 'active'): TutorSession => ({
  id:'learning',revision:1,status,user_intent:'弄懂 KV Cache 的计算收益与显存代价',explicit_constraints:[],
  current_focus:{interpretation:'',target_object_refs:[],capability_targets:[]},
  material_scope:[{source_id:source,scope_refs:[],role:'primary'}],default_teaching_intent:null,path_instance_ref:null,progress_ref:null,
});
async function setup(page: Page, session?: TutorSession, enabled = false) {
  const mutations: TutorMutation[] = [], questions: string[] = [], errors: string[] = [];
  page.on('pageerror',error=>errors.push(error.stack ?? error.message));
  let state: TutorState = {control:{enabled,revision:0,current_tutor_session_id:session?.id ?? null},sessions:session?{[session.id]:session}:{}};
  let failNext = false, starts = 0;
  const turns: any[] = [];
  const history = () => ({active_session_id:'chat',sessions:[],current:{id:'chat',book_id:'book',turns,goals:[]}});
  const profile = {profile_id:'technical_learning',profile_version:'1',ui_slots:[],slots:[],layout_presets:[],allowed_layout_actions:[],agent_tools:[]};
  const reader = {book_id:'book',revision:0,viewport:{anchor_lid:'1.1',top_lid:'1.1',bottom_lid:'1.2',width:1,visible_lids:['1.1','1.2']},profile,
    layout:{rev:0,open_slots:[],panel_sizes:{},slot_order:[],pinned_evidence:[]}};
  await page.route('**/api/**',async route=>{
    const request=route.request(),url=new URL(request.url()),path=url.pathname;
    const body=request.postData()?request.postDataJSON():null;
    let result: unknown;
    if(path==='/api/desktop/status') result={desktop_host:false,reader_only:true,active_book:true};
    else if(path==='/api/book/build_workbench') result={book_id:'book',readiness:{route:'reader'},input:{manifest:null},source_review:{unresolved:[]}};
    else if(path==='/api/book/manifest') result={tree:[
      {lid:'1',kind:'chapter',children:['1.1','1.2'],span:{start:0,end:180},title:'推理中的缓存'},
      {lid:'1.1',kind:'paragraph',children:[],span:{start:8,end:100}},
      {lid:'1.2',kind:'paragraph',children:[],span:{start:100,end:180}},
    ],stats_by_lid:{}};
    else if(path==='/api/book/asset_manifest') result={images:[]};
    else if(path==='/api/book/source_fingerprint') result={book_id:'book',source_fingerprint:'fixture'};
    else if(path==='/api/book/source_manifest') return route.fulfill({status:404,json:{message:'Markdown fixture'}});
    else if(path==='/api/book/text') { const lid=url.searchParams.get('lid'); result={lid,text:lid==='1'?'# 推理中的缓存':lid==='1.1'?'生成下一个词时，模型需要用到前面的上下文。KV Cache 保存历史 token 的 Key 和 Value，供后续生成使用。':'缓存减少了重复计算，也增加了显存占用。理解缓存的收益时，需要同时考虑计算与存储。'}; }
    else if(path==='/api/profile/manifest') result=profile;
    else if(path==='/api/reader/state') result=reader;
    else if(path==='/api/memory/recall') result=[];
    else if(path==='/api/book/library') result={root:'',books:[]};
    else if(path==='/api/profile/memory') result={current_book_id:'book',facts:[],pending_candidates:[],evidence:[],collection_rules:[],status:{pending_sensitive_confirmation:false},snapshot:{revision:0,facts:[]}};
    else if(path==='/api/profile/backfill') result={jobs:[],sessions:[]};
    else if(path==='/api/build_intent/artifacts') result={overlay:null};
    else if(path==='/api/build_intent/usage.event' || path==='/api/reader/viewport') result={};
    else if(path==='/api/agent/history') result=history();
    else if(path==='/api/tutor/state') result=state;
    else if(path==='/api/tutor/readiness') result={status:'ready',source_id:'book',source_revision:'s1',limitations:[],reason:'可用',teaching_assets:{status:'ready',teaching_map_revision:'v1',limitations:[],reason:'可用'}};
    else if(path==='/api/tutor/understanding') result={rows:[],interpretations:[],stale:false,next:null};
    else if(path==='/api/tutor/mutate') {
      mutations.push(body);
      if(failNext) { failNext=false; return route.fulfill({status:503,json:{error_code:'TEST_UNAVAILABLE',category:'unavailable',message:'保存暂不可用'}}); }
      const action=(body as TutorMutation).action;
      if(action.kind==='set_enabled') {
        state.control.enabled=action.enabled;
        const current=state.sessions[state.control.current_tutor_session_id ?? ''];
        if(!action.enabled && current?.status==='active') current.status='paused';
      } else if(action.kind==='start') {
        const previous=state.sessions[state.control.current_tutor_session_id ?? ''];
        if(previous?.status==='active') previous.status='paused';
        const next={...makeSession(),id:'goal-'+state.control.revision,user_intent:action.user_intent,material_scope:action.material_scope};
        state.sessions[next.id]=next; state.control.current_tutor_session_id=next.id;
      } else if(action.kind==='resume') { state.sessions[action.session_id]!.status='active'; state.control.current_tutor_session_id=action.session_id; }
      else if(action.kind==='pause' || action.kind==='end') state.sessions[action.session_id]!.status=action.kind==='pause'?'paused':'ended';
      else if(action.kind==='set_mode') state.sessions[action.session_id]!.default_teaching_intent=action.mode;
      state.control.revision++; result=state;
    } else if(path==='/api/tutor/start') { starts++; result={started:true,message:'继续围绕当前学习目标讲解'}; }
    else if(path==='/api/agent/runs') {
      questions.push(body.message);
      if(state.control.enabled && !state.control.current_tutor_session_id) {
        const first={...makeSession(),user_intent:body.message}; state.sessions[first.id]=first; state.control.current_tutor_session_id=first.id; state.control.revision++;
      }
      const id='turn-'+questions.length;
      const outcome={answer:'KV Cache 复用了历史 token 的 Key 和 Value。我们可以结合原文，进一步比较计算收益与显存成本。',incomplete:false,warning:null,turns:1,tokens_spent:1,effects:[],trace:[],memory_updates:[],profile_usage:{snapshot_revision:0,injected_fact_ids:[],claimed_used_fact_ids:[],influences:[]}};
      turns.push({turn_id:id,user_turn_ordinal:turns.length,user:body.message,status:'completed',outcome,question_source_label:null,question_quote:null,effect_labels:[]});
      result={book_id:'book',session_id:'chat',turn_id:id};
    } else if(/^\/api\/agent\/runs\/turn-\d+$/.test(path)) {
      const turn=turns.find(turn=>turn.turn_id===path.split('/').pop());
      result={descriptor:{book_id:'book',session_id:'chat',turn_id:turn.turn_id},last_seq:1,execution_state:'completed',persistence_state:'saved',activities:[],final_view:turn,error:null};
    } else if(path.endsWith('/events')) return route.fulfill({contentType:'text/event-stream',body:': fixture\n\n'});
    else return route.fulfill({status:404,json:{message:'Unused fixture route: '+path}});
    await route.fulfill({json:result});
  });
  await page.route('**/learning-entry-fixture',route=>route.fulfill({contentType:'text/html',body:`<!doctype html><html lang="zh-CN"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"></head><body><div id="app"></div><script type="module">
    import {createApp} from '/node_modules/.vite/deps/vue.js';
    import App from '/src/App.vue'; import '/src/style.css';
    import {installAppViewportHeightFallback} from '/src/app-viewport-height.ts';
    installAppViewportHeightFallback();createApp(App).mount('#app');
  </script></body></html>`}));
  await page.goto('/learning-entry-fixture');
  await expect(page.locator('.app-loading')).toHaveCount(0);
  if((page.viewportSize()?.width ?? 1280)<1024) await page.locator('.workspace-mobile-nav').getByRole('button',{name:'问答',exact:true}).click();
  await expect(page.getByRole('switch',{name:'连续教学',exact:true})).toBeVisible();
  return {mutations,questions,errors,state,starts:()=>starts,failNext:()=>{failNext=true;}};
}

test('actual App starts from chat, opens understanding directly, and preserves pause and drafts',async({page})=>{
  await page.setViewportSize({width:1440,height:960});
  const fixture=await setup(page);
  const toggle=page.getByRole('switch',{name:'连续教学',exact:true});
  const input=page.locator('.agent-input textarea');
  const details=page.locator('.chat-secondary-actions').getByRole('button',{name:'学习详情',exact:true});
  const panel=page.getByRole('dialog',{name:'学习详情',exact:true});
  expect(fixture.questions).toEqual([]);
  await expect(page.locator('.topbar .tutor-control')).toHaveCount(0);
  await expect(page.locator('.conversation-learning-goal')).toHaveCount(0);
  mkdirSync(output,{recursive:true}); await page.screenshot({path:resolve(output,'first-desktop.png')});
  await input.fill('还没发送的想法');
  await page.getByRole('button',{name:'我的理解',exact:true}).click();
  await expect(panel.getByRole('tab',{name:'我的理解',exact:true})).toHaveAttribute('aria-selected','true');
  await panel.getByRole('button',{name:'关闭',exact:true}).click();
  await expect(input).toHaveValue('还没发送的想法');
  expect(fixture.mutations).toHaveLength(0);
  await input.fill('');
  await toggle.click(); await expect(toggle).toHaveAttribute('aria-checked','true');
  expect(fixture.questions).toEqual([]); expect(fixture.starts()).toBe(0);
  await page.getByRole('button',{name:'帮我讲清当前这一节'}).click();
  await expect(input).toHaveValue('帮我梳理当前这一节的核心概念。');
  expect(fixture.questions).toEqual([]);
  await page.locator('.agent-input').getByRole('button',{name:'发送',exact:true}).click();
  await expect(page.locator('.goal-text')).toHaveText('帮我梳理当前这一节的核心概念。');
  await expect(page.locator('.a-msg')).toBeVisible();
  await page.screenshot({path:resolve(output,'active-desktop.png')});
  await toggle.click(); await expect(toggle).toHaveAttribute('aria-checked','false');
  await expect(page.getByRole('button',{name:'开启并继续这次学习'})).toBeVisible();
  await toggle.click(); await input.fill('只解释这段');
  await page.locator('.agent-input').getByRole('button',{name:'发送',exact:true}).click();
  await expect(page.locator('.turn')).toHaveCount(2);
  await expect(page.locator('.goal-resume')).toContainText('连续教学已暂停');
  await page.getByRole('button',{name:'继续这次学习',exact:false}).click();
  await expect(page.locator('.goal-resume')).toHaveCount(0);
  await expect.poll(()=>fixture.starts()).toBe(1);
  await page.getByRole('button',{name:'问答操作',exact:true}).click();
  await details.click(); await expect(panel.getByRole('tab',{name:'当前学习',exact:true})).toHaveAttribute('aria-selected','true');
  await panel.getByRole('button',{name:'关闭',exact:true}).click();
  await expect(page.getByRole('button',{name:'问答操作',exact:true})).toBeFocused();
  expect(fixture.errors).toEqual([]);
});

test('failed saves preserve confirmed state and the goal draft, then retry the same operation',async({page})=>{
  const fixture=await setup(page,makeSession(),true);
  const toggle=page.getByRole('switch',{name:'连续教学'});
  fixture.failNext(); await toggle.click();
  await expect(page.locator('.tutor-control [role=alert]')).toContainText('保存暂不可用');
  await expect(toggle).toHaveAttribute('aria-checked','true');
  await expect(toggle).toBeDisabled();
  await page.locator('.tutor-control').getByRole('button',{name:'重试',exact:true}).click();
  await expect(toggle).toHaveAttribute('aria-checked','false');
  expect(fixture.mutations[0]).toEqual(fixture.mutations[1]);
  await page.getByRole('button',{name:'开启并继续这次学习'}).click();
  await expect(toggle).toHaveAttribute('aria-checked','true');
  await expect(page.locator('.goal-resume')).toHaveCount(0);
  await expect.poll(()=>fixture.starts()).toBe(1);
  await expect(page.locator('.a-msg')).toBeVisible();
  await page.getByRole('button',{name:'调整目标',exact:true}).click();
  await page.getByRole('textbox',{name:'学习目标',exact:true}).fill('比较计算量与显存占用');
  fixture.failNext(); await page.getByRole('button',{name:'保存目标',exact:true}).click();
  await expect(page.getByRole('textbox',{name:'学习目标',exact:true})).toHaveValue('比较计算量与显存占用');
  await expect(page.locator('.goal-text')).toHaveText('弄懂 KV Cache 的计算收益与显存代价');
  await page.getByRole('dialog',{name:'调整目标',exact:true}).getByRole('button',{name:'重试',exact:true}).click();
  await expect(page.locator('.goal-text')).toHaveText('比较计算量与显存占用');
  await expect(page.getByRole('textbox',{name:'学习目标',exact:true})).toHaveCount(0);
  expect(fixture.mutations.at(-1)).toEqual(fixture.mutations.at(-2));
  expect(fixture.errors).toEqual([]);
});

for(const width of [390,320]) test(`phone layout, full screen and direct understanding at ${width}px`,async({page})=>{
  await page.setViewportSize({width,height:844});
  const fixture=await setup(page,makeSession('book','paused'),true);
  const input=page.locator('.agent-input textarea');
  await input.fill('保留这份草稿');
  expect(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth)).toBe(true);
  await expect(input).toBeInViewport();
  await expect(page.getByRole('button',{name:'继续这次学习',exact:false})).toBeInViewport();
  mkdirSync(output,{recursive:true}); await page.screenshot({path:resolve(output,`paused-${width}.png`)});
  await page.getByRole('button',{name:'我的理解',exact:true}).click();
  const panel=page.getByRole('dialog',{name:'学习详情',exact:true});
  await expect(panel.getByRole('tab',{name:'我的理解',exact:true})).toHaveAttribute('aria-selected','true');
  await panel.getByRole('button',{name:'关闭',exact:true}).click();
  await expect(input).toHaveValue('保留这份草稿');
  await page.getByRole('button',{name:'问答全屏',exact:true}).click();
  await expect(page.getByRole('switch',{name:'连续教学'})).toHaveCount(1);
  await page.setViewportSize({width,height:500});
  await expect(input).toBeInViewport();
  await expect(page.locator('.agent-input').getByRole('button',{name:'发送',exact:true})).toBeInViewport();
  expect(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth)).toBe(true);
  expect(fixture.errors).toEqual([]);
});

test('a different book offers an explicit new goal without resuming or overwriting the old session',async({page})=>{
  const old=makeSession('another-book','paused');
  const fixture=await setup(page,old,true);
  await expect(page.locator('.conversation-learning-goal')).toContainText('当前学习属于其他书籍');
  await expect(page.locator('.goal-text')).toHaveCount(0);
  await expect(page.locator('.goal-resume')).toHaveCount(0);
  expect(fixture.mutations).toHaveLength(0);
  await page.getByRole('button',{name:'为这本书设定目标'}).click();
  await page.getByRole('textbox',{name:'学习目标',exact:true}).fill('读懂当前书籍');
  await page.getByRole('button',{name:'保存目标',exact:true}).click();
  await expect(page.locator('.goal-text')).toHaveText('读懂当前书籍');
  expect(fixture.state.sessions.learning?.material_scope[0].source_id).toBe('another-book');
  expect(fixture.state.sessions.learning?.status).toBe('paused');
  expect(fixture.questions).toHaveLength(0);
  expect(fixture.errors).toEqual([]);
});
