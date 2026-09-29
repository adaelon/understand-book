import fs from 'node:fs';
import {createRequire} from 'node:module';
const require=createRequire(new URL('../../../packages/web/package.json',import.meta.url));
const {chromium}=require('@playwright/test');
const browser=await chromium.launch({headless:true});
const results=[];
const batch=process.argv[2]??'';
const prefix=batch?batch+'/':'';
try {
  const page=await browser.newPage({viewport:{width:1280,height:900}});
  const errors=[];page.on('pageerror',e=>errors.push(e.message));
  await page.goto('http://127.0.0.1:8770/ex10-svg-konva/index.html');
  await page.locator('iframe').first().waitFor();
  if(await page.locator('iframe').count()!==4)throw Error('Expected four original pages');
  await page.screenshot({path:new URL(prefix+'viewer.png',import.meta.url).pathname.replace(/^\/([A-Za-z]:)/,'$1'),fullPage:true});
  for(const name of ['B1','C1']) {
    await page.goto(`http://127.0.0.1:8770/ex10-svg-konva/${prefix}runs/${name}/standalone.html`);
    await page.waitForFunction(()=>!!window.presentationScene);
    const etaSelector=batch==='batch3'&&name==='C1'?'#etaRange':'#eta';
    const range=page.locator(etaSelector);await range.scrollIntoViewIfNeeded();
    const box=await range.boundingBox();
    await page.mouse.move(box.x+box.width*.4,box.y+box.height/2);await page.mouse.down();
    await page.mouse.move(box.x+box.width*.65,box.y+box.height/2,{steps:10});
    const drag=await page.evaluate(selector=>({eta:document.querySelector(selector).value,scene:window.presentationScene.snapshot(),state:window.ex10Read()}),etaSelector);
    await page.mouse.up();
    const play=page.locator(name==='B1'?'#play':batch==='batch3'?'#playBtn':'#btn-play');
    await play.click();await page.waitForTimeout(200);
    const during=await page.evaluate(()=>window.presentationScene.snapshot());
    await play.click();
    const paused=await page.evaluate(()=>window.presentationScene.snapshot());
    results.push({name,drag,during,paused,pass:drag.scene.semantic_state===0&&drag.scene.transition_progress===0&&during.playing&&!paused.playing&&paused.transition_progress>0});
  }
  const report={identity:'Standalone original-page preview; not a Reader persistence test',errors,results};
  fs.writeFileSync(new URL(prefix+'viewer-check.json',import.meta.url),JSON.stringify(report,null,2));
  if(errors.length||results.some(r=>!r.pass))throw Error(JSON.stringify(report));
  console.log(JSON.stringify(report));
} finally {await browser.close()}
