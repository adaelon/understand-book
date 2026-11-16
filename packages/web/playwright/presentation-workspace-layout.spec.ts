import { expect, test, type Page } from '@playwright/test';
import { writeFile } from 'node:fs/promises';

const reference = { presentation_id: 'layout-example', revision: 1 };
const prompt = '这段教学说明依次解释计算图、执行步骤、上下文状态、权重集合和形状汇总。观察每个参数如何影响结果，再写下你的理解。';
const teachingPrompt = Array.from({ length: 18 }, () => prompt).join('\n\n');
const learningTitle = '我想知道这一章整体是怎样的，各部分又是怎样联系起来的。我刚开始学习，希望先弄清楚从哪里看起。';
const listeningTitle = '第一乐章：一支有分量的队伍，一直在往前走';
const listeningGoal = '告诉我第一乐章整体是怎样的，结构、情绪和动机是怎样的。我完全不懂音乐术语，希望知道应该听什么。';
const listeningAnswer = `先听开头的半分钟。这一次只留意两件事：声音有没有重复，音高有没有换位置。

### 第一遍，只听拍子

从 10.09 秒听到 39.76 秒。先别看谱，听那一组音是不是隔一会儿就回来。跟着轻轻点两下，感觉这支队伍怎样往前走。

### 第二遍，只听方向

再听同一小段。这次把注意力从拍子移到高低上。前面像是同一组音反复敲，后面像是每次换一个更低的音。

### 把听到的放回整段

左边的四个色块是整段录音里的四个部分。最后一段约 235 秒，和第一段的长度接近。可以先记住这个比例，听完开头再回来看。`;
const listeningScene = `<!doctype html><html lang="zh-CN"><meta charset="utf-8"><style>
  *{box-sizing:border-box} body{margin:0;padding:24px;background:#faf9f5;color:#28241e;font:17px/1.8 system-ui}
  section{padding:20px 24px;margin-bottom:16px;border:1px solid #e5ded2;border-radius:14px;background:#fffcf6}
  h2,h3{margin:0 0 8px} p{margin:6px 0 12px} .bars{display:flex;gap:4px}.bars span{padding:12px 2px;text-align:center;background:#eeeae0;border-radius:8px;font-size:14px}.bars span:last-child{background:#90400e;color:white}small{color:#746c60}
  </style><section><h2>同一组音反复敲</h2><p>一小节两下，音高始终不变。它给出的是拍子和重量，还没给出方向。</p>
  <h3>每次换一个更低的音</h3><p>右边换着音往下走。两种写法一起出现，听起来既稳，又一直在推。</p></section>
  <section><h2>四段结构：最后一段几乎和第一轮一样长</h2><p>横条按各段在录音里所占的时间排列。</p>
  <div class="bars">${[['呈示部',29.3],['展开部',24.7],['再现部',22.7],['尾声',23.3]].map(([label,pct]) => `<span style="flex:${pct}">${label}<br>${pct}%</span>`).join('')}</div>
  <p><small>全长 1009.8 秒</small></p><h3>第 4 段 · 尾声</h3><p>12:56.2 – 16:51.0　约 235 秒，占全曲 23.3%。</p></section></html>`;
const outcome = {
  answer: prompt, answer_view: { parts: [{ kind: 'markdown', text: prompt }, { kind: 'presentation', ...reference }], sources: [] },
  incomplete: false, warning: null, turns: 1, tokens_spent: 0, effects: [], trace: [], memory_updates: [],
  profile_usage: { snapshot_revision: 0, injected_fact_ids: [], influences: [], claimed_used_fact_ids: [] },
};

