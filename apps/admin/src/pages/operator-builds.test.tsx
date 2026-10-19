// @vitest-environment happy-dom
import {act} from 'react';
import {createRoot,type Root} from 'react-dom/client';
import {QueryClientProvider} from '@tanstack/react-query';
import {afterEach,beforeEach,expect,it,vi} from 'vitest';
import {OperationsProvider} from '@/components/operation';
import {queryClient,session} from '@/lib/api';
import {OperatorBuildsPage} from './operator-builds';
import type {OperatorBuild} from '@/lib/operator-builds';

const existing={book_id:'existing-book',publication_id:'exact-old'},built={book_id:'new-book',publication_id:'exact-new'};
let root:Root|undefined,host:HTMLDivElement,fetcher:ReturnType<typeof vi.fn>,task:OperatorBuild;
let existingKind:'book'|'video';
const writes=()=>fetcher.mock.calls.filter(([,init])=>init?.method==='POST');
async function settle(){await act(async()=>{await new Promise(r=>setTimeout(r,20));});}
async function click(label:string){const button=[...document.querySelectorAll('button')].find(b=>b.textContent===label);expect(button,label).toBeDefined();await act(async()=>button!.click());await settle();}
async function fill(label:string,value:string){const input=[...document.querySelectorAll('label')].find(l=>l.textContent===label)?.querySelector<HTMLInputElement|HTMLTextAreaElement>('input,textarea');expect(input,label).toBeDefined();await act(async()=>{const proto=input!.tagName==='TEXTAREA'?HTMLTextAreaElement.prototype:HTMLInputElement.prototype;Object.getOwnPropertyDescriptor(proto,'value')!.set!.call(input,value);input!.dispatchEvent(new Event('input',{bubbles:true}));input!.dispatchEvent(new Event('change',{bubbles:true}));});}
async function check(label:string){const input=[...document.querySelectorAll('label')].find(l=>l.textContent===label)?.querySelector<HTMLInputElement>('input[type=checkbox]');expect(input,label).toBeDefined();await act(async()=>input!.click());}
async function submit(){await act(async()=>document.querySelector('[role=dialog] form')!.dispatchEvent(new Event('submit',{bubbles:true,cancelable:true})));await settle();}
async function mount(){host=document.createElement('div');document.body.append(host);root=createRoot(host);await act(async()=>root!.render(<QueryClientProvider client={queryClient}><OperationsProvider><OperatorBuildsPage/></OperationsProvider></QueryClientProvider>));await settle();}
beforeEach(async()=>{
  existingKind='book';
  (globalThis as typeof globalThis & {IS_REACT_ACT_ENVIRONMENT:boolean}).IS_REACT_ACT_ENVIRONMENT=true;sessionStorage.clear();queryClient.clear();
  task={task_id:'two-books',name:'准备两本课程资料',state:'awaiting_confirmation',revision:4,created_at:1792281600,updated_at:1792281600,pause_requested:false,max_micro_cny:1000000,reserved_per_execution_micro_cny:10000,visible_to_users:['A','B'],
    materials:[{material_id:'one',label:'已有书',state:'reused',published_book_ref:existing},{material_id:'two',label:'新书',state:'planned',plan:{review_markdown:'本次生成：基础语义抽取\n复用已有成果：原文',model_runtime:{provider:'probe',model:'scripted',max_output_tokens:4096}}}],
    budget:{max_micro_cny:1000000,provider_cost_micro_cny:0,unknown_count:2,reserved_micro_cny:0,pending_micro_cny:20000,remaining_micro_cny:980000}};
  fetcher=vi.fn(async(url:string,init?:RequestInit)=>{
    if(url==='/api/auth/me')return Response.json({user_id:'operator',csrf_token:'csrf',capabilities:{admin:true}});
    if(init?.method==='POST'){const body=JSON.parse(String(init.body));return Response.json({operation_id:body.operation_id,user_id:'operator',actor:'operator',kind:'operator-builds',created_at:1792281600,ok:true,parameters:body,operator_build:task});}
    if(url.startsWith('/api/admin/operator-builds?'))return Response.json({items:[task],total:1});
    if(url.startsWith('/api/admin/operator-builds/'))return Response.json(task);
    if(url.startsWith('/api/admin/charges?'))return Response.json({items:[
      {call_id:'video-call-confirmed',model:'scripted',purpose:'video_translation',state:'settled',provider_cost_micro_cny:12000,reserved_micro_cny:20000},
      {call_id:'video-call-unknown',model:'scripted',purpose:'video_visual',state:'pending',provider_cost_micro_cny:null,reserved_micro_cny:20000},
    ],total:2});
    if(url.startsWith('/api/admin/books?'))return Response.json({books:[{published_book_ref:existing,material_kind:existingKind},{published_book_ref:{book_id:'unrelated',publication_id:'outside'}}],total:2});
    throw new Error(`Unexpected ${url}`);
  });vi.stubGlobal('fetch',fetcher);await session.refresh();
});
afterEach(async()=>{if(root){await act(async()=>root!.unmount());root=undefined;host.remove();}queryClient.clear();vi.unstubAllGlobals();});

