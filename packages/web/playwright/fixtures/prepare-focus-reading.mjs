import { readFile, writeFile, mkdir, copyFile, stat } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';
import path from 'node:path';

const repo = fileURLToPath(new URL('../../../../', import.meta.url));
const output = path.join(repo, 'tmp/focus-reading-fp1/public/fp1');
await mkdir(output, { recursive: true });
const readJson = async file => JSON.parse(await readFile(file, 'utf8'));
const inventory = [];
const definitions = [
  ['epub', 'understandbook'], ['complex', 'ai-infra-book-complete'], ['mixed', 'kimik3'],
];
for (const [name, id] of definitions) {
  const dir = path.join(repo, '.understand-book', id);
  const source = await readFile(path.join(dir, 'source.txt'), 'utf8');
  const base = await readJson(path.join(dir, 'base.json'));
  const leaves = base.lid_nodes.filter(node => !node.children.length);
  const assets = (await readJson(path.join(dir, 'asset_manifest.json'))).images;
  const manifest = await readJson(path.join(dir, 'source_manifest.json'));
  const longest = leaves.reduce((a, n, i) => n.kind === 'paragraph' && n.span.end - n.span.start > leaves[a].span.end - leaves[a].span.start ? i : a, leaves.findIndex(n => n.kind === 'paragraph'));
  let start = Math.max(0, longest - 24);
  if (name === 'complex') {
    // Select the heaviest 96-leaf mixture, by actual formula/table/image content.
    let best = -1;
    for (let i = 0; i + 240 <= leaves.length; i += 24) {
      const score = leaves.slice(i, i + 96).reduce((sum, n) => sum + (['formula', 'table', 'image'].includes(n.kind) ? source.slice(n.span.start, n.span.end).length : 0), 0);
      if (score > best) { best = score; start = i; }
    }
  }
  const selected = leaves.slice(start, start + 240);
  const segments = [];
  const materialDir = path.join(output, name);
  await mkdir(materialDir, { recursive: true });
  for (const node of selected) {
    let asset = assets.find(a => a.lid === node.lid) ?? null;
    if (asset?.stored_path) {
      const filename = path.basename(asset.stored_path);
      await copyFile(path.join(dir, asset.stored_path), path.join(materialDir, filename));
      asset = { ...asset, url_path: `/fp1/${name}/${filename}` };
    }
    segments.push({ lid: node.lid, kind: node.kind, text: source.slice(node.span.start, node.span.end), formula: null, imageAsset: asset });
  }
  for (let i = 0; i < segments.length; i += 24) {
    await writeFile(path.join(materialDir, `${i / 24}.json`), JSON.stringify(segments.slice(i, i + 24)));
  }
  const largest = kind => leaves.filter(n => n.kind === kind).sort((a,b) => (b.span.end-b.span.start)-(a.span.end-a.span.start))[0];
  const stress = ['paragraph','formula','table','code'].map(largest).filter(Boolean).map(node => ({ lid: node.lid, kind: node.kind, text: source.slice(node.span.start,node.span.end), formula:null,imageAsset:null }));
  await writeFile(path.join(materialDir, 'stress.json'), JSON.stringify(stress));
  let imageStress;
  if(name==='complex') {
    const sizes=[];
    for(const asset of assets.filter(a=>a.stored_path?.endsWith('.svg'))) {
      const svg=await readFile(path.join(dir,asset.stored_path),'utf8');
      const view=svg.match(/viewBox=["']([\d.\s-]+)["']/)?.[1].trim().split(/\s+/).map(Number);
      const width=Number(svg.match(/<svg[^>]*\bwidth=["']([\d.]+)["']/)?.[1])||view?.[2]||0;
      const height=Number(svg.match(/<svg[^>]*\bheight=["']([\d.]+)["']/)?.[1])||view?.[3]||0;
      sizes.push({asset,width,height,pixels:width*height});
    }
    const largest=sizes.sort((a,b)=>b.pixels-a.pixels).slice(0,6);
    const imageSegments=[];
    for(const {asset} of largest) {
      await copyFile(path.join(dir,asset.stored_path),path.join(materialDir,path.basename(asset.stored_path)));
      const node=leaves.find(n=>n.lid===asset.lid);
      imageSegments.push({lid:node.lid,kind:node.kind,text:source.slice(node.span.start,node.span.end),formula:null,imageAsset:{...asset,url_path:`/fp1/${name}/${path.basename(asset.stored_path)}`}});
    }
    await writeFile(path.join(materialDir,'images.json'),JSON.stringify(imageSegments));
    imageStress=largest.map(({asset,...size})=>({lid:asset.lid,...size}));
  }
  inventory.push({ name, bookId: id, canonicalKind: manifest.canonical_source.kind, source: `.understand-book/${id}/source.txt`, sourceModified: (await stat(path.join(dir,'source.txt'))).mtime.toISOString(), sourceBytes: Buffer.byteLength(source), sourceUtf16: source.length, leaves: leaves.length, kinds: leaves.reduce((counts,n)=>(counts[n.kind]=(counts[n.kind]??0)+1,counts),{}), sampleStart: start, sampleLeaves: segments.length, firstLid: selected[0].lid, lastLid: selected.at(-1).lid, chunks: Math.ceil(segments.length/24), stress: stress.map(s=>({lid:s.lid,kind:s.kind,utf16:s.text.length})), imageStress });
}
const pdfDir = path.join(repo, '.understand-book/kimik3');
await copyFile(path.join(pdfDir,'paper.pdf'), path.join(output,'paper.pdf'));
await copyFile(path.join(pdfDir,'source_manifest.json'), path.join(output,'pdf-manifest.json'));
await copyFile(path.join(pdfDir,'pdf_source_map.json'), path.join(output,'pdf-map.json'));
const pdf = await stat(path.join(pdfDir,'paper.pdf'));
await writeFile(path.join(output,'inventory.json'), JSON.stringify({ generatedAt:new Date().toISOString(), materials:inventory, pdf:{ source:'.understand-book/kimik3/paper.pdf',bytes:pdf.size,modified:pdf.mtime.toISOString() } }, null, 2));
console.log(JSON.stringify({ output, materials: inventory.map(m=>({name:m.name,firstLid:m.firstLid,lastLid:m.lastLid,sourceBytes:m.sourceBytes})) }));
