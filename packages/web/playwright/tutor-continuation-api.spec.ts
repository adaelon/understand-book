import { expect, test } from '@playwright/test';
import { setup, scenario } from './fixtures/tutor-continuation';
import { writeFileSync } from 'node:fs';
test.skip(!process.env.TC5_BROWSER_API, 'Start continuation_tc5_browser_api first');
test.use({ video: 'on' });
test.afterEach(async ({page}) => { await page.goto('about:blank'); await page.unrouteAll({behavior:'ignoreErrors'}); });
for (const presentation of [false,true]) test(`tc5_actual_rust_api_${presentation ? 'presentation' : 'ordinary'}`, async ({page,request}) => {
  test.setTimeout(60000);
  await request.get(`http://127.0.0.1:4196/reset${presentation ? '?presentation=1' : ''}`);
  await setup(page,'markdown',true);
  // Rendering uses the shared real source fixture. All interaction endpoints
  // below go through the Rust router, RunCoordinator and real SQLite/JSONL.
  await page.route('**/api/**', async route => {
    const url=new URL(route.request().url()),path=url.pathname.slice(4)+url.search;
    if (!path.startsWith('/agent/') && !path.startsWith('/tutor/')) return route.fallback();
    const response=await request.fetch(`http://127.0.0.1:4196${path}`,{method:route.request().method(),data:route.request().postData() ?? undefined});
    const value=await response.json();
    if(path.includes('/events')) return route.fulfill({contentType:'text/event-stream',body:`event: run.snapshot\ndata: ${JSON.stringify({turn_id:value.descriptor?.turn_id,seq:value.last_seq,elapsed_ms:0,type:'run.snapshot',payload:value})}\n\n`});
    return route.fulfill({status:response.status(),json:value});
  });
  await page.reload();
  await expect(page.locator('.agent-input textarea')).toBeVisible();
  await page.locator('.agent-input textarea').fill(scenario.draft);
  if(presentation) {
    await expect(page.locator('.agent-presentation iframe')).toBeVisible();
    await page.locator('.agent-presentation').getByRole('button',{name:'展开',exact:true}).click();
    await expect(page.locator('.right-rail')).toHaveClass(/presentation-workspace/);
  }
  const button=page.getByRole('button',{name:'Compare two equal-distance journeys',exact:true});
  await expect(button).toHaveCount(1); await expect(button).toBeEnabled();
  await button.scrollIntoViewIfNeeded(); await button.focus(); await expect(button).toBeFocused();
  await page.screenshot({path:`../../docs/performance/tutor-continuation-tc5/api-${presentation ? 'presentation' : 'ordinary'}.png`});
  await page.keyboard.press('Enter');
  await expect.poll(async()=>{const r=await request.get('http://127.0.0.1:4196/evidence');return (await r.json()).history.current.turns[1]?.status;}).toBe('completed');
  const evidence=await (await request.get('http://127.0.0.1:4196/evidence')).json();
  expect(evidence.history.current.turns).toHaveLength(2);
  const turn=evidence.history.current.turns[1];
  expect(turn.domain.interaction.action.origin).toBe('ui_choice');
  expect(turn.domain.interaction.action.teaching_choice.option_id).toBe('compare');
  expect(evidence.events.filter((e:any)=>e.kind==='continuation_chosen')).toHaveLength(1);
  expect(evidence.provider_calls).toBe(3);
  if(presentation) expect(turn.domain.interaction.presentation_follow_up.reference.revision).toBe(1);
  await expect(page.locator('.agent-input textarea')).toHaveValue(scenario.draft);
  await expect(page.getByText('接续的原回答',{exact:true})).toHaveCount(1);
});

test('tc5_actual_multi_user_browser_request_preserves_choice',async({page,request})=>{
  test.setTimeout(60000);
  await setup(page,'markdown',true);
  const binding=await (await request.get('http://127.0.0.1:4196/network-reset')).json();
  await page.route('**/tc5-network',route=>route.fulfill({contentType:'text/html',body:`<!doctype html><html><head><meta charset="utf-8"></head><body><div id="app"></div><script type="module">
    import {createApp} from '/node_modules/.vite/deps/vue.js';import App from '/src/App.vue';import '/src/style.css';
    import {installIdentity,installWorkspace} from '/src/network-context.ts';
    installIdentity({user_id:'A',csrf_token:'fixture'});installWorkspace(${JSON.stringify(binding)},'tc0-page');createApp(App).mount('#app');
  </script></body></html>`}));
  const submitted: any[]=[];
  await page.route('**/api/**',async route=>{
    const url=new URL(route.request().url());
    if(url.pathname==='/api/desktop/status') return route.fulfill({json:{desktop_host:false,reader_only:true,active_book:true}});
    const event=url.pathname.match(/^(.*\/agent\/runs\/[^/]+)\/events$/);
    const path=event ? event[1] : url.pathname+url.search;
    const response=await request.fetch(`http://127.0.0.1:4196/network${path}`,{method:route.request().method(),data:route.request().postData() ?? undefined});
    const value=await response.json();
    if(route.request().method()==='POST' && path.endsWith('/agent/runs')) submitted.push({request:route.request().postDataJSON(),response:value});
    if(event) {
      const snapshot=value.snapshot ?? {descriptor:value,last_seq:1,execution_state:value.dispatch_state==='completed'?'completed':'running',persistence_state:value.turn?'saved':'pending',activities:[],final_view:value.turn,error:null};
      return route.fulfill({contentType:'text/event-stream',body:`event: run.snapshot\ndata: ${JSON.stringify({turn_id:value.turn_id,seq:snapshot.last_seq,elapsed_ms:0,type:'run.snapshot',payload:snapshot})}\n\n`});
    }
    return route.fulfill({status:response.status(),json:value});
  });
  await page.goto('/tc5-network');
  await expect(page.locator('.agent-input textarea')).toBeVisible();
  await page.locator('.agent-input textarea').fill(scenario.draft);
  const choice=page.getByRole('button',{name:'Compare two equal-distance journeys',exact:true});
  await expect(choice).toBeVisible(); await choice.click();
  await expect.poll(()=>submitted.length).toBe(1);
  expect(submitted[0].request).toMatchObject({message:'',session_id:binding.selected_chat,teaching_choice:{option_id:'compare'}});
  const history=await (await request.post(`http://127.0.0.1:4196/network/api/workspaces/${binding.workspace_id}/chat/history`,{data:{attachment_id:'tc0-page',generation:binding.generation,expected_revision:binding.revision}})).json();
  expect(history.result.current.turns).toHaveLength(2);
  const turn=history.result.current.turns[1];
  expect(turn.status).toBe('completed'); expect(turn.domain.interaction.action.teaching_choice.option_id).toBe('compare');
  expect(submitted[0].request.expected_chat_head).toBe(history.result.current.turns[0].turn_id);
  await expect(page.locator('.agent-input textarea')).toHaveValue(scenario.draft);
  await page.screenshot({path:'../../docs/performance/tutor-continuation-tc5/api-network.png'});
  writeFileSync('../../docs/performance/tutor-continuation-tc5/api/network.json',JSON.stringify({submitted,history},null,2));
});
