import { expect, test, type Page } from '@playwright/test';
import type { PresentationFeedback } from '../src/generated/PresentationFeedback';

const reference = { presentation_id: 'feedback-example', revision: 1 };
const outcome = { answer: '观察一次更新', answer_view: { parts: [{ kind: 'presentation', ...reference }], sources: [] },
  incomplete: false, warning: null, turns: 1, tokens_spent: 0, effects: [], trace: [], memory_updates: [],
  profile_usage: { snapshot_revision: 0, injected_fact_ids: [], influences: [], claimed_used_fact_ids: [] } };
type Surface = 'inline' | 'workspace' | 'linked';

async function openTools(page: Page) {
  if ((page.viewportSize()?.width ?? 1280) > 700) return;
  const button = page.getByRole('button', { name: '演示工具', exact: true });
  await expect(button).toBeVisible();
  if (await button.getAttribute('aria-expanded') === 'false') await button.click();
}

async function mountFeedback(page: Page, surface: Surface = 'inline') {
  const requests: { path: string; body: any }[] = [];
  const records = new Map<number, PresentationFeedback>();
  let failSet = false, failAttach = false;
  let stateRevision = 0;
  const workspace = { workspace_id: 'w', generation: 2, revision: 3, selected_chat: 's',
    published_book_ref: { book_id: 'b', publication_id: 'publication' }, reader: {} };
  await page.route('**/presentation-feedback-fixture', route => route.fulfill({ contentType: 'text/html', body: `<!doctype html>
    <html lang="zh-CN"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"></head>
    <body><div id="app"></div><script type="module">
    import {createApp,defineComponent,h,ref} from '/node_modules/.vite/deps/vue.js';
    import RightRail from '/src/components/RightRail.vue';
    import AgentPresentation from '/src/components/AgentPresentation.vue';
    import {installIdentity,installWorkspace} from '/src/network-context.ts';
    import {renderMarkdown} from '/src/md.ts';
    import '/src/style.css';
    ${surface !== 'inline' ? `installIdentity({user_id:'A',csrf_token:'csrf'});installWorkspace(${JSON.stringify(workspace)},'${surface === 'linked' ? 'child' : 'parent'}',${surface === 'linked'});` : ''}
    createApp(defineComponent({setup(){
      const expanded=ref(null),input=ref(''),sent=ref(''),revision=ref(1);
      window.changeFeedbackVersion=n=>revision.value=n;
      const follow=(message,receipt)=>{sent.value=JSON.stringify({message,receipt})};
      return()=>h('main',{style:${JSON.stringify(surface === 'linked' ? 'height:100dvh;display:flex;flex-direction:column' : 'max-width:760px;margin:16px auto;padding:8px')}},[
        h('output',{'data-testid':'feedback-follow-up',hidden:true},sent.value),
        ${surface === 'linked' ? `h(AgentPresentation,{sessionId:'s',turnId:'t',reference:{presentation_id:'feedback-example',revision:revision.value},onFollowUp:follow})` : `h(RightRail,{
          chat:[{turnId:'t',user:'观察一次更新',outcome:{...${JSON.stringify(outcome)},answer_view:{parts:[{kind:'presentation',presentation_id:'feedback-example',revision:revision.value}],sources:[]}},pending:false,
            questionAnchorLid:null,questionQuote:null,questionSelection:null,effectLabels:[]}],
          presentationWorkspace:expanded.value,'onUpdate:presentationWorkspace':v=>expanded.value=v,
          chatSessions:[],activeChatSessionId:'s',agentInput:input.value,'onUpdate:agentInput':v=>input.value=v,
          sending:false,selectedLid:null,selectedFormula:null,contextNotes:[],contextHighlights:[],renderMarkdown,
          effLabel:()=>'',effState:()=>undefined,isGoto:()=>false,showEffectPrimary:()=>false,showEffectSecondary:()=>false,
          effectPrimaryLabel:()=>'',effectSecondaryLabel:()=>'',gotoBack:()=>'',askDraft:null,onPresentationFollowUp:follow})`}
      ]);
    }})).mount('#app');
    </script></body></html>` }));
  await page.route('**/api/**', async route => {
    const path = new URL(route.request().url()).pathname;
    const body = route.request().method() === 'POST' ? route.request().postDataJSON() ?? {} : {};
    requests.push({ path, body });
    const network = path.startsWith('/api/workspaces/');
    if (path === '/api/workspaces/w') { await route.fulfill({ json: workspace }); return; }
    const operation = path.match(/presentation(?:\.feedback\.|\/feedback\/)(read|set|clear|attach_state)$/)?.[1];
    let result: unknown;
    if (operation) {
      const revision = body.reference.revision;
      const old = records.get(revision) ?? { session_id: 's', turn_id: 't', reference: body.reference,
        feedback_revision: 0, selection: null, saved_state: null, created_at: null, updated_at: null };
      if (operation === 'set' && failSet || operation === 'attach_state' && failAttach) {
        failSet = false; failAttach = false;
        await route.fulfill({ status: 500, json: { error_code: 'PRESENTATION_STORAGE_FAILED', message: '现场或反馈写入失败' } }); return;
      }
      if (operation === 'set' || operation === 'clear') {
        const selection = operation === 'clear' ? null : body.selection;
        if (JSON.stringify(selection) !== JSON.stringify(old.selection)) records.set(revision, { ...old, selection,
          feedback_revision: old.feedback_revision + 1, saved_state: null, created_at: old.created_at ?? '2026-10-16T03:00:00Z', updated_at: '2026-10-16T03:00:00Z' });
      }
      if (operation === 'attach_state') {
        expect(body.feedback_revision).toBe(old.feedback_revision);
        records.set(revision, { ...old, saved_state: body.saved_state });
      }
      result = records.get(revision) ?? old;
    } else if (/presentation[./]read$/.test(path)) result = {
      reference: body.reference, title: '一次更新', entrypoint: 'index.html', animation_assets: {}, assumptions: [], sources: [], initial_state: {},
      restored_state: null, restored_state_revision: null, readable_view: { parts: [], sources: [] },
      content_files: { 'index.html': '<!doctype html><html><body><h1>观察一次更新</h1><label>步数 <input id="count" type="range" min="0" max="10" value="2"></label><output id="result">2</output><script>document.querySelector("#count").oninput=e=>document.querySelector("#result").textContent=e.target.value;presentation.registerStateReader(()=>({values:{count:Number(document.querySelector("#count").value)},observed_result:document.querySelector("#result").textContent}));</script></body></html>' },
    };
    else if (/presentation(?:\.state\.save|\/save)$/.test(path)) result = { session_id: body.session_id, turn_id: body.turn_id, reference: body.reference, state_revision: ++stateRevision, saved_state_ref: `scene-${stateRevision}` };
    else if (path.endsWith('/tutor/continuation')) result = { status: 'unavailable' };
    else if (path.endsWith('/tutor/activities')) result = { activities: [] };
    else { await route.fulfill({ status: 404, json: { message: `Unexpected route: ${path}` } }); return; }
    await route.fulfill({ json: network ? { ...workspace, result } : result });
  });
  await page.goto('/presentation-feedback-fixture');
  await expect(page.frameLocator('.agent-presentation iframe').locator('#count')).toHaveValue('2');
  if (surface === 'workspace') await page.getByRole('button', { name: '展开', exact: true }).click();
  await openTools(page);
  await expect(page.getByRole('button', { name: '喜欢', exact: true })).toBeEnabled();
  return { requests, records, failSet: () => { failSet = true; }, failAttach: () => { failAttach = true; } };
}

