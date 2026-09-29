import {test,expect} from '@playwright/test';
import fs from 'node:fs';
import path from 'node:path';

test('Reader short viewport supports actual mouse/touch slider, last controls and question',async({browser})=>{
  test.setTimeout(90000);
  const records:any[]=[];
  const output=path.resolve(process.env.EX11_SHORT_OUTPUT??'../../docs/performance/ex11-local-demonstrations/ex11.5/after');fs.mkdirSync(output,{recursive:true});
  for(const touch of [false,true]){
    const context=await browser.newContext({viewport:{width:640,height:240},hasTouch:touch});
    const page=await context.newPage();page.setDefaultTimeout(8000);
    await page.request.post('http://127.0.0.1:4175/reset-scene');
    await page.route('**/api/**',async route=>{const response=await route.fetch({url:route.request().url().replace(/^.*\/api/,'http://127.0.0.1:4175')});await route.fulfill({response})});
    await page.goto('/agent-presentation-visual.html');const frame=page.frameLocator('.agent-presentation iframe');
    await expect(frame.locator('#eta')).toBeVisible();await page.getByRole('button',{name:'展开',exact:true}).click();
    await frame.locator('#eta').scrollIntoViewIfNeeded();
    const old=await frame.locator('#eta').inputValue();const box=(await frame.locator('#eta').boundingBox())!;
    const start=box.x+14+(box.width-28)*Number(old)/1.5,end=box.x+14+(box.width-28)*.3,y=box.y+box.height/2;
    const geometry=await page.locator('.agent-presentation').evaluate(root=>Object.fromEntries(['header','iframe','.presentation-follow-up','.presentation-readable'].map(s=>{const n=root.querySelector(s)!;const b=n.getBoundingClientRect();return [s,{x:b.x,y:b.y,width:b.width,height:b.height}]})));
    const innerHit=await frame.locator('#eta').evaluate(n=>{const b=n.getBoundingClientRect();const hit=document.elementFromPoint(b.x+14+(b.width-28)*Number((n as HTMLInputElement).value)/1.5,b.y+b.height/2);(window as any).__pointer=[];for(const t of ['pointerdown','pointermove','pointercancel','pointerup','input'])document.addEventListener(t,e=>(window as any).__pointer.push({type:t,id:(e.target as Element)?.id}),true);return {hit:hit?.id,tag:hit?.tagName,frameHeight:innerHeight,rect:{x:b.x,y:b.y,width:b.width,height:b.height}}});
    if(touch){const cdp=await context.newCDPSession(page);await cdp.send('Input.dispatchTouchEvent',{type:'touchStart',touchPoints:[{x:start,y}]});for(let i=1;i<=12;i++)await cdp.send('Input.dispatchTouchEvent',{type:'touchMove',touchPoints:[{x:start+(end-start)*i/12,y}]});await cdp.send('Input.dispatchTouchEvent',{type:'touchEnd',touchPoints:[]});}
    else{await page.mouse.move(start,y);await page.mouse.down();await page.mouse.move(end,y,{steps:12});await page.mouse.up();}
    const current=await frame.locator('#eta').inputValue();
    const events=await frame.locator('body').evaluate(()=>(window as any).__pointer);
    await page.screenshot({path:path.join(output,`${touch?'touch':'mouse'}.png`)});
    records.push({touch,old,current,geometry,innerHit,events});fs.writeFileSync(path.join(output,'geometry.json'),JSON.stringify(records,null,2));
    expect(current).not.toBe(old);
    const last=frame.locator('button[data-eta]').last();await last.click();await expect(frame.locator('#eta')).toHaveValue('1.1');
    await page.getByRole('textbox',{name:'针对当前现场追问'}).fill('Explain this short screen');await page.getByRole('button',{name:'发送追问',exact:true}).click();
    await expect(page.getByTestId('follow-up-status')).toHaveText('追问已完成');
    records.at(-1).lastControlAndQuestion=true;fs.writeFileSync(path.join(output,'geometry.json'),JSON.stringify(records,null,2));
    await page.unrouteAll({behavior:'ignoreErrors'});await context.close();
  }
});
