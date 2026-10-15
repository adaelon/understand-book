import { createApp, h, nextTick, ref, shallowRef } from 'vue';
import ReaderPane, { type Segment } from '../../src/components/ReaderPane.vue';
import PdfReaderPane from '../../src/components/PdfReaderPane.vue';
import { renderInlineMarkdown, renderFormulaSource, renderTableSource, renderMarkdown } from '../../src/md';
import { createMarkdownDomSourceMap } from '../../src/markdown-source-map';
import { defaultTypography } from '../../src/reader-typography';
import { FOCUS_READING_BUDGET, focusPageGeometry } from '../../src/reader-pagination-budget';
import 'katex/dist/katex.min.css';
import '../../src/style.css';
import './focus-reading.css';

const segments = shallowRef<Segment[]>([]);
const mode = ref('continuous');
const typography = ref({ ...defaultTypography });
const pdf = shallowRef<any>(null);
const parsedSizes = new Map<string, number>();
const preparations: any[] = [];
const longTasks: any[] = [];
const inputs: any[] = [];
const sourceMaps = new WeakMap<HTMLElement, { source:string; child:ChildNode|null; map:ReturnType<typeof createMarkdownDomSourceMap> }>();
let parseMs = 0;
let parseCalls = 0;
let currentPage = 0;
let columnCount = 1;
let geometry = focusPageGeometry(innerWidth - 64, 19);
new PerformanceObserver(list => longTasks.push(...list.getEntries().map(e => ({ start: e.startTime, duration: e.duration })))).observe({ type:'longtask',buffered:true });

