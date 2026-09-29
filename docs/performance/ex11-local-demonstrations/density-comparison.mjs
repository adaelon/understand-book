import fs from 'node:fs';
import path from 'node:path';
import {createRequire} from 'node:module';
const require=createRequire(new URL('../../../packages/web/package.json',import.meta.url));
const {chromium}=require('@playwright/test');
const css=fs.readFileSync(new URL('../../../packages/web/src/presentation.css',import.meta.url),'utf8');
const output=path.resolve(process.argv[4]);fs.mkdirSync(output,{recursive:true});
const browser=await chromium.launch({headless:true});
const observations=[];
try {
  for(const [label,file] of [['before',process.argv[2]],['after',process.argv[3]]]){
    const content=JSON.parse(fs.readFileSync(file,'utf8'));
    let html=content.content_files[content.entrypoint];
    for(const [name,source] of Object.entries(content.content_files))if(name.endsWith('.js'))html=html.replace(`<script data-presentation-library="konva" src="${name}"></script>`,()=>`<script>${source}</script>`);
    for(const viewport of [{width:960,height:720},{width:320,height:420},{width:640,height:240}]){
      const page=await browser.newPage({viewport});
      await page.evaluate(initialState=>{window.presentation={initialState,registerStateReader:()=>{},registerStateRestorer:()=>{},commitState:()=>{}};},content.initial_state);
      await page.setContent(`<style>${css}</style>${html}`);
      await page.locator('#eta').evaluate(n=>{n.value='.8';n.dispatchEvent(new Event('input',{bubbles:true}));});
      await page.evaluate(()=>window.presentationScene.seek({semantic_state:2,transition_progress:0}));
      await page.screenshot({path:path.join(output,`${label}-${viewport.width}.png`)});
      observations.push({label,viewport,eta:.8,actual:await page.evaluate(()=>window.presentationScene.snapshot()),
        firstGraphicY:await page.locator('canvas').first().evaluate(n=>n.getBoundingClientRect().y)});
      await page.close();
    }
  }
}finally{await browser.close();}
fs.writeFileSync(path.join(output,'observations.json'),JSON.stringify(observations,null,2));
console.log(JSON.stringify(observations,null,2));
