import {test,expect,type Page} from '@playwright/test';
import {mkdirSync} from 'node:fs';
import {resolve} from 'node:path';
import {boardFixture,boardFixtureReceipt,boardFixturePresentation} from '@understand-book/web/reader/fixtures/board-workspace';

const output = resolve('../../docs/performance/personal-notes-20261030/placements');
async function setup(page:Page, initial='对话') {
  const notes = new Map<string,any>(), attachments = new Map<string,any>(), states = new Map<number,any>();
  const requests:{path:string;body:any}[]=[];
  const board=boardFixture(); board.document.board_id='board-probability'; board.document.items=board.document.items.slice(0,1);board.document.links=[];
  board.view.selected_item_id=board.document.items[0].item_id;board.view.reading_item_id=null;
  const presentation=boardFixturePresentation();presentation.title='先验与证据';
  presentation.content_files={'index.html':`<!doctype html><html><head><meta charset="utf-8"><style>body{font:16px/1.9 system-ui;background:#fffdf7;color:#4b4033;padding:28px}button{border:1px solid #d5c7b0;border-radius:8px;padding:8px 20px;background:#f8efdf;color:#83633d}h1{font-size:25px}</style></head><body><h1 id="title"></h1><p>改变先验，观察同一份证据的含义。</p><button id="next">下一页</button><script>let page='先验';function render(){document.getElementById('title').textContent=page}document.getElementById('next').onclick=()=>{page=page==='先验'?'证据':'先验';render();presentation.commitState()};presentation.registerStateReader(()=>({values:{page},visible_step:page,observed_result:page,source_ref_ids:[]}));presentation.registerStateRestorer(state=>{page=state.values.page?.page??'先验';render()});render();</script></body></html>`};
  await page.route('**/api/**',async route=>{
    const path=new URL(route.request().url()).pathname,body=route.request().postDataJSON()??{};requests.push({path,body});let data:any={};
    if(path.endsWith('/auth/me'))data={user_id:'placement-reader',csrf_token:'test'};
    else if(path.endsWith('/library'))data={books:[]};
    else if(path.endsWith('/videos'))data={videos:[]};
    else if(path.endsWith('/learning-workspaces'))data={workspaces:[]};
    else if(path.endsWith('/reading-resumption'))data=null;
    else if(path.includes('/workspace-releases'))data={items:[],total:0};
    else if(path.endsWith('/me/notes/list'))data={notes:[...notes.values()],next_cursor:null};
    else if(path.endsWith('/me/notes/create')||path.endsWith('/me/notes/revise')){
      const id=body.note_id||`note-${notes.size+1}`,revision=(notes.get(id)?.current_revision??0)+1;
      data={note_id:id,current_revision:revision,created_at:new Date().toISOString(),value:{...body.value,revision,saved_at:new Date().toISOString(),record:{content:body.value.content,note:body.value.source,anchor:{lid:null,concept:null}}}};
      notes.set(id,data);
    }
    else if(path.endsWith('/me/notes/read'))data=notes.get(body.note_id);
    else if(path.endsWith('/me/notes/history'))data={revisions:[notes.get(body.note_id)?.value],discussions:[]};
    else if(path.endsWith('/me/notes/links'))data={links:[]};
    else if(path.endsWith('/me/notes/attachments'))data={items:[...attachments.values()].filter(a=>!body.note_id||a.note_id===body.note_id).map(a=>({attachment:a,note:notes.get(a.note_id)}))};
    else if(path.endsWith('/me/notes/attach')){
      data=[...attachments.values()].find(a=>a.note_id===body.note_id&&JSON.stringify(a.target)===JSON.stringify(body.target));
      if(!data){data={attachment_id:`attachment-${attachments.size+1}`,note_id:body.note_id,target:body.target,label:body.target.kind==='message'?'对话 · 原句':'演示 · '+body.target.visible_step,created_at:new Date().toISOString()};attachments.set(data.attachment_id,data);}
    }
    else if(path.endsWith('/me/notes/unattach')){attachments.delete(body.attachment_id);data={removed:true};}
    else if(path.endsWith('/me/notes/attach_board')){
      let item=board.document.items.find(i=>i.target.kind==='personal_note'&&i.target.reference.note_id===body.note_id);
      if(!item){item={...structuredClone(board.document.items[0]),item_id:'placed-note',title:'我的想法',position:{x:400,y:0},target:{kind:'personal_note',reference:{note_id:body.note_id}}};board.document.items.push(item);board.document.revision++;}
      if(body.item_id&&!board.document.links.length)board.document.links.push({link_id:'thought-link',from:{kind:'item',item_id:item.item_id},to:{kind:'item',item_id:body.item_id},label:'我的想法',based_on:[]});
      data={board_id:board.document.board_id,item_id:item.item_id};
    }
    else if(path.endsWith('/me/notes/board_locations'))data={items:board.document.items.filter(i=>i.target.kind==='personal_note'&&i.target.reference.note_id===body.note_id).map(i=>({kind:'board',board_id:board.document.board_id,workspace_id:board.document.learning_workspace_id,item_id:i.item_id,label:board.document.title,book_id:board.document.book_id,publication_id:board.document.publication_id}))};
    else if(path.endsWith('/boards/list'))data={items:[{...board.document,item_count:board.document.items.length}]};
    else if(path.endsWith('/boards/read')){for(const item of board.document.items)if(item.target.kind==='personal_note'){const n=notes.get(item.target.reference.note_id);item.title=n.value.title||'我的想法';item.readable_content=n.value.record.content;item.target.reference.revision=n.current_revision;}data=board;}
    else if(path.endsWith('/boards/conversation'))data={sessions:[]};
    else if(path.endsWith('/boards/update')){for(const p of body.positions??[]){const item=board.document.items.find(i=>i.item_id===p.item_id);if(item)item.position=p.position;}board.document.revision++;data=board.document;}
    else if(path.endsWith('/boards/view.save')){Object.assign(board.view,body.view);data={saved:true};}
    else if(path.endsWith('/agent/presentation.read'))data={...presentation,restored_state:body.saved_state?states.get(body.saved_state.state_revision):null,restored_state_revision:body.saved_state?.state_revision??null};
    else if(path.endsWith('/agent/presentation.state.save')){const revision=states.size+1;states.set(revision,body.state);data={...boardFixtureReceipt,state_revision:revision,saved_state_ref:`state-${revision}`};}
    else if(path.includes('/agent/presentation.feedback'))data={selection:null,revision:0};
    else if(path.endsWith('/memory/prepare_note'))data={content:'',title:null,authorship:'user',context:{session_id:'chat-original',turn_id:'turn-original',label:body.label},source_refs:[],change_note:null,based_on_turn:null,source:null,capture:null};
    await route.fulfill({json:data});
  });
  await page.goto('/');await expect(page.getByRole('button',{name:'记一下',exact:true})).toBeVisible();
  await openScene(page,initial);
  return {notes,attachments,requests,board,states};
}
async function openScene(page:Page,initial='对话') {await page.evaluate(async mode=>{const path='/playwright/fixtures/thought-placements.ts';(await import(path)).openPlacementScene(mode);},initial);}
async function selectSecondQuote(page:Page){await page.locator('[data-paragraph="second"]').evaluate(el=>{const range=document.createRange();range.setStart(el.firstChild!,0);range.setEnd(el.firstChild!,8);const s=getSelection()!;s.removeAllRanges();s.addRange(range);el.dispatchEvent(new PointerEvent('pointerup',{bubbles:true}));});}

