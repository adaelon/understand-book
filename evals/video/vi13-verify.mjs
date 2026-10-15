import { readFileSync, writeFileSync, mkdirSync, copyFileSync } from 'node:fs';
import { execFileSync, spawn } from 'node:child_process';
import { createServer } from 'node:http';
import path from 'node:path';
import assert from 'node:assert/strict';
const read=f=>JSON.parse(readFileSync(f,'utf8'));
const save=(f,v)=>{mkdirSync(path.dirname(f),{recursive:true});writeFileSync(f,JSON.stringify(v,null,2)+'\n');};
const workspace=path.resolve('evals/video/generated/vi11/workspace'),root=path.resolve('evals/video/generated/vi13');
const out=path.resolve('evals/video/results/vi13-20261010');
const link=read(path.join(workspace,'video_publication.json')),directory=path.resolve(link.directory);
const exe=process.env.VI13_GRAPH_EXE||path.join(process.env.CARGO_TARGET_DIR||'target','debug/examples/video_graph.exe');
const frameExe=process.env.VI13_FRAME_EXE||path.join(process.env.CARGO_TARGET_DIR||'target','debug/examples/video_frames_probe.exe');
const queries=['einsum','bfloat16','x','GPU序列号'];save(path.join(root,'queries.json'),queries);
function recall(dir){return JSON.parse(execFileSync(exe,[dir,path.join(root,'queries.json')],{encoding:'utf8',windowsHide:true,maxBuffer:16*1024*1024}));}
const graph=recall(directory);save(path.join(root,'recall.json'),graph);
const einsum=graph.results[0].candidates.find(c=>c.name.includes('einsum'));
const visual=graph.results[1].candidates.find(c=>c.name==='bfloat16');
assert(einsum&&visual);assert.equal(einsum.video_graph.evidence_status,'candidate_only');
assert.equal(visual.occurrences.length,0);
assert(graph.results[2].candidates.filter(n=>n.name==='x').length>=2,'same-name different-meaning objects must survive');
assert.equal(graph.results[3].error_code,'CONCEPT_NOT_FOUND');
const contributions=['graph-vi13-einsum','graph-vi13-visual','graph-vi13-text','graph-vi13-confirm'].map(batch=>({batch,
  input:read(path.join(workspace,'.build/video-graph',batch,'input.json')),
  accepted:read(path.join(workspace,'.build/video-graph',batch,'accepted.json')),
  contribution:read(path.join(directory,'video-enrichments',batch,'graph.json')),
  calls:readFileSync(path.join(workspace,'.build/video-graph',batch,'mcp-events.jsonl'),'utf8').trim().split('\n').map(JSON.parse)}));
const original=contributions[0].input.publication;
const oldDirectory=path.join(path.dirname(directory),original.publication_id);
assert.equal(recall(oldDirectory).nodes.length,0,'original publication remains unchanged');
for(const file of ['source.txt','base.json','transcript_map.json','transcript_translations.json']) assert(readFileSync(path.join(directory,file)).equals(readFileSync(path.join(oldDirectory,file))),file);
const manifest=read(path.join(directory,'video_source_manifest.json'));
const coverage=manifest.coverage.find(c=>c.capability==='graph');assert(coverage.accepted.length&&coverage.pending.length);
const map=read(path.join(directory,manifest.transcript_map_path));
for(const text of graph.text_reads) assert(text.text.includes(map.segments.find(s=>s.text_range.lid===text.lid).text));
const confirmations=contributions.flatMap(c=>c.accepted.candidate.nodes.filter(n=>n.same_as).map(n=>({batch:c.batch,...n.same_as})));
assert(confirmations.length>0,'a real repeat with pixels must explicitly resolve an existing object');
for(const c of confirmations) {
  const node=graph.nodes.find(n=>n.id===c.node_id);assert(node&&node.evidence.length>=2);
}
for(const c of contributions) {
  assert(!c.calls.some(call=>call.isError));
  assert.equal(c.calls.reduce((n,call)=>n+call.images,0),c.input.media.images.length);
  assert.deepEqual(c.calls.filter(c=>c.tool).map(c=>c.tool),['executor.open','executor.input.next','executor.generation.start','executor.submit_candidate']);
}
const jointRelations=graph.edges.filter(e=>e.evidence?.some(s=>s.target.kind==='joint'));assert(jointRelations.length);
let active,requests=0;
const recorder=createServer(async(req,res)=>{let body='';for await(const b of req)body+=b;save(path.join(active,'http-body.json'),JSON.parse(body));requests++;
  res.writeHead(200,{'Content-Type':'application/json'});res.end(JSON.stringify({choices:[{message:{content:'Transport observation receipt only.'}}]}));});