function render(seg: Segment) {
  const start = performance.now();
  const html = seg.kind === 'table' ? renderTableSource(seg.text)
    : seg.kind === 'formula' ? renderFormulaSource(seg.text)
    : seg.kind === 'code' ? seg.text.replace(/&/g,'&amp;').replace(/</g,'&lt;')
    : renderInlineMarkdown(seg.text.replace(/^\s{0,3}#{1,6}\s+/,''));
  parseMs += performance.now() - start;
  parseCalls++;
  parsedSizes.set(seg.lid, html.length * 2);
  return html;
}
createApp({ setup: () => () => h('main',{ class:['fp1',mode.value] },[
  h('header', {}, 'FP1 · production Reader component / local pagination experiment'),
  mode.value === 'pdf' && pdf.value ? h(PdfReaderPane, { sourceManifest:pdf.value.manifest,sourceMap:pdf.value.map,pdfUrl:'/fp1/paper.pdf',activeLid:null,selectedLid:null })
    : h(ReaderPane, {
      typography:typography.value, contextKey:'fp1', segments:segments.value, viewportAnchor:null,selectedLid:null,
      renderSeg:render,renderMarkdown,markdownHeadingLevel:(s:Segment)=>/^\s{0,3}#{1,6}\s/.test(s.text)?(s.text.match(/#+/)![0].length):null,
      isAsset:(s:Segment)=>['code','table','image'].includes(s.kind),isHighlighted:()=>false,highlightsOf:()=>[],highlightCardsOf:()=>[],visibleNotes:[],hlExcerpt:()=>'',imageMeta:()=>null,
      imageAsset:(lid:string)=>segments.value.find(s=>s.lid===lid)?.imageAsset??null,
    }),
]) }).mount('#app');

function layout(mobile = false) {
  const padding = innerWidth <= 600 ? 16 : 32;
  geometry = focusPageGeometry(innerWidth - padding * 2, typography.value.fontSizePx, mobile);
  const root = document.documentElement;
  root.style.setProperty('--fp-width', `${innerWidth-padding*2}px`);
  root.style.setProperty('--fp-column', `${geometry.pageWidth}px`);
  root.style.setProperty('--fp-gap', `${geometry.gap}px`);
  root.style.setProperty('--fp-height', `${innerHeight-110}px`);
}
async function ready() {
  await nextTick();
  const text = segments.value.map(s=>s.text).join('');
  await Promise.all([400,600].map(w=>document.fonts.load(`${w} ${typography.value.fontSizePx}px "Noto Serif SC Variable"`,text)));
  const images = Array.from(document.querySelectorAll<HTMLImageElement>('.reader-pane img'));
  // The experimental ready gate eagerly decodes its LOCAL batch only.
  // The continuous baseline retains the component's lazy-loading behavior.
  if (mode.value === 'columns') images.forEach(img=>img.loading='eager');
  await Promise.all(images.filter(img=>mode.value==='columns'||img.complete).map(img=>img.decode()));
  await document.fonts.ready;
  await nextTick();
  document.querySelector('.prose')?.getBoundingClientRect();
  columnCount = resources().columns ?? 1;
}
async function load(material:string, chunk:number|string=0, append=false) {
  const start = performance.now(), beforeParse = parseMs, beforeCalls = parseCalls;
  const response = await fetch(`/fp1/${material}/${chunk}.json`);
  if (!response.ok) throw new Error(`Missing FP1 material ${material}/${chunk}`);
  const incoming:Segment[] = await response.json();
  const fetched = performance.now();
  if (!append) parsedSizes.clear();
  segments.value = append ? [...segments.value,...incoming] : incoming;
  await ready();
  const result = { material,chunk,append,startedAt:start,fetchMs:fetched-start,prepareMs:performance.now()-fetched,totalMs:performance.now()-start,parseMs:parseMs-beforeParse,parseCalls:parseCalls-beforeCalls,readyAt:performance.now(),...resources() };
  preparations.push(result);
  performance.mark(`fp1-ready-${preparations.length}`);
  return result;
}
function resources() {
  const root = document.querySelector('.reader-pane,.pdf-reader-pane') ?? document.querySelector('main')!;
  const walker = document.createTreeWalker(root,NodeFilter.SHOW_ALL);
  let domNodes=1; while(walker.nextNode()) domNodes++;
  const images = Array.from(root.querySelectorAll('img')).map(img=>({width:img.naturalWidth,height:img.naturalHeight,complete:img.complete}));
  const canvases = Array.from(root.querySelectorAll('canvas')).map(c=>({width:c.width,height:c.height}));
  const article=document.querySelector<HTMLElement>('.prose');
  return { leaves:segments.value.length,sourceBytes:segments.value.reduce((s,n)=>s+n.text.length*2,0),parsedBytes:[...parsedSizes.values()].reduce((a,b)=>a+b,0),domNodes,images,decodedImagePixels:images.reduce((n,i)=>n+i.width*i.height,0),canvases,canvasPixels:canvases.reduce((n,c)=>n+c.width*c.height,0),heapBytes:(performance as any).memory?.usedJSHeapSize??null,columns:mode.value==='columns'&&article?Math.round((article.scrollWidth+geometry.gap)/(geometry.pageWidth+geometry.gap)):null };
}

// Experimental geometry probe, intentionally outside the timed interaction.
// Binary-search each text piece's next column boundary; do not scan characters.
function mapColumns() {
  const started=performance.now();
  const article=document.querySelector<HTMLElement>('.prose')!;
  const origin=article.getBoundingClientRect().left;
  const stride=geometry.pageWidth+geometry.gap;
  const column=(rect:DOMRect)=>Math.max(0,Math.floor((rect.left-origin+0.5)/stride));
  const output:any[]=[];
  let probes=0, unpaintedWhitespace=0, alignmentMs=0, mapBuilds=0;
  for(const seg of segments.value) {
    const el=article.querySelector<HTMLElement>(`[data-lid="${seg.lid}"]`)!;
    if(!el) throw new Error(`Missing LID ${seg.lid}`);
    const asset=['table','code','image'].includes(seg.kind);
    if(asset) {
      output.push({lid:seg.lid,kind:seg.kind,sourceLength:seg.text.length,runs:[{page:column(el.getBoundingClientRect()),start:0,end:seg.text.length}],expected:[{start:0,end:seg.text.length}],atomic:true});
      continue;
    }
    const cached=sourceMaps.get(el);
    const at=performance.now();
    const reusable=cached?.source===seg.text&&cached.child===el.firstChild;
    const map=reusable?cached.map:createMarkdownDomSourceMap(seg.text,el);
    alignmentMs+=performance.now()-at;
    if(!reusable){mapBuilds++;sourceMaps.set(el,{source:seg.text,child:el.firstChild,map});}
    const runs:any[]=[];
    for(const piece of map.domPieces) {
      if(piece.kind==='text') {
        const node=piece.node as Text;
        const range=document.createRange();
        const pageAt=(offset:number)=>{
          range.setStart(node,offset+(piece.domStart??0));range.setEnd(node,offset+(piece.domStart??0)+1);probes++;
          const rect=range.getBoundingClientRect();
          if(!rect.width&&!rect.height) {
            // A collapsed space at a column break has no painted rectangle.
            // Retain its source span with the preceding fragment, never page 0.
            if(!/\s/.test(node.data[offset+(piece.domStart??0)]))throw new Error(`Unpainted non-whitespace ${seg.lid}:${offset}`);
            unpaintedWhitespace++;
            return null;
          }
          return column(rect);
        };
        let start=0; const length=piece.end-piece.start;
        while(start<length) {
          const page=pageAt(start)??runs.at(-1)?.page??column(el.getClientRects()[0]);
          let lo=start+1,hi=length;
          while(lo<hi) {const mid=Math.floor((lo+hi)/2);const at=pageAt(mid);if(at===null||at===page)lo=mid+1;else hi=mid;}
          for(const r of map.sourceRangesForSemanticRange(piece.start+start,piece.start+lo))runs.push({page,...r});
          start=lo;
        }
      } else {
        for(const r of map.sourceRangesForSemanticRange(piece.start,piece.end))runs.push({page:column((piece.node as Element).getBoundingClientRect()),...r});
      }
    }
    output.push({lid:seg.lid,kind:seg.kind,sourceLength:seg.text.length,runs,expected:map.sourceRangesForSemanticRange(0,map.semanticText.length),boxWidth:el.getBoundingClientRect().width});
  }
  return {durationMs:performance.now()-started,alignmentMs,mapBuilds,probes,unpaintedWhitespace,geometry,leaves:output};
}

// Real keyboard input timestamp -> handler and geometry update are diagnostics.
// Only browser trace/filmstrip may report a submitted/presented frame.
document.addEventListener('keydown',event=>{
  if(!['ArrowRight','ArrowLeft'].includes(event.key))return;
  const pane=document.querySelector<HTMLElement>('.reader-pane')!;
  const handler=performance.now();
  if(mode.value==='columns') {
    currentPage=Math.max(0,Math.min(columnCount-1,currentPage+(event.key==='ArrowRight'?geometry.pagesPerScreen:-geometry.pagesPerScreen)));
    pane.scrollLeft=currentPage*(geometry.pageWidth+geometry.gap);
  } else pane.scrollTop+=event.key==='ArrowRight'?400:-400;
  inputs.push({inputAt:event.timeStamp,handlerAt:handler,committedAt:performance.now(),scrollTop:pane.scrollTop,scrollLeft:pane.scrollLeft});
  performance.mark(`fp1-input-${inputs.length}`);
});

Object.assign(window,{fp1:{
  load,resources,mapColumns,budget:FOCUS_READING_BUDGET,
  report:()=>({preparations,longTasks,inputs,parseMs,parseCalls}),
  async columns(fontSize=19,mobile=false) {mode.value='columns';typography.value={...typography.value,fontSizePx:fontSize};layout(mobile);currentPage=0;await ready();return resources();},
  async continuous(){mode.value='continuous';currentPage=0;await nextTick();},
  async fitLocalWindow(){
    const original=segments.value;
    const attempts:any[]=[];
    const maxPages=geometry.pagesPerScreen*FOCUS_READING_BUDGET.maxResidentScreens;
    let lo=1,hi=original.length,best=0;
    const started=performance.now();
    while(lo<=hi){
      const count=Math.floor((lo+hi)/2),at=performance.now();
      segments.value=original.slice(0,count);parsedSizes.clear();await ready();
      const r=resources();
      const fits=(r.columns??0)<=maxPages&&r.domNodes<=FOCUS_READING_BUDGET.maxDomNodes&&r.sourceBytes<=FOCUS_READING_BUDGET.maxSourceBytes&&r.parsedBytes<=FOCUS_READING_BUDGET.maxParsedBytes&&r.decodedImagePixels<=FOCUS_READING_BUDGET.maxDecodedImagePixels;
      attempts.push({count,prepareMs:performance.now()-at,fits,...r});
      if(fits){best=count;lo=count+1;}else hi=count-1;
    }
    segments.value=original.slice(0,best);parsedSizes.clear();await ready();
    return {selected:best,available:original.length,maxPages,fittingMs:performance.now()-started,attempts,resident:resources()};
  },
  async pdf(){pdf.value={manifest:await(await fetch('/fp1/pdf-manifest.json')).json(),map:await(await fetch('/fp1/pdf-map.json')).json()};mode.value='pdf';await nextTick();},
  async stress(){
    segments.value=[...segments.value,{lid:'stress.long',kind:'paragraph',text:('中文长段 **strong** and English $x^2$，保留原文。').repeat(160),formula:null,imageAsset:null},{lid:'stress.code',kind:'code',text:Array.from({length:100},(_,i)=>`line ${i}: `+'wide_identifier_'.repeat(18)).join('\n'),formula:null,imageAsset:null}];
    await ready();
  },
  async schedulerSamples(count=120){const times:number[]=[];await new Promise<void>(resolve=>{const tick=(t:number)=>{times.push(t);if(times.length>=count)resolve();else requestAnimationFrame(tick);};requestAnimationFrame(tick);});return times.slice(1).map((t,i)=>t-times[i]);},
}});
