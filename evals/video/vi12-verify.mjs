// Fixed VI0a questions -> VI11 visual search -> VI5 production frame reader
// and HTTP model-request recorder. No semantic model inference is claimed.
import { readFileSync, writeFileSync, mkdirSync, copyFileSync } from 'node:fs';
import { execFileSync, spawn } from 'node:child_process';
import { createServer } from 'node:http';
import path from 'node:path';
import assert from 'node:assert/strict';
const read = f => JSON.parse(readFileSync(f,'utf8'));
const save = (f,v) => { mkdirSync(path.dirname(f),{recursive:true}); writeFileSync(f,JSON.stringify(v,null,2)+'\n'); };
const root = path.resolve('evals/video/generated/vi12'); mkdirSync(root,{recursive:true});
const out = path.resolve('evals/video/results/vi12-20261010');
const searchExe = path.resolve(process.env.VI12_SEARCH_EXE || path.join(process.env.CARGO_TARGET_DIR || 'target','debug/examples/video_search.exe'));
const frameExe = path.resolve(process.env.VI12_FRAME_EXE || 'target/debug/examples/video_frames_probe.exe');
const link = read('evals/video/generated/vi11/workspace/video_publication.json');
const directory = path.resolve(link.directory);
const manifest = read(path.join(directory,'video_source_manifest.json'));
const samples = read('evals/video/samples.json').samples;
const range = (start_ms,end_ms,video=manifest.material.video) => ({video,start_ms,end_ms});
const cases = [
  {id:'V01-float16',sample:'V01',query:'torch.float16 assert x == 0',time:555123},
  {id:'V01-bfloat16',sample:'V01',query:'torch.bfloat16 assert x != 0',time:650000},
  {id:'V02-origin',sample:'V02',query:'Google Brain',time:600000},
  {id:'V02-range',sample:'V02',query:'内存 动态范围',time:615000},
  {id:'V02-resolution',sample:'V02',query:'黄色强调 精度取舍',time:630000},
  {id:'V02-result',sample:'V02',query:'1.001e-8',time:650000},
  {id:'V03-first',sample:'V03',query:'einsum',time:1180000},
  {id:'V03-later',sample:'V03',query:'einsum',time:1220000},
  {id:'V04-natural',sample:'V04',query:samples.find(s=>s.id==='V04').question,time:1210000},
  {id:'V06-none',sample:'V06',query:'GPU 序列号',within:range(595000,655000),none:true},
  {id:'unindexed-current',query:'float16',within:range(2000000,2001000),none:true,direct:2000000},
];
function search(dir, entries, name) {
  const input = path.join(root,`${name}-queries.json`);
  save(input,entries.map(c=>({query:c.query,...(c.within?{within:c.within}:{})})));
  const r = JSON.parse(execFileSync(searchExe,[dir,input],{windowsHide:true,encoding:'utf8',maxBuffer:8*1024*1024}));
  save(path.join(root,`${name}-search.json`),r); return r;
}
const lecture = search(directory,cases,'lecture');
assert.deepEqual(lecture.results[0].coverage.accepted,manifest.coverage.find(c=>c.capability==='visual_index').accepted);
// The VI11 supplement is a controlled silent teaching fixture, not a registered
// lecture. Make a read-ready fixture copy, retaining its original synthetic
// context texts and identities; do not mutate its accepted contributions.
const source = path.resolve('evals/video/generated/vi11/supplement'), supplement = path.join(root,'supplement');
const sm = read(path.join(source,'video_source_manifest.json')), map = read(path.join(source,'transcript_map.json'));
const catalog = read(path.join(source,'video_enrichments.json'));
for(const file of [sm.source.imported_path,'video_enrichments.json',...catalog.batches.flatMap(b=>b.files)]) {
  mkdirSync(path.dirname(path.join(supplement,file)),{recursive:true}); copyFileSync(path.join(source,file),path.join(supplement,file));
}
let text=''; const nodes=[];
for(const [i,s] of map.segments.entries()) {
  const start=text.length; text+=s.text+'\n';
  nodes.push({lid:s.text_range.lid,path:[i+1],kind:'paragraph',span:{start,end:text.length},children:[]});
  s.text_range.range={start:0,end:s.text.length};
}
map.raw_result_path='fixture-context.json'; map.machine_transcription={engine:'controlled-fixture',model:'synthetic-context-not-speech',language:'en'};
sm.coverage=[{capability:'transcript',accepted:[range(0,6000,sm.material.video)],pending:[],failed:[]}];
save(path.join(supplement,'video_source_manifest.json'),sm); save(path.join(supplement,sm.transcript_map_path),map);
save(path.join(supplement,'fixture-context.json'),{note:'Synthetic occurrence contexts from VI11; this video is silent.'});
save(path.join(supplement,'base.json'),{book_id:sm.material.book_id,lid_nodes:nodes,graph_nodes:[],graph_edges:[]});
writeFileSync(path.join(supplement,'source.txt'),text);
const sp=read(path.join(directory,'publication.json'));
sp.reference={book_id:sm.material.book_id,publication_id:'vi12-controlled-fixture'}; sp.previous_publication=null;
sp.source_fingerprint='controlled-fixture-not-a-registered-publication'; sp.files={};
save(path.join(supplement,'publication.json'),sp);
const repeatedCases=[
  {id:'V08-first',sample:'V08',query:'float16 ==',time:1000},
  {id:'V08-change',sample:'V08',query:'bfloat16 !=',time:3000},
  {id:'V08-return',sample:'V08',query:'float16 ==',time:5000},
];
const repeated=search(supplement,repeatedCases,'supplement');
// A change description may legitimately mention the preceding ==/float16 state.
// Assert both actual A occurrences survive, rather than banning that extra hit.
const sameCode=repeated.results[0].candidates.filter(c=>c.screen_text.includes('dtype=torch.float16'));
assert.deepEqual(sameCode.map(c=>c.transcript_segment_ids),[['context-0'],['context-2']]);
const baseline=read('evals/video/generated/vi11/baseline-publication.json');
const oldDirectory=path.join(path.dirname(directory),baseline.reference.publication_id);
const old=search(oldDirectory,[{query:'float16'}],'old-publication');
assert.equal(old.results[0].total_matches,0); assert.equal(old.results[0].coverage.accepted.length,0);
let currentDir; let requests=0;
const recorder=createServer(async(req,res)=>{
  let body='';for await(const chunk of req)body+=chunk;
  save(path.join(currentDir,'http-body.json'),JSON.parse(body));requests++;
  res.writeHead(200,{'Content-Type':'application/json'});
  res.end(JSON.stringify({choices:[{message:{content:'RECORDING ONLY: actual frame transport, no semantic inference.'}}]}));
});
await new Promise(r=>recorder.listen(0,'127.0.0.1',r));
const env={...process.env,OPENCODE_API_KEY:'vi12-local-recorder',OPENCODE_BASE_URL:`http://127.0.0.1:${recorder.address().port}`,
  FLUID_LLM_MODEL:'vi12-recorder',UNDERSTAND_BOOK_PROVIDER:'native'};
