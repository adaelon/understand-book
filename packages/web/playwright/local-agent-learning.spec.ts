import { expect, test } from '@playwright/test';
import { openFollowUpBook } from './fixtures/follow-up-conversations';
import { scenario } from './fixtures/tutor-continuation';

test('本机 Agent 在设置中连接，按发问时的选择发送，个人连接可撤销', async ({page}, info) => {
  const fixture = await openFollowUpBook(page, 1440, 900);
  let selected: string | null = null, enabled = false;
  const agent = { id:'opencode', name:'OpenCode', command:'opencode', args:['acp'], available:true,
    configured:false, managed:false, installable:true, login:'opencode auth login' };
  const actions: string[] = [];
  await page.route('**/api/desktop/status', route => route.fulfill({json:{desktop_host:true,reader_only:false,active_book:true,library_root_available:true}}));
  await page.route('**/api/model-settings', route => route.fulfill({json:{configured:false,source:'none',settings:null}}));
  await page.route('**/api/local-agent/extensions', route => route.fulfill({json:{skills:[],scope:'多出来的八分'}}));
  await page.route('**/api/local-agent/settings', route => {
    const body = route.request().postDataJSON();
    if (body) actions.push(body.action);
    if (body?.action === 'connect') agent.configured = true;
    if (body?.action === 'select') selected = body.id;
    return route.fulfill({json:{agents:[agent],selected_agent_id:selected,probe:body?.action === 'probe' ? {agent_info:{name:'OpenCode',version:'fixture'}} : null}});
  });
  let access = {book_ids:[] as string[],write:false,navigate:false,manage:false};
  await page.route('**/api/personal/connection', route => {
    const body = route.request().postDataJSON();
    if (body?.action === 'enable') { enabled = true; access = body; }
    if (body?.action === 'revoke') enabled = false;
    return route.fulfill({json:{ ...access, enabled, materials:[{book_id:scenario.book_id,title:'多出来的八分'}],
      token:body?.action === 'enable' ? 'test-credential' : undefined }});
  });
  await page.route('**/api/personal/navigation', route => route.fulfill({json:{pending:null,change_id:0}}));
  await page.route('**/api/agent/follow-ups*', route => route.fulfill({json:{nodes:[]}}));
  await page.reload();
  const picker = page.getByRole('combobox',{name:'当前问答 Agent'});
  await picker.selectOption(':settings');
  const settings = page.getByRole('dialog',{name:'设置',exact:true});
  const section = settings.getByRole('region',{name:'我的 Agent'});
  await section.getByRole('button',{name:/OpenCode/}).click();
  await section.getByRole('button',{name:'连接已有安装'}).click();
  await section.getByRole('button',{name:'测试连接'}).click();
  await expect(section.getByRole('status')).toContainText('连接成功');
  await section.getByRole('button',{name:'用于问答'}).click();
  await expect.poll(() => selected).toBe('opencode');
  await settings.screenshot({path:info.outputPath('settings.png')});
  await settings.getByText('个人学习连接',{exact:true}).click();
  await settings.getByLabel('保存笔记、高亮和待续问题').check();
  await settings.getByRole('button',{name:'启用个人连接'}).click();
  await expect(settings.getByLabel('连接凭证（仅此次显示）')).toHaveValue('test-credential');
  await settings.getByRole('button',{name:'撤销个人连接'}).click();
  await expect(settings.getByLabel('连接凭证（仅此次显示）')).toHaveCount(0);
  await settings.getByRole('button',{name:'关闭设置'}).click();
  await expect(picker).toHaveValue('opencode');
  let acceptRun!: () => void;
  const waiting = new Promise<void>(resolve => { acceptRun = resolve; });
  await page.route('**/api/agent/runs', async route => { await waiting; await route.fallback(); }, {times:1});
  await page.locator('.agent-input textarea').fill('为什么需要对照？');
  await page.getByRole('button',{name:'发送',exact:true}).click();
  await expect(page.getByText('可以先写补充，本轮结束后发送。')).toBeVisible();
  await page.locator('.agent-input textarea').fill('接着读。');
  await page.locator('.agent-input textarea').press('Control+Enter');
  expect(fixture.runs.length).toBe(0);
  acceptRun();
  await expect.poll(() => fixture.runs[0]?.local_agent_id).toBe('opencode');
  await expect(page.locator('[data-turn-id="sent-1"] .answer-markdown')).toBeVisible();
  await expect(page.locator('[data-turn-id="sent-1"] .turn-agent-name')).toHaveText('本轮 Agent · OpenCode');
  await page.screenshot({path:info.outputPath('composer.png')});
  await page.reload();
  await expect(page.locator('.agent-input textarea')).toHaveValue('接着读。');
  await picker.selectOption('');
  await page.getByRole('button',{name:'发送',exact:true}).click();
  await expect.poll(() => fixture.runs.length).toBe(2);
  expect(fixture.runs[1]).not.toHaveProperty('local_agent_id');
  await page.reload();
  await expect(page.locator('[data-turn-id="sent-1"] .turn-agent-name')).toHaveText('本轮 Agent · OpenCode');
  await expect(page.locator('[data-turn-id="sent-2"] .turn-agent-name')).toHaveCount(0);
  expect(actions).toEqual(['connect','probe','select','select']);
});

