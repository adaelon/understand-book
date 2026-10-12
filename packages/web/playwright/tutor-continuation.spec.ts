import { expect, test } from '@playwright/test';
import { scenario, setup, selectSource } from './fixtures/tutor-continuation';

for (const surface of ['markdown','pdf'] as const) for (const ready of [true,false]) {
  test(`continuation_baseline_${surface}_${ready ? 'ready' : 'missing_foundation'}_manual_prompt`, async ({page}) => {
    const f = await setup(page,surface,ready);
    await selectSource(page,surface);
    await page.locator('.hl-popover').getByRole('button',{name:'问 AI',exact:true}).click();
    const input=page.locator('.agent-input textarea');
    await expect(input).toHaveValue('');
    expect(f.requests).toHaveLength(0);
    await input.fill('Explain this selection');
    await page.locator('.agent-input').getByRole('button',{name:'发送',exact:true}).click();
    await expect.poll(()=>f.requests.length).toBe(1);
    expect(f.requests[0].question_quote).toMatchObject({lid:'1.1',quote:scenario.quote,ranges:[{lid:'1.1',range:{start:0,end:scenario.quote.length}}]});
    expect(f.requests[0].message).toBe('Explain this selection');
    expect(f.errors).toEqual([]);
  });

  test(`continuation_tc2_${surface}_${ready ? 'ready' : 'missing_foundation'}_direct_help`, async ({page}) => {
    const f = await setup(page,surface,ready);
    await page.locator('.agent-input textarea').fill(scenario.draft);
    await selectSource(page,surface);
    const action=page.locator('.hl-popover').getByRole('button',{name:'帮我弄懂这段',exact:true});
    await expect(action,'TC2 direct-help entry is missing; existing 问 AI needs a prompt').toBeVisible({timeout:1500});
    await action.click();
    await expect.poll(()=>f.requests.length).toBe(1);
    expect(f.requests[0]).toMatchObject({message:'',entry_action:'explain_selection',question_quote:{quote:scenario.quote}});
    await expect(page.locator('.agent-input textarea')).toHaveValue(scenario.draft);
  });
}

test('continuation_tc5_answer_excerpt_is_sent_from_the_original_answer',async({page})=>{
  const f=await setup(page,'markdown',true,true);
  const answer=page.locator('.ans-text').filter({hasText:scenario.answer});
  await expect(answer).toBeVisible();
  await answer.evaluate((element,excerpt)=>{
    const node=element.querySelector('p')!.firstChild!;
    const start=node.textContent!.indexOf(excerpt),range=document.createRange();
    range.setStart(node,start);range.setEnd(node,start+excerpt.length);
    const selection=window.getSelection()!;selection.removeAllRanges();selection.addRange(range);
    element.dispatchEvent(new MouseEvent('mouseup',{bubbles:true}));
  },scenario.answer_excerpt);
  const action=page.locator('.answer-popover').getByRole('button',{name:'这一步没跟上',exact:true});
  await expect(action,'TC5 answer selection only offers notes/sharing today').toBeVisible({timeout:1500});
  await action.click();
  await expect.poll(()=>f.requests.length).toBe(1);
  expect(f.requests[0]).toMatchObject({entry_action:'clarify_answer',answer_ref:{turn_id:'tc0-answer',excerpt:scenario.answer_excerpt}});
  expect(f.requests[0].teaching_ref).toBeUndefined();
});

