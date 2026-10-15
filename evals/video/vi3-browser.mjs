import { createRequire } from 'node:module';
import { readFileSync, writeFileSync, mkdirSync } from 'node:fs';
import path from 'node:path';
import assert from 'node:assert/strict';
const require=createRequire(new URL('../../packages/web/package.json',import.meta.url));
const {chromium}=require('@playwright/test');
const workspace=path.resolve(process.argv[2]||'evals/video/generated/vi2/moved-workspace');
const output=path.resolve(process.argv[3]||'evals/video/generated/vi3');
const browser=await chromium.launch({channel:'msedge',headless:true});
try {
  const page=await browser.newPage({viewport:{width:1280,height:960}});
  await page.goto('http://localhost:5175');
  await page.waitForFunction(()=>document.querySelector('video')?.readyState>=2);
  const player=await page.locator('video').elementHandle();
  await player.evaluate(v=>{v.currentTime=555.123;v.playbackRate=1.25;});
  await page.waitForFunction(()=>{const v=document.querySelector('video');return !v.seeking&&v.readyState>=2;});
  await page.getByRole('region',{name:'英文转写'}).waitFor({timeout:30*60*1000});
  const map=JSON.parse(readFileSync(path.join(workspace,'transcript_map.json')));
  assert(await player.evaluate(v=>v===document.querySelector('video')));
  assert.equal(await player.evaluate(v=>v.currentTime),555.123);
  assert.equal(await player.evaluate(v=>v.playbackRate),1.25);
  const buttons=page.locator('.transcript button');
  assert.equal(await buttons.count(),map.segments.length);
  const checks=[];
  for(const index of [0,Math.floor(map.segments.length/2),map.segments.length-1]) {
    const target=map.segments[index].location.start_ms/1000;
    await buttons.nth(index).click();
    await page.waitForFunction(t=>{const v=document.querySelector('video');return !v.seeking&&v.readyState>=2&&Math.abs(v.currentTime-t)<.2;},target);
    checks.push({segment:map.segments[index].segment_id,target,actual:await page.locator('video').evaluate(v=>v.currentTime)});
  }
  const before=await page.locator('video').evaluate(v=>{v.playbackRate=1.5;return v.currentTime;});
  await page.reload();
  await page.waitForFunction(t=>{const v=document.querySelector('video');return v?.readyState>=2&&!v.seeking&&Math.abs(v.currentTime-t)<.2;},before);
  assert.equal(await page.locator('video').evaluate(v=>v.playbackRate),1.5);
  assert(await page.locator('textarea').isDisabled());
  mkdirSync(output,{recursive:true});await page.screenshot({path:path.join(output,'transcript-reader.png'),fullPage:true});
  const result={date:new Date().toISOString(),browser:browser.version(),first_publication_preserved_player:true,
    initial_position:555.123,initial_rate:1.25,checks,restored_time:before,restored_rate:1.5,video_questions:'pending VI5–VI7'};
  writeFileSync(path.join(output,'browser-verification.json'),JSON.stringify(result,null,2));console.log(JSON.stringify(result,null,2));
}finally{await browser.close();}
