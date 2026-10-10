const fs = require('node:fs');
const path = require('node:path');
const {pathToFileURL} = require('node:url');
const {chromium} = require(process.env.BOOK_PLAYWRIGHT || 'C:/Users/Lenovo/.cache/codex-runtimes/codex-primary-runtime/dependencies/node/node_modules/playwright');
const work=path.join(__dirname,'work');
const directory=path.join(work,'preview/EPUB/EPUB');
(async()=>{
  const browser=await chromium.launch({executablePath:process.env.BOOK_BROWSER || 'C:/Program Files (x86)/Microsoft/Edge/Application/msedge.exe',headless:true});
  const page=await browser.newPage();
  const failures=[];
  const snapshots=[];
  const files=fs.readdirSync(directory).filter(n=>n.endsWith('.xhtml'));
  for (const width of [390,900]) {
    await page.setViewportSize({width,height:1000});
    for(const name of files) {
      await page.goto(pathToFileURL(path.join(directory,name)).href);
      await page.evaluate(()=>document.fonts.ready);
      const result=await page.evaluate(()=>({
        scroll:document.documentElement.scrollWidth,viewport:innerWidth,
        broken:[...document.images].filter(i=>!i.complete||i.naturalWidth===0).map(i=>i.src),
        overflow:[...document.querySelectorAll('table,pre,img')].filter(e=>e.getBoundingClientRect().right>innerWidth+1).map(e=>({tag:e.tagName,width:e.getBoundingClientRect().width,text:e.textContent.slice(0,80)}))
      }));
      if(result.scroll>width+1||result.broken.length||result.overflow.length) failures.push({width,name,...result});
      const sample=(width===390&&['ch013.xhtml','ch022.xhtml','ch026.xhtml'].includes(name))||(width===900&&['title_page.xhtml','ch014.xhtml','ch026.xhtml'].includes(name));
      if(sample) {
        if(name==='ch026.xhtml') await page.locator('img.math-display').scrollIntoViewIfNeeded();
        if(name==='ch013.xhtml') await page.locator('img').last().scrollIntoViewIfNeeded();
        if(name==='ch014.xhtml') await page.locator('pre').first().scrollIntoViewIfNeeded();
        if(name==='ch022.xhtml') await page.locator('table').first().scrollIntoViewIfNeeded();
        const dest=path.join(work,`preview-${width}-${name}.png`);
        await page.screenshot({path:dest});snapshots.push(path.basename(dest));
      }
    }
  }
  const report={status:failures.length?'failed':'passed',viewports:[390,900],documents:files.length,checks:files.length*2,snapshots,failures};
  fs.writeFileSync(path.join(work,'layout-verification.json'),JSON.stringify(report,null,2));
  console.log(JSON.stringify(report,null,2));await browser.close();
})().catch(e=>{console.error(e);process.exit(1)});