async function mountWorkspace(page: Page, listening = false, sidebar = false, question?: string) {
  page.on('pageerror', error => { throw error; });
  const title = listening ? listeningGoal : learningTitle;
  await page.route('**/presentation-layout-fixture', route => route.fulfill({ contentType: 'text/html', body: `<!doctype html>
    <html lang="zh-CN"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><style>
    .sidebar-fixture{display:grid;grid-template-columns:minmax(0,1fr) clamp(320px,25vw,480px);height:calc(100dvh - 48px);margin-top:48px}
    .sidebar-fixture > iframe{width:100%;height:100%;border:0}main.sidebar-fixture .right-rail{grid-area:auto;grid-column:2;grid-row:1;height:100%}
    @media(max-width:1023px){.sidebar-fixture{grid-template-columns:1fr}.sidebar-fixture > iframe{display:none}main.sidebar-fixture .right-rail{grid-column:1}}
    </style></head>
    <body><div id="app"></div><script type="module">
    import {createApp,defineComponent,h,ref} from '/node_modules/.vite/deps/vue.js';
    import RightRail from '/src/components/RightRail.vue';
    import ConversationLearningGoal from '/src/components/ConversationLearningGoal.vue';
    import TutorControl from '/src/components/TutorControl.vue';
    import {renderMarkdown} from '/src/md.ts';
    import '/src/style.css';
    createApp(defineComponent({setup(){
      const workspace=ref(null),fullscreen=ref(false),input=ref(''),enabled=ref(true),sending=ref(false);
      const goal={id:'goal',status:'open',last_stop_reason:'AGENT_RUN_CANCELLED',interpretation:'请结合演示再解释一次',requirements:[],result_refs:[],working:{items:[]}};
      const chat=[{turnId:'t',user:'继续任务',outcome:${JSON.stringify(question ? { ...outcome, answer: prompt + '\n\n' + question,
        answer_view: { parts: [...outcome.answer_view.parts, { kind: 'socratic_question', text: question }], sources: [] } } : outcome)},hasTeaching:true,pending:false,
        questionAnchorLid:null,questionQuote:null,questionSelection:null,effectLabels:[]}];
      if (${listening}) chat.push({...chat[0],turnId:'reply',user:'我应该去听什么呢？',hasTeaching:false,
        outcome:{...chat[0].outcome,answer:${JSON.stringify(listeningAnswer)},answer_view:{parts:[{kind:'markdown',text:${JSON.stringify(listeningAnswer)}}],sources:[]}}});
      if (${sidebar}) chat.shift();
      return()=>h('main',${sidebar ? "{class:'sidebar-fixture video-workspace'}" : "{style:'max-width:760px;margin:24px auto;padding:8px'}"},[
        ${sidebar ? `h('header',{style:'position:fixed;inset:0 0 auto;height:48px;padding:8px 20px;border-bottom:1px solid #e5ded2;background:#faf9f5'},'Understand Book　　贝多芬第九交响曲 · 第一乐章'), h('iframe',{srcdoc:${JSON.stringify(listeningScene)}}),` : ''}
        h(RightRail,{chat,chatGoals:[goal],musicMode:true,learningTitle:${JSON.stringify(title)},
          presentationWorkspace:workspace.value,'onUpdate:presentationWorkspace':v=>workspace.value=v,
          fullscreen:fullscreen.value,onToggleFullscreen:()=>fullscreen.value=!fullscreen.value,
          chatSessions:[],activeChatSessionId:'s',agentInput:input.value,'onUpdate:agentInput':v=>input.value=v,
          onPresentationFollowUp:(message,receipt)=>document.documentElement.dataset.sceneReply=JSON.stringify({message,receipt}),
          sending:sending.value,canStop:sending.value,onSendAgent:()=>sending.value=true,onStopAgent:()=>sending.value=false,
          selectedLid:null,selectedFormula:null,contextNotes:[],contextHighlights:[],renderMarkdown,
          effLabel:()=>'',effState:()=>undefined,isGoto:()=>false,showEffectPrimary:()=>false,showEffectSecondary:()=>false,
          effectPrimaryLabel:()=>'',effectSecondaryLabel:()=>'',gotoBack:()=>'',askDraft:null},
          {'learning-goal':()=>h(ConversationLearningGoal,{session:{user_intent:${JSON.stringify(title)},status:'active'},enabled:enabled.value,busy:false,available:true,ready:true,error:''}),
           'tutor-control':()=>[h(TutorControl,{enabled:enabled.value,label:'围绕你的目标逐步学',busy:false,unavailable:false,error:'',onToggle:()=>enabled.value=!enabled.value}),
             ...['阅读时提议练习','整理我的学习脉络'].map(label=>h('label',{class:'practice-margin-setting'},[h('input',{type:'checkbox'}),label]))],
           'learning-actions':()=>h('button',{},'我的理解'),
           'learning-details':()=>h('button',{},'学习详情')})]);
    }})).mount('#app');
    </script></body></html>` }));
  await page.route('**/api/**', async route => {
    const path = new URL(route.request().url()).pathname;
    const body = route.request().postDataJSON() ?? {};
    let result: unknown = {};
    if (path.endsWith('/attachments')) result = { items: [] };
    if (path.endsWith('/agent/presentation.read')) result = {
      reference, title: listening ? listeningTitle : '第 6 章 全书结构与语义候选的两条命运', entrypoint: 'index.html',
      content_files: { 'index.html': listening ? listeningScene : `<!doctype html><html><body><h1>模型架构演示</h1><label>参数 <input id="parameter" value="5"></label>${Array.from({ length: 10 }, () => `<p>${prompt}</p>`).join('')}<button id="last-control">最后一个操作</button></body></html>` },
      sources: [], action_contract: [], initial_state: {}, restored_state: null, restored_state_revision: null,
      readable_view: { parts: [], sources: [] }, animation_assets: {}, assumptions: [],
    };
    if (path.endsWith('/tutor/activities')) result = { activities: body.reference && !listening ? [{ delivery_ref: 'delivery',
      move: question ? { move_id: 'explain', prompt: teachingPrompt, actions: [], presentation: reference,
        response_expectation: { public_prompt: question, block_refs: ['parameter'], socratic: true } }
        : { move_id: 'explain', prompt: teachingPrompt, actions: ['submit'] }, status: 'active', attempted: false, help_seen: [],
    }] : [] };
    if (path.endsWith('/agent/presentation.state.save')) result = { session_id: 's', turn_id: 't', reference, state_revision: 1, saved_state_ref: 'scene' };
    if (path.endsWith('/tutor/display') && !question) { await route.abort('failed'); return; }
    await route.fulfill({ json: result });
  });
  await page.goto('/presentation-layout-fixture');
  if (sidebar) {
    await expect(page.locator('[data-turn-id="reply"] .answer-markdown')).toBeVisible();
    return;
  }
  if (listening) {
    await expect(page.frameLocator('.agent-presentation iframe').getByText('同一组音反复敲', { exact: true })).toBeVisible();
    return;
  }
  await expect(page.frameLocator('.agent-presentation iframe').locator('#parameter')).toHaveValue('5');
  await expect(page.locator('.agent-presentation .tutor-activities article')).toHaveCount(1);
}

