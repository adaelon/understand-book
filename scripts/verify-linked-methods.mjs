// Independent formulas + actual SVG projections, exercised inside the shipping
// opaque Reader iframe. No model providers or external services are used.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {createRequire} from 'node:module';
const root=path.resolve(path.dirname(fileURLToPath(import.meta.url)),'..');
const web=createRequire(path.join(root,'packages/web/package.json'));
const {chromium}=web('@playwright/test');
const {build}=createRequire(web.resolve('vite/package.json'))('esbuild');
const catalog=JSON.parse(fs.readFileSync(path.join(root,'skills/presentation/methods/catalog.json'),'utf8'));
const selected=new Set(process.argv.slice(2)),methods=catalog.methods.filter(m=>m.id!=='spatial-linked-views'&&(!selected.size||selected.has(m.id)));
const output=path.resolve(process.env.LINKED_OUTPUT??path.join(root,'tmp/linked-methods-verification'));fs.mkdirSync(output,{recursive:true});
const assembly=await build({entryPoints:[path.join(root,'packages/web/src/presentation-document.ts')],bundle:true,format:'iife',globalName:'ProductionDocument',write:false,plugins:[{name:'raw',setup(b){b.onResolve({filter:/\?raw$/},a=>({path:path.resolve(a.resolveDir,a.path.slice(0,-4)),namespace:'raw'}));b.onLoad({filter:/.*/,namespace:'raw'},a=>({contents:fs.readFileSync(a.path,'utf8'),loader:'text'}));}}]});
const browser=await chromium.launch({channel:process.env.METHOD_LIBRARY_BROWSER??'msedge',headless:true});
const report={status:'running',geometry_cases:0,reopens:0,exports:0,contexts:[],provider_calls:0};let serial=0;
const close=(a,b,label)=>assert.ok(Math.abs(a-b)<1e-6,`${label}: ${a} != ${b}`);
const seek=(f,q)=>f.evaluate(q=>window.presentationScene.seek({semantic_state:Math.floor(q),transition_progress:q-Math.floor(q)}),q);
const snap=f=>f.evaluate(()=>window.presentationScene.snapshot());
const model=f=>f.evaluate(()=>window.explanationDemo.inspect());
const number=async(f,sel,attr)=>Number(await f.locator(sel).getAttribute(attr));
const drawing=f=>f.evaluate(()=>({scene:document.querySelector('#scene').innerHTML,detail:document.querySelector('#detail').innerHTML,readout:document.querySelector('#readout').textContent,reading:document.querySelector('#detail-reading').textContent}));
async function parameter(f,key,v){await f.locator('#'+key).evaluate((n,v)=>{if(n.type==='checkbox')n.checked=v;else n.value=String(v);n.dispatchEvent(new Event('input',{bubbles:true}));n.dispatchEvent(new Event('change',{bubbles:true}));},v);}
async function mount(context,m,restored=null){
 const page=await context.newPage(),errors=[],requests=[];page.on('pageerror',e=>errors.push(e.message));page.on('request',r=>{if(/^https?:/.test(r.url()))requests.push(r.url());});
 await page.setContent('<body style="margin:0"><iframe sandbox="allow-scripts" style="border:0;display:block;width:100%;height:100vh"></iframe></body>');await page.addScriptTag({content:assembly.outputFiles[0].text});
 const base=path.join(root,m.root),metadata=JSON.parse(fs.readFileSync(path.join(base,'example/author.json'),'utf8'));
 const view={...metadata,entrypoint:'index.html',sources:[],restored_state:restored,content_files:{'index.html':fs.readFileSync(path.join(base,'example/index.html'),'utf8')}};
 await page.evaluate(view=>{window.messages=[];addEventListener('message',e=>window.messages.push(e.data));document.querySelector('iframe').srcdoc=ProductionDocument.presentationDocument(view,'linked-test');},view);
 await page.waitForFunction(()=>window.messages.some(m=>m.kind==='ready'));
 const frame=page.frames()[1];await frame.waitForFunction(()=>window.explanationDemo);return {page,frame,errors,requests};
}
async function message(page,kind){const id='linked-'+ ++serial;await page.evaluate(({kind,id})=>document.querySelector('iframe').contentWindow.postMessage({channel:'linked-test',kind,request_id:id},'*'),{kind,id});await page.waitForFunction(id=>window.messages.some(m=>m.request_id===id),id);const result=await page.evaluate(id=>window.messages.find(m=>m.request_id===id),id);assert.notEqual(result.kind,'static-error',result.message);return result;}
async function geometry(f,id){
 if(id==='discrete-update'){
  for(const eta of [0,.25,.5,.75,1,1.2]){await parameter(f,'eta',eta);for(const q of [0,2.35,7.999,8]){await seek(f,q);const k=Math.floor(q),p=q-k,values=Array.from({length:9},(_,i)=>2+2*(1-2*eta)**i),x=values[k]+(values[Math.min(k+1,8)]-values[k])*p,lo=Math.min(0,...values),hi=Math.max(4,...values),margin=(hi-lo)*.12,xmin=lo-margin,xmax=hi+margin,ymax=Math.max((xmin-2)**2,(xmax-2)**2);close(await number(f,'#moving-point','cx'),64+(x-xmin)/(xmax-xmin)*592,'parabola x');close(await number(f,'#moving-point','cy'),324-(x-2)**2/ymax*265,'parabola y');const errorMax=Math.max(2,...values.map(x=>Math.abs(x-2)));close(await number(f,'#error-point','cy'),146-(x-2)/errorMax*106,'linked error');report.geometry_cases++;}}
 }else if(id==='object-transform'){
  for(const s of [-1,0,.8,1])for(const q of [0,.35,1]){await parameter(f,'shear',s);await seek(f,q);const pts=(await f.locator('#shape').getAttribute('points')).split(' ').map(p=>p.split(',').map(Number));const area=Math.abs(pts.reduce((a,p,i)=>a+p[0]*pts[(i+1)%4][1]-p[1]*pts[(i+1)%4][0],0)/2);close(area,22500,'actual polygon area');close(pts[2][0],400+150*s*q,'mapped corner');close(await number(f,'#basis-end','cx'),230+140*s*q,'linked basis');close((await model(f)).perimeter,2+2*Math.sqrt(1+(s*q)**2),'perimeter');report.geometry_cases++;}
 }else if(id==='compare-one-factor'){
  for(const speed of [0,1,2,3])for(const q of [0,.35,1]){await parameter(f,'speed',speed);await seek(f,q);close(await number(f,'#runner-a','cx'),80+180*q,'A lane');close(await number(f,'#runner-b','cx'),80+180*speed*q,'B lane');close(await number(f,'#distance-b','cy'),250-70*speed*q,'B curve');close((await model(f)).gap,(speed-1)*10*q,'gap');report.geometry_cases++;}
 }else if(id==='part-whole'){
  for(const share of [0,1,40,99,100])for(const q of [0,.35,1]){await parameter(f,'share',share);await seek(f,q);const widths=await Promise.all(['a','b','c'].map(c=>number(f,'#part-'+c,'width')));close(widths.reduce((a,b)=>a+b,0),520,'conserved total length');close(widths[0],5.2*share,'A length');close(widths[1],5.2*.6*(100-share),'B length');close(await number(f,'#part-c','y'),83+176*q,'split position');const colored=await f.locator('#detail rect').evaluateAll(ns=>ns.reduce((sum,n)=>sum+Number(n.getAttribute('width')),0));close(colored,2700+42,'100 cells plus 3 legend squares');report.geometry_cases++;}
 }else if(id==='process-reveal'){
  for(const [a,b,result] of [[1,4,9],[0,8,0],[3,2,18]]){await parameter(f,'increment',a);await parameter(f,'threshold',b);await seek(f,1);assert.match(await f.locator('#reading').textContent(),/筛选尚未完成/);await seek(f,3);assert.equal((await model(f)).sum,result);assert.match(await f.locator('#reading').textContent(),new RegExp(''+result));assert.ok((await f.locator('#scene').textContent()).includes(String(result)));const before=(await model(f));await f.locator('#scene').press('End');assert.equal((await model(f)).sum,before.sum);assert.deepEqual((await model(f)).filtered,before.filtered);report.geometry_cases++;}
 }else if(id==='evidence-chain'){
  for(const controlled of [true,false]){await parameter(f,'controlled',controlled);await seek(f,0);assert.equal(await f.locator('#mean-a').count(),0);await seek(f,3);close(await number(f,'#mean-a','x1'),145+11*36,'actual A mean');close(await number(f,'#mean-b','x1'),145+9*36,'actual B mean');assert.match(await f.locator('#reading').textContent(),controlled?/尚不能推出/:/不能将差异归因/);close((await model(f)).difference,2,'observed difference unchanged');report.geometry_cases++;}
 }else if(id==='chart-reveal'){
  for(const [dataset,values] of [['positive',[12,18,15,24]],['signed',[-6,0,12,24]],['zero',[0,6,12,24]]])for(const q of [0,.35,1,3]){await parameter(f,'dataset',dataset);await seek(f,q);for(let i=0;i<4;i++){const h=Math.abs(values[i])*7*Math.min(q,1);close(await number(f,'#bar-'+i,'height'),h,'bar height');close(await number(f,'#bar-'+i,'y'),values[i]>=0?272-h:272,'zero baseline');}if(q===3)assert.match(await f.locator('#reading').textContent(),dataset==='positive'?/2 倍.*100%/:/不计算增长倍数/);report.geometry_cases++;}
 }else if(id==='scale-zoom'){
  for(const focus of [-.001,0,.0005,.001])for(const q of [0,.35,1,2,2.999,3]){await parameter(f,'focus',focus);await seek(f,q);close(await number(f,'#target','width'),.6*10**q,'physical target width');close(await number(f,'#target','x'),360+(-.0005-focus)*600*10**q,'fixed world target');close(await number(f,'#viewport-box','width'),580/10**q,'overview window');close(await number(f,'#focus-marker','x1'),320+focus/.005*580,'linked focus');report.geometry_cases++;}
 }else throw new Error('Missing independent oracle '+id);
}
async function drag(page,frame,id,from,to){
 await frame.locator('#'+id).scrollIntoViewIfNeeded();
 const coords=await frame.locator('#'+id).evaluate((svg,points)=>points.map(([x,y])=>{const p=new DOMPoint(x,y).matrixTransform(svg.getScreenCTM());return {x:p.x,y:p.y};}),[from,to]);
 await page.mouse.move(coords[0].x,coords[0].y);await page.mouse.down();await page.mouse.move(coords[1].x,coords[1].y,{steps:5});await page.mouse.up();
}
try{
 for(const method of methods){
  for(const file of method.files)assert.ok(fs.statSync(path.join(root,method.root,file)).isFile());
  for(const viewport of [{width:960,height:720},{width:320,height:420},{width:640,height:240}]){
   const context=await browser.newContext({viewport,hasTouch:viewport.width!==960,reducedMotion:'reduce'}),{page,frame,errors,requests}=await mount(context,method);
   const max=Number(await frame.locator('#position').getAttribute('max'));assert.equal((await snap(frame)).playing,false);
   if(viewport.width===960)await geometry(frame,method.id);
   await seek(frame,.35);const stable=await drawing(frame);await seek(frame,0);await seek(frame,.35);assert.deepEqual(await drawing(frame),stable,'repeat seek actual SVG and text');
   await seek(frame,0);await frame.locator('#next').click();assert.equal((await snap(frame)).semantic_state,1);await frame.locator('#back').click();assert.equal((await snap(frame)).semantic_state,0);
   await frame.locator('#play').click();await frame.waitForFunction(()=>window.presentationScene.snapshot().transition_progress>.03);await frame.locator('#play').click();assert.equal((await snap(frame)).playing,false);
   await frame.locator('#position').press('End');assert.equal((await snap(frame)).semantic_state,max);assert.equal(await frame.locator('#play').isDisabled(),true);await frame.locator('#position').press('Home');assert.equal((await snap(frame)).semantic_state,0);
   await frame.locator('#scene').press('Home');await seek(frame,.35);const before=await snap(frame);await drag(page,frame,'scene',[200,100],[470,270]);assert.notDeepEqual(await snap(frame),before,'primary diagram direct manipulation');
   await frame.locator('#scene').press('Home');await frame.locator('#scene').press('ArrowRight');assert.equal((await snap(frame)).playing,false);
   await drag(page,frame,'detail',[100,80],[450,180]);assert.equal((await snap(frame)).playing,false);
   await frame.locator('[data-chapter="1"]').click();assert.ok((await snap(frame)).semantic_state+(await snap(frame)).transition_progress>0,'guided observation works');
   await seek(frame,max-.001);const expected=await drawing(frame),state=await snap(frame),saved=await message(page,'snapshot');
   const reopened=await mount(context,method,saved.state);assert.deepEqual(await snap(reopened.frame),state,'state restoration');assert.deepEqual(await drawing(reopened.frame),expected,'actual drawing restoration');report.reopens++;
   const exported=await message(page,'export-static');assert.equal(exported.kind,'static-result');assert.deepEqual(await snap(frame),state,'export must not change state');
   const exportedIds=await page.evaluate(svg=>{const d=new DOMParser().parseFromString(svg,'image/svg+xml');return ['scene','detail','readout'].map(id=>!!d.querySelector('#'+id));},exported.image.svg);assert.deepEqual(exportedIds,[true,true,true],'both diagrams and readable explanation exported');report.exports++;
   assert.equal(await frame.evaluate(()=>document.documentElement.scrollWidth>innerWidth+1),false,'horizontal overflow');
   const small=await frame.locator('button,input,select').evaluateAll(ns=>ns.filter(n=>n.getBoundingClientRect().width<43.9||n.getBoundingClientRect().height<43.9).map(n=>({id:n.id,width:n.getBoundingClientRect().width,height:n.getBoundingClientRect().height})));assert.deepEqual(small,[],'touch target size');
   if(viewport.width!==640){await seek(frame,method.id==='discrete-update'?2.35:method.id==='object-transform'?.85:method.id==='compare-one-factor'?.7:max);for(const [suffix,sel] of [['','header'],['-scene','.experiment'],['-linked','.linked']]){await frame.locator(sel).evaluate(n=>scrollTo(0,n.getBoundingClientRect().top+scrollY-8));await page.screenshot({path:path.join(output,`${method.id}-${viewport.width}${suffix}.png`)});}}
   assert.deepEqual(errors,[]);assert.deepEqual(reopened.errors,[]);assert.deepEqual(requests,[]);assert.deepEqual(reopened.requests,[]);report.contexts.push({id:method.id,revision:method.revision,viewport,status:'passed'});await context.close();
  }
  console.log('PASS',method.id+'@'+method.revision);
 }
 report.status='passed';
}catch(e){report.status='failed';report.error=e.stack;throw e;}finally{fs.writeFileSync(path.join(output,'report.json'),JSON.stringify(report,null,2)+'\n');await browser.close();}
console.log(JSON.stringify({status:report.status,geometry_cases:report.geometry_cases,reopens:report.reopens,exports:report.exports,output}));
