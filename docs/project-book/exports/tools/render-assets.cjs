const fs = require('node:fs');
const path = require('node:path');
const { chromium } = require(process.env.BOOK_PLAYWRIGHT || 'C:/Users/Lenovo/.cache/codex-runtimes/codex-primary-runtime/dependencies/node/node_modules/playwright');
const katexRoot = path.dirname(require.resolve('katex/package.json'));
const here = __dirname;
const out = path.dirname(here);
const jobs = JSON.parse(fs.readFileSync(path.join(here, 'work/media.json'), 'utf8'));
(async () => {
  const browser = await chromium.launch({ executablePath: process.env.BOOK_BROWSER || 'C:/Program Files (x86)/Microsoft/Edge/Application/msedge.exe', headless: true });
  const page = await browser.newPage({viewport:{width:1800,height:1200},deviceScaleFactor:2});
  await page.setContent('<html><head><meta charset="utf-8"></head><body><div id="output"></div></body></html>');
  await page.addStyleTag({content:'body {margin:0;background:white;} #output {display:inline-block;padding:16px;font-family:"Microsoft YaHei",sans-serif;} svg {max-width:none !important;}'});
  await page.addScriptTag({path:path.join(here,'vendor/mermaid-10.9.3.min.js')});
  // Use a file-backed page for KaTeX fonts; there are no remote resources.
  const cssPath = path.join(katexRoot,'dist/katex.min.css').replaceAll('\\','/');
  const css = fs.readFileSync(cssPath,'utf8').replace(/url\(fonts\//g,'url(file:///'+path.dirname(cssPath)+'/fonts/');
  await page.addStyleTag({content:css});
  await page.addScriptTag({path:path.join(katexRoot,'dist/katex.min.js')});
  await page.evaluate(() => mermaid.initialize({startOnLoad:false,theme:'base',securityLevel:'strict',fontFamily:'Microsoft YaHei',themeVariables:{fontSize:'18px',primaryColor:'#eef4f6',primaryTextColor:'#163748',primaryBorderColor:'#718e9d',lineColor:'#5d798a'},flowchart:{htmlLabels:false,useMaxWidth:false},sequence:{useMaxWidth:false}}));
  const record=[];
  for (const [i,job] of jobs.entries()) {
    const destination=path.join(out,'assets',job.name+'.png');
    await page.evaluate(async ({job,i}) => {
      const root=document.getElementById('output'); root.innerHTML='';
      root.style.fontSize='18px';root.style.padding='16px';root.style.lineHeight='normal';
      if (job.kind==='mermaid') {
        let rendered=await mermaid.render('diagram'+i,job.source);
        root.innerHTML=rendered.svg;
        if (root.querySelector('svg').viewBox.baseVal.width>950 && /^flowchart LR/m.test(job.source)) {
          rendered=await mermaid.render('portrait'+i,job.source.replace(/^flowchart LR/m,'flowchart TD'));
          root.innerHTML=rendered.svg;
        }
        const svg=root.querySelector('svg');const box=svg.viewBox.baseVal;
        svg.setAttribute('width',String(Math.ceil(box.width)));svg.setAttribute('height',String(Math.ceil(box.height)));
      } else {
        root.style.fontSize=job.display?'24px':'20px';
        root.style.padding=job.display?'8px':'1px';root.style.lineHeight='1';
        root.innerHTML=katex.renderToString(job.source,{displayMode:job.display,throwOnError:true,output:'html'});
      }
      await document.fonts.ready;
    },{job,i});
    const box=await page.locator('#output').boundingBox();
    await page.locator('#output').screenshot({path:destination});
    record.push({name:job.name,...box});
    process.stdout.write(job.name+' '+Math.round(box.width)+'x'+Math.round(box.height)+'\n');
  }
  fs.writeFileSync(path.join(here,'work/render-record.json'),JSON.stringify(record,null,2));
  await browser.close();
})().catch(e=>{console.error(e);process.exit(1)});
