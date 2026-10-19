import fs from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { createRequire } from 'node:module';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const require = createRequire(path.resolve(root, '../../../packages/web/package.json'));
const { chromium } = require('@playwright/test');
const output = path.resolve(root, '../../../output/last-hit-positioning-preview');
await fs.mkdir(output, {recursive:true});
const browser = await chromium.launch({headless:true});
const page = await browser.newPage({viewport:{width:1440,height:1000},deviceScaleFactor:1});
const errors=[];
page.on('pageerror', error=>errors.push(error.message));
await page.goto(pathToFileURL(path.join(root,'book.html')).href);
await page.evaluate(()=>document.fonts.ready);
const desktop=await page.evaluate(()=>({
    images:[...document.querySelectorAll('article img')].map(img=>({src:img.getAttribute('src'),ok:img.complete&&img.naturalWidth>0})),
    brokenAnchors:[...document.querySelectorAll('a[href^="#"]')].filter(a=>!document.getElementById(a.getAttribute('href').slice(1))).map(a=>a.textContent),
    overflow:document.documentElement.scrollWidth>innerWidth,
    math:document.querySelectorAll('math').length,
    headings:document.querySelectorAll('article h2').length,
}));
await page.screenshot({path:path.join(output,'desktop-opening.png')});
await page.getByRole('heading',{name:'第五章 对手刚出手能不能上前',exact:true}).scrollIntoViewIfNeeded();
await page.screenshot({path:path.join(output,'desktop-chapter-five.png')});
await page.getByRole('heading',{name:'A3 把距离变成区间',exact:true}).scrollIntoViewIfNeeded();
await page.screenshot({path:path.join(output,'desktop-math.png')});
await page.setViewportSize({width:390,height:844});
await page.evaluate(()=>{document.querySelector('nav details').open=false;scrollTo(0,0)});
await page.screenshot({path:path.join(output,'mobile-opening.png')});
const mobile=await page.evaluate(()=>({overflow:document.documentElement.scrollWidth>innerWidth,
    width:document.documentElement.scrollWidth,
    outside:[...document.querySelectorAll('article p, article span.katex, article a, article code')]
      .filter(el=>el.getBoundingClientRect().right>innerWidth&& !el.closest('pre,table'))
      .slice(0,10).map(el=>({tag:el.tagName,right:el.getBoundingClientRect().right,text:el.textContent.slice(0,160)}))}));
await page.getByRole('heading',{name:'A3 把距离变成区间',exact:true}).scrollIntoViewIfNeeded();
await page.screenshot({path:path.join(output,'mobile-math.png')});
const result={desktop,mobile,errors};
await fs.writeFile(path.join(output,'layout-check.json'),JSON.stringify(result,null,2)+'\n');
await browser.close();
console.log(JSON.stringify(result,null,2));
if(desktop.images.some(img=>!img.ok)||desktop.brokenAnchors.length||desktop.overflow||mobile.overflow||errors.length)process.exitCode=1;
