import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import {createRequire} from 'node:module';
import {fileURLToPath} from 'node:url';
const root=path.resolve(path.dirname(fileURLToPath(import.meta.url)),'..');
const web=createRequire(path.join(root,'packages/web/package.json'));
const {chromium}=web('@playwright/test');
const {build}=createRequire(web.resolve('vite/package.json'))('esbuild');
const base=path.join(root,'skills/presentation/methods/spatial-linked-views/v1');
const output=path.resolve(process.env.SPATIAL_OUTPUT??path.join(root,'tmp/spatial-method-verification'));
fs.mkdirSync(output,{recursive:true});
// Use the shipping document assembly, CSP, opaque iframe, state bridge and export.
const assembly=await build({entryPoints:[path.join(root,'packages/web/src/presentation-document.ts')],bundle:true,format:'iife',globalName:'ProductionDocument',write:false,
  plugins:[{name:'raw',setup(b){b.onResolve({filter:/\?raw$/},args=>({path:path.resolve(args.resolveDir,args.path.slice(0,-4)),namespace:'raw'}));b.onLoad({filter:/.*/,namespace:'raw'},args=>({contents:fs.readFileSync(args.path,'utf8'),loader:'text'}));}}]});
const three=fs.readFileSync(path.join(root,'assets/presentation/three-0.180.0/three.min.js'),'utf8');
const browser=await chromium.launch({channel:process.env.METHOD_LIBRARY_BROWSER??'msedge',headless:true});
const report={status:'running',geometry_cases:0,reopens:0,exports:0,topics:[],provider_calls:0};
let serial=0;
const close=(a,b,label)=>assert.ok(Math.abs(a-b)<1e-7,`${label}: ${a} != ${b}`);
const seek=(f,q)=>f.evaluate(q=>window.presentationScene.seek({semantic_state:Math.floor(q),transition_progress:q-Math.floor(q)}),q);
const snapshot=f=>f.evaluate(()=>window.presentationScene.snapshot());
async function parameter(f,key,value){await f.locator('#'+key).evaluate((n,v)=>{n.value=String(v);n.dispatchEvent(new Event('input',{bubbles:true}));n.dispatchEvent(new Event('change',{bubbles:true}));},value);}
async function mount(context,topic,restored=null,noWebGL=false){
  const page=await context.newPage(),errors=[],requests=[];
  page.on('pageerror',e=>errors.push(e.message));page.on('request',r=>{if(/^https?:/.test(r.url()))requests.push(r.url());});
  await page.addInitScript(()=>{
    window.canvasCalls={};
    for(const name of ['clearRect','arc','fillRect']){const original=CanvasRenderingContext2D.prototype[name];CanvasRenderingContext2D.prototype[name]=function(...args){
      const id=this.canvas.id;if(id==='diagram'||id==='detail'){if(name==='clearRect')window.canvasCalls[id]=[];else(window.canvasCalls[id]??=[]).push({name,args});}
      return original.apply(this,args);
    };}
  });
  if(noWebGL)await page.addInitScript(()=>{const get=HTMLCanvasElement.prototype.getContext;HTMLCanvasElement.prototype.getContext=function(type,...args){return type.startsWith('webgl')?null:get.call(this,type,...args);};});
  await page.setContent('<body style="margin:0"><iframe sandbox="allow-scripts" style="border:0;display:block;width:100%;height:100vh"></iframe></body>');
  await page.addScriptTag({content:assembly.outputFiles[0].text});
  const file=topic==='gear'?'example/index.html':'adaptations/spring.html';
  const metadata=JSON.parse(fs.readFileSync(path.join(base,topic==='gear'?'example/author.json':'adaptations/spring-author.json'),'utf8'));
  const view={...metadata,entrypoint:'index.html',sources:[],restored_state:restored,
    content_files:{'index.html':fs.readFileSync(path.join(base,file),'utf8'),'libraries/three-0.180.0.min.js':three}};
  await page.evaluate(view=>{window.messages=[];addEventListener('message',e=>window.messages.push(e.data));document.querySelector('iframe').srcdoc=ProductionDocument.presentationDocument(view,'spatial-test');},view);
  await page.waitForFunction(()=>window.messages.some(m=>m.kind==='ready'),{},{timeout:20000});
  const frame=page.frames()[1];await frame.waitForFunction(()=>window.linkedDemo);
  return {page,frame,errors,requests};
}
async function message(page,kind){const id='request-'+ ++serial;await page.evaluate(({kind,id})=>document.querySelector('iframe').contentWindow.postMessage({channel:'spatial-test',kind,request_id:id},'*'),{kind,id});await page.waitForFunction(id=>window.messages.some(m=>m.request_id===id),id);const result=await page.evaluate(id=>window.messages.find(m=>m.request_id===id),id);assert.notEqual(result.kind,'static-error',result.message);return result;}
async function visual(page,frame,name){
  // Capture real scroll positions. Extending an offscreen iframe for a full-page
  // screenshot can omit painted text in Chromium's composited WebGL surface.
  for(const [suffix,selector] of [['','header'],['-diagrams','.diagrams'],['-reading','#readout']]){
    await frame.locator(selector).evaluate(n=>window.scrollTo(0,n.getBoundingClientRect().top+scrollY-12));
    await frame.evaluate(()=>new Promise(r=>requestAnimationFrame(()=>requestAnimationFrame(r))));
    await page.screenshot({path:path.join(output,name+suffix+'.png')});
  }
}
async function verifyModel(frame,topic){
  if(topic==='gear'){
    for(const teeth of [20,30,40,60]){await parameter(frame,'teeth',teeth);for(const q of [0,.25,.35]){
      await seek(frame,q);const {model:m,geometry:g}=await frame.evaluate(()=>window.linkedDemo.inspect());
      close(g.angles[0],2*Math.PI*q,'input mesh angle');close(g.angles[1],Math.PI-Math.PI/teeth-2*Math.PI*q*20/teeth,'output mesh angle');
      close(m.outputRpm,-12*20/teeth,'speed');close(m.outputTorque,2*teeth/20,'torque');close(g.centers[1]-g.centers[0],20+teeth,'center distance');report.geometry_cases++;
      const diagram=await frame.evaluate(()=>({w:document.querySelector('#diagram').clientWidth,h:document.querySelector('#diagram').clientHeight,arcs:window.canvasCalls.diagram.filter(c=>c.name==='arc'&&c.args[2]===3).map(c=>c.args)}));
      const scale=Math.min((diagram.w-44)/(2*(20+teeth)+4),(diagram.h-65)/(2*(teeth+2)));
      close(diagram.arcs[0][0],diagram.w/2-teeth*scale+Math.cos(2*Math.PI*q)*20*scale*.8,'2D input marker x');
      close(diagram.arcs[1][1],(diagram.h-35)/2-Math.sin(g.angles[1])*teeth*scale*.8,'2D output marker y');
    }}
    await parameter(frame,'rpm',0);assert.equal(await frame.locator('#play').isDisabled(),true);await parameter(frame,'torque',0);assert.equal((await frame.evaluate(()=>window.linkedDemo.inspect())).model.outputTorque,0);
    await parameter(frame,'rpm',12);await parameter(frame,'torque',2);await parameter(frame,'teeth',40);
  }else{
    for(const [mass,k,A] of [[1,12,.6],[.5,4,.8],[2,20,.8],[2,4,.05],[1,12,0],[.5,20,.3]]){
      await parameter(frame,'mass',mass);await parameter(frame,'stiffness',k);await parameter(frame,'amplitude',A);
      for(const q of [0,.25,.35,.5,.75]){
        await seek(frame,q);const {model:m,geometry:g}=await frame.evaluate(()=>window.linkedDemo.inspect());
        const x=A*Math.cos(2*Math.PI*q),v=-A*Math.sqrt(k/mass)*Math.sin(2*Math.PI*q);
        close(m.x,x,'displacement');close(m.velocity,v,'velocity');close(m.period,2*Math.PI*Math.sqrt(mass/k),'period');close(m.force,-k*x,'force');close(m.potential+m.kinetic,.5*k*A*A,'energy');
        close(g.massX,40*x,'actual mass mesh');close(g.coilEnd,g.massX-10,'actual coil endpoint');report.geometry_cases++;
        const diagram=await frame.evaluate(()=>({h:document.querySelector('#diagram').clientHeight,dot:window.canvasCalls.diagram.find(c=>c.name==='arc'&&c.args[2]===5).args,bars:window.canvasCalls.detail.filter(c=>c.name==='fillRect').map(c=>c.args)}));
        close(diagram.dot[1],(27+diagram.h-38)/2-x*(diagram.h-38-27)/2/.8,'2D displacement marker');
        close(diagram.bars[0][3],.5*k*x*x/6.4*145,'actual potential bar');close(diagram.bars[1][3],.5*mass*v*v/6.4*145,'actual kinetic bar');
      }
    }
    await parameter(frame,'mass',1);await parameter(frame,'stiffness',12);await parameter(frame,'amplitude',.6);
  }
}
try{
  for(const topic of ['gear','spring'])for(const viewport of [{width:960,height:720},{width:320,height:420},{width:640,height:240}]){
    const context=await browser.newContext({viewport,hasTouch:viewport.width!==960,reducedMotion:'reduce'});
    const {page,frame,errors,requests}=await mount(context,topic);
    assert.equal(await frame.evaluate(()=>window.linkedDemo.inspect().webgl),true,'actual WebGL required');
    assert.equal((await snapshot(frame)).playing,false);
    if(viewport.width===960)await verifyModel(frame,topic);
    await seek(frame,.35);const before=await frame.evaluate(()=>window.linkedDemo.inspect());await seek(frame,0);await seek(frame,.35);assert.deepEqual(await frame.evaluate(()=>window.linkedDemo.inspect()),before);
    await frame.locator('#front').click();const model=await frame.evaluate(()=>window.linkedDemo.inspect().model);await frame.locator('#spatial').press('ArrowRight');assert.deepEqual(await frame.evaluate(()=>window.linkedDemo.inspect().model),model);
    const bounds=await frame.locator('#spatial').boundingBox();await page.mouse.move(bounds.x+bounds.width*.45,bounds.y+bounds.height*.5);await page.mouse.down();await page.mouse.move(bounds.x+bounds.width*.58,bounds.y+bounds.height*.55,{steps:5});await page.mouse.up();assert.ok((await snapshot(frame)).yaw>.12);
    await frame.locator('#diagram').press('ArrowRight');close((await snapshot(frame)).transition_progress,.37,'direct diagram operation');
    await frame.locator('#play').click();await frame.waitForFunction(()=>window.presentationScene.snapshot().transition_progress>.39);await frame.locator('#play').click();assert.equal((await snapshot(frame)).playing,false);
    await parameter(frame,'explode',.7);assert.equal(await frame.locator('#play').isDisabled(),true);
    const saved=await message(page,'snapshot'),stable=await snapshot(frame);
    const reopened=await mount(context,topic,saved.state);assert.deepEqual(await snapshot(reopened.frame),stable);assert.deepEqual(await reopened.frame.evaluate(()=>window.linkedDemo.inspect()),await frame.evaluate(()=>window.linkedDemo.inspect()));report.reopens++;
    const exported=await message(page,'export-static');assert.equal(exported.kind,'static-result');assert.deepEqual(await snapshot(frame),stable);
    const images=await page.evaluate(svg=>[...new DOMParser().parseFromString(svg,'image/svg+xml').querySelectorAll('img')].map(n=>n.getAttribute('src')),exported.image.svg);assert.equal(images.length,3);
    const pixelProof=await frame.evaluate(async url=>{const img=new Image();img.src=url;await img.decode();const c=document.createElement('canvas');c.width=img.width;c.height=img.height;const ctx=c.getContext('2d');ctx.drawImage(img,0,0);const pixels=ctx.getImageData(0,0,c.width,c.height).data;let visible=0;const colors=new Set();for(let i=0;i<pixels.length;i+=4){if(pixels[i+3]>100){visible++;colors.add(`${pixels[i]},${pixels[i+1]},${pixels[i+2]}`);}}return {visible,colors:colors.size,width:c.width,height:c.height};},images[0]);
    assert.ok(pixelProof.visible>500&&pixelProof.colors>100,JSON.stringify(pixelProof));report.exports++;
    fs.writeFileSync(path.join(output,`${topic}-${viewport.width}-3d.png`),Buffer.from(images[0].split(',')[1],'base64'));
    fs.writeFileSync(path.join(output,`${topic}-${viewport.width}-state.json`),JSON.stringify(saved.state,null,2));
    await parameter(frame,'explode',0);await frame.locator('#view-reset').click();await seek(frame,.35);
    assert.equal(await frame.evaluate(()=>document.body.scrollWidth>innerWidth+1),false,'no horizontal overflow');
    await visual(page,frame,`${topic}-${viewport.width}`);
    assert.deepEqual(errors,[]);assert.deepEqual(reopened.errors,[]);assert.deepEqual(requests,[]);assert.deepEqual(reopened.requests,[]);
    report.topics.push({topic,viewport,pixelProof,status:'passed'});await context.close();
    console.log(`${topic} ${viewport.width}×${viewport.height}: geometry, interaction, reopen and export passed`);
  }
  const context=await browser.newContext({viewport:{width:960,height:720}});
  const unavailable=await mount(context,'spring',null,true);
  assert.equal(await unavailable.frame.locator('#fallback').isVisible(),true);
  await parameter(unavailable.frame,'amplitude',.8);await seek(unavailable.frame,.5);close((await unavailable.frame.evaluate(()=>window.linkedDemo.inspect())).model.x,-.8,'2D fallback model');assert.deepEqual(unavailable.errors,[]);await context.close();
  report.fallback='passed';report.status='passed';
  const captureSource=fs.readFileSync(path.join(root,'packages/web/src/presentation-static.js'),'utf8');
  const unsafe=await browser.newPage();await unsafe.setContent('<canvas width="64" height="64"></canvas>');
  await unsafe.addScriptTag({content:`window.freezeCanvas=(${captureSource});`});
  const discarded=await unsafe.evaluate(async()=>{await document.fonts.ready;const gl=document.querySelector('canvas').getContext('webgl2');gl.clearColor(1,0,0,1);gl.clear(gl.COLOR_BUFFER_BIT);try{window.freezeCanvas();return 'unexpected success';}catch(e){return e.message;}});
  assert.match(discarded,/preserveDrawingBuffer/);report.discarded_buffer='reported';await unsafe.close();
}finally{await browser.close();fs.writeFileSync(path.join(output,'report.json'),JSON.stringify(report,null,2)+'\n');}
