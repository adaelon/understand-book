import { readFileSync, writeFileSync, mkdirSync, copyFileSync } from 'node:fs';
import { execFileSync, spawn } from 'node:child_process';
import { createServer } from 'node:http';
import path from 'node:path';
import assert from 'node:assert/strict';
const read=f=>JSON.parse(readFileSync(f,'utf8'));
const save=(f,v)=>{mkdirSync(path.dirname(f),{recursive:true});writeFileSync(f,JSON.stringify(v,null,2)+'\n');};
const workspace=path.resolve('evals/video/generated/vi11/workspace'),root=path.resolve('evals/video/generated/vi14');
const out=path.resolve('evals/video/results/vi14-20261010');
const link=read(path.join(workspace,'video_publication.json')),directory=path.resolve(workspace,link.directory);
const exe=process.env.VI14_SEARCH_EXE||path.join(process.env.CARGO_TARGET_DIR||'target','debug/examples/video_search.exe');
const frameExe=process.env.VI14_FRAME_EXE||path.join(process.env.CARGO_TARGET_DIR||'target','debug/examples/video_frames_probe.exe');
const batch='discourse-vi14-einsum';
const input=read(path.join(workspace,'.build/video-discourse',batch,'input.json'));
const contribution=read(path.join(directory,'video-enrichments',batch,'discourse.json'));
const oldDirectory=path.join(path.dirname(directory),input.publication.publication_id);
const question=read('evals/video/samples.json').samples.find(s=>s.id==='V04').question;
// The second query isolates the V04 summation subproblem; both snapshots
// receive exactly the same request bytes.
const queries=[{query:question},{query:'求和'},{query:'GPU serial number 序列号'}];save(path.join(root,'queries.json'),queries);
function recall(dir){return JSON.parse(execFileSync(exe,[dir,path.join(root,'queries.json')],{encoding:'utf8',windowsHide:true,maxBuffer:16*1024*1024}));}
const before=recall(oldDirectory),after=recall(directory);save(path.join(root,'before.json'),before);save(path.join(root,'after.json'),after);
for(let i=0;i<queries.length;i++) {
  assert.deepEqual(before.results[i].candidates,after.results[i].candidates,'visual input and baseline candidates must be identical');
  assert.equal(before.results[i].discourse.total_matches,0);
}
assert(after.results[0].discourse.total_matches>0);assert.equal(after.results[2].discourse.total_matches,0);
const expansion=after.results[1].discourse.candidates.find(c=>!c.matched_terms && c.via.some(v=>['explains','exemplifies','depends_on'].includes(v.relation.type)));
assert(expansion,'one candidate must be reached solely through a teaching relation');
const via=expansion.via.find(v=>['explains','exemplifies','depends_on'].includes(v.relation.type));
const seed=after.results[1].discourse.candidates.find(c=>c.discourse_id===via.matched_item_id);assert(seed);
for(const file of ['source.txt','base.json','transcript_map.json','transcript_translations.json'])
  assert(readFileSync(path.join(directory,file)).equals(readFileSync(path.join(oldDirectory,file))),file);
const oldCatalog=read(path.join(oldDirectory,'video_enrichments.json'));
for(const b of oldCatalog.batches) for(const file of b.files)
  assert(readFileSync(path.join(directory,file)).equals(readFileSync(path.join(oldDirectory,file))),`prior contribution ${file}`);
const map=read(path.join(directory,'transcript_map.json'));
for(const t of after.text_reads) assert(t.text.includes(map.segments.find(s=>s.text_range.lid===t.lid).text));
const calls=readFileSync(path.join(workspace,'.build/video-discourse',batch,'mcp-events.jsonl'),'utf8').trim().split('\n').map(JSON.parse);
assert(!calls.some(c=>c.isError));assert.equal(calls.reduce((n,c)=>n+c.images,0),input.media.images.length);
assert.deepEqual(calls.filter(c=>c.tool).map(c=>c.tool),['executor.open','executor.input.next','executor.generation.start','executor.submit_candidate']);
let active,requests=0;
const recorder=createServer(async(req,res)=>{let body='';for await(const b of req)body+=b;save(path.join(active,'http-body.json'),JSON.parse(body));requests++;
  res.writeHead(200,{'Content-Type':'application/json'});res.end(JSON.stringify({choices:[{message:{content:'Transport observation receipt only.'}}]}));});