test('Socratic dialogue continues from the live presentation without regenerating it', async ({ page }, info) => {
  await page.setViewportSize({ width: 1366, height: 900 });
  const question = '把参数从 5 调到 7，你觉得是哪一步会先发生变化？';
  await mountWorkspace(page, false, false, question);
  await page.getByRole('button', { name: '展开', exact: true }).click();
  const frame = page.frameLocator('.agent-presentation iframe');
  await frame.locator('#parameter').fill('7');
  await expect(page.getByText(question, { exact: true })).toHaveCount(1);
  await expect(page.getByRole('textbox', { name: '说说你的理解' })).toHaveCount(0);
  await expect(page.locator('.socratic-question')).toContainText(question);
  await page.locator('.socratic-question').scrollIntoViewIfNeeded();
  await page.screenshot({ path: info.outputPath('socratic-presentation.png') });
  const words = '我猜输入张量先变，因为我改的是它的维度。';
  await page.locator('.agent-input textarea').fill(words);
  await page.locator('.agent-input .send-agent').click();
  await expect.poll(() => page.locator('html').getAttribute('data-scene-reply')).toContain(words);
  const reply = JSON.parse((await page.locator('html').getAttribute('data-scene-reply'))!);
  expect(reply.receipt).toMatchObject({ session_id: 's', turn_id: 't', reference, saved_state_ref: 'scene' });
  await expect(frame.locator('#parameter')).toHaveValue('7');
  await expect(page.locator('.agent-presentation iframe')).toHaveAttribute('data-load-count', '1');
});

