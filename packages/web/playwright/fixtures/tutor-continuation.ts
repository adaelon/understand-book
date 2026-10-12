import { expect, type Page } from '@playwright/test';
import { readFileSync } from 'node:fs';

// The server and browser consume the same small material and excerpts.
const root = new URL('../../../../crates/server/src/tests/fixtures/tutor-continuation/', import.meta.url);
export const scenario = JSON.parse(readFileSync(new URL('scenario.json', root), 'utf8')) as {
  book_id: string; title: string; quote: string; answer: string; answer_excerpt: string; draft: string;
};
export const source = readFileSync(new URL('source.md', root), 'utf8').replace(/\r\n/g, '\n');
const paragraphs = source.trim().split('\n\n');

// Same minimal PDF construction as pdf-selection-actions.spec.ts; every paragraph
// is drawn as text so the real PDF.js text layer, selection and resolver run.
function pdfFixture() {
  const lines = [scenario.title, scenario.quote, 'Equal distances need not take equal times.',
    'The total time must be positive.', paragraphs[2]];
  const content = `BT /F1 12 Tf 50 740 Td ${lines.map((line, index) => `${index ? '0 -24 Td ' : ''}(${line}) Tj`).join('\n')} ET`;
  const objects = ['<< /Type /Catalog /Pages 2 0 R >>', '<< /Type /Pages /Kids [3 0 R] /Count 1 >>',
    '<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Resources << /Font << /F1 4 0 R >> >> /Contents 5 0 R >>',
    '<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>',
    `<< /Length ${Buffer.byteLength(content)} >>\nstream\n${content}\nendstream`];
  let body = '%PDF-1.4\n';
  const offsets = [0];
  for (const [i, object] of objects.entries()) { offsets.push(Buffer.byteLength(body)); body += `${i + 1} 0 obj\n${object}\nendobj\n`; }
  const xref = Buffer.byteLength(body);
  body += `xref\n0 6\n0000000000 65535 f \n${offsets.slice(1).map(offset => `${String(offset).padStart(10, '0')} 00000 n \n`).join('')}trailer\n<< /Size 6 /Root 1 0 R >>\nstartxref\n${xref}\n%%EOF\n`;
  return Buffer.from(body);
}

