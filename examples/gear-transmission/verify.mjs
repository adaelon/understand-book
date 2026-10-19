import assert from 'node:assert/strict';
import { createRequire } from 'node:module';
import { fileURLToPath } from 'node:url';
import path from 'node:path';
import fs from 'node:fs';

const here=path.dirname(fileURLToPath(import.meta.url)),root=path.resolve(here,'../..');
const require=createRequire(path.join(root,'packages/web/package.json'));
const {chromium}=require('@playwright/test');
const output=path.join(here,'verification');fs.mkdirSync(output,{recursive:true});
const base=process.env.GEAR_DEMO_URL||'http://127.0.0.1:4198/';
const browser=await chromium.launch({channel:'msedge',headless:true});
const report={math:'Run model.test.mjs separately',browser:'Edge / Chromium',viewports:[],checks:[],errors:[],externalRequests:[]};
const state=p=>p.evaluate(()=>window.gearDemo.snapshot());
const close=(a,b)=>assert.ok(Math.abs(a-b)<1e-8,`${a} != ${b}`);
async function range(p,id,value){await p.locator('#'+id).evaluate((n,value)=>{n.value=value;n.dispatchEvent(new Event('input',{bubbles:true}));},String(value));}
async function ready(p){await p.goto(base);await p.waitForFunction(()=>document.body.dataset.ready==='true');assert.equal(await p.locator('body').getAttribute('data-render'),'webgl');}
function observe(p){p.on('pageerror',e=>report.errors.push(e.message));p.on('request',r=>{if(!r.url().startsWith(base)&&!r.url().startsWith('data:'))report.externalRequests.push(r.url());});}
try{
  const context=await browser.newContext({viewport:{width:1440,height:1080},reducedMotion:'reduce',deviceScaleFactor:1});
  const p=await context.newPage();observe(p);await ready(p);
  assert.equal((await state(p)).playing,false);report.checks.push('reduced-motion preference starts paused');
  await p.screenshot({path:path.join(output,'desktop.png')});
  for(const teeth of [20,30,40,60]){
    await p.locator(`[data-teeth="${teeth}"]`).first().click();
    const m=await p.evaluate(()=>window.gearDemo.model());close(m.outputRpm,-12*20/teeth);close(m.outputTorque,2*teeth/20);
    assert.equal(await p.locator('[data-read="teeth"]').first().innerText(),String(teeth));
  }report.checks.push('all four gear selections update rendered model and readings');
  await range(p,'speed',18);await range(p,'torque-input',3.2);
  assert.equal(await p.locator('[data-read="output-rpm"]').first().innerText(),'6');
  assert.equal(await p.locator('[data-read="output-torque"]').innerText(),'9.6');
  report.checks.push('parameter changes update speed, force and torque immediately');
  await p.locator('#play').click();const before=(await state(p)).angle;
  await p.waitForFunction(before=>window.gearDemo.snapshot().angle>before+.1,before);
  await p.locator('#play').click();const paused=(await state(p)).angle;
  await p.evaluate(()=>new Promise(resolve=>requestAnimationFrame(()=>requestAnimationFrame(resolve))));close((await state(p)).angle,paused);
  report.checks.push('play advances the model; pause holds the angle');
  const box=await p.locator('#assembly').boundingBox(),camera=(await state(p)).camera;
  await p.mouse.move(box.x+box.width*.45,box.y+box.height*.45);await p.mouse.down();await p.mouse.move(box.x+box.width*.45+80,box.y+box.height*.45+35,{steps:8});await p.mouse.up();
  assert.notDeepEqual((await state(p)).camera,camera);
  await p.locator('[data-view="front"]').click();assert.equal((await state(p)).camera.tilt,0);
  await p.locator('#assembly').focus();await p.keyboard.press('ArrowDown');assert.ok((await state(p)).camera.tilt>0);
  report.checks.push('pointer orbit, front view and keyboard orbit');
  await range(p,'explode',1);assert.equal((await state(p)).playing,false);assert.equal(await p.locator('#play').isDisabled(),true);
  await p.locator('#hero-figure').screenshot({path:path.join(output,'exploded.png')});
  await range(p,'explode',0);assert.equal(await p.locator('#play').isEnabled(),true);
  report.checks.push('exploded assembly pauses and prevents impossible running contact');
  await range(p,'speed',24);await p.locator('#one-turn').click();
  await p.waitForFunction(()=>Math.abs(window.gearDemo.snapshot().angle-2*Math.PI)<1e-8,{},{timeout:10000});
  await p.locator('#ratio').screenshot({path:path.join(output,'one-turn.png')});
  await p.locator('#ratio-scene').focus();await p.keyboard.press('ArrowRight');close((await state(p)).angle,2*Math.PI+Math.PI/30);
  const rb=await p.locator('#ratio-scene').boundingBox();const old=(await state(p)).angle;
  // The large driven wheel is centred right of the image midpoint; drag on its upper face.
  await p.mouse.move(rb.x+rb.width*.56,rb.y+rb.height*.35);await p.mouse.down();await p.mouse.move(rb.x+rb.width*.61,rb.y+rb.height*.43,{steps:6});await p.mouse.up();
  assert.notEqual((await state(p)).angle,old);report.checks.push('single turn ends at 2π; keyboard and direct gear dragging change phase');
  await range(p,'contact-phase',0);await p.locator('#contact').screenshot({path:path.join(output,'contact-start.png')});
  await range(p,'contact-phase',1);await p.locator('#contact').screenshot({path:path.join(output,'contact-end.png')});
  await range(p,'contact-phase',.24);await p.locator('#contact').screenshot({path:path.join(output,'contact.png')});
  await range(p,'unwrap',2.3);await p.locator('#involute').screenshot({path:path.join(output,'involute-end.png')});
  await p.locator('#unwrap-play').click();await p.waitForFunction(()=>window.gearDemo.snapshot().unwrap>0&&window.gearDemo.snapshot().unwrap<1);await p.locator('#unwrap-play').click();
  await range(p,'unwrap',1.1);report.checks.push('contact slider endpoints and involute replay');
  await p.locator('#save').click();const stored=await state(p);
  await ready(p);assert.deepEqual(await state(p),{...stored,playing:false});
  await p.locator('[data-teeth="20"]').first().click();await p.locator('#restore').click();assert.deepEqual(await state(p),{...stored,playing:false});
  report.checks.push('save, browser reload and explicit restore preserve the complete scene');
  const canvasSize=await p.locator('#assembly').evaluate(n=>[n.width,n.height]);
  const download=p.waitForEvent('download');await p.locator('#export').click();const file=await download;await file.saveAs(path.join(output,'export.png'));
  assert.deepEqual(await p.locator('#assembly').evaluate(n=>[n.width,n.height]),canvasSize);
  const png=fs.readFileSync(path.join(output,'export.png'));assert.equal(png.readUInt32BE(16),1600);assert.equal(png.readUInt32BE(20),1040);assert.ok(png.length>20000);
  report.checks.push('PNG export includes a populated 1600 × 1040 image');
  await p.locator('#reset').click();await p.locator('[data-view="perspective"]').click();
  for(const id of ['ratio','torque','contact','involute'])await p.locator('#'+id).screenshot({path:path.join(output,id+'.png')});
  report.viewports.push({width:1440,height:1080,overflow:await p.evaluate(()=>document.documentElement.scrollWidth>innerWidth)});
  await context.close();
  for(const width of [390,320,768]){
    const context=await browser.newContext({viewport:{width,height:844},reducedMotion:'reduce',isMobile:width<600,hasTouch:width<600,deviceScaleFactor:1});
    const p=await context.newPage();observe(p);await ready(p);
    const overflow=await p.evaluate(()=>document.documentElement.scrollWidth>innerWidth);assert.equal(overflow,false);
    await p.screenshot({path:path.join(output,`width-${width}.png`)});
    await p.locator('[data-teeth="60"]').first().click();
    await p.locator('#hero-figure').screenshot({path:path.join(output,`three-${width}.png`)});
    for(const id of ['torque','contact','involute'])await p.locator('#'+id).screenshot({path:path.join(output,`${id}-${width}.png`)});
    if(width===390){await range(p,'unwrap',2.3);await p.locator('#involute').screenshot({path:path.join(output,'involute-end-mobile.png')});}
    report.viewports.push({width,height:844,overflow});await context.close();
  }
  assert.deepEqual(report.errors,[]);assert.deepEqual(report.externalRequests,[]);assert.ok(report.viewports.every(v=>!v.overflow));
  report.result='passed';fs.writeFileSync(path.join(output,'report.json'),JSON.stringify(report,null,2));console.log(JSON.stringify(report,null,2));
}finally{await browser.close();}