for (const [surface, width, height] of [['inline', 960, 720], ['workspace', 1536, 800], ['linked', 390, 844]] as const) {
  test(`SE5 ${surface}: keyboard vote, optional details, exact scene, change and withdrawal`, async ({ page }, info) => {
    await page.setViewportSize({ width, height });
    const { requests, records } = await mountFeedback(page, surface);
    const frame = page.frameLocator('.agent-presentation iframe');
    await frame.locator('#count').press('End');
    await expect(frame.locator('#result')).toHaveText('10');
    const like = page.getByRole('button', { name: '喜欢', exact: true });
    await like.focus(); await page.keyboard.press('Enter');
    await expect(like).toHaveAttribute('aria-pressed', 'true');
    await expect(page.getByText('已附上反馈时的现场', { exact: false })).toBeVisible();
    expect(records.get(1)?.selection).toEqual({ choice: 'like', reasons: [], comment: '' });
    await page.getByRole('button', { name: '补充原因', exact: true }).click();
    await page.getByRole('checkbox', { name: '交互有用' }).check();
    await page.getByRole('textbox', { name: '补充（选填）' }).fill('拖动后这一步看清楚了');
    await page.getByRole('button', { name: '保存补充', exact: true }).click();
    await expect.poll(() => records.get(1)?.selection?.comment).toBe('拖动后这一步看清楚了');
    await expect(page.getByText('已附上反馈时的现场', { exact: false })).toBeVisible();
    await page.screenshot({ path: info.outputPath(`feedback-${surface}.png`) });
    await page.getByRole('button', { name: '带入追问', exact: true }).click();
    await expect(page.getByTestId('feedback-follow-up')).toBeEmpty();
    await expect(frame.locator('#count')).toHaveValue('10');
    if (surface === 'inline') {
      const originalReceipt = records.get(1)!.saved_state;
      const input = page.getByRole('textbox', { name: '针对当前演示追问' });
      const draft = await input.inputValue();
      await frame.locator('#count').press('Home');
      await input.fill(`${draft} 请继续解释`);
      await page.getByRole('button', { name: '发送追问', exact: true }).click();
      await expect(page.getByTestId('feedback-follow-up')).toContainText('请继续解释');
      expect(JSON.parse(await page.getByTestId('feedback-follow-up').textContent() ?? '{}').receipt).toEqual(originalReceipt);
    }
    await expect(page.locator('.agent-presentation iframe')).toHaveAttribute('data-load-count', '1');
    await openTools(page);
    const dislike = page.getByRole('button', { name: '不喜欢', exact: true });
    await dislike.focus(); await page.keyboard.press('Space');
    await expect(dislike).toHaveAttribute('aria-pressed', 'true');
    await dislike.press('Enter');
    await expect(dislike).toHaveAttribute('aria-pressed', 'false');
    await expect(page.getByText('反馈已撤回', { exact: true })).toBeVisible();
    expect(requests.some(r => /agent\/(?:runs|chat)$/.test(r.path))).toBe(false);
    if (surface !== 'inline') for (const request of requests.filter(r => r.path.includes('/feedback/'))) {
      expect(request.body).toMatchObject({ attachment_id: surface === 'linked' ? 'child' : 'parent', generation: 2, expected_revision: 3 });
    }
  });
}

