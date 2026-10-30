import { chromium, expect } from '@playwright/test';
import { readFile, writeFile } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';
import path from 'node:path';
const output=path.dirname(fileURLToPath(import.meta.url));
const origin=process.env.BOARD_VISUAL_ORIGIN??'http://127.0.0.1:4184';
const browser=await chromium.launch({headless:true});
const page=await browser.newPage({viewport:{width:1440,height:1000}});
try {
  await page.goto(`${origin}/@fs/${process.cwd().replaceAll('\\','/')}/docs/performance/board-appearance-20261028/preview.html`);
  await page.waitForFunction(()=>Boolean(window.boardAppearanceExample));
  const example=await page.evaluate(()=>window.boardAppearanceExample);
  const document={board_id:'appearance-preview',revision:1,title:'上下文状态',...example};
  const css=await readFile('packages/web/dist-board-appearance/board-preview/renderer.css','utf8');
  const js=await readFile('packages/web/dist-board-appearance/board-preview/renderer.js','utf8');
  const requests=[],errors=[];
  await page.route('http://board-preview.local/**',route=>{
    const url=new URL(route.request().url());requests.push(url.pathname);
    if(url.pathname==='/renderer.css')return route.fulfill({body:css,contentType:'text/css'});
    if(url.pathname==='/renderer.js')return route.fulfill({body:js,contentType:'application/javascript'});
    return route.fulfill({contentType:'text/html',body:`<!doctype html><html><head><meta charset="UTF-8"><link rel="stylesheet" href="/renderer.css"></head><body><div id="app"></div><script>window.__boardPreviewDocument=${JSON.stringify(document).replaceAll('<','\\u003c')};window.__boardPreviewCandidateItemIds=${JSON.stringify(document.items.map(item=>item.item_id))};</script><script src="/renderer.js"></script></body></html>`});
  });
  page.on('pageerror',error=>errors.push(error.message));
  await page.goto('http://board-preview.local/');
  await page.waitForFunction(()=>window.__boardPreviewReady===true);
  await expect(page.locator('[data-board-rich-ready="false"]')).toHaveCount(0);
  const result=await page.evaluate(()=>({inspection:window.__boardPreviewInspect(),math:document.querySelectorAll('.katex').length,fonts:[...document.fonts].filter(font=>font.family.startsWith('KaTeX')).map(font=>({family:font.family,status:font.status}))}));
  expect(result.inspection.issues).toEqual([]);expect(result.math).toBeGreaterThanOrEqual(5);
  expect(result.fonts.some(font=>font.status==='loaded')).toBe(true);
  expect(errors).toEqual([]);expect(requests.some(url=>/woff|ttf/.test(url))).toBe(false);
  await page.screenshot({path:path.join(output,'board-preview-isolated.png')});
  const mathItem=(id,text,x,width,height)=>({...example.items[1],item_id:id,title:id,position:{x,y:0},size:{width,height},target:{...example.items[1].target,graphic:{kind:'shape',shape:'rectangle',color:'#557da5',text}}});
  document.items=[
    mathItem('fraction',String.raw`归一化权重
$$\frac{e^{q_i^T k_j/\sqrt{d_k}}}{\sum_{m=1}^{n}e^{q_i^T k_m/\sqrt{d_k}}}$$`,0,260,140),
    mathItem('matrix',String.raw`状态矩阵
\[S_t=\begin{bmatrix}a_{11}&a_{12}\\a_{21}&a_{22}\end{bmatrix}\]`,330,260,140),
  ];document.links=[];
  await page.reload();await page.waitForFunction(()=>window.__boardPreviewReady===true);
  const complex=await page.evaluate(()=>window.__boardPreviewInspect());
  expect(complex.issues).toEqual([]);await expect(page.locator('.katex')).toHaveCount(2);
  await page.screenshot({path:path.join(output,'board-complex-equations.png')});
  await writeFile(path.join(output,'isolated-verification.json'),JSON.stringify({result,complex,requests,errors},null,2));
  console.log(JSON.stringify({issues:result.inspection.issues,complexIssues:complex.issues,math:result.math,loadedMathFonts:result.fonts.filter(font=>font.status==='loaded').length,requests,errors},null,2));
}finally{await browser.close();}