await new Promise(resolve=>recorder.listen(0,'127.0.0.1',resolve));
const env={...process.env,OPENCODE_API_KEY:'vi13-recorder',OPENCODE_BASE_URL:`http://127.0.0.1:${recorder.address().port}`,FLUID_LLM_MODEL:'vi13-recorder',UNDERSTAND_BOOK_PROVIDER:'native'};
const checks=[];
try {
  const seen=new Set();
  for(const candidate of [einsum,visual]) for(const hint of candidate.video_graph.read_hints) {
    const time=hint.sampling[0];if(seen.has(time))continue;seen.add(time);
    active=path.join(root,`frame-${time}`);mkdirSync(active,{recursive:true});save(path.join(active,'input.json'),{question:`Read support for ${candidate.name}`,read:hint});
    const child=spawn(frameExe,[directory,path.join(active,'input.json'),active],{env,windowsHide:true,stdio:['ignore','pipe','pipe']});
    let diagnostic='';child.stderr.on('data',b=>diagnostic+=b);child.stdout.resume();
    assert.equal(await new Promise((r,j)=>{child.on('close',r);child.on('error',j);}),0,diagnostic);
    const projection=read(path.join(active,'read.json')),observations=read(path.join(active,'observations.json')),body=read(path.join(active,'http-body.json'));
    const png=readFileSync(path.join(active,'frame-0.png'));
    const images=body.messages.flatMap(m=>Array.isArray(m.content)?m.content:[]).filter(c=>c.type==='image_url');
    assert.equal(images[0].image_url.url,`data:image/png;base64,${png.toString('base64')}`);
    assert.equal(projection.frames[0].read.frame.actual_pts_ms,time);
    assert.deepEqual(observations[0].publication,link.reference);
    const source=contributions.flatMap(c=>c.contribution.frames).find(f=>f.frame.actual_pts_ms===time);
    // PNG encoder settings can differ: compare decoded original-resolution pixels.
    const pixels=file=>execFileSync(process.env.FFMPEG||'ffmpeg',['-v','error','-i',file,'-f','rawvideo','-pix_fmt','rgb24','-'],{windowsHide:true,maxBuffer:16*1024*1024});
    assert(pixels(path.join(active,'frame-0.png')).equals(pixels(path.join(directory,source.path))));
    mkdirSync(path.join(out,'frames'),{recursive:true});copyFileSync(path.join(active,'frame-0.png'),path.join(out,'frames',`${time}.png`));
    checks.push({candidate:candidate.node_id,time,read_hint:hint,read:projection,observations,request_pixels_equal:true,source_pixels_equal:true});
  }
} finally {recorder.close();}
const runs=readFileSync(path.join(root,'runs.jsonl'),'utf8').trim().split('\n').map(JSON.parse).filter(r=>!r.reused);
save(path.join(out,'verification.json'),{status:'passed',publication:link.reference,input_publication:original,nodes:graph.nodes.length,edges:graph.edges.length,
  graph_coverage:coverage,original_source_and_720_translations_preserved:true,old_publication_unchanged:true,
  same_name_x:graph.results[2].candidates.filter(n=>n.name==='x').map(n=>({node_id:n.node_id,identity:n.video_graph.identity})),
  pure_visual_without_lids:true,confirmations,joint_relations:jointRelations,text_reads:graph.text_reads,frame_requests:requests,checks,
  extraction_model_runs:runs.length,extraction_elapsed_ms:runs.reduce((n,r)=>n+r.elapsed_ms,0),usage:runs.map(r=>({task:r.task,usage:r.events.find(e=>e.type==='turn.completed')?.usage})),
  limitations:['Graph extraction used real Codex native images and transcript. Read-time HTTP recorder verifies transport and observation, not a live-model semantic answer.','Graph coverage is bounded to these accepted work units, not the whole lecture.']});
console.log(JSON.stringify({status:'passed',nodes:graph.nodes.length,edges:graph.edges.length,confirmations:confirmations.length,frame_requests:requests}));