test('SE5 mobile failure retry, saved feedback survives reopening, versions remain separate', async ({ page }, info) => {
  await page.setViewportSize({ width: 320, height: 420 });
  const fixture = await mountFeedback(page);
  fixture.failSet();
  const like = page.getByRole('button', { name: '喜欢', exact: true });
  await like.click();
  await expect(page.getByRole('alert')).toContainText('反馈未保存');
  await expect(like).toHaveAttribute('aria-pressed', 'false');
  await page.getByRole('button', { name: '重试反馈' }).click();
  await expect(like).toHaveAttribute('aria-pressed', 'true');
  await expect(page.getByText('已附上反馈时的现场', { exact: false })).toBeVisible();
  const sets = fixture.requests.filter(r => r.path.endsWith('feedback.set'));
  expect(sets[1].body).toEqual(sets[0].body);
  await page.reload();
  await openTools(page);
  await expect(like).toHaveAttribute('aria-pressed', 'true');
  await page.evaluate(() => (window as any).changeFeedbackVersion(2));
  await openTools(page);
  await expect(like).toHaveAttribute('aria-pressed', 'false');
  await like.click();
  await expect.poll(() => fixture.records.get(2)?.selection?.choice).toBe('like');
  await page.getByRole('button', { name: '补充原因', exact: true }).click();
  const comment = page.getByRole('textbox', { name: '补充（选填）' });
  await comment.fill('保留手机键盘输入的草稿');
  await page.setViewportSize({ width: 320, height: 320 });
  await expect(page.getByRole('button', { name: '演示工具', exact: true })).toHaveAttribute('aria-expanded', 'true');
  await expect(comment).toHaveValue('保留手机键盘输入的草稿');
  await page.getByRole('button', { name: '保存补充', exact: true }).click();
  await expect.poll(() => fixture.records.get(2)?.selection?.comment).toBe('保留手机键盘输入的草稿');
  await page.screenshot({ path: info.outputPath('feedback-mobile.png') });
  expect(fixture.records.get(1)?.feedback_revision).toBe(1);
  expect(await page.evaluate(() => document.documentElement.scrollWidth)).toBeLessThanOrEqual(320);
});