test('ordinary conversation keeps its long reply readable beside the score and preserves its draft', async ({ page }, info) => {
  await page.setViewportSize({ width: 1524, height: 694 });
  await mountWorkspace(page, true, true);
  const input = page.locator('.agent-input textarea');
  const sizes = [];
  for (const viewport of [{ width: 1524, height: 694 }, { width: 1900, height: 883 }, { width: 390, height: 844 }]) {
    await page.setViewportSize(viewport);
    await page.locator('[data-turn-id="reply"] .answer-markdown p').first().evaluate(p => p.scrollIntoView({ block: 'start' }));
    const panel = (await page.locator('.right-rail').boundingBox())!;
    const transcript = (await page.locator('.transcript').boundingBox())!;
    const composer = (await page.locator('.agent-input').boundingBox())!;
    sizes.push({ ...viewport, panel: panel.height, transcript: transcript.height, composer: composer.height });
    await page.screenshot({ path: info.outputPath(`ordinary-${viewport.width}.png`) });
    expect(transcript.height).toBeGreaterThan(panel.height * .65);
    expect(composer.height).toBeLessThanOrEqual(120);
  }
  await writeFile(info.outputPath('ordinary-reading-space.json'), JSON.stringify(sizes, null, 2));
  await page.setViewportSize({ width: 1524, height: 694 });
  await input.fill('那这几个音要让我感受什么呢，我');
  await page.getByRole('button', { name: '学习设置', exact: true }).click();
  const settings = page.getByRole('dialog', { name: '学习设置', exact: true });
  await expect(settings).toContainText(listeningGoal);
  await settings.getByRole('switch', { name: '连续教学' }).click();
  await expect(settings.getByRole('switch', { name: '连续教学' })).toHaveAttribute('aria-checked', 'false');
  await settings.getByRole('checkbox', { name: '阅读时提议练习' }).check();
  await page.keyboard.press('Escape');
  await expect(page.getByRole('button', { name: '添加图片', exact: true })).toBeVisible();
  await page.getByRole('switch', { name: '演示页', exact: true }).click();
  await page.keyboard.press('Escape');
  await expect(page.getByRole('switch', { name: '演示页', exact: true })).toBeVisible();
  await expect(input).toHaveValue('那这几个音要让我感受什么呢，我');
  await page.getByRole('button', { name: '问答全屏', exact: true }).click();
  await page.getByRole('button', { name: '退出问答全屏', exact: true }).click();
  await expect(input).toHaveValue('那这几个音要让我感受什么呢，我');
  await page.getByRole('button', { name: '发送', exact: true }).click();
  await page.locator('.agent-input').getByRole('button', { name: '停止', exact: true }).click();
  await expect(page.getByRole('button', { name: '发送', exact: true })).toBeEnabled();
  await expect(page.locator('.agent-input .learning-dock')).toHaveCount(0);
});

