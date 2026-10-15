// Exercise the installed server and built Reader with bundled media dependencies.
import assert from 'node:assert/strict';
import {createRequire} from 'node:module';
import {spawn} from 'node:child_process';
import {copyFileSync,cpSync,mkdirSync,readFileSync,writeFileSync,openSync,readSync,closeSync,statSync} from 'node:fs';
import path from 'node:path';
const [serverArg,webArg,bookArg,installArg,videoResourcesArg]=process.argv.slice(2);
if(!installArg)throw new Error('Usage: vi18-reader.mjs SERVER WEB_DIST PUBLISHED_BOOK INSTALL_DIR [VIDEO_RESOURCES]');
const require=createRequire(new URL('../../packages/web/package.json',import.meta.url));
const {chromium}=require('@playwright/test');
const install=path.resolve(installArg),book=path.resolve(bookArg),out=path.join(install,'reader-verification');mkdirSync(out,{recursive:true});
const executable=path.join(install,'server.exe');copyFileSync(path.resolve(serverArg),executable);
const web=path.join(install,'resources/web-dist');cpSync(path.resolve(webArg),web,{recursive:true});
if(videoResourcesArg)cpSync(path.resolve(videoResourcesArg),path.join(install,'resources/video'),{recursive:true});
const manifest=JSON.parse(readFileSync(path.join(book,'video_source_manifest.json'),'utf8'));
const publication=JSON.parse(readFileSync(path.join(book,'publication.json'),'utf8')).reference;
const env={...process.env,PATH:process.env.SystemRoot+'\\System32',FFMPEG:'',FFPROBE:'',UNDERSTAND_BOOK_ADDR:'127.0.0.1:0',
  UNDERSTAND_BOOK_WEB_DIST:web,UNDERSTAND_BOOK_LIBRARY_ROOT:path.join(out,'library'),UNDERSTAND_BOOK_PRIVATE_DIR:path.join(out,'private'),UNDERSTAND_BOOK_MEMORY_DIR:path.join(out,'memory')};
const child=spawn(executable,[book],{cwd:install,env,windowsHide:true,stdio:['ignore','pipe','pipe']});
let log='',browser,page;
const requests=[],errors=[],started=Date.now();
child.stdout.on('data',data=>log+=data);child.stderr.on('data',data=>log+=data);
try{
  const origin=await new Promise((resolve,reject)=>{
    const timer=setTimeout(()=>{clearInterval(poll);reject(new Error('Installed Reader server did not become ready'));},60000);
    const poll=setInterval(()=>{const found=log.match(/server listening at (http:\/\/[^\s]+)/);if(found){clearTimeout(timer);clearInterval(poll);resolve(found[1]);}},100);
    child.once('error',reject);child.once('exit',code=>{clearTimeout(timer);clearInterval(poll);reject(new Error(`Reader exited ${code}: ${log}`));});
  });
  browser=await chromium.launch({channel:'msedge',headless:true});
  page=await browser.newPage({viewport:{width:1440,height:1000}});page.on('pageerror',error=>errors.push(error.message));
  page.on('response',response=>requests.push({elapsed_ms:Date.now()-started,url:response.url(),status:response.status()}));
  page.on('requestfailed',request=>requests.push({elapsed_ms:Date.now()-started,url:request.url(),error:request.failure()}));
  await page.goto(origin);await page.waitForFunction(()=>{const v=document.querySelector('.video-reader video');return v?.readyState>=2&&v.videoWidth===1920;},undefined,{timeout:120000});
  const initial_ready_ms=Date.now()-started;
  const positions=[],duration=manifest.source.duration_ms/1000;
  for(const seconds of (duration>4620?[555.123,1200,4620]:[1,4,duration-1])){
    await page.locator('.video-reader video').evaluate((v,t)=>{v.currentTime=t;},seconds);
    await page.waitForFunction(t=>{const v=document.querySelector('.video-reader video');return !v.seeking&&v.readyState>=2&&Math.abs(v.currentTime-t)<.2;},seconds);
    positions.push(await page.locator('.video-reader video').evaluate(v=>({time:v.currentTime,width:v.videoWidth,height:v.videoHeight})));
  }
  const start=duration>600?555.123:1;
  await page.locator('.video-reader video').evaluate(async(v,t)=>{v.currentTime=t;v.muted=true;v.playbackRate=1.5;await v.play();},start);
  await page.waitForFunction(t=>document.querySelector('.video-reader video').currentTime>t+1,start);
  const paused=await page.locator('.video-reader video').evaluate(v=>{v.pause();return v.currentTime;});
  await page.reload();await page.waitForFunction(t=>{const v=document.querySelector('.video-reader video');return v?.readyState>=2&&!v.seeking&&Math.abs(v.currentTime-t)<.2;},paused);
  assert.equal(await page.locator('.video-reader video').evaluate(v=>v.playbackRate),1.5);
  const route=await page.locator('.video-reader video').evaluate(v=>v.currentSrc);
  const range=await fetch(route,{headers:{Range:'bytes=100-131'}});assert.equal(range.status,206);
  const expected=Buffer.alloc(32),fd=openSync(path.join(book,manifest.source.imported_path),'r');readSync(fd,expected,0,32,100);closeSync(fd);
  assert.deepEqual(Buffer.from(await range.arrayBuffer()),expected);
  const head=await fetch(route,{method:'HEAD'});assert.equal(Number(head.headers.get('content-length')),statSync(path.join(book,manifest.source.imported_path)).size);
  assert.deepEqual(errors,[]);await page.screenshot({path:path.join(out,'reader.png'),fullPage:true});
  const result={status:'passed',at:new Date().toISOString(),platform:process.platform,browser:browser.version(),publication,installed_server:executable,
    built_web:web,system_only_path:env.PATH,duration_ms:manifest.source.duration_ms,initial_ready_ms,positions,paused_at:paused,playback_rate_restored:1.5,exact_original_byte_range:true,head_length:head.headers.get('content-length'),page_errors:errors};
  writeFileSync(path.join(out,'verification.json'),JSON.stringify(result,null,2)+'\n');console.log(JSON.stringify(result));
}catch(error){
  if(page){await page.screenshot({path:path.join(out,'failure.png'),fullPage:true});writeFileSync(path.join(out,'failure.txt'),await page.locator('body').innerText());}
  throw error;
}finally{writeFileSync(path.join(out,'server.log'),log);writeFileSync(path.join(out,'requests.json'),JSON.stringify({errors,requests},null,2));await browser?.close();child.kill();}
