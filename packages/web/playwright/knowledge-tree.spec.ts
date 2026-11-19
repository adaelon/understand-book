import { expect, test } from '@playwright/test';
import { setup, scenario } from './fixtures/tutor-continuation';

for(const width of [1366,390])test(`knowledge branches, fruit and continuation remain usable at ${width}px`,async({page},testInfo)=>{
  await page.addInitScript(()=>localStorage.setItem('understand-book.product-tour.v1.local.reader','done'));
  const fixture=await setup(page,'markdown',true,true);
  const reference=(id:string)=>({source_id:scenario.book_id,source_revision:'tc0-source',publication_id:null,artifact:'pass1',entry_id:id});
  const node=(id:string,label:string,child_count=0,fruit_count=0)=>({id,label,parent_id:id==='root'?null:'root',reference:reference(id),
    kind:id==='root'?'material':'claim',summary:id==='time'?'总时间从出发计到抵达，也包括途中停车的时间。':'',sources:id==='root'?[]:[{kind:'text',lid:'1.1'}],relations:[],child_count,fruit_count});
  const root=node('root','认识运动',4,1),time=node('time','总时间包含途中停车',0,1);
  const fruit={focus_ref:'focus-time',source_id:scenario.book_id,source_revision:'tc0-source',workspace_id:null,label:'总时间包含停车时间',capability:'explanation',
    target:{learning_focus:'总时间包含停车时间',expected_performance:'说明时间范围',capability:'explanation',source_bindings:[{source_id:scenario.book_id,source_revision:'tc0-source',start_lid:'1.1',end_lid:'1.1'}],object_refs:[]},
    demonstrated:['能说明停车期间仍在整段行程之中'],difficulties:[],unconfirmed:['在新的情境中运用还有待确认'],evidence_ref:'e-time',evidence_refs:['e-time'],note_refs:width>1000?[{note_id:'old-note',revision:1}]:[],next_step:'比较一次有停车的行程',
    occurred_at:'2026-11-05T09:00:00+08:00',source_session_id:'tc0-chat',source_turn_id:'tc0-answer',assistance:'在已提供讲解下',nature:'hypothesis',learner_quote:'停车时虽然没有走路，但这段时间还在整趟行程里。',correction:null,
    knowledge_links:[{reference:reference('time'),quote:'这段时间还在整趟行程里'}]};
  const seen:unknown[]=[];
  await page.route('**/api/knowledge/tree',route=>{
    const body=route.request().postDataJSON();seen.push(body);const selected=body.node_id==='time'?time:root;
    const branches=[{...time,understanding:[fruit]},node('distance','总路程包含去程与回程')];
    return route.fulfill({json:{node:selected,children:branches,map_groups:[{topic:root,nodes:branches}],
      breadcrumbs:[{id:'root',label:'认识运动'},...(selected.id==='root'?[]:[{id:'time',label:time.label}])],next:null,total:selected.child_count,fruits:[fruit],current_ids:['root'],modality:'text',evidence_watermark:1}});
  });
  await page.route('**/api/knowledge/resume',route=>route.fulfill({json:{context:{book_id:scenario.book_id,publication_id:null,workspace_id:null,session_id:'tc0-chat',turn_id:'tc0-answer',label:fruit.label},source_revision:'tc0-source',target:fruit.target,understanding_ref:{workspace_id:null,evidence_ref:'e-time'}}}));
  await page.setViewportSize({width,height:width>1000?1000:844});await page.reload();
  if(width<1024)await page.getByRole('navigation',{name:'阅读工作区'}).getByRole('button',{name:'问答',exact:true}).click();
  else await page.getByRole('button',{name:'学习设置',exact:true}).click();
  await page.getByRole('button',{name:'知识树',exact:true}).click();
  const tree=page.getByRole('main',{name:'我的理解',exact:true});
  await expect(tree.getByText('总时间包含途中停车',{exact:true})).toBeVisible();
  await page.screenshot({path:testInfo.outputPath(`knowledge-tree-${width}.png`)});
  await tree.getByRole('button',{name:/总路程包含去程与回程/}).click();
  await tree.getByRole('button',{name:'看看原文',exact:true}).click();
  const source = page.getByRole('dialog',{name:'引用来源',exact:true});
  await expect(source.locator('mark').first()).toBeInViewport();
  await source.getByRole('button',{name:'关闭来源预览',exact:true}).click();
  await page.getByRole('banner').getByRole('button',{name:'我的理解',exact:true}).click();
  await expect(tree).toBeVisible();
  await tree.getByRole('button',{name:/总时间包含途中停车/}).click();
  await tree.getByText('查看理解依据',{exact:true}).click();
  await expect(tree.getByText(fruit.learner_quote,{exact:true})).toHaveCount(1);
  await expect(tree.getByText('在已提供讲解下',{exact:true})).toBeVisible();
  const discuss=tree.getByRole('button',{name:'接着这一点'});
  await discuss.scrollIntoViewIfNeeded();
  await page.screenshot({path:testInfo.outputPath(`knowledge-fruit-${width}.png`)});
  expect(await page.evaluate(()=>document.documentElement.scrollWidth<=window.innerWidth)).toBe(true);
  await discuss.click();
  expect(fixture.requests).toHaveLength(0);
  await tree.getByRole('button',{name:'发送到原讨论'}).click();
  await expect.poll(()=>fixture.requests.length).toBe(1);
  expect(fixture.requests[0]).toMatchObject({understanding_ref:{evidence_ref:'e-time'},entry_action:'continue_answer'});
  expect(fixture.requests[0].note_refs).toBeUndefined();expect(fixture.errors).toEqual([]);
  expect(seen.length).toBeGreaterThanOrEqual(2);
});

test('starting from a knowledge branch retains the selected occurrence as well as its identity',async({page})=>{
  await page.addInitScript(()=>localStorage.setItem('understand-book.product-tour.v1.local.reader','done'));
  const fixture=await setup(page,'markdown',true,true);
  const reference={source_id:scenario.book_id,source_revision:'tc0-source',publication_id:null,artifact:'pass1',entry_id:'concept:time'};
  const leaf={id:'time-in-second-section',label:'总时间',kind:'concept',reference,parent_id:'root',summary:'',sources:[{kind:'text',lid:'1.2'}],relations:[],child_count:0,fruit_count:0};
  const root={...leaf,id:'root',label:'运动',kind:'material',parent_id:null,child_count:1};
  await page.route('**/api/knowledge/tree',route=>{
    const selected=route.request().postDataJSON().node_id===leaf.id?leaf:root;
    return route.fulfill({json:{node:selected,children:[leaf],map_groups:[{topic:root,nodes:[leaf]}],breadcrumbs:[{id:'root',label:'运动'}],next:null,total:selected.child_count,fruits:[],current_ids:[],modality:'text',entry_point:selected.id==='root'?leaf:null}});
  });
  await page.setViewportSize({width:1366,height:1000});await page.reload();
  await page.getByRole('button',{name:'学习设置',exact:true}).click();await page.getByRole('button',{name:'知识树',exact:true}).click();
  const tree=page.getByRole('main',{name:'我的理解',exact:true});
  await tree.getByRole('button',{name:/总时间/}).click();
  await tree.getByRole('button',{name:'从这里探索',exact:true}).click();
  await expect.poll(()=>fixture.requests.length).toBe(1);
  expect(fixture.requests[0].knowledge_focus).toEqual({reference,node_id:leaf.id});expect(fixture.errors).toEqual([]);
});