test('expanded discussion keeps a long answer readable with learning controls and a stopped task', async ({ page }, info) => {
  await page.setViewportSize({ width: 1524, height: 694 });
  await mountWorkspace(page, true);
  await page.getByRole('button', { name: '展开', exact: true }).click();
  const transcript = page.locator('.transcript');
  const input = page.locator('.agent-input textarea');
  const layout = async () => ({
    toolbar: (await page.locator('.workspace-tools').boundingBox())!.height,
    transcript: (await transcript.boundingBox())!.height,
    composer: (await page.locator('.agent-input').boundingBox())!.height,
  });
  const sizes = [];
  for (const viewport of [{ width: 1524, height: 694 }, { width: 1905, height: 867 }]) {
    await page.setViewportSize(viewport);
    await page.locator('[data-turn-id="reply"] .answer-markdown p').first().evaluate(p => p.scrollIntoView({ block: 'start' }));
    const size = await layout();
    sizes.push({ ...viewport, ...size });
    await page.screenshot({ path: info.outputPath(`discussion-${viewport.width}.png`) });
    expect(size.transcript).toBeGreaterThan(viewport.height * .65);
    expect(size.toolbar).toBeLessThanOrEqual(52);
  }
  await writeFile(info.outputPath('reading-space.json'), JSON.stringify(sizes, null, 2));
  await input.fill('请保留这句追问');
  await page.getByRole('button', { name: '学习设置', exact: true }).click();
  const settings = page.getByRole('dialog', { name: '学习设置', exact: true });
  await expect(settings).toContainText(listeningGoal);
  await settings.getByRole('button', { name: '调整目标' }).click();
  await expect(settings.getByRole('textbox', { name: '学习目标' })).toHaveValue(listeningGoal);
  await page.keyboard.press('Escape');
  await expect(settings).toBeVisible();
  await settings.getByRole('switch', { name: '连续教学' }).click();
  await expect(settings.getByRole('switch', { name: '连续教学' })).toHaveAttribute('aria-checked', 'false');
  await settings.getByRole('checkbox', { name: '阅读时提议练习' }).check();
  await page.keyboard.press('Escape');
  await expect(settings).toBeHidden();
  await expect(page.getByRole('button', { name: '学习设置', exact: true })).toBeFocused();
  await page.getByRole('button', { name: /查看任务/ }).click();
  await expect(page.getByRole('dialog', { name: '当前任务详情' })).toContainText('请结合演示再解释一次');
  await page.getByRole('button', { name: '关闭任务详情' }).click();
  const binding = page.getByRole('button', { name: '附上当前演示状态', exact: true });
  await expect(binding).toHaveAttribute('aria-pressed', 'true');
  await binding.click();
  await expect(binding).toHaveAttribute('aria-pressed', 'false');
  await expect(input).toHaveValue('请保留这句追问');
  await expect(page.getByRole('button', { name: '添加图片' })).toBeVisible();
  await page.getByRole('switch', { name: '演示页' }).click();
  await page.keyboard.press('Escape');
  await expect(page.getByRole('switch', { name: '演示页' })).toBeVisible();
  await expect(input).toHaveValue('请保留这句追问');
  await expect(page.locator('.agent-presentation iframe')).toHaveAttribute('data-load-count', '1');
  await page.getByRole('button', { name: '发送', exact: true }).click();
  await page.locator('.agent-input').getByRole('button', { name: '停止', exact: true }).click();
  await expect(page.getByRole('button', { name: '发送', exact: true })).toBeEnabled();
});

test('long teaching and a failed display receipt leave the expanded page its full height and preserve drafts', async ({ page }, info) => {
  await page.setViewportSize({ width: 1536, height: 700 });
  await mountWorkspace(page);
  const frame = page.frameLocator('.agent-presentation iframe');
  await frame.locator('#parameter').fill('7');
  await page.getByRole('textbox', { name: '活动回答', exact: true }).fill('先比较模型保存的参数和本步激活的参数');
  await page.getByRole('button', { name: '展开', exact: true }).click();
  const presentation = page.locator('.agent-presentation.expanded');
  const bounds = (await presentation.boundingBox())!;
  expect((await presentation.locator('iframe').boundingBox())!.height).toBeGreaterThan(bounds.height * .9);
  await expect(presentation.locator('.tutor-activities')).toHaveCount(0);
  const teaching = page.locator('.workspace-teaching .tutor-activities');
  await expect(teaching.locator('[role=alert]')).toContainText('Failed to fetch');
  await expect(teaching.getByRole('textbox', { name: '活动回答' })).toHaveValue('先比较模型保存的参数和本步激活的参数');
  await teaching.getByRole('textbox', { name: '活动回答' }).fill('在演示中继续补充的草稿');
  await teaching.getByRole('button', { name: '提交回答', exact: true }).scrollIntoViewIfNeeded();
  const action = (await teaching.getByRole('button', { name: '提交回答', exact: true }).boundingBox())!;
  expect(action.x).toBeGreaterThanOrEqual(bounds.x + bounds.width);
  await frame.locator('#last-control').click();
  await page.screenshot({ path: info.outputPath('expanded-long-teaching.png') });
  await page.getByRole('button', { name: '收起', exact: true }).click();
  await expect(page.locator('.agent-presentation').getByRole('textbox', { name: '活动回答' })).toHaveValue('在演示中继续补充的草稿');
  await page.getByRole('button', { name: '展开', exact: true }).click();
  await expect(frame.locator('#parameter')).toHaveValue('7');
  await expect(page.locator('.agent-presentation iframe')).toHaveAttribute('data-load-count', '1');
  await expect(page.locator('.workspace-teaching').getByRole('textbox', { name: '活动回答' })).toHaveValue('在演示中继续补充的草稿');
});