export async function setup(page: Page, surface: 'markdown' | 'pdf', ready: boolean, delivered = false, suggestions = false) {
  const requests: Record<string, unknown>[] = [], errors: string[] = [];
  page.on('pageerror', error => errors.push(error.message));
  const profile = { profile_id:'technical_learning', profile_version:'1', ui_slots:[], slots:[], layout_presets:[], allowed_layout_actions:[], agent_tools:[] };
  const reader = { book_id:scenario.book_id, revision:0, viewport:{ anchor_lid:'1.1', top_lid:'1.1', bottom_lid:'1.2', width:1, visible_lids:['1.1','1.2'] },
    profile, layout:{ rev:0, open_slots:[], panel_sizes:{}, slot_order:[], pinned_evidence:[] } };
  const outcome = { answer:scenario.answer, answer_view:{parts:[{kind:'markdown',text:scenario.answer}],sources:[]}, incomplete:false,
    effects:[], trace:suggestions ? [{tool:'tutor.step',input:{},output:{}}] : [], memory_updates:[], turns:1, tokens_spent:1, profile_usage:{snapshot_revision:0,injected_fact_ids:[],claimed_used_fact_ids:[],influences:[]} };
  const tutorState = {control:{enabled:suggestions,revision:0,current_tutor_session_id:null},sessions:{}};
  const displayed: Record<string, unknown>[] = [];
  const mutations: Record<string, unknown>[] = [];
  const turns: Record<string, unknown>[] = delivered ? [{ turn_id:'tc0-answer',user_turn_ordinal:1,user:'Explain average speed',status:'completed',outcome,
    question_source_label:'1.1',question_quote:{quote:scenario.quote,lid:'1.1',status:'resolved'},effect_labels:[] }] : [];
  const firstStart = source.indexOf(paragraphs[1]);
  await page.route('**/api/**', async route => {
    const url = new URL(route.request().url()), path = url.pathname;
    const body = route.request().method() === 'POST' ? route.request().postDataJSON() : {};
    let result: unknown;
    if (path === '/api/desktop/status') result = {desktop_host:false,reader_only:true,active_book:true};
    else if (path === '/api/book/build_workbench') result = {book_id:scenario.book_id,readiness:{route:'reader'},input:{manifest:null},source_review:{unresolved:[]}};
    else if (path === '/api/book/manifest') result = {tree:[
      {lid:'1',kind:'chapter',children:['1.1','1.2'],span:{start:0,end:source.length},title:scenario.title},
      {lid:'1.1',kind:'paragraph',children:[],span:{start:firstStart,end:firstStart+paragraphs[1].length}},
      {lid:'1.2',kind:'paragraph',children:[],span:{start:source.indexOf(paragraphs[2]),end:source.trimEnd().length}},
    ],stats_by_lid:{}};
    else if (path === '/api/book/asset_manifest') result = {images:[]};
    else if (path === '/api/book/source_fingerprint') result = {book_id:scenario.book_id,source_fingerprint:'tc0-source'};
    else if (path === '/api/book/source_manifest') {
      if (surface === 'markdown') return route.fulfill({status:404,json:{message:'Markdown fixture'}});
      result = {version:'source_manifest.v2',book_id:scenario.book_id,
        canonical_source:{kind:'reconciled_markdown',path:'source.txt',citation_anchor:'lid',sha256:'tc0-source'},
        original_pdf:{path:'tc0.pdf',sha256:'tc0-pdf',citation_anchor:false},
        capabilities:{view_pdf:{status:'available'},resolve_pdf_selection:{status:'available'},project_lid_to_pdf:{status:'degraded'},project_ranges_to_pdf:{status:'degraded'}}};
    } else if (path === '/api/book/pdf/original') return route.fulfill({contentType:'application/pdf',body:pdfFixture()});
    else if (path === '/api/book/pdf_source_map') result = {version:'pdf_source_map.v1',book_id:scenario.book_id,
      coordinate_system:{space:'pdf_user_space',origin:'bottom_left',unit:'pt',rotation_applied:false},
      pages:[{pageIndex:0,page_label:'1',width:612,height:792,rotate:0,view:[0,0,612,792]}],entries:[],excluded_regions:[],page_region_index:{},page_excluded_index:{},config_hash:'tc0'};
    else if (path === '/api/reader/pdf_selection.resolve') result = {status:'resolved',resolution_basis:'exact',
      ranges:[{lid:'1.1',range:{start:0,end:scenario.quote.length},source_span:{start:firstStart,end:firstStart+scenario.quote.length},quote_markdown:scenario.quote}],quote_markdown:scenario.quote};
    else if (path === '/api/reader/pdf_lid.project') result = {lid:body.lid,status:'unmapped',rects:[]};
    else if (path === '/api/book/text') { const lid=url.searchParams.get('lid'); result={lid,text:paragraphs[lid === '1' ? 0 : lid === '1.1' ? 1 : 2]}; }
    else if (path === '/api/profile/manifest') result = profile;
    else if (path === '/api/reader/state') result = reader;
    else if (path === '/api/memory/recall') result = [];
    else if (path === '/api/book/library') result = {root:'',books:[]};
    else if (path === '/api/profile/memory') result = {facts:[],pending_candidates:[],evidence:[],collection_rules:[],status:{pending_sensitive_confirmation:false},snapshot:{revision:0,facts:[]}};
    else if (path === '/api/profile/backfill') result = {jobs:[],sessions:[]};
    else if (path === '/api/build_intent/artifacts') result = {overlay:null};
    else if (path === '/api/build_intent/usage.event' || path === '/api/reader/viewport') result = {};
    else if (path === '/api/agent/history') result = {active_session_id:'tc0-chat',sessions:[],current:{id:'tc0-chat',book_id:scenario.book_id,turns,goals:[]}};
    else if (path === '/api/tutor/state') result = tutorState;
    else if (path === '/api/tutor/mutate') { mutations.push(body); tutorState.control.enabled = body.action.enabled; tutorState.control.revision++; result = tutorState; }
    else if (path === '/api/tutor/readiness') result = {status:ready?'ready':'preparing',source_id:scenario.book_id,source_revision:'tc0-source',limitations:[],
      reason:ready?'可用':'缺少 book_structure.json',blocked_stage:ready?null:'book_structure',teaching_assets:{status:'preparing',teaching_map_revision:null,limitations:[],reason:'无可选教具'}};
    else if (path === '/api/tutor/activities') result = {activities: suggestions && body.turn_id === 'tc0-answer' ? [{delivery_ref:'tc5-delivery',move:{move_id:'explain',prompt:'',actions:[]},status:'active',attempted:false,help_seen:[],
      continuation:{delivery_ref:'tc5-delivery',origin_turn_id:'tc0-answer',expected_chat_head:'tc0-answer',target_summary:'平均速度的分母',
        availability:requests.some(r=>r.teaching_choice) ? {status:'consumed',accepted_turn_id:'tc2-turn-1'} : turns.length > 1 ? {status:'stale'} : {status:'eligible'},
        continuation:{recommended_option_id:'compare',options:[{option_id:'compare',label:'比较两段等长行程',purpose:'分清总时间与段数',move_kind:'compare'},
          {option_id:'stop',label:'加入一次停顿',purpose:'看看时间增加的影响',move_kind:'observe'}]}}}] : []};
    else if (path === '/api/tutor/continuation-displayed') { displayed.push(body); result={event_id:'seen'}; }
    else if (path === '/api/tutor/display') result={event_id:'activity-seen'};
    else if (path === '/api/agent/reading-action-status') {
      const index=requests.findIndex(r=>r.client_request_id===body.client_request_id);
      result=index < 0 ? {accepted:false} : {book_id:scenario.book_id,session_id:'tc0-chat',turn_id:`tc2-turn-${index+1}`};
    }
    else if (path === '/api/agent/runs') {
      requests.push(body);
      const turn_id = `tc2-turn-${requests.length}`;
      turns.push({turn_id,user_turn_ordinal:turns.length+1,user:body.display_user ?? body.message,status:'completed',outcome,
        question_source_label:'1.1',question_quote:body.question_quote ?? null,effect_labels:[]});
      result = {book_id:scenario.book_id,session_id:'tc0-chat',turn_id};
    } else if (path.startsWith('/api/agent/runs/')) {
      const turn_id = path.split('/')[4], turn = turns.find(t => t.turn_id === turn_id);
      result = {descriptor:{book_id:scenario.book_id,session_id:'tc0-chat',turn_id},last_seq:1,
        execution_state:'completed',persistence_state:'saved',activities:[],final_view:turn,error:null};
      if (path.endsWith('/events')) return route.fulfill({contentType:'text/event-stream',body:`event: run.snapshot\ndata: ${JSON.stringify({turn_id,seq:1,elapsed_ms:0,type:'run.snapshot',payload:result})}\n\n`});
    }
    else return route.fulfill({status:404,json:{message:`Unused TC0 fixture route: ${path}`}});
    await route.fulfill({json:result});
  });
  await page.route('**/tc0-fixture', route => route.fulfill({contentType:'text/html',body:`<!doctype html><html><head><meta charset="utf-8"></head><body><div id="app"></div><script type="module">
    import {createApp} from '/node_modules/.vite/deps/vue.js';import App from '/src/App.vue';import '/src/style.css';createApp(App).mount('#app');
  </script></body></html>`}));
  await page.setViewportSize({width:1440,height:960});
  await page.goto('/tc0-fixture');
  await expect(page.locator('.app-loading')).toHaveCount(0);
  await expect(page.locator('.agent-input textarea')).toBeVisible();
  return {requests,errors,mutations,displayed,turns};
}

export async function selectSource(page: Page, surface: 'markdown' | 'pdf') {
  const node = surface === 'pdf' ? page.locator('.pdf-text-layer span').filter({hasText:scenario.quote}).first()
    : page.locator('.reader-pane [data-lid="1.1"]').first();
  await expect(node).toContainText(scenario.quote);
  await node.evaluate((element, quote) => {
    const walker = document.createTreeWalker(element, NodeFilter.SHOW_TEXT);
    let text: Node | null;
    while ((text=walker.nextNode())) {
      const start=text.textContent!.indexOf(quote);
      if(start<0) continue;
      const range=document.createRange();range.setStart(text,start);range.setEnd(text,start+quote.length);
      const selection=window.getSelection()!;selection.removeAllRanges();selection.addRange(range);
      element.dispatchEvent(new MouseEvent('mouseup',{bubbles:true}));document.dispatchEvent(new Event('selectionchange'));return;
    }
    throw new Error('TC0 quote absent from rendered text');
  },scenario.quote);
  await expect(page.locator('.hl-popover').getByRole('button',{name:'问 AI',exact:true})).toBeVisible();
}