for (const width of [1440,390]) test(`continuation_tc5_keyboard_choice_and_correction_${width}`, async ({page}) => {
  const f=await setup(page,'markdown',true,true,true);
  await page.setViewportSize({width,height:900});
  if(width<1024) await page.locator('.workspace-mobile-nav').getByRole('button',{name:'问答',exact:true}).click();
  await page.locator('.agent-input textarea').fill(scenario.draft);
  const choice=page.getByRole('button',{name:'比较两段等长行程',exact:true});
  await expect(choice).toHaveCount(1); await expect(choice).toBeVisible();
  await choice.focus(); await expect(choice).toBeFocused();
  await page.screenshot({path:`../../docs/performance/tutor-continuation-tc5/choice-${width}.png`});
  await page.keyboard.press('Enter');
  await expect.poll(()=>f.requests.length).toBe(1);
  expect(f.requests[0]).toMatchObject({message:'',teaching_choice:{delivery_ref:'tc5-delivery',option_id:'compare'},expected_chat_head:'tc0-answer',question_anchor_lid:null});
  expect(f.requests[0].teaching_ref).toBeUndefined();
  await expect(page.locator('.agent-input textarea')).toHaveValue(scenario.draft);
  await expect(page.getByRole('button',{name:'查看已接纳的任务',exact:true})).toBeVisible();
  await page.locator('.turn').last().getByRole('button',{name:'这一步没跟上',exact:true}).click();
  await expect.poll(()=>f.requests.length).toBe(2);
  expect(f.requests[1]).toMatchObject({entry_action:'clarify_answer',answer_ref:{turn_id:'tc2-turn-1'},expected_chat_head:'tc2-turn-1'});
  await expect(page.locator('.agent-input textarea')).toHaveValue(scenario.draft);
  expect(f.displayed.length).toBeGreaterThan(0); expect(f.errors).toEqual([]);
});

test('continuation_tc5_free_question_never_inherits_choice', async ({page}) => {
  const f=await setup(page,'markdown',true,true,true);
  await page.locator('.tutor-continuation').getByRole('button',{name:'换个问题'}).click();
  await expect(page.locator('.agent-input textarea')).toBeFocused();
  await page.locator('.agent-input textarea').fill('不是不懂计算，是不懂为什么要用总时间');
  await page.locator('.agent-input').getByRole('button',{name:'发送',exact:true}).click();
  await expect.poll(()=>f.requests.length).toBe(1);
  expect(f.requests[0].teaching_choice).toBeUndefined(); expect(f.requests[0].entry_action).toBeUndefined();
  await expect(page.getByRole('button',{name:'比较两段等长行程',exact:true})).toHaveCount(0);
});

test('continuation_tc5_local_refresh_queries_unknown_action_before_retry', async ({page}) => {
  const f=await setup(page,'markdown',true,true,true);
  let original: any;
  await page.route('**/api/agent/runs', async route => { original=route.request().postDataJSON(); await route.abort(); });
  await page.getByRole('button',{name:'比较两段等长行程',exact:true}).click();
  await expect(page.getByRole('button',{name:'核对原操作',exact:true})).toBeVisible();
  await page.reload();
  await expect(page.getByRole('button',{name:'核对原操作',exact:true})).toBeVisible();
  const queries: string[]=[];
  await page.route('**/api/agent/reading-action-status',route=>{queries.push('query');return route.fulfill({json:{accepted:false}});});
  await page.unroute('**/api/agent/runs');
  // Restore only the endpoint override, keeping the fixture's normal route.
  let retried:any;
  await page.route('**/api/agent/runs',route=>{queries.push('send');retried=route.request().postDataJSON();return route.fulfill({json:{kind:'activity_help',delivery_refs:['original']}});});
  await page.getByRole('button',{name:'重试原操作',exact:true}).click();
  await expect.poll(()=>queries.length).toBe(2);
  expect(queries).toEqual(['query','send']); expect(retried).toEqual(original);
  await expect(page.getByRole('button',{name:'核对原操作',exact:true})).toHaveCount(0);
  expect(f.requests).toHaveLength(0);
});

