import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
const root=path.dirname(fileURLToPath(import.meta.url));
const runs=['batch3/runs/B1','batch3/runs/C1','runs/B1','runs/C1','batch2/runs/B1'];
const items=[];
for(const relative of runs){
  const name=path.basename(relative),label=`${relative.startsWith('batch3')?'第三批':relative.startsWith('batch2')?'第二批':'第一批'} ${name}`;
  const dir=path.join(root,relative),viewPath=path.join(dir,'view.json');
  const delivered=fs.existsSync(viewPath);
  const diagnosticPath=path.join(dir,'diagnostic-content.json');
  if(!delivered&&!fs.existsSync(diagnosticPath)){items.push({name,label,status:JSON.parse(fs.readFileSync(path.join(dir,'verdict.json'),'utf8')).label,file:null});continue}
  const view=JSON.parse(fs.readFileSync(delivered?viewPath:diagnosticPath,'utf8'));
  const config=JSON.stringify(view.initial_state).replaceAll('<','\\u003c');
  const bootstrap=`<script>window.presentation={initialState:${config},registerStateReader(fn){window.ex10Read=fn},registerStateRestorer(fn){window.ex10Restore=fn},commitState(){}};</script>`;
  const original=view.content_files[view.entrypoint];
  const html=original.replace(/<head\b[^>]*>/i,match=>match+bootstrap);
  fs.writeFileSync(path.join(dir,'standalone.html'),html===original?bootstrap+original:html);
  const verdictPath=path.join(dir,'verdict.json');
  const status=fs.existsSync(verdictPath)?JSON.parse(fs.readFileSync(verdictPath,'utf8')).label:delivered?'已交付，待验收':'未交付；原始候选诊断预览';
  items.push({name,label,status,file:`${relative}/standalone.html`});
}
const escape=s=>s.replaceAll('&','&amp;').replaceAll('<','&lt;').replaceAll('"','&quot;');
fs.writeFileSync(path.join(root,'index.html'),`<!doctype html><html lang="zh-CN"><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>学习率：SVG / Konva 对照</title>
<style>body{margin:0;padding:24px;background:#f7f6f2;color:#203044;font:16px/1.6 system-ui}h1{margin:0;font-size:26px}p{max-width:900px}.grid{display:grid;grid-template-columns:repeat(auto-fit,minmax(min(100%,440px),1fr));gap:20px}section{background:white;border:1px solid #d6dbe1;border-radius:14px;overflow:hidden}header{padding:12px 18px}h2{margin:0;font-size:20px}.status{color:#8c4028}iframe{width:100%;height:760px;border:0}a{color:#185d9a}</style>
<h1>学习率：SVG / Konva 制作对照</h1><p>每个页面来自独立的 Resident 制作会话。拖动学习率，看图形与数值如何响应；也可以播放、暂停、定位和回退。页面保留 Agent 原稿，验收结果见各卡片。</p>
<div class="grid">${items.map(item=>`<section><header><h2>${escape(item.label)} · ${item.name.startsWith('B')?'原生 SVG':'Konva 10.7.0'}</h2><span class="status">${escape(item.status)}</span>${item.file?` · <a href="${item.file}" target="_blank">单独打开</a>`:''}</header>${item.file?`<iframe title="${item.name}" sandbox="allow-scripts" src="${item.file}"></iframe>`:'<p>本次没有可交付页面；原始响应与失败原因已保留。</p>'}</section>`).join('')}</div>
<p>这里提供独立操作预览；Reader 的保存、重开与追问验收单独记录。<a href="../explorable-explanation-ex10-svg-konva-batch3-20260928.md">完整报告</a> · 该实验尚未测量真实读者的学习收益。</p></html>`);
console.log(JSON.stringify(items));
