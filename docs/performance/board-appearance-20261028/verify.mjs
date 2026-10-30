import { chromium, expect } from '@playwright/test';
import { mkdir, writeFile } from 'node:fs/promises';
import path from 'node:path';
const output = path.dirname(new URL(import.meta.url).pathname.replace(/^\/([A-Za-z]:)/, '$1'));
const origin = process.env.BOARD_VISUAL_ORIGIN ?? 'http://127.0.0.1:4184';
const prefix = `${origin}/@fs/${process.cwd().replaceAll('\\','/')}`;
const browser = await chromium.launch({headless:true});
const page = await browser.newPage({viewport:{width:1440,height:1000},deviceScaleFactor:1});
const errors=[];page.on('pageerror',error=>errors.push(error.message));
const checks=[];
async function ready() {
  await page.locator('[data-item-id="state"]').waitFor();
  await page.evaluate(()=>document.fonts.ready);
  await expect.poll(()=>page.locator('[data-board-rich-ready="false"]').count()).toBe(0);
}
async function inspect(name) {
  const result = await page.locator('[data-board-graphic]').evaluateAll(elements=>elements.map(svg=>{
    const rect=svg.getBoundingClientRect(),bounds=svg.viewBox.baseVal,text=svg.querySelector('[data-board-drawing-text]'),rich=svg.querySelector('[data-board-rich-text]');
    let clipped=false;
    if(text){const box=text.getBBox();clipped=box.x < -1 || box.y < -1 || box.x+box.width>bounds.width+1 || box.y+box.height>bounds.height+1;}
    if(rich){
      clipped ||= rich.dataset.boardRichOverflow==='true';
      for(const element of [rich,...rich.querySelectorAll('.katex-html')]){
        const box=element.getBoundingClientRect();clipped ||= box.left<rect.left-1 || box.right>rect.right+1 || box.top<rect.top-1 || box.bottom>rect.bottom+1;
      }
    }
    return {id:svg.dataset.itemId,clipped,math:svg.querySelectorAll('.katex').length,fontSize:rich?.dataset.boardRichFontSize};
  }));
  expect(result.filter(item=>item.clipped),`${name}: text stays in its retained bounds`).toEqual([]);
  checks.push({name,items:result});
}
try{
  await mkdir(output,{recursive:true});
  await page.goto(`${prefix}/docs/performance/board-playback-fix-20261028/playback.html?reset=1`);
  await ready();
  await page.getByRole('button',{name:'显示全部内容',exact:true}).click();
  await inspect('existing-board-desktop');
  expect(await page.locator('[data-item-id="update"] .katex').count()).toBeGreaterThan(0);
  expect(await page.locator('[data-item-id="read"] .katex').count()).toBeGreaterThan(0);
  await page.screenshot({path:path.join(output,'existing-board.png')});
  await page.getByRole('button',{name:'播放讲解',exact:true}).click();
  await expect.poll(()=>page.locator('[data-item-id]').count(),{timeout:20000}).toBe(11);
  await expect(page.getByRole('button',{name:'播放讲解',exact:true})).toBeVisible({timeout:6000});
  await expect(page.locator('[data-board-link]')).toHaveCount(10);
  await inspect('existing-board-playback-completed');
  await page.goto(`${prefix}/docs/performance/board-appearance-20261028/preview.html`);
  await ready();
  await inspect('designed-board-desktop');
  await page.screenshot({path:path.join(output,'board-overview.png')});
  await page.getByRole('button',{name:'状态更新',exact:true}).click();
  await inspect('designed-board-focus');
  await page.screenshot({path:path.join(output,'board-equations.png')});
  const drawing=page.locator('[data-item-id="update"]');
  const before=await drawing.getAttribute('style');
  const box=await drawing.boundingBox();
  await page.mouse.move(box.x+box.width/2,box.y+box.height/2);await page.mouse.down();
  await page.mouse.move(box.x+box.width/2+25,box.y+box.height/2+20,{steps:4});await page.mouse.up();
  await expect.poll(()=>drawing.getAttribute('style')).not.toBe(before);
  await drawing.locator('[role="button"]').focus();await page.keyboard.press('Enter');
  await expect(drawing).toHaveClass(/selected/);
  checks.push({name:'formula-node-drag-and-keyboard',passed:true});
  await page.setViewportSize({width:390,height:844});
  await page.goto(`${prefix}/docs/performance/board-appearance-20261028/preview.html`);await ready();
  await page.getByRole('button',{name:'状态更新',exact:true}).click();
  await inspect('designed-board-narrow');
  await page.screenshot({path:path.join(output,'board-narrow.png')});
  expect(errors).toEqual([]);
  await writeFile(path.join(output,'verification.json'),JSON.stringify({checks,errors},null,2));
  console.log(JSON.stringify({checks:checks.map(check=>check.name),errors},null,2));
}finally{await browser.close();}