const checks=[];
try {
  for(const [entries,results,dir,m] of [[cases,lecture.results,directory,manifest],[repeatedCases,repeated.results,supplement,sm]]) {
    for(const [i,c] of entries.entries()) {
      const result=results[i]; assert.equal(result.evidence_status,'candidate_only');
      if(c.none) assert.equal(result.total_matches,0,c.id);
      const hit=c.time===undefined?null:result.candidates.find(h=>h.location.start_ms<=c.time && h.location.end_ms>c.time);
      if(c.time!==undefined) assert(hit,`${c.id}: useful candidate not recalled`);
      const check={id:c.id,question:samples.find(s=>s.id===c.sample)?.question,query:c.query,publication:result.publication,
        total_matches:result.total_matches,candidate_rank:hit?result.candidates.indexOf(hit)+1:null,candidate:hit,coverage:result.coverage};
      if(c.direct!==undefined) assert.deepEqual(result.coverage.gaps,[c.within]);
      const time=c.time??c.direct;
      if(time!==undefined) {
        currentDir=path.join(root,c.id); mkdirSync(currentDir,{recursive:true});
        const location=hit?range(time,Math.min(hit.location.end_ms,time+1000),m.material.video):c.within;
        const input=path.join(currentDir,'input.json');save(input,{question:check.question||'Inspect the current unindexed frame',read:{location,sampling:[time]}});
        const child=spawn(frameExe,[dir,input,currentDir],{env,windowsHide:true,stdio:['ignore','pipe','pipe']});
        let diagnostic='';child.stderr.on('data',b=>diagnostic+=b);child.stdout.resume();
        const exit=await new Promise((r,j)=>{child.on('close',r);child.on('error',j)}); assert.equal(exit,0,diagnostic);
        const body=read(path.join(currentDir,'http-body.json')), projection=read(path.join(currentDir,'read.json'));
        const observations=read(path.join(currentDir,'observations.json'));
        const images=body.messages.flatMap(m=>Array.isArray(m.content)?m.content:[]).filter(c=>c.type==='image_url');
        const png=readFileSync(path.join(currentDir,'frame-0.png'));
        assert.equal(images.length,1);assert.equal(images[0].image_url.url,`data:image/png;base64,${png.toString('base64')}`);
        assert.equal(observations.length,1);assert.deepEqual(observations[0].publication,result.publication);
        const frame=projection.frames[0].read.frame; assert(frame.actual_pts_ms>=time&&frame.actual_pts_ms-time<34);
        check.read=projection;check.observations=observations;check.png_bytes=png.length;check.request_pixels_equal=true;
        mkdirSync(path.join(out,'frames'),{recursive:true});copyFileSync(path.join(currentDir,'frame-0.png'),path.join(out,'frames',`${c.id}.png`));
      }
      checks.push(check);
    }
  }
} finally { recorder.close(); }
save(path.join(out,'verification.json'),{status:'passed',semantic_inference:false,publication:link.reference,
  visual_coverage_ms:150000,lecture_search_ms:lecture.elapsed_ms,supplement_search_ms:repeated.elapsed_ms,
  search_model_calls:0,frame_recording_requests:requests,old_publication_has_no_index:true,checks,
  limitations:['Natural-language search uses literal descriptors; the Resident may extract/rephrase query concepts.',
    'Supplement is a read-ready controlled fixture with synthetic occurrence context, not source speech or a registered publication.',
    'HTTP recorder confirms actual image projection and observation receipts, not a live-model semantic answer.']});
console.log(JSON.stringify({status:'passed',queries:checks.length,frame_reads:requests,lecture_search_ms:lecture.elapsed_ms}));
