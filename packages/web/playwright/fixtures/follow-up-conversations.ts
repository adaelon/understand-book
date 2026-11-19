import { expect, type Page } from '@playwright/test';
import { scenario, setup } from './tutor-continuation';
import type { FollowUpOrigin, FollowUpStart } from '../../src/follow-up-conversations';

const rootText = '还不能凭这 8 分判断软件的作用。先看对照组的学生学了什么，再看研究者怎样分组。\n\n“随机分组”的随机指什么？如果决定谁用软件时采用随机分组，就能让两组原有的差异更难系统地偏向一边。';
const groupText = '也上课。对照组照常上课，另一组用软件学习同一单元。还要对照两组的课时安排。';
const questions = {
  root: '多出来的 8 分，是软件带来的吗？',
  control: '对照组也上课吗？',
  hours: '两组的课时安排一样吗？',
};
const chapter = [
  {lid:'1',kind:'chapter',text:'多出来的八分',children:['1.1','1.2','1.3','1.4']},
  {lid:'1.1',kind:'paragraph',text:'两组学生学习同一单元，最后做了同一份测验。一组使用学习软件，另一组照常上课。'},
  {lid:'1.2',kind:'table',text:'| 学习方式 | 测验平均分 |\n| --- | --- |\n| 使用学习软件 | 84 分 |\n| 原有学习方式 | 76 分 |'},
  {lid:'1.3',kind:'section',text:'分组之前',children:['1.3.1','1.3.2']},
  {lid:'1.3.1',kind:'paragraph',text:'使用软件的那组平均高出 8 分。要解释这个结果，还得知道学生是怎样分组的，以及两组上了什么课。'},
  {lid:'1.3.2',kind:'paragraph',text:'学生原来的成绩如何？谁先报名参加？这些信息决定我们还能想到哪些解释。'},
  {lid:'1.4',kind:'section',text:'教室里发生的事',children:['1.4.1']},
  {lid:'1.4.1',kind:'paragraph',text:'打开教学安排，看看两组上了几节课、用了什么教材。如果使用软件的学生每周还多练一小时，额外练习也可能带来差距。'},
];

export const quote = '随机分组';
export const secondOccurrence = rootText.lastIndexOf(quote);
export const firstQuestion = '抽签决定谁参加，还是决定谁用软件？';
const firstAnswer = '已经有学生参加后，研究者抽签决定谁用软件。';
type Chat = {id:string; title:string; origin?:FollowUpOrigin; turns:ReturnType<typeof turn>[]};

function turn(id:string, user:string, answer:string, ordinal=1) {
  return {turn_id:id,user_turn_ordinal:ordinal,user,status:'completed',outcome:{
    answer,answer_view:{parts:[{kind:'markdown',text:answer}],sources:[]},incomplete:false,
    effects:[],trace:[],memory_updates:[],turns:1,tokens_spent:1,
    profile_usage:{snapshot_revision:0,injected_fact_ids:[],claimed_used_fact_ids:[],influences:[]},
  },effect_labels:[],question_source_label:null,question_quote:null};
}
function origin(parent:'root'|'control', parentText:string, phrase:string, question:string):FollowUpOrigin {
  const start=parentText.indexOf(phrase);
  return {parent_session_id:parent,parent_turn_id:parent+'-answer',parent_question:questions[parent],
    anchor:{start_part:0,start_offset:start,end_part:0,end_offset:start+phrase.length,excerpt:phrase},
    question,client_request_id:parent+'-'+phrase};
}