it('creates exact reuse and missing source inputs with one platform budget, without starting a model',async()=>{
  await mount();await click('创建准备任务');await fill('准备任务名称','两书复习');await fill('平台预算上限（元）','1.234567');await fill('每次执行预计占用（元）','0.001');await fill('成果授权账号（空格或逗号分隔）','A, B A');await check('existing-book · exact-old');await fill('资料名称','待构建书');await fill('来源文件或工作区位置','E:/source/book.md');await fill('成果根目录','E:/build');await submit();
  expect(writes()).toHaveLength(1);expect(writes()[0][0]).toBe('/api/admin/operator-builds');const body=JSON.parse(writes()[0][1].body);
  expect(body).toMatchObject({name:'两书复习',max_micro_cny:1234567,reserved_per_execution_micro_cny:1000,visible_to_users:['A','B'],materials:[{published_book_ref:existing},{target_input:'E:/source/book.md',root_dir:'E:/build',pass2:'disabled'}]});
  expect(body.materials[1]).toMatchObject({material_kind:'book',build_tutor:false,required_capabilities:['text','tutor']});
});

it('creates and starts an MP4 in one submission with the entered budget and no Tutor assets',async()=>{
  task={...task,state:'draft',revision:0,materials:[{material_id:'source-1',label:'视频课',state:'pending',material_kind:'video',build_tutor:false}]};
  await mount();await click('创建准备任务');await fill('准备任务名称','视频课准备');await fill('平台预算上限（元）','2');await fill('每次执行预计占用（元）','0.02');await fill('成果授权账号（空格或逗号分隔）','A');await fill('资料名称','视频课');await fill('来源文件或工作区位置','E:/source/lecture.MP4');await fill('成果根目录','E:/build');
  const dialog=document.querySelector('[role=dialog]')!;
  expect(dialog.textContent).toContain('不预生成 Tutor 教学材料');expect(dialog.textContent).not.toContain('额外预构建 Tutor 教学材料');
  expect([...dialog.querySelectorAll('button')].find(b=>b.type==='submit')!.textContent).toBe('创建并开始构建');
  await submit();
  expect(writes()).toHaveLength(2);const created=JSON.parse(writes()[0][1].body);
  expect(created.materials).toMatchObject([{material_kind:'video',build_tutor:false,required_capabilities:['text','video','translation','visual_index','graph','discourse']}]);
  expect(writes()[1][0]).toBe(`/api/admin/operator-builds/${created.task_id}/start`);
  expect(JSON.parse(writes()[1][1].body)).toMatchObject({expected_revision:0,max_micro_cny:2000000});
  expect(document.querySelector('[aria-label="运营构建任务详情"]')).not.toBeNull();
  expect(document.querySelector('[role=dialog]')).toBeNull();
});