for (const width of [390, 820]) test(`reading the discussion at ${width}px uses the full width and keeps the presentation mounted`, async ({ page }, info) => {
  await page.setViewportSize({ width, height: 844 });
  await mountWorkspace(page, true);
  await page.getByRole('button', { name: '展开', exact: true }).click();
  const settings = page.getByRole('button', { name: '学习设置', exact: true });
  await expect(settings).toBeHidden();
  await page.getByRole('button', { name: '展开讨论', exact: true }).click();
  const panel = (await page.locator('.agent-panel').boundingBox())!;
  expect(panel.width).toBeGreaterThan(width - 24);
  expect((await page.locator('.transcript').boundingBox())!.height).toBeGreaterThan(844 * .72);
  await page.locator('[data-turn-id="reply"] .answer-markdown p').first().evaluate(p => p.scrollIntoView({ block: 'start' }));
  await page.screenshot({ path: info.outputPath(`discussion-${width}.png`) });
  await settings.click();
  await page.getByRole('button', { name: '关闭学习设置' }).click();
  await page.getByRole('button', { name: '收起讨论', exact: true }).click();
  await expect(page.frameLocator('.agent-presentation iframe').getByRole('heading', { name: '同一组音反复敲' })).toBeVisible();
  await expect(page.locator('.agent-presentation iframe')).toHaveAttribute('data-load-count', '1');
});

for (const width of [390, 320]) test(`mobile ${width}: compact tools keep the canvas still and preserve zoom and state`, async ({ page }, info) => {
  await page.setViewportSize({ width, height: 844 });
  await mountWorkspace(page);
  await page.getByRole('button', { name: '展开', exact: true }).click();
  const frame = page.frameLocator('.agent-presentation iframe');
  await frame.locator('#parameter').fill('7');
  const iframe = page.locator('.agent-presentation iframe');
  const before = (await iframe.boundingBox())!;
  const toolbar = (await page.locator('.workspace-tools').boundingBox())!;
  expect(toolbar.height).toBeLessThanOrEqual(96);
  expect(before.height).toBeGreaterThan(720);
  await expect(page.getByRole('button', { name: '导出长图', exact: true })).toBeHidden();
  await page.screenshot({ path: info.outputPath('mobile-presentation.png') });
  await page.getByRole('button', { name: '演示工具', exact: true }).click();
  const panel = page.locator('.presentation-tools.is-open');
  await expect(panel).toBeVisible();
  const panelBounds = (await panel.boundingBox())!;
  expect(panelBounds.x).toBeGreaterThanOrEqual(0);
  expect(panelBounds.x + panelBounds.width).toBeLessThanOrEqual(width);
  await page.getByRole('button', { name: '缩小演示', exact: true }).click();
  await expect(frame.locator('html')).toHaveCSS('zoom', '0.9');
  expect(await iframe.boundingBox()).toEqual(before);
  await page.screenshot({ path: info.outputPath('mobile-presentation-tools.png') });
  await page.keyboard.press('Escape');
  await expect(panel).toHaveCount(0);
  await expect(page.locator('.agent-presentation')).toHaveClass(/expanded/);
  await page.getByRole('button', { name: '展开讨论', exact: true }).click();
  await page.getByRole('button', { name: '演示工具', exact: true }).click();
  await page.getByRole('button', { name: '文字说明与来源', exact: true }).click();
  await expect(page.getByRole('button', { name: '演示工具', exact: true })).toHaveAttribute('aria-expanded', 'false');
  await page.getByRole('button', { name: '收起讨论', exact: true }).click();
  await page.getByRole('button', { name: '关闭文字说明与来源', exact: true }).click();
  await expect(frame.locator('#parameter')).toHaveValue('7');
  await expect(iframe).toHaveAttribute('data-load-count', '1');
  await info.attach('mobile-layout.json', { contentType: 'application/json', body: JSON.stringify({ width, toolbarHeight: toolbar.height, canvasHeight: before.height }) });
});

