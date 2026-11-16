import { expect, test } from '@playwright/test';
import { setup, scenario } from './fixtures/tutor-continuation';

for(const width of [1366,390])test(`primary understanding map preserves the reader and exposes changes at ${width}px`,async({page},testInfo)=>{
  await page.addInitScript(()=>localStorage.setItem('understand-book.product-tour.v1.local.reader','done'));
  const fixture=await setup(page,'markdown',true,true);
  const reference=(id:string)=>({source_id:scenario.book_id,source_revision:'tc0-source',publication_id:null,artifact:'pass1',entry_id:id});
  const record={focus_ref:'total-time',source_id:scenario.book_id,source_revision:'tc0-source',workspace_id:null,label:'总时间包含停车',capability:'explanation',target:null,
    demonstrated:['能解释停车仍属于整段行程'],difficulties:[],unconfirmed:['尚未观察新情境中的独立应用'],evidence_ref:'time-change',evidence_refs:['time-change'],note_refs:[],next_step:'换一段有停车的行程，试着判断总时间。',
    occurred_at:null,source_session_id:'tc0-chat',source_turn_id:'tc0-answer',assistance:'在已提供帮助下',nature:'hypothesis',learner_quote:'停车时虽然没有走，但仍在整段行程中。',correction:null,
    learner_summary:{headline:'你把停车时间也算进了整段行程。',explanation:'从只计算走动的时间，到区分走动时间与整段行程的总时间；这次是在讨论后说清楚的。',next_step:'换一段有停车的行程，试着判断总时间。'},
    previous:{evidence_ref:'before',learner_quote:'停车不算运动时间，所以可以忽略。',assistance:'本次未记录提示或先前讲解'},knowledge_links:[{reference:reference('time'),quote:'仍在整段行程中'}]};
  const node=(id:string,label:string,patch:Record<string,unknown>={})=>({id,label,parent_id:'root',reference:reference(id),kind:'claim',summary:'对照原文、猜想和观察，逐步弄清这个关系。',sources:[{kind:'text',lid:'1.1'}],relations:[],child_count:0,fruit_count:0,understanding:[],...patch});
  const time=node('time','总时间为什么包含停车？',{understanding:[record],fruit_count:1});
  const groups=[
    {topic:node('motion','描述一段运动',{child_count:3,understanding:[record]}),nodes:[time,node('distance','怎样区分位移与路程？'),node('average','平均速度取决于哪些量？')]},
    {topic:node('compare','比较不同的运动',{child_count:2}),nodes:[node('segments','分段速度能直接平均吗？'),node('return','往返为什么需要分别计算？')]},
    {topic:node('use','把理解用起来',{child_count:2}),nodes:[node('transfer','换一段行程，还能说清吗？'),node('question','用什么观察验证你的判断？')]},
  ];
  const root=node('root','运动与时间',{parent_id:null,child_count:3,understanding:[record]});
  await page.route('**/api/knowledge/tree',route=>route.fulfill({json:{node:root,children:groups.map(g=>g.topic),map_groups:groups,breadcrumbs:[{id:'root',label:root.label}],next:null,total:3,fruits:[record],current_ids:['root','motion','time'],modality:'text'}}));
  await page.setViewportSize({width,height:width>1000?960:844});await page.reload();
  if(width<1024)await page.getByRole('navigation',{name:'阅读工作区'}).getByRole('button',{name:'问答',exact:true}).click();
  await page.locator('.agent-input textarea').fill('这段思考先留在这里');
  await page.getByRole('banner').getByRole('button',{name:'我的理解',exact:true}).click();
  const map=page.getByRole('main',{name:'我的理解',exact:true});
  await expect(map.getByRole('heading',{name:'理解地图'})).toBeVisible();
  await expect(map.getByText(record.learner_summary.headline,{exact:true})).toBeVisible();
  if(width<460){await expect(map.locator('.topic-questions').nth(1)).toBeHidden();}
  await page.screenshot({path:testInfo.outputPath(`map-${width}.png`)});
  await map.locator('.question-node').first().click();
  await expect(map.locator('.selected-understanding')).toContainText('这次是在讨论后说清楚的');
  await expect(map.locator('.selected-understanding')).not.toContainText('不要把');
  await map.locator('.selected-understanding').screenshot({path:testInfo.outputPath(`change-${width}.png`)});
  expect(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth)).toBe(true);
  await map.getByRole('button',{name:'返回学习',exact:true}).click();
  await expect(page.locator('.agent-input textarea')).toHaveValue('这段思考先留在这里');
  expect(fixture.errors).toEqual([]);
});