it('leaves a saved video task available to retry when its start is rejected',async()=>{
  task={...task,state:'draft',revision:0,materials:[{material_id:'source-1',label:'视频课',state:'pending',material_kind:'video',build_tutor:false}]};
  const original=fetcher.getMockImplementation()!;
  fetcher.mockImplementation(async(url:string,init?:RequestInit)=>url.endsWith('/start')?Response.json({error_code:'BUILD_BUDGET_EXHAUSTED',message:'exhausted'},{status:409}):original(url,init));
  await mount();await click('创建准备任务');await fill('准备任务名称','视频课准备');await fill('平台预算上限（元）','2');await fill('每次执行预计占用（元）','0.02');await fill('成果授权账号（空格或逗号分隔）','A');await fill('资料名称','视频课');await fill('来源文件或工作区位置','E:/source/lecture.mp4');await fill('成果根目录','E:/build');await submit();
  expect(writes()).toHaveLength(2);expect(document.querySelector('[role=dialog]')).toBeNull();
  expect(document.body.textContent).toContain('本任务剩余预算不足');
  const detail=document.querySelector('[aria-label="运营构建任务详情"]')!;
  expect([...detail.querySelectorAll('button')].find(button=>button.textContent==='开始视频构建')?.disabled).toBe(false);
  expect(sessionStorage.getItem('understand-book:admin-operation:operator')).toBeNull();
});

it('shows actual per-stage progress and never turns unknown totals into a completed percentage',async()=>{
  task={...task,state:'running',materials:[{material_id:'video',label:'视频课',state:'running',material_kind:'video',plan:{review_markdown:'准备视频字幕和画面'},observation:{progress:{stage:'translation',completed:2,total:5,unit:'batches',percent:99}}}]};
  await mount();await click('查看构建任务');
  let progress=document.querySelector('[aria-label="视频课构建进度"]')!;
  expect(progress.textContent).toContain('正在翻译字幕 · 本阶段 2 / 5 批（40%）');
  expect(progress.textContent).not.toContain('99%');expect(progress.querySelector('progress')?.value).toBe(2);
  task={...task,materials:[{...task.materials[0],observation:{progress:{stage:'transcript',completed:0,total:null,unit:'chunks',percent:100}}}]};
  await click('刷新构建状态');progress=document.querySelector('[aria-label="视频课构建进度"]')!;
  expect(progress.textContent).toContain('正在转写字幕');expect(progress.textContent).toContain('暂不估算百分比');expect(progress.textContent).not.toContain('100%');expect(progress.querySelector('progress')).toBeNull();
});

it('keeps a video draft startable and can raise the budget when continuing completed units',async()=>{
  task={...task,state:'draft',materials:[{material_id:'video',label:'视频课',state:'pending',material_kind:'video'}]};
  await mount();await click('查看构建任务');await click('开始视频构建');await submit();
  expect(writes()[0][0]).toBe('/api/admin/operator-builds/two-books/start');
  task={...task,state:'interrupted',error:'预算不足',materials:[{...task.materials[0],state:'running',observation:{progress:{stage:'translation',completed:2,total:5,unit:'batches'}}}]};
  await click('刷新构建状态');expect(document.body.textContent).toContain('已完成的单元会保留');await click('继续构建');await fill('确认平台预算上限（元）','3');await submit();
  expect(writes()[1][0]).toBe('/api/admin/operator-builds/two-books/continue');expect(JSON.parse(writes()[1][1].body).max_micro_cny).toBe(3000000);
});

