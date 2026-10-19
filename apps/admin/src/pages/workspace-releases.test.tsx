// @vitest-environment happy-dom
import { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { QueryClientProvider } from '@tanstack/react-query';
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import { OperationsProvider } from '@/components/operation';
import { queryClient, session } from '@/lib/api';
import type { WorkspaceRelease } from '@/lib/workspace-releases';
import { WorkspaceReleasesPage } from './workspace-releases';

const first={book_id:'first-book',publication_id:'exact-v1'},second={book_id:'second-book',publication_id:'exact-v2'};
const readiness={status:'incomplete' as const,materials:[{published_book_ref:first,available_capabilities:['text'],missing_capabilities:['tutor'],publication_available:true}],audience:[{user_id:'A',ready:false,missing_grants:[second]}]};
const original:WorkspaceRelease={release_id:'learning',version:1,latest_version:2,created_at:1792281600,publisher_user_id:'operator',visible_to_users:['A'],manifest:{name:'公开课程',publisher:'学习团队',public_goal:'学习深度学习',applicability:'入门学习',prerequisites:'基础代数',materials:[{published_book_ref:first,required_capabilities:['text']}],topic_links:[],suggested_route:[]},readiness_at_publication:readiness,availability:readiness};
let root:Root|undefined;let host:HTMLDivElement;let fetcher:ReturnType<typeof vi.fn>;let releases:WorkspaceRelease[];
let mutate:(body:Record<string,unknown>)=>Promise<Response>;
const text=()=>document.body.textContent??'';
const writes=()=>fetcher.mock.calls.filter(([,init])=>init?.method==='POST');
const receipt=(body:Record<string,unknown>)=>({operation_id:body.operation_id,actor:'operator',user_id:'operator',kind:'workspace-releases',created_at:1792281600,ok:true,parameters:body,workspace_release:{...original,version:Number(body.expected_version)+1,manifest:body.manifest,visible_to_users:body.visible_to_users}});
async function settle(){await act(async()=>{await new Promise(resolve=>setTimeout(resolve,20));});}
async function mount(){host=document.createElement('div');document.body.append(host);root=createRoot(host);await act(async()=>root!.render(<QueryClientProvider client={queryClient}><OperationsProvider><WorkspaceReleasesPage/></OperationsProvider></QueryClientProvider>));await settle();}
async function unmount(){if(root){await act(async()=>root!.unmount());root=undefined;host.remove();}}
async function click(label:string){const button=[...document.querySelectorAll('button')].find(b=>b.textContent===label);expect(button,label).toBeDefined();await act(async()=>button!.click());await settle();}
async function fill(name:string,value:string){const input=document.querySelector<HTMLInputElement|HTMLTextAreaElement>(`[name="${name}"]`)!;expect(input,name).not.toBeNull();await act(async()=>{const prototype=input.tagName==='TEXTAREA'?HTMLTextAreaElement.prototype:HTMLInputElement.prototype;Object.getOwnPropertyDescriptor(prototype,'value')!.set!.call(input,value);input.dispatchEvent(new Event('input',{bubbles:true}));input.dispatchEvent(new Event('change',{bubbles:true}));});}
async function check(label:string){const input=[...document.querySelectorAll('label')].find(l=>l.textContent===label)?.querySelector<HTMLInputElement>('input[type=checkbox]');expect(input,label).toBeDefined();await act(async()=>input!.click());}
async function submit(){await act(async()=>document.querySelector('[role=dialog] form')!.dispatchEvent(new Event('submit',{bubbles:true,cancelable:true})));await settle();}
beforeEach(async()=>{
  (globalThis as typeof globalThis & {IS_REACT_ACT_ENVIRONMENT:boolean}).IS_REACT_ACT_ENVIRONMENT=true;
  sessionStorage.clear();queryClient.clear();releases=[original];mutate=async body=>Response.json(receipt(body));
  fetcher=vi.fn(async(url:string,init?:RequestInit)=>{
    if(url==='/api/auth/me')return Response.json({user_id:'operator',csrf_token:'csrf',capabilities:{admin:true}});
    if(init?.method==='POST')return mutate(JSON.parse(String(init.body)));
    if(url.startsWith('/api/admin/operations/'))return Response.json({error_code:'NOT_FOUND'},{status:404});
    if(url.startsWith('/api/admin/books?'))return Response.json({books:[{published_book_ref:first},{published_book_ref:second}],total:2});
    if(url.startsWith('/api/admin/workspace-releases?'))return Response.json({items:releases,total:releases.length,limit:20,offset:0});
    if(url==='/api/admin/workspace-releases/learning/versions/1')return Response.json(original);
    throw new Error(`Unexpected ${url}`);
  });vi.stubGlobal('fetch',fetcher);await session.refresh();
});
afterEach(async()=>{await unmount();queryClient.clear();vi.unstubAllGlobals();});

it('publishes only explicitly entered public fields and selected exact versions, then shows incomplete readiness',async()=>{
  await mount();await click('发布新工作区');await fill('release_id','exam');await fill('name','期末冲刺');await fill('publisher','课程组');await fill('public_goal','准备期末考试');await fill('applicability','大学一年级');await fill('prerequisites','基础数学');await fill('visible_to_users','A, B A');
  await check('first-book · exact-v1');await check('second-book · exact-v2');await check('资料理解');await submit();
  expect(writes()).toHaveLength(1);const body=JSON.parse(writes()[0][1].body);
  expect(body).toMatchObject({release_id:'exam',expected_version:0,visible_to_users:['A','B'],manifest:{public_goal:'准备期末考试',materials:[{published_book_ref:first,required_capabilities:['text','tutor']},{published_book_ref:second,required_capabilities:['text']}]}});
  expect(Object.keys(body.manifest).sort()).toEqual(['applicability','materials','name','prerequisites','public_goal','publisher','suggested_route','topic_links'].sort());expect(body.learning_workspace_id).toBeUndefined();
  expect(text()).toContain('尚未完整就绪');await click('查看版本');expect(text()).toContain('缺少能力 tutor');expect(text()).toContain('授权未齐全 second-book · exact-v2');
});
it('revises the selected public version against latest revision without mutating the old manifest',async()=>{
  await mount();await click('修订为新版本');await fill('public_goal','新版公开目标');await submit();
  const body=JSON.parse(writes()[0][1].body);expect(body.expected_version).toBe(2);expect(body.release_id).toBe('learning');expect(body.manifest.public_goal).toBe('新版公开目标');expect(original.manifest.public_goal).toBe('学习深度学习');
});
it('retains the entire publication operation through disconnect and retries the original identity',async()=>{
  mutate=async()=>{throw new TypeError('connection lost');};await mount();await click('修订为新版本');await submit();const body=JSON.parse(writes()[0][1].body);expect(text()).toContain('有一笔操作待核对');
  await unmount();await mount();expect(text()).toContain(body.operation_id);expect(text()).toContain('共享工作区名称');mutate=async originalBody=>Response.json(receipt(originalBody));await click('按原操作重试');
  expect(writes()).toHaveLength(2);expect(JSON.parse(writes()[1][1].body)).toEqual(body);expect(sessionStorage.length).toBe(0);
});
it('archives the shared entry through an operation receipt while explaining existing instances remain',async()=>{
  await mount();await click('归档共享入口');expect(text()).toContain('已开始学习的工作区和旧来源继续保留');await submit();
  expect(writes()[0][0]).toBe('/api/admin/workspace-releases/learning/archive');expect(JSON.parse(writes()[0][1].body)).toMatchObject({archived:true});
});