test('个人 MCP 的并排请求在 Reader 显示原文后才回执', async ({page}, info) => {
  await openFollowUpBook(page, 1440, 900);
  await page.route('**/api/desktop/status', route => route.fulfill({json:{desktop_host:true,reader_only:false,active_book:true,library_root_available:true}}));
  await page.route('**/api/local-agent/settings', route => route.fulfill({json:{agents:[],selected_agent_id:null}}));
  await page.route('**/api/personal/connection', route => route.fulfill({json:{
    enabled:true,book_ids:[scenario.book_id],write:false,navigate:true,manage:false,materials:[]
  }}));
  const sources = [
    {lid:'1.1',text:'两组学生学习同一单元，最后做了同一份测验。'},
    {lid:'1.3.1',text:'使用软件的那组平均高出 8 分。'}
  ].map(source => ({
    text:source.text, title:'多出来的八分 · ' + source.lid, book_dir:'/books/eight',
    source_ref:{book_id:scenario.book_id,source_revision:'tc0-source',lid:source.lid}
  }));
  let receipt: { id:string; displayed:boolean } | undefined;
  await page.route('**/api/personal/navigation', route => {
    if (route.request().method() === 'POST') receipt = route.request().postDataJSON();
    return route.fulfill({json:{change_id:0,pending:receipt ? null : {id:'compare-1',kind:'compare',sources}}});
  });
  await page.reload();
  const comparison = page.getByRole('dialog',{name:'并排原文'});
  for (const source of sources) await expect(comparison).toContainText(source.text);
  await expect.poll(() => receipt).toEqual({id:'compare-1',displayed:true});
  await comparison.screenshot({path:info.outputPath('compare.png')});
  await comparison.getByRole('button',{name:'关闭',exact:true}).click();
  await expect(comparison).toHaveCount(0);
});

test('从历史回合返回原选区时只标记保存的文字', async ({page}) => {
  await openFollowUpBook(page, 1440, 900, 'quoted');
  await page.route('**/api/reader/goto', route => route.fulfill({json:{
    viewport:{anchor_lid:'1.1',top_lid:'1.1',bottom_lid:'1.2',width:2,visible_lids:['1.1','1.2']},
    texts:[{lid:'1.1',text:'两组学生学习同一单元，最后做了同一份测验。一组使用学习软件，另一组照常上课。'},{lid:'1.2',text:'测验平均分'}]
  }}));
  await page.getByRole('button',{name:'回到原选区'}).click();
  await expect(page.locator('.reader-pane .source-focus-mark')).toHaveText('学生');
});

test('本机 Agent 失败后定向检查原连接，保留原问题和选区', async ({page}) => {
  const fixture = await openFollowUpBook(page, 1440, 900, 'failed-agent');
  await page.route('**/api/desktop/status', route => route.fulfill({json:{desktop_host:true,reader_only:false,active_book:true,library_root_available:true}}));
  const probes: string[] = [];
  await page.route('**/api/local-agent/settings', route => {
    const body = route.request().postDataJSON();
    if (body?.action === 'probe') probes.push(body.id);
    return route.fulfill({json:{agents:[{id:'opencode',name:'OpenCode',command:'opencode',args:['acp'],configured:true,available:true}],
      selected_agent_id:null,probe:body?.action === 'probe' ? {agent_info:{name:'OpenCode'}} : null}});
  });
  await page.route('**/api/personal/connection',route => route.fulfill({json:{enabled:false,book_ids:[],materials:[]}}));
  await page.route('**/api/local-agent/extensions',route => route.fulfill({json:{skills:[]}}));
  await page.route('**/api/model-settings',route => route.fulfill({json:{configured:false,settings:null}}));
  await page.reload();
  const turn = page.locator('[data-turn-id="root-answer"]');
  await expect(turn).toContainText('请检查连接和终端登录状态');
  await expect(turn).not.toContainText('hidden transport diagnostics');
  await turn.getByRole('button',{name:'检查本轮 Agent 连接'}).click();
  const settings = page.getByRole('dialog',{name:'设置',exact:true});
  await expect(settings.getByRole('status')).toContainText('连接成功');
  expect(probes).toEqual(['opencode']);
  await settings.getByRole('button',{name:'关闭设置'}).click();
  await turn.getByRole('button',{name:'编辑后重试'}).click();
  await expect(page.locator('.agent-input textarea')).toHaveValue('多出来的 8 分，是软件带来的吗？');
  await expect(page.locator('.ask-draft blockquote')).toHaveText('学生');
  expect(fixture.runs).toEqual([]);
});
