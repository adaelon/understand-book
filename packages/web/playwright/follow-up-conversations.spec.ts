import { expect, test, type Page } from '@playwright/test';
import { firstQuestion, openFollowUpBook, quote, secondOccurrence } from './fixtures/follow-up-conversations';

const draft = '我先猜，两组可能一开始就不同。';
const input = (page:Page) => page.locator('.agent-input textarea');
const node = (page:Page,id:string) => page.locator('.follow-up-path-panel:visible button[data-session-id="'+id+'"]');
const pathButton = (page:Page) => page.getByRole('button',{name:'追问路径',exact:true});
const turn = (page:Page,id:string) => page.locator('.transcript [data-turn-id="'+id+'"]');
const preview = (page:Page) => page.locator('.follow-up-preview:visible');
const readingPosition = (page:Page) => page.locator('.reader-pane').evaluate(el=>({
  left:el.getBoundingClientRect().left,width:el.clientWidth,scroll:el.scrollTop,
}));

async function selectSecondOccurrence(page:Page) {
  const paragraph=turn(page,'root-answer').locator('.answer-markdown p').last();
  await paragraph.scrollIntoViewIfNeeded();
  await paragraph.evaluate((element,phrase)=>{
    const walker=document.createTreeWalker(element,NodeFilter.SHOW_TEXT);
    const matches:{text:Node; start:number}[]=[];
    while(walker.nextNode()){
      const text=walker.currentNode,source=text.textContent??'';
      for(let offset=source.indexOf(phrase);offset>=0;offset=source.indexOf(phrase,offset+phrase.length)) {
        matches.push({text,start:offset});
      }
    }
    const last=matches.at(-1)!;
    const range=document.createRange();range.setStart(last.text,last.start);range.setEnd(last.text,last.start+phrase.length);
    const selection=window.getSelection()!;selection.removeAllRanges();selection.addRange(range);
    document.dispatchEvent(new Event('selectionchange'));
  },quote);
  await page.getByRole('button',{name:'另开追问',exact:true}).click();
}

test('分栏先预览已有问题，划选第二处相同原句另开，再取回原草稿',async({page},info)=>{
  const fixture=await openFollowUpBook(page,1440,900);
  await input(page).fill(draft);
  const before=await readingPosition(page);
  await pathButton(page).click();
  const tray=page.locator('[data-follow-up-host="tray"]');
  await expect(tray).toBeVisible();
  await node(page,'control').click();
  await expect(preview(page)).toContainText('对照组也上课吗？');
  await expect(preview(page)).toContainText('也上课。');
  await expect(node(page,'root')).toHaveAttribute('aria-current','true');
  await expect(turn(page,'root-answer')).toBeVisible();
  expect(await readingPosition(page)).toEqual(before);
  expect(fixture.selections).toEqual([]);expect(fixture.starts).toEqual([]);expect(fixture.runs).toEqual([]);
  await page.screenshot({path:info.outputPath('reading-preview.png')});
  await page.getByRole('button',{name:'进入这条对话',exact:true}).click();
  await expect(turn(page,'control-answer')).toContainText('对照组照常上课');
  await input(page).fill('这条是对照组的草稿。');
  await page.getByRole('button',{name:'回到原回答',exact:true}).click();
  await expect(input(page)).toHaveValue(draft);
  await expect(turn(page,'root-answer')).toContainText(quote);
  await selectSecondOccurrence(page);
  await page.getByRole('textbox',{name:'我想问'}).fill(firstQuestion);
  expect(fixture.starts).toHaveLength(0);expect(fixture.runs).toHaveLength(0);
  await page.getByRole('button',{name:'开始这条追问',exact:true}).click();
  await expect(turn(page,'sent-1')).toContainText('研究者抽签决定谁用软件');
  expect(fixture.starts).toHaveLength(1);expect(fixture.runs).toHaveLength(1);
  expect(fixture.starts[0]).toMatchObject({parent_session_id:'root',parent_turn_id:'root-answer',question:firstQuestion,
    anchor:{excerpt:quote,start_part:0,end_part:0,start_offset:secondOccurrence,end_offset:secondOccurrence+quote.length}});
  expect(fixture.runs[0].client_request_id).toBe(fixture.starts[0].client_request_id);
  await page.getByRole('button',{name:'回到原回答',exact:true}).click();
  await expect(turn(page,'root-answer')).toBeVisible();
  await expect(input(page)).toHaveValue(draft);
  const restored=turn(page,'root-answer').locator('mark[data-follow-up-id="new-child"]').first();
  await expect(restored).toBeInViewport();
  expect(await restored.evaluate(el=>{
    const prefix=document.createRange();
    prefix.selectNodeContents(el.closest('.answer-markdown')!);
    prefix.setEndBefore(el);
    return prefix.toString();
  })).toContain(quote);
  await restored.click();
  await expect(preview(page)).toContainText(firstQuestion);
  await expect(turn(page,'root-answer')).toBeVisible();
  await page.getByRole('button',{name:'收起追问路径',exact:true}).click();
  await expect(input(page)).toHaveValue(draft);
  expect(fixture.runs).toHaveLength(1);
  expect(await readingPosition(page)).toEqual(before);
  expect(fixture.errors).toEqual([]);
});