test('continuation_tc2_first_help_enable_and_continue_preserves_draft', async ({page}) => {
  const f = await setup(page,'markdown',true);
  await page.locator('.agent-input textarea').fill(scenario.draft);
  await selectSource(page,'markdown');
  await page.getByRole('button',{name:'帮我弄懂这段',exact:true}).click();
  await expect(page.locator('.reading-continuation').getByRole('button',{name:'开启连续教学',exact:true})).toBeVisible();
  expect(f.requests).toHaveLength(1);
  expect(f.requests[0]).toMatchObject({message:'',entry_action:'explain_selection',source_id:scenario.book_id,
    source_revision:'tc0-source',session_id:'tc0-chat',expected_chat_head:null});
  await page.screenshot({path:'../../docs/performance/tutor-continuation-tc2/first-help.png'});
  await page.locator('.reading-continuation').getByRole('button',{name:'开启连续教学',exact:true}).click();
  const next = page.getByRole('button',{name:'继续理解这段',exact:true});
  await expect(next).toBeVisible();
  expect(f.mutations).toHaveLength(1);
  expect(f.requests).toHaveLength(1);
  await next.click();
  await expect.poll(()=>f.requests.length).toBe(2);
  expect(f.requests[1]).toMatchObject({message:'',entry_action:'continue_answer',
    answer_ref:{turn_id:'tc2-turn-1'},expected_chat_head:'tc2-turn-1',question_anchor_lid:null});
  expect(f.requests[1].teaching_ref).toBeUndefined();
  await expect(page.locator('.agent-input textarea')).toHaveValue(scenario.draft);
  await expect(page.locator('.ans-text')).toHaveCount(2);
  await page.screenshot({path:'../../docs/performance/tutor-continuation-tc2/continue-answer.png'});
  expect(f.errors).toEqual([]);
});

test('continuation_tc2_enable_failure_never_sends_answer_request', async ({page}) => {
  const f = await setup(page,'markdown',true,true);
  await page.route('**/api/tutor/mutate', route => route.fulfill({status:503,json:{error_code:'LEARNING_STORAGE_UNAVAILABLE',category:'unavailable',message:'写入失败'}}));
  await page.locator('.reading-continuation').getByRole('button',{name:'开启连续教学',exact:true}).click();
  await expect(page.getByRole('button',{name:'继续理解这段',exact:true})).toHaveCount(0);
  expect(f.requests).toHaveLength(0);
  await expect(page.locator('.reading-continuation').getByRole('button',{name:'开启连续教学',exact:true})).toBeDisabled();
});

test('continuation_tc2_activity_help_does_not_leave_a_phantom_chat_turn', async ({page}) => {
  await setup(page,'markdown',true);
  await page.route('**/api/agent/runs', route => route.fulfill({json:{kind:'activity_help',delivery_refs:['original-question']}}));
  await page.locator('.agent-input textarea').fill(scenario.draft);
  await selectSource(page,'markdown');
  await page.getByRole('button',{name:'帮我弄懂这段',exact:true}).click();
  await expect(page.getByText('这段内容属于当前待答题', {exact:false})).toBeVisible();
  await expect(page.getByText('操作：帮我弄懂这段',{exact:true})).toHaveCount(0);
  await expect(page.locator('.agent-input textarea')).toHaveValue(scenario.draft);
});


test('continuation_tc2_original_question_navigation_preserves_unsent_draft', async ({page}) => {
  await setup(page,'markdown',true);
  await page.route('**/api/agent/runs', route => route.fulfill({json:{kind:'activity_help',delivery_refs:['original-question'],
    activity:{session_id:'original-chat',turn_id:'original-turn'}}}));
  let selected = '';
  await page.route('**/api/agent/history/select', route => {
    selected = route.request().postDataJSON().session_id;
    return route.fulfill({json:{active_session_id:'original-chat',sessions:[],
      current:{id:'original-chat',book_id:scenario.book_id,turns:[],goals:[]}}});
  });
  await page.locator('.agent-input textarea').fill(scenario.draft);
  await selectSource(page,'markdown');
  await page.getByRole('button',{name:'帮我弄懂这段',exact:true}).click();
  await page.getByRole('button',{name:'前往原题帮助',exact:true}).click();
  await expect.poll(()=>selected).toBe('original-chat');
  await expect(page.locator('.agent-input textarea')).toHaveValue(scenario.draft);
});