it('opens only the task cost receipts and distinguishes unknown supplier costs from confirmed costs',async()=>{
  await mount();await click('查看构建任务');await click('查看费用回执');
  expect(fetcher.mock.calls.some(([path])=>path==='/api/admin/charges?parent_task_ref=two-books&cost_owner=platform&limit=20&offset=0')).toBe(true);
  const panel=document.querySelector('[aria-label="构建费用回执"]')!;
  const pending=[...panel.querySelectorAll('tr')].find(row=>row.textContent?.includes('video-call-unknown'))!;
  const confirmed=[...panel.querySelectorAll('tr')].find(row=>row.textContent?.includes('video-call-confirmed'))!;
  expect(pending.textContent).toContain('待核算');expect(pending.textContent).toContain('未知');expect(pending.textContent).toContain('0.02');
  expect(confirmed.textContent).toContain('已结算');expect(confirmed.textContent).toContain('0.012');
});
it('confirms the displayed plan revision, keeps unknown costs visible, and publishes only the task outputs',async()=>{
  await mount();await click('查看构建任务');expect(document.body.textContent).toContain('2 笔费用未完整确认');expect(document.body.textContent).toContain('基础语义抽取');await click('确认计划并启动');await submit();
  expect(writes()[0][0]).toBe('/api/admin/operator-builds/two-books/start');expect(JSON.parse(writes()[0][1].body)).toMatchObject({expected_revision:4,max_micro_cny:1000000});
  task={...task,state:'ready',revision:9,materials:[task.materials[0],{...task.materials[1],state:'imported',published_book_ref:built}]};await click('刷新构建状态');await click('选择成果并发布');
  expect(document.querySelector('[role=dialog]')!.textContent).not.toContain('unrelated');await fill('发布标识','shared-course');await fill('工作区名称','课程资料');await fill('公开发布者名称','课程组');await fill('公开学习目标','完成课程学习');await fill('适用范围','大学课程');await fill('先修要求','基础数学');await submit();
  expect(writes()[1][0]).toBe('/api/admin/operator-builds/two-books/publish');expect(JSON.parse(writes()[1][1].body)).toMatchObject({release:{release_id:'shared-course',expected_version:0,visible_to_users:['A','B'],manifest:{materials:[{published_book_ref:existing},{published_book_ref:built}]}}});
});
it('shows an Engine decision at the connected host and keeps pause available',async()=>{
  task={...task,state:'running',materials:[task.materials[0],{...task.materials[1],state:'running',observation:{waiting_for_decision:true,message:'确认是否继续当前阶段'}}]};
  await mount();await click('查看构建任务');expect(document.body.textContent).toContain('等待决定：请在已连接的构建宿主回答后继续。');
  await click('暂停构建');await submit();expect(writes()[0][0]).toBe('/api/admin/operator-builds/two-books/pause');expect(JSON.parse(writes()[0][1].body)).toMatchObject({expected_revision:4});
});

it('reuses a video with its media capability and preserves that requirement when publishing',async()=>{
  existingKind='video';
  task={...task,state:'ready',materials:[{material_id:'one',label:'课程视频',state:'reused',published_book_ref:existing,required_capabilities:['text','video']}]};
  await mount();await click('创建准备任务');await fill('准备任务名称','视频课程');await fill('平台预算上限（元）','1');await fill('每次执行预计占用（元）','0.01');await fill('成果授权账号（空格或逗号分隔）','A');await check('existing-book · exact-old');await submit();
  expect(JSON.parse(writes()[0][1].body).materials).toMatchObject([{published_book_ref:existing,required_capabilities:['text','video']}]);
  await click('查看构建任务');await click('选择成果并发布');
  const capability=(label:string)=>[...document.querySelectorAll('label')].find(l=>l.textContent===label)!.querySelector<HTMLInputElement>('input')!;
  expect(capability('视频伴读').checked).toBe(true);expect(capability('资料理解').checked).toBe(false);
  await fill('发布标识','video-course');await fill('工作区名称','视频课程');await fill('公开发布者名称','课程组');await fill('公开学习目标','理解课程');await fill('适用范围','入门');await fill('先修要求','无需先修知识');await submit();
  expect(JSON.parse(writes()[1][1].body).release.manifest.materials).toEqual([{published_book_ref:existing,required_capabilities:['text','video']}]);
});