test('SE5 short linked window retries the original scene and sends feedback only on explicit follow-up', async ({ page }, info) => {
  await page.setViewportSize({ width: 640, height: 240 });
  const fixture = await mountFeedback(page, 'linked');
  fixture.failAttach();
  await page.getByRole('button', { name: '不喜欢', exact: true }).click();
  await expect(page.getByRole('button', { name: '重试附加原现场' })).toBeVisible();
  await page.frameLocator('iframe').locator('#count').press('End');
  await page.getByRole('button', { name: '重试附加原现场' }).click();
  await expect(page.getByText('已附上反馈时的现场', { exact: false })).toBeVisible();
  const attaches = fixture.requests.filter(r => r.path.endsWith('feedback/attach_state'));
  expect(attaches[1].body.saved_state).toEqual(attaches[0].body.saved_state);
  await page.getByRole('button', { name: '补充原因', exact: true }).click();
  await page.getByRole('button', { name: '带入追问', exact: true }).click();
  await expect(page.getByTestId('feedback-follow-up')).toBeEmpty();
  await page.getByRole('button', { name: '发送追问', exact: true }).click();
  await expect(page.getByTestId('feedback-follow-up')).toContainText('我不喜欢');
  const sent = JSON.parse(await page.getByTestId('feedback-follow-up').innerText());
  expect(sent.receipt).toEqual(attaches[0].body.saved_state);
  await page.screenshot({ path: info.outputPath('feedback-short.png') });
});

test('SE5 ordinary follow-up still captures the current scene and leaves a failed question retryable', async ({ page }) => {
  const fixture = await mountFeedback(page);
  const frame = page.frameLocator('iframe');
  await frame.locator('#count').press('End');
  await expect(page.getByText('演示状态已保存', { exact: true })).toBeVisible();
  await page.getByRole('textbox', { name: '针对当前演示追问' }).fill('为什么是 10？');
  await page.route('**/api/agent/presentation.state.save', route => route.fulfill({ status: 500,
    json: { error_code: 'PRESENTATION_STORAGE_FAILED', message: '现场写入失败' } }), { times: 1 });
  await page.getByRole('button', { name: '发送追问', exact: true }).click();
  await expect(page.getByText(/追问未发送：.*现场写入失败/)).toBeVisible();
  await expect(page.getByTestId('feedback-follow-up')).toBeEmpty();
  await expect(page.getByRole('textbox', { name: '针对当前演示追问' })).toHaveValue('为什么是 10？');
  await page.getByRole('button', { name: '发送追问', exact: true }).click();
  await expect(page.getByTestId('feedback-follow-up')).toContainText('为什么是 10？');
  const saved = fixture.requests.filter(r => r.path.endsWith('presentation.state.save')).at(-1)!;
  expect(saved.body.state.values.page.count).toBe(10);
  expect(fixture.records.size).toBe(0);
});
