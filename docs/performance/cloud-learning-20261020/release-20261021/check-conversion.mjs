import fs from 'node:fs';
import assert from 'node:assert/strict';
import {getDocument} from 'pdfjs-dist/legacy/build/pdf.mjs';
import {createCanvas} from '@napi-rs/canvas';
const root='tmp/cloud-release-20261021';const results=[];
for(const format of ['pptx','ppt']){
 const task=getDocument({data:new Uint8Array(fs.readFileSync(`${root}/converted26-${format}.pdf`)),useSystemFonts:true});
 const d=await task.promise;assert.equal(d.numPages,3);
 const pages=[];
 for(let i=1;i<=d.numPages;i++){
  const page=await d.getPage(i);const text=(await page.getTextContent()).items.map(x=>x.str).join(' ');pages.push(text);
  if(i>=2){const viewport=page.getViewport({scale:1.5});const canvas=createCanvas(viewport.width,viewport.height);await page.render({canvasContext:canvas.getContext('2d'),viewport,canvas}).promise;fs.writeFileSync(`${root}/${format}-page${i}.png`,canvas.toBuffer('image/png'));}
 }
 assert.match(pages[1],/x²/);assert.match(pages[1],/2\s+5/);assert.match(pages[2],/hidden/i);
 results.push({format,pages:d.numPages,text:pages});await task.destroy();
}
const result={status:'passed',converter:'LibreOffice 26.8.1.1',results};fs.writeFileSync(`${root}/conversion-check.json`,JSON.stringify(result,null,2));console.log(JSON.stringify(result,null,2));
