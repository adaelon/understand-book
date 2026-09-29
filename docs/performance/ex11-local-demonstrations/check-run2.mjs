// Diagnose the untouched second model draft, including actual render and saved-frame loss.
import fs from 'node:fs';
import path from 'node:path';
import {createRequire} from 'node:module';
const require=createRequire(new URL('../../../packages/web/package.json',import.meta.url));
const {chromium}=require('@playwright/test');
const root=new URL('./ex11.2-run2/',import.meta.url);
const content=JSON.parse(fs.readFileSync(new URL('candidate-content.json',root),'utf8'));
let html=content.content_files[content.entrypoint];
for(const [file,source] of Object.entries(content.content_files)) {
  if(file.endsWith('.js'))html=html.replace(`<script data-presentation-library="konva" src="${file}"></script>`,()=>`<script>${source}</script>`);
}
const css=fs.readFileSync(new URL('../../../packages/web/src/presentation.css',import.meta.url),'utf8');
const browser=await chromium.launch({headless:true});
const records=[];
try {
  for(const viewport of [{width:960,height:720},{width:320,height:420},{width:640,height:240}]) {
    const page=await browser.newPage({viewport});
    await page.evaluate(initialState=>{
      window.presentation={initialState,registerStateReader:f=>window.readState=f,registerStateRestorer:f=>window.restoreState=f,commitState:()=>{}};
    },content.initial_state);
    await page.setContent(`<style>${css}</style>${html}`);
    await page.locator('#eta').evaluate(n=>{n.value='.8';n.dispatchEvent(new Event('input',{bubbles:true}));});
    await page.evaluate(()=>window.presentationScene.seek({semantic_state:2,transition_progress:.5}));
    const before=await page.evaluate(()=>({scene:window.presentationScene.snapshot(),saved:window.readState(),
      firstGraphicY:document.querySelector('canvas').getBoundingClientRect().y,
      draggable:window.Konva.stages.flatMap(s=>s.find(n=>n.draggable())).length,
      text:document.querySelector('#readout').innerText}));
    await page.screenshot({path:path.join(root.pathname.replace(/^\/(\w:)/,'$1'),`density-${viewport.width}-eta0.8-k2-p0.5.png`)});
    await page.evaluate(saved=>{
      window.presentationScene.seek({semantic_state:0,transition_progress:0});
      window.restoreState({values:{page:saved.values},visible_step:saved.visible_step});
    },before.saved);
    const after=await page.evaluate(()=>window.presentationScene.snapshot());
    records.push({viewport,before,after,restored:JSON.stringify(before.scene)===JSON.stringify(after)});
    await page.close();
  }
} finally {await browser.close();}
fs.writeFileSync(new URL('independent-check.json',root),JSON.stringify(records,null,2));
console.log(JSON.stringify(records.map(({viewport,before,after,restored})=>({viewport,firstGraphicY:before.firstGraphicY,saved:before.saved,after,restored})),null,2));