test('a selected sentence keeps its occurrence, survives a placement retry and follows later edits',async({page})=>{
  await page.setViewportSize({width:1440,height:1000});const fixture=await setup(page);
  const errors:string[]=[];page.on('pageerror',e=>errors.push(e.message));
  await selectSecondQuote(page);await page.getByRole('button',{name:'在这句话下记想法'}).click();
  await page.getByRole('textbox',{name:'想法正文',exact:true}).fill('不能只看检测准确率，还要看这类事件原本有多常见。');
  await page.route('**/api/me/notes/attach',r=>r.fulfill({status:503,json:{message:'暂时离线',error_code:'TEMPORARY',category:'temporary'}}),{times:1});
  await page.getByRole('button',{name:'保存并放置',exact:true}).click();
  await expect(page.getByRole('alert')).toContainText('想法已保存，放置未完成');
  expect(fixture.notes.size).toBe(1);expect(fixture.attachments.size).toBe(0);
  await page.reload();await expect(page.getByRole('button',{name:'记一下',exact:true})).toBeVisible();await openScene(page);
  await page.getByRole('button',{name:'记一下',exact:true}).click();
  await page.getByRole('button',{name:'重试放置',exact:true}).click();
  const thought=page.locator('[data-paragraph="second"] + .thought-inline-mount .attached-thought');
  await expect(thought).toBeVisible();expect(fixture.notes.size).toBe(1);
  expect([...fixture.attachments.values()][0].target.occurrence).toBe(1);
  await thought.getByRole('button',{name:'打开 · 第 1 版 ↗'}).click();await page.getByRole('button',{name:'继续写',exact:true}).click();
  await page.getByRole('textbox',{name:'想法正文',exact:true}).fill('先验与似然一起决定后验，我要同时比较两类人群。');
  await page.getByRole('button',{name:'保存想法',exact:true}).click();
  await expect(thought).toContainText('第 2 版');await expect(thought).toContainText('先验与似然一起决定后验');
  await page.getByRole('button',{name:'收起我的想法',exact:true}).click();
  mkdirSync(output,{recursive:true});await page.screenshot({path:resolve(output,'sentence-thought-1440.png')});
  await thought.locator('summary').click();await thought.getByRole('button',{name:'从这里移除'}).click();
  await expect(thought).toHaveCount(0);expect(fixture.notes.size).toBe(1);expect(errors).toEqual([]);
});