await new Promise(resolve=>recorder.listen(0,'127.0.0.1',resolve));
const env={...process.env,OPENCODE_API_KEY:'vi14-recorder',OPENCODE_BASE_URL:`http://127.0.0.1:${recorder.address().port}`,FLUID_LLM_MODEL:'vi14-recorder',UNDERSTAND_BOOK_PROVIDER:'native'};
const checks=[];
try {
  const seen=new Set();
  for(const candidate of [seed,expansion]) for(const hint of candidate.read_hints) {
    const time=hint.sampling[0];if(seen.has(time))continue;seen.add(time);
    active=path.join(root,`frame-${time}`);mkdirSync(active,{recursive:true});save(path.join(active,'input.json'),{question,read:hint});
    const child=spawn(frameExe,[directory,path.join(active,'input.json'),active],{env,windowsHide:true,stdio:['ignore','pipe','pipe']});
    let diagnostic='';child.stderr.on('data',b=>diagnostic+=b);child.stdout.resume();
    assert.equal(await new Promise((r,j)=>{child.on('close',r);child.on('error',j);}),0,diagnostic);
    const projection=read(path.join(active,'read.json')),observations=read(path.join(active,'observations.json')),body=read(path.join(active,'http-body.json'));
    const png=readFileSync(path.join(active,'frame-0.png'));
    const images=body.messages.flatMap(m=>Array.isArray(m.content)?m.content:[]).filter(c=>c.type==='image_url');
    assert.equal(images[0].image_url.url,`data:image/png;base64,${png.toString('base64')}`);
    assert.equal(projection.frames[0].read.frame.actual_pts_ms,time);assert.deepEqual(observations[0].publication,link.reference);
    const source=contribution.frames.find(f=>f.frame.actual_pts_ms===time);
    const pixels=file=>execFileSync(process.env.FFMPEG||'ffmpeg',['-v','error','-i',file,'-f','rawvideo','-pix_fmt','rgb24','-'],{windowsHide:true,maxBuffer:16*1024*1024});
    assert(pixels(path.join(active,'frame-0.png')).equals(pixels(path.join(directory,source.path))));
    mkdirSync(path.join(out,'frames'),{recursive:true});copyFileSync(path.join(active,'frame-0.png'),path.join(out,'frames',`${time}.png`));
    checks.push({discourse_id:candidate.discourse_id,time,read_hint:hint,read:projection,observations,request_pixels_equal:true,source_pixels_equal:true});
  }
} finally {recorder.close();}
assert(checks.length>=2);
const runs=readFileSync(path.join(root,'runs.jsonl'),'utf8').trim().split('\n').map(JSON.parse).filter(r=>!r.reused);
save(path.join(out,'verification.json'),{status:'passed',publication:link.reference,input_publication:input.publication,
  question,queries,items:contribution.items.length,relations:contribution.items.reduce((n,i)=>n+i.relations.length,0),
  comparison:queries.map((q,i)=>({query:q.query,before:{visual:before.results[i].total_matches,discourse:before.results[i].discourse.total_matches},after:{visual:after.results[i].total_matches,discourse:after.results[i].discourse.total_matches}})),
  relation_only_candidate:expansion,matched_seed:seed,discourse_coverage:after.results[0].discourse.coverage,
  original_source_and_720_translations_preserved:true,prior_graph_and_visual_contributions_preserved:true,old_publication_unchanged:true,
  text_reads:after.text_reads,frame_requests:requests,checks,extraction_model_runs:runs.length,extraction_elapsed_ms:runs.reduce((n,r)=>n+r.elapsed_ms,0),
  usage:runs.map(r=>({task:r.task,usage:r.events.find(e=>e.type==='turn.completed')?.usage})),
  limitations:['Ablation establishes changed candidates and a relation-only route; no claim of reduced total reads or improved answer quality.',
    'Real model extracted the teaching structure. Read-time HTTP recorder verifies transport/observation, not live-model answers.','Bounded lecture segment; whole-lecture production remains VI18.']});
console.log(JSON.stringify({status:'passed',items:contribution.items.length,relation:via.relation.type,frame_requests:requests}));
