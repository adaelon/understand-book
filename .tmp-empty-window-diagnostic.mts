import {readFileSync, readdirSync, writeFileSync} from 'node:fs';
import { createRequire } from 'node:module';
import path from 'node:path';
import {loadBookSource} from './packages/core/src/book-source.ts';
import {splitWindows} from './packages/core/src/window.ts';
import {xhtmlToBlocks} from './packages/core/src/epub-adapter.ts';
const require = createRequire(new URL('./packages/core/package.json', import.meta.url));
const {unzipSync,strFromU8}=require('fflate');
const {parse}=require('node-html-parser');
const epub='E:/allwork/download/失败的逻辑 事情因何出错，世间有无妙策.epub';
const dir='E:/allwork/download/agent/understand-book/.understand-book/logic-of-failure/.build/automatic-build/v3/artifacts/pass1/pass1-window.full.v1';
const loaded=loadBookSource(epub), windows=splitWindows(loaded.lidNodes,loaded.source), byLid=new Map(loaded.lidNodes.map(n=>[n.lid,n]));
const files=unzipSync(new Uint8Array(readFileSync(epub))); const container=strFromU8(files['META-INF/container.xml']); const opfPath=/full-path="([^"]+)"/.exec(container)![1]; const opf=strFromU8(files[opfPath]);const opfDir=opfPath.includes('/')?opfPath.slice(0,opfPath.lastIndexOf('/')+1):'';
const manifest=new Map([...opf.matchAll(/<item\s[^>]*>/g)].map(m=>[/\bid="([^"]+)"/.exec(m[0])?.[1],/\bhref="([^"]+)"/.exec(m[0])?.[1]]));
const blockMeta:any[]=[]; const docs:any[]=[];
for(const m of opf.matchAll(/<itemref\s[^>]*\bidref="([^"]+)"/g)) {
 const href=manifest.get(m[1]); if(!href)continue;const p=opfDir+decodeURIComponent(href),html=strFromU8(files[p]), root=parse(html),blocks=xhtmlToBlocks(html,p);
 const els=root.querySelectorAll('p,li,blockquote,h1,h2,h3,h4,h5,h6,img');
 const doc={path:p,first:blockMeta.length,count:blocks.length,headingTags:root.querySelectorAll('h1,h2,h3,h4,h5,h6').length,classes:Object.fromEntries([...new Set(els.map((e:any)=>e.getAttribute('class')??''))].map(c=>[c,els.filter((e:any)=>(e.getAttribute('class')??'')===c).length]))};
 docs.push(doc);
 let ei=0;
 for(const b of blocks){const matches=els.map((e:any,i:number)=>({e,i})).filter((v:any)=>v.i>=ei && v.e.text.replace(/\s+/g,' ').trim()===b.text);const match=matches[0];if(match)ei=match.i;
 blockMeta.push({path:p,kind:b.assetKind??b.kind,class:match?.e.getAttribute('class')??'',id:match?.e.getAttribute('id')??'',tag:match?.e.rawTagName??'',images:b.assetKind==='image',text:b.text});}
}
const rows=windows.map(w=>{const a=JSON.parse(readFileSync(path.join(dir,w.id+'.json'),'utf8'));const texts=w.leafLids.map(l=>loaded.source.slice(byLid.get(l)!.span.start,byLid.get(l)!.span.end));const meta=blockMeta[w.id];return {id:w.id,empty:a.payload.nodes.length===0,nodeCount:a.payload.nodes.length,leaves:w.leafLids.length,chars:texts.join('').length,tokens:w.tokens,kinds:w.leafLids.map(l=>byLid.get(l)!.kind),doc:meta?.path,class:meta?.class,tag:meta?.tag,text:texts.join('\n'),dispatch:a.provenance.executor};});
const quant=(xs:number[])=>{xs.sort((a,b)=>a-b);return {min:xs[0],p25:xs[Math.floor(xs.length*.25)],median:xs[Math.floor(xs.length*.5)],p75:xs[Math.floor(xs.length*.75)],p90:xs[Math.floor(xs.length*.9)],max:xs.at(-1),mean:xs.reduce((a,b)=>a+b,0)/xs.length}};
const stats=(rs:any[])=>({count:rs.length,chars:quant(rs.map(r=>r.chars)),tokens:quant(rs.map(r=>r.tokens)),leaves:quant(rs.map(r=>r.leaves)),image:rs.filter(r=>r.kinds.includes('image')).length,lengths:Object.fromEntries([10,20,40,80,160,320,640,1280].map(n=>[n,rs.filter(r=>r.chars<=n).length]))});
const groups=(key:string)=>{const out:any={};for(const r of rows){const k=r[key];out[k]??={total:0,empty:0,chars:0};out[k].total++;out[k].empty+=+r.empty;out[k].chars+=r.chars;}return out;};
const dispatchGroups:any={};for(const r of rows)(dispatchGroups[r.dispatch]??=[]).push(r);const position:any={};for(const rs of Object.values(dispatchGroups) as any[][]){rs.sort((a,b)=>a.id-b.id).forEach((r,i)=>{position[i]??={total:0,empty:0};position[i].total++;position[i].empty+=+r.empty;});}
const summary={sourceChars:loaded.source.length,sourceBlocks:loaded.blocks.length,headingBlocks:loaded.blocks.filter(b=>b.kind==='heading').length,nodes:loaded.lidNodes.length,rootNodes:loaded.lidNodes.filter(n=>n.path.length===1).length,windows:windows.length,all:stats(rows),empty:stats(rows.filter(r=>r.empty)),nonempty:stats(rows.filter(r=>!r.empty)),byClass:groups('class'),byDoc:groups('doc'),dispatchGroups:Object.keys(dispatchGroups).length,byDispatchPosition:position,docs};
writeFileSync('C:/Users/Lenovo/AppData/Local/Temp/empty-window-stats.json',JSON.stringify(summary,null,2));
writeFileSync('C:/Users/Lenovo/AppData/Local/Temp/empty-window-rows.json',JSON.stringify(rows.map(({text,dispatch,...r})=>r),null,2));
if(!process.argv.includes('--samples')&&!process.argv.includes('--all-empty'))console.log(JSON.stringify({...summary,docs:undefined,byDoc:undefined,byClass:undefined},null,2));
if(process.argv.includes('--samples'))console.log(JSON.stringify(rows.filter(r=>r.empty).sort((a,b)=>b.chars-a.chars).slice(0,80).map(({dispatch,...r})=>r),null,2));
if(process.argv.includes('--all-empty'))console.log(rows.filter(r=>r.empty).map(r=>r.id+'\t'+r.text).join('\n'));
if(process.argv.includes('--headings'))console.log(rows.filter(r=>/^第[一二三四五六七]章/.test(r.text.replace(/\s+/g,''))&&!/\d/.test(r.text)).map(r=>r.id+'\t'+r.text).join('\n'));
if(process.argv.includes('--classify')) {
 const pages=new Set([161,280,304,318,358,398,447,465,513,524,601,643,720,727,760,790,808,828,864,898,911,922,1120,1214,1246,1356,1362,1376,1426,1441,1454,1507,1513,1527,1700,1716]);
 const metadata=new Set([4,5,18,20,21,22,24,25,33,34,35,36,37,38,53]);
 const structure=new Set([44,55,67,77,80,96,97,98,99,100,102,103,104,109,110,163,1554,1715]);
 const labels=new Set([227,618,648,872,1325,1616,1618,1633,1634,1638]);
 const references=new Set([1306,1545]);
 const prose=new Set([204,331,393,412,495,552,575,734,809,913,1021,1072,1344,1346,1347,1389,1414,1467,1487,1586,1648,1677,1679,1696]);
 const categ=(r:any)=>{const t=r.text.replace(/\s+/g,'');if(r.kinds.includes('image'))return'image_reference';if(metadata.has(r.id))return'frontmatter_metadata';if(structure.has(r.id))return'heading_or_toc';if(pages.has(r.id)||/^\d+[|]?失败的逻辑$/.test(t)||/^(第[一二三四五六七]章|引言).+\d+$/.test(t)||[61,71,79,90].includes(r.id))return'running_header_or_page';if(/^注释[：:]?$/.test(t))return'footnote_section_label';if(references.has(r.id))return'bibliography_fragment';if(prose.has(r.id))return'context_dependent_prose';if(labels.has(r.id))return'figure_label_or_ocr_fragment';if(r.id===735)return'formula_fragment';if(/^[\d\s.·,，()（）%+—\-]+$/.test(t))return'numeric_figure_tick';return'UNCLASSIFIED';};
 const classified=rows.filter(r=>r.empty).map(r=>({id:r.id,category:categ(r)}));const counts:any={};for(const r of classified)counts[r.category]=(counts[r.category]??0)+1;
 writeFileSync('C:/Users/Lenovo/AppData/Local/Temp/empty-window-classification.json',JSON.stringify({basis:'All 432 empty source fragments manually reviewed; rules plus exact nonsemantic ids preserve category mapping. No raw source retained.',counts,classified},null,2));
 console.log(JSON.stringify({counts,unclassified:classified.filter(r=>r.category==='UNCLASSIFIED').map(r=>({id:r.id,text:rows[r.id].text}))},null,2));
 const imageCounts=docs.map(d=>{const html=strFromU8(files[d.path]),root=parse(html),imgs=root.querySelectorAll('img');return {doc:d.path,total:imgs.length,extracted:xhtmlToBlocks(html,d.path).filter((b:any)=>b.assetKind==='image').length};});
 console.log(JSON.stringify({images:imageCounts.reduce((a,r)=>({total:a.total+r.total,extracted:a.extracted+r.extracted}),{total:0,extracted:0}),spineDocs:docs.length,emptyDocs:docs.filter(d=>!d.count).length,headingTags:docs.reduce((a,d)=>a+d.headingTags,0),emptyByDoc:Object.fromEntries(Object.entries(summary.byDoc).filter(([k,v]:any)=>v.empty).map(([k,v]:any)=>[k,{total:v.total,empty:v.empty}]))},null,2));
}