test('尚未发送的首问在切换支线和刷新后仍能继续编辑',async({page})=>{
  const fixture=await openFollowUpBook(page,1440,900);
  const editor=page.getByRole('textbox',{name:'我想问'});
  const pending=page.locator('.follow-up-path-panel:visible').getByRole('button',{name:/随机分组.*待追问/});
  await selectSecondOccurrence(page);
  await editor.fill(firstQuestion);
  const originalId=await pending.getAttribute('data-session-id');
  expect(originalId).toMatch(/^draft:/);
  await page.getByRole('button',{name:'收起追问路径',exact:true}).click();
  await pathButton(page).click();
  await node(page,'control').click();
  await page.getByRole('button',{name:'进入这条对话',exact:true}).click();
  await expect(turn(page,'control-answer')).toBeVisible();
  await page.getByRole('button',{name:'回到原回答',exact:true}).click();
  await expect(turn(page,'root-answer')).toBeVisible();
  await page.reload();
  await pathButton(page).click();
  await expect(pending).toHaveCount(1);
  await expect(pending).toHaveAttribute('data-session-id',originalId!);
  await pending.click();
  await expect(editor).toHaveValue(firstQuestion);
  await editor.fill('如果两个班自己选组，会怎样？');
  await node(page,'control').click();
  await pending.click();
  await expect(editor).toHaveValue('如果两个班自己选组，会怎样？');
  expect(fixture.selections).toEqual(['control','root']);
  expect(fixture.starts).toEqual([]);
  expect(fixture.runs).toEqual([]);
  expect(fixture.errors).toEqual([]);
});

test('宽屏全屏和紧凑全屏按可用空间投影同一棵树',async({page},info)=>{
  const fixture=await openFollowUpBook(page,1440,900);
  await input(page).fill(draft);
  await page.getByRole('button',{name:'问答全屏',exact:true}).click();
  const side=page.locator('[data-follow-up-host="side"]');
  await expect(side).toBeVisible();await node(page,'hours').click();
  await expect(preview(page)).toContainText('两组的课时安排一样吗？');
  const bounds=await side.boundingBox(),transcript=await page.locator('.transcript').boundingBox();
  expect(bounds!.x+bounds!.width).toBeLessThanOrEqual(transcript!.x+1);
  await expect(turn(page,'root-answer')).toBeVisible();
  await page.screenshot({path:info.outputPath('fullscreen-preview.png')});
  await page.setViewportSize({width:1100,height:800});
  await expect(side).toBeHidden();
  await pathButton(page).click();
  await expect(page.locator('[data-follow-up-host="tray"]')).toBeVisible();
  expect(fixture.selections).toEqual([]);expect(fixture.runs).toEqual([]);
  await page.getByRole('button',{name:'退出问答全屏',exact:true}).click();
  await expect(input(page)).toHaveValue(draft);
  expect(fixture.errors).toEqual([]);
});

for(const size of [{width:390,height:844},{width:320,height:568}]) {
  test('手机 '+size.width+'px 可预览、进入、回原句和阅读',async({page},info)=>{
    const fixture=await openFollowUpBook(page,size.width,size.height);
    await input(page).fill(draft);await input(page).blur();
    await pathButton(page).click();
    const sheet=page.getByRole('dialog',{name:'追问路径',exact:true});
    await expect(sheet).toBeVisible();await node(page,'control').click();
    await expect(preview(page)).toContainText('对照组也上课吗？');
    const enter=sheet.getByRole('button',{name:'进入这条对话',exact:true});
    await enter.scrollIntoViewIfNeeded();await expect(enter).toBeInViewport();
    expect(await page.evaluate(()=>document.documentElement.scrollWidth)).toBeLessThanOrEqual(size.width);
    expect(fixture.selections).toEqual([]);
    await page.screenshot({path:info.outputPath('phone-'+size.width+'.png')});
    await enter.click();await expect(sheet).toBeHidden();
    await expect(turn(page,'control-answer')).toBeVisible();
    await expect(input(page)).not.toBeFocused();
    await page.getByRole('button',{name:'回到原回答',exact:true}).click();
    await expect(turn(page,'root-answer')).toBeVisible();
    await expect(input(page)).toHaveValue(draft);
    await page.getByRole('button',{name:'返回阅读',exact:true}).click();
    await expect(page.locator('.reader-pane')).toBeVisible();
    await page.locator('.workspace-mobile-nav').getByRole('button',{name:'问答',exact:true}).click();
    await expect(turn(page,'root-answer')).toBeVisible();await expect(input(page)).toHaveValue(draft);
    expect(fixture.runs).toEqual([]);expect(fixture.errors).toEqual([]);
  });
}