// Only HTTP persistence and execution are simulated. Selection, rendering, navigation,
// per-chat drafts, viewport projection and source positioning use the production App.
export async function openFollowUpBook(page:Page, width:number, height:number, historyCase?: 'quoted' | 'failed-agent') {
  const base=await setup(page,'markdown',true,true);
  const chats:Chat[]=[
    {id:'root',title:'八分差从哪来',turns:[turn('root-answer',questions.root,rootText)]},
    {id:'control',title:'对照组',origin:origin('root',rootText,'对照组',questions.control),turns:[turn('control-answer',questions.control,groupText)]},
    {id:'hours',title:'课时安排',origin:origin('control',groupText,'课时安排',questions.hours),turns:[turn('hours-answer',questions.hours,'这取决于实验实际的课程记录。')]},
  ];
  if (historyCase) Object.assign(chats[0].turns[0], {question_quote:{
    label:'正文',quote:'学生',status:'resolved',evidence_ranges:[{
      start_lid:'1.1',end_lid:'1.1',ranges:[{lid:'1.1',range:{start:2,end:4}}]
    }]
  }});
  if (historyCase === 'failed-agent') Object.assign(chats[0].turns[0], {status:'failed',outcome:null,
    error:{error_code:'LOCAL_AGENT_UNAVAILABLE',category:'validation',message:'hidden transport diagnostics'},
    domain:{local_agent:{id:'opencode',name:'OpenCode'},effects:[]}});
  let selected='root';
  const selections:string[]=[], runs:Record<string,unknown>[]=[], starts:FollowUpStart[]=[];
  const history=()=>({active_session_id:selected,
    current:{...chats.find(c=>c.id===selected),book_id:scenario.book_id,goals:[]},
    sessions:chats.map(c=>({...c,turn_count:c.turns.length,created_at:'2026-11-18',updated_at:'2026-11-18'}))});
  const book=chapter.map(p=>p.text).join('\n\n');
  const lids=chapter.map(p=>p.lid);
  await page.route('**/api/book/manifest', route=>route.fulfill({json:{tree:chapter.map(p=>{
    const end=chapter.filter(item=>item.lid===p.lid||item.lid.startsWith(p.lid+'.')).at(-1)!;
    return {lid:p.lid,kind:p.kind,span:{start:book.indexOf(p.text),end:book.indexOf(end.text)+end.text.length},
      children:p.children??[],title:p.children?p.text:undefined};
  }),stats_by_lid:{}}}));
  await page.route('**/api/book/text?*',route=>{
    const q=new URL(route.request().url()).searchParams,lid=q.get('lid')!,end=q.get('end');
    const first=lids.indexOf(lid),last=end?lids.indexOf(end):first;
    return route.fulfill({json:{lid,text:chapter.slice(first,last+1).map(p=>p.text).join('\n\n')}});
  });
  await page.route('**/api/agent/history',route=>route.fulfill({json:history()}));
  await page.route('**/api/agent/history/select',route=>{
    selected=route.request().postDataJSON().session_id; selections.push(selected);
    return route.fulfill({json:history()});
  });
  await page.route(/\/api\/agent\/follow-ups(?:\?|$)/,async route=>{
    if (route.request().method()==='GET') return route.fulfill({json:{nodes:chats.map(c=>({
      session_id:c.id,title:c.title,origin:c.origin,question:c.turns.at(-1)?.user??c.origin?.question,
      answer:c.turns.at(-1)?.outcome?.answer,status:c.turns.length?'ready':'unsent',
    }))}});
    const body=route.request().postDataJSON() as FollowUpStart;starts.push(body);
    const c={id:'new-child',title:body.anchor.excerpt,origin:{...body,parent_question:questions.root},turns:[]};
    chats.push(c);return route.fulfill({json:{session_id:c.id}});
  });
  await page.route('**/api/agent/runs',route=>{
    const body=route.request().postDataJSON(); runs.push(body);
    const c=chats.find(chat=>chat.id===selected)!;
    const t=turn('sent-'+runs.length,body.display_user??body.message,firstAnswer,c.turns.length+1);
    if (body.local_agent_id) Object.assign(t, {domain:{local_agent:{id:body.local_agent_id,name:'OpenCode'},effects:[]}});
    c.turns.push(t);return route.fulfill({json:{book_id:scenario.book_id,session_id:c.id,turn_id:t.turn_id}});
  });
  await page.route('**/api/agent/runs/**',route=>{
    const url=new URL(route.request().url()),id=url.pathname.split('/')[4];
    const c=chats.find(chat=>chat.turns.some(t=>t.turn_id===id))!;
    const snapshot={descriptor:{book_id:scenario.book_id,session_id:c.id,turn_id:id},last_seq:1,
      execution_state:'completed',persistence_state:'saved',activities:[],final_view:c.turns.find(t=>t.turn_id===id),error:null};
    return url.pathname.endsWith('/events')
      ? route.fulfill({contentType:'text/event-stream',body:'event: run.snapshot\ndata: '+JSON.stringify({turn_id:id,seq:1,elapsed_ms:0,type:'run.snapshot',payload:snapshot})+'\n\n'})
      : route.fulfill({json:snapshot});
  });
  await page.setViewportSize({width,height});await page.reload();
  if(width<1024) await page.locator('.workspace-mobile-nav').getByRole('button',{name:'问答',exact:true}).click();
  if (historyCase === 'failed-agent') await expect(page.locator('[data-turn-id="root-answer"] .failed-question')).toBeVisible();
  else await expect(page.locator('[data-turn-id="root-answer"] .answer-markdown')).toContainText(quote);
  return {starts,runs,selections,errors:base.errors};
}