test('the same saved thought can be placed beside a canvas element without changing its origin',async({page})=>{
  await page.setViewportSize({width:1440,height:1000});const fixture=await setup(page);
  await page.locator('[data-paragraph="first"]').click();
  await page.getByRole('button',{name:'记一下',exact:true}).click();
  await page.getByRole('textbox',{name:'想法正文',exact:true}).fill('相同证据，放在不同人群里，结论可能不同。');
  await page.getByRole('button',{name:'保存想法',exact:true}).click();expect(fixture.attachments.size).toBe(0);
  await page.getByRole('button',{name:'记一下',exact:true}).click();await page.getByRole('button',{name:'我的想法',exact:true}).click();await page.locator('.note-list button').first().click();
  await page.locator('[data-placement-scene] > header').getByRole('button',{name:'画布',exact:true}).click();
  await expect(page.locator('.board-card')).toHaveCount(1);
  await page.getByRole('button',{name:'放到…',exact:true}).click();
  await expect(page.getByLabel('选择想法放置位置')).toContainText('关联画布中选中的元素');
  mkdirSync(output,{recursive:true});await page.screenshot({path:resolve(output,'placement-menu-1440.png')});
  await page.getByRole('region',{name:'我的想法',exact:true}).screenshot({path:resolve(output,'placement-window.png')});
  await page.getByRole('button',{name:/关联画布中选中的元素/}).click();
  await expect(page.locator('.board-card')).toHaveCount(2);
  expect(fixture.board.document.links).toHaveLength(1);expect(fixture.notes.size).toBe(1);
  expect([...fixture.notes.values()][0].value.context.origin.kind).toBe('message');
  await page.getByRole('button',{name:'收起我的想法',exact:true}).click();
  await page.screenshot({path:resolve(output,'canvas-thought-1440.png')});
});

test('presentation thoughts follow the recorded page and restore its exact scene',async({page})=>{
  await page.setViewportSize({width:1440,height:1000});const fixture=await setup(page,'演示');
  const frame=page.frameLocator('.agent-presentation iframe');await expect(frame.locator('h1')).toHaveText('先验');
  await page.locator('.agent-presentation').getByRole('button',{name:'记一下',exact:true}).click();
  await page.getByRole('textbox',{name:'想法正文',exact:true}).fill('先明确事件原来的比例，再看证据。');
  await page.getByRole('button',{name:'保存并放置',exact:true}).click();
  await expect(page.locator('.presentation-thoughts')).toContainText('先明确事件原来的比例');
  await frame.getByRole('button',{name:'下一页'}).click();await expect(frame.locator('h1')).toHaveText('证据');
  await expect(page.locator('.presentation-thoughts')).toHaveCount(0);
  await page.locator('.presentation-other-thoughts summary').click();await page.locator('.presentation-other-thoughts button').click();
  await expect(frame.locator('h1')).toHaveText('先验');await expect(page.locator('.presentation-thoughts')).toContainText('先明确事件原来的比例');
  expect(fixture.notes.size).toBe(1);expect([...fixture.attachments.values()][0].target.visible_step).toBe('先验');
  mkdirSync(output,{recursive:true});await page.screenshot({path:resolve(output,'presentation-thought-1440.png')});
});

