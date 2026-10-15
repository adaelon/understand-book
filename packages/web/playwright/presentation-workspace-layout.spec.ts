import { expect, test, type Page } from '@playwright/test';

const reference = { presentation_id: 'layout-example', revision: 1 };
const prompt = '这段教学说明依次解释计算图、执行步骤、上下文状态、权重集合和形状汇总。观察每个参数如何影响结果，再写下你的理解。';
const teachingPrompt = Array.from({ length: 18 }, () => prompt).join('\n\n');
const outcome = {
  answer: prompt, answer_view: { parts: [{ kind: 'markdown', text: prompt }, { kind: 'presentation', ...reference }], sources: [] },
  incomplete: false, warning: null, turns: 1, tokens_spent: 0, effects: [], trace: [], memory_updates: [],
  profile_usage: { snapshot_revision: 0, injected_fact_ids: [], influences: [], claimed_used_fact_ids: [] },
};

async function mountWorkspace(page: Page) {
  await page.route('**/presentation-layout-fixture', route => route.fulfill({ contentType: 'text/html', body: `<!doctype html>
    <html lang="zh-CN"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"></head>
    <body><div id="app"></div><script type="module">
    import {createApp,defineComponent,h,ref} from '/node_modules/.vite/deps/vue.js';
    import RightRail from '/src/components/RightRail.vue';
    import {renderMarkdown} from '/src/md.ts';
    import '/src/style.css';
    createApp(defineComponent({setup(){
      const workspace=ref(null),fullscreen=ref(false),input=ref('');
      const chat=[{turnId:'t',user:'继续任务',outcome:${JSON.stringify(outcome)},hasTeaching:true,pending:false,
        questionAnchorLid:null,questionQuote:null,questionSelection:null,effectLabels:[]}];
      return()=>h('main',{style:'max-width:760px;margin:24px auto;padding:8px'},[
        h(RightRail,{chat,
          presentationWorkspace:workspace.value,'onUpdate:presentationWorkspace':v=>workspace.value=v,
          fullscreen:fullscreen.value,onToggleFullscreen:()=>fullscreen.value=!fullscreen.value,
          chatSessions:[],activeChatSessionId:'s',agentInput:input.value,'onUpdate:agentInput':v=>input.value=v,
          sending:false,selectedLid:null,selectedFormula:null,contextNotes:[],contextHighlights:[],renderMarkdown,
          effLabel:()=>'',effState:()=>undefined,isGoto:()=>false,showEffectPrimary:()=>false,showEffectSecondary:()=>false,
          effectPrimaryLabel:()=>'',effectSecondaryLabel:()=>'',gotoBack:()=>'',askDraft:null},
          {'learning-goal':()=>h('div',{class:'conversation-learning-goal'},'本次学习：理解模型架构')})]);
    }})).mount('#app');
    </script></body></html>` }));
  await page.route('**/api/**', async route => {
    const path = new URL(route.request().url()).pathname;
    const body = route.request().postDataJSON() ?? {};
    let result: unknown = {};
    if (path.endsWith('/agent/presentation.read')) result = {
      reference, title: '模型架构演示', entrypoint: 'index.html',
      content_files: { 'index.html': `<!doctype html><html><body><h1>模型架构演示</h1><label>参数 <input id="parameter" value="5"></label>${Array.from({ length: 10 }, () => `<p>${prompt}</p>`).join('')}<button id="last-control">最后一个操作</button></body></html>` },
      sources: [], initial_state: {}, restored_state: null, restored_state_revision: null,
      readable_view: { parts: [], sources: [] }, animation_assets: {}, assumptions: [],
    };
    if (path.endsWith('/tutor/activities')) result = { activities: body.reference ? [{ delivery_ref: 'delivery',
      move: { move_id: 'explain', prompt: teachingPrompt, actions: ['submit'] }, status: 'active', attempted: false, help_seen: [],
    }] : [] };
    if (path.endsWith('/agent/presentation.state.save')) result = { session_id: 's', turn_id: 't', reference, state_revision: 1, saved_state_ref: 'scene' };
    if (path.endsWith('/tutor/display')) { await route.abort('failed'); return; }
    await route.fulfill({ json: result });
  });
  await page.goto('/presentation-layout-fixture');
  await expect(page.frameLocator('.agent-presentation iframe').locator('#parameter')).toHaveValue('5');
  await expect(page.locator('.agent-presentation .tutor-activities article')).toHaveCount(1);
}

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