test('opening a presentation from fullscreen uses the split and restores fullscreen on collapse', async ({ page }, info) => {
  await page.setViewportSize({ width: 1536, height: 700 });
  await mountWorkspace(page);
  await page.getByRole('button', { name: '问答全屏', exact: true }).click();
  await page.locator('.agent-input textarea').fill('保留主输入草稿');
  await page.getByRole('button', { name: '展开', exact: true }).click();
  const presentation = (await page.locator('.agent-presentation.expanded').boundingBox())!;
  const panel = (await page.locator('.agent-panel').boundingBox())!;
  expect(panel.width).toBeCloseTo(1536 * .37 - 24, 0);
  expect(panel.x).toBeGreaterThan(presentation.x + presentation.width);
  const input = (await page.locator('.agent-input').boundingBox())!;
  expect(input.x).toBeGreaterThanOrEqual(panel.x);
  expect(input.x + input.width).toBeLessThanOrEqual(panel.x + panel.width);
  await page.getByRole('slider', { name: '演示宽度', exact: true }).fill('72');
  expect((await page.locator('.agent-panel').boundingBox())!.width).toBeCloseTo(1536 * .28 - 24, 0);
  await page.screenshot({ path: info.outputPath('expanded-from-fullscreen.png') });
  await page.getByRole('button', { name: '收起', exact: true }).click();
  await expect(page.locator('.right-rail')).toHaveClass(/fullscreen/);
  expect((await page.locator('.right-rail').boundingBox())!.width).toBe(1536);
  await expect(page.locator('.agent-input textarea')).toHaveValue('保留主输入草稿');
  await expect(page.locator('.agent-presentation iframe')).toHaveAttribute('data-load-count', '1');
});

test('mobile discussion retains presentation teaching and the same live frame', async ({ page }, info) => {
  await page.setViewportSize({ width: 390, height: 844 });
  await mountWorkspace(page);
  await page.getByRole('button', { name: '展开', exact: true }).click();
  const presentation = page.locator('.agent-presentation.expanded');
  const bounds = (await presentation.boundingBox())!;
  expect((await presentation.locator('iframe').boundingBox())!.height).toBeGreaterThan(bounds.height * .9);
  await page.getByRole('button', { name: '展开讨论', exact: true }).click();
  const response = page.locator('.workspace-teaching').getByRole('textbox', { name: '活动回答' });
  await response.fill('手机上的教学草稿');
  await expect(response).toHaveValue('手机上的教学草稿');
  const box = (await response.boundingBox())!;
  expect(box.x).toBeGreaterThanOrEqual(0);
  expect(box.x + box.width).toBeLessThanOrEqual(390);
  await page.screenshot({ path: info.outputPath('mobile-discussion.png') });
  await page.getByRole('button', { name: '收起讨论', exact: true }).click();
  await expect(page.frameLocator('.agent-presentation iframe').locator('#parameter')).toHaveValue('5');
  await expect(page.locator('.agent-presentation iframe')).toHaveAttribute('data-load-count', '1');
});