test('the placement picker and writing controls remain usable on a narrow phone',async({page})=>{
  await page.setViewportSize({width:390,height:844});await setup(page);
  await selectSecondQuote(page);await page.getByRole('button',{name:'记一下',exact:true}).click();
  await page.getByRole('textbox',{name:'想法正文',exact:true}).fill('先记住这个疑问，继续看材料。');
  await page.getByRole('button',{name:'放到…',exact:true}).click();
  const picker=page.getByLabel('选择想法放置位置');await expect(picker).toBeInViewport();
  const bounds=(await picker.boundingBox())!;expect(bounds.x).toBeGreaterThanOrEqual(0);expect(bounds.x+bounds.width).toBeLessThanOrEqual(390);
  mkdirSync(output,{recursive:true});await page.screenshot({path:resolve(output,'placement-menu-390.png')});
  await picker.getByRole('button',{name:/选中的这句话下面/}).click();
  await expect(page.getByRole('textbox',{name:'想法正文',exact:true})).toHaveValue('先记住这个疑问，继续看材料。');
  await page.getByRole('button',{name:'保存并放置',exact:true}).click();await expect(page.locator('.attached-thought')).toBeVisible();
});

test('global capture freezes the visible presentation while defaulting to only save',async({page})=>{
  await page.setViewportSize({width:1440,height:1000});const fixture=await setup(page,'演示');
  const frame=page.frameLocator('.agent-presentation iframe');await expect(frame.locator('h1')).toHaveText('先验');
  await page.locator('.thought-toolbar').getByRole('button',{name:'记一下',exact:true}).click();
  const input=page.getByRole('textbox',{name:'想法正文',exact:true});await expect(input).toBeFocused();await input.fill('先写下来，暂时不放进演示。');
  await expect(page.getByRole('button',{name:'保存想法',exact:true})).toBeEnabled();
  await frame.getByRole('button',{name:'下一页'}).click();await expect(frame.locator('h1')).toHaveText('证据');
  await page.getByRole('button',{name:'保存想法',exact:true}).click();
  expect([...fixture.notes.values()][0].value.context.origin.visible_step).toBe('先验');expect(fixture.attachments.size).toBe(0);
});

test('a board covering the selected conversation becomes the capture source and available placement',async({page})=>{
  await page.setViewportSize({width:1440,height:1000});const fixture=await setup(page);
  await page.locator('[data-paragraph="first"]').click();
  await page.evaluate(async()=>{const path='/playwright/fixtures/thought-placements.ts';(await import(path)).coverWithBoard();});
  await expect(page.locator('.board-card')).toHaveCount(1);
  await expect(page.locator('[data-paragraph="first"]')).toHaveCount(1);
  await page.getByRole('button',{name:'记一下',exact:true}).click();
  await page.getByRole('textbox',{name:'想法正文',exact:true}).fill('现在是在画布上产生的想法。');
  await page.getByRole('button',{name:'放到…',exact:true}).click();
  const picker=page.getByLabel('选择想法放置位置');
  await expect(picker).toContainText('关联画布中选中的元素');
  await expect(picker).not.toContainText('这条消息下面');
  await picker.getByRole('button',{name:/只保存想法/}).click();
  await page.getByRole('button',{name:'保存想法',exact:true}).click();
  const context=[...fixture.notes.values()][0].value.context;
  expect(context.board_id).toBe('board-probability');expect(context.origin).toBeUndefined();
  expect(fixture.attachments.size).toBe(0);
});

test('the recording entry avoids the active board composer and ignores the conversation behind it', async ({page}) => {
  await page.setViewportSize({width:1440,height:900}); await setup(page);
  await page.evaluate(async () => { const path='/playwright/fixtures/send-button.ts'; (await import(path)).openConversation(); });
  await page.locator('textarea[data-workspace-input="agent"]').fill('背景对话里还在写的长问题。\n'.repeat(8));
  await page.evaluate(async () => { const path='/playwright/fixtures/thought-placements.ts'; (await import(path)).coverWithBoard(); });
  await expect(page.locator('.board-card')).toHaveCount(1);
  for (const width of [1440,390]) {
    await page.setViewportSize({width,height:900});
    if (width===390) await page.locator('.board-mobile-switch').getByRole('button',{name:'讨论',exact:true}).click();
    const input=page.getByRole('textbox',{name:'在画板上提问',exact:true});
    await input.fill('船加重以后，浮力会怎样变化？');
    await expect.poll(async()=>{
      const entry=(await page.locator('.thought-toolbar').boundingBox())!, composer=(await page.locator('.board-conversation').boundingBox())!;
      return Math.abs(entry.y + entry.height + 12 - composer.y);
    }).toBeLessThan(1);
    await page.getByRole('button',{name:'发送画板追问',exact:true}).click({trial:true});
  }
});
