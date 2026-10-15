import assert from 'node:assert/strict';
import {existsSync,readFileSync,readdirSync,statSync,mkdirSync,writeFileSync} from 'node:fs';
import path from 'node:path';
const root=path.resolve(process.env.VI18_OUTPUT||'evals/video/generated/vi18');
const out=path.resolve('evals/video/results/vi18-20261011');
const read=(f:string)=>JSON.parse(readFileSync(f,'utf8').replace(/^\uFEFF/,''));
const production=read(path.join(root,'production.json')),workspace=path.join(root,'workspace');
for(const stage of ['visual','graph','discourse'])assert.equal(read(path.join(root,`${stage}-result.json`)).status,'complete',`${stage} production`);
const link=read(path.join(workspace,'video_publication.json')),directory=path.resolve(workspace,link.directory);
const publication=read(path.join(directory,'publication.json')),manifest=read(path.join(directory,'video_source_manifest.json'));
const map=read(path.join(directory,'transcript_map.json')),zh=read(path.join(directory,'transcript_translations.json'));
assert.deepEqual(publication.reference,link.reference);assert.equal(map.segments.length,720);
assert.deepEqual(new Set(zh.map((s:any)=>s.segment_id)),new Set(map.segments.map((s:any)=>s.segment_id)));
const catalog=read(path.join(directory,'video_enrichments.json'));
function intervals(capability:string){
  const merged:{start_ms:number;end_ms:number}[]=[];
  for(const r of catalog.batches.flatMap((b:any)=>b.coverage.filter((c:any)=>c.capability===capability).flatMap((c:any)=>c.accepted)).sort((a:any,b:any)=>a.start_ms-b.start_ms)){
    const last=merged.at(-1);if(last&&r.start_ms<=last.end_ms)last.end_ms=Math.max(last.end_ms,r.end_ms);else merged.push({start_ms:r.start_ms,end_ms:r.end_ms});
  }
  return merged;
}
const coverage=Object.fromEntries(['translation','visual_index','graph','discourse'].map(capability=>[capability,intervals(capability)]));
for(const capability of ['visual_index','graph','discourse'])assert.deepEqual(coverage[capability],[{start_ms:0,end_ms:manifest.source.duration_ms}],`${capability} full lecture`);
const files=catalog.batches.flatMap((b:any)=>b.files);
const artifacts=(suffix:string)=>files.filter((f:string)=>f.endsWith(suffix)).map((f:string)=>read(path.join(directory,f)));
const units=artifacts('/visual_units.json').flatMap((a:any)=>a.units),nodes=artifacts('/graph.json').flatMap((a:any)=>a.nodes),edges=artifacts('/graph.json').flatMap((a:any)=>a.edges);
const discourse=artifacts('/discourse.json').flatMap((a:any)=>a.items);
const interrupted=read(path.join(root,'interruption.json'));
const workerResults=production.workers.map((w:any)=>{
  const plan=read(path.join(w.workspace,'.build/video-visual',w.analysis,'plan.json'));
  assert(plan.tasks.every((t:any)=>t.status==='accepted'));
  for(const id of interrupted.accepted.find((p:any)=>p.part===w.id+1).task_ids){
    const task=plan.tasks.find((t:any)=>t.id===id);assert.equal(task.status,'accepted');assert.equal(task.attempts,1,'previously accepted task must not run again');
  }
  const selections=read(path.join(w.workspace,'.build/lecture18-semantics.json'));
  return {part:w.id+1,visual_prepared_at:plan.created_at,visual_tasks:plan.tasks.length,prepared_frames:plan.frames.length,semantic_tasks_per_stage:selections.length,
    segment_ids:[...new Set(selections.flatMap((s:any)=>s.segment_ids))]};
});
assert.deepEqual(new Set(workerResults.flatMap((w:any)=>w.segment_ids)),new Set(map.segments.map((s:any)=>s.segment_id)));
const runs:any[]=[];
function walk(dir:string){
  if(!existsSync(dir))return;
  for(const e of readdirSync(dir,{withFileTypes:true})){
    const file=path.join(dir,e.name);if(e.isDirectory())walk(file);
    else if(e.name==='run.json'&&file.includes(`${path.sep}runs${path.sep}`))runs.push({file:path.relative(root,file).replaceAll('\\','/'),...read(file)});
  }
}
for(const w of production.workers)for(const stage of ['visual','graph','discourse'])walk(path.join(w.workspace,`.build/video-${stage}`));
for(const w of production.workers)walk(path.join(w.workspace,'.build/video-semantic-series'));
const candidateFeedback=production.workers.flatMap((w:any)=>['graph','discourse'].flatMap(stage=>{
  const stageRoot=path.join(w.workspace,`.build/video-${stage}`);
  return readdirSync(stageRoot).filter(batch=>batch.startsWith(`lecture18-${stage}-`)).flatMap(batch=>{
    const file=path.join(stageRoot,batch,'last-rejected-candidate.json');
    return existsSync(file)?[{part:w.id+1,stage,batch,feedback:read(file).feedback,
      accepted:existsSync(path.join(w.workspace,`video-enrichments/${batch}/${stage}.json`))}]:[];
  });
}));
const usage=runs.flatMap(r=>r.events??[]).filter(e=>e.type==='turn.completed').map(e=>e.usage);
const source=path.resolve('evals/video/generated/vi11/workspace');
for(const name of ['source.txt','base.json','transcript_map.json'])assert(readFileSync(path.join(source,name)).equals(readFileSync(path.join(directory,name))));
assert.equal(statSync(path.join(directory,manifest.source.imported_path)).size,statSync(path.join(source,manifest.source.imported_path)).size);
const result={status:'passed',at:new Date().toISOString(),root,publication:link.reference,published_directory:directory,material:manifest.material,duration_ms:manifest.source.duration_ms,
  installed_semantic_runtime:read(path.join(root,'installed-semantic-runtime.json')),
  english_segments:map.segments.length,chinese_segments:zh.length,coverage,workers:workerResults,
  artifacts:{visual_units:units.length,lecture_visual_units:units.filter((u:any)=>u.analysis_id.startsWith('lecture18-')).length,
    graph_contribution_nodes:nodes.length,unique_graph_nodes:new Set(nodes.map((n:any)=>n.id)).size,graph_edges:edges.length,
    discourse_items:discourse.length,discourse_relations:discourse.reduce((n:number,i:any)=>n+i.relations.length,0)},
  checks:{all_spoken_segments_selected:true,all_visual_tasks_accepted:true,completed_visual_tasks_not_repeated:true,canonical_english_unchanged:true},
  runs:runs.map(({events,...run})=>({...run,turn_usage:(events??[]).filter((e:any)=>e.type==='turn.completed').map((e:any)=>e.usage)})),
  run_log_note:'Raw event logs remain at each run file under root; this report retains run metadata and reported turn usage.',
  candidate_feedback:candidateFeedback,
  elapsed_since_visual_preparation_ms:Date.now()-Math.min(...workerResults.map((w:any)=>Date.parse(w.visual_prepared_at))),
  model_elapsed_ms:runs.reduce((n,r)=>n+(r.elapsed_ms??0),0),
  usage:{input_tokens:usage.reduce((n,u)=>n+(u.input_tokens??0),0),cached_input_tokens:usage.reduce((n,u)=>n+(u.cached_input_tokens??0),0),output_tokens:usage.reduce((n,u)=>n+(u.output_tokens??0),0),
    api_cost:null,cost_note:'CLI reports token usage; monetary charges are not returned. Interrupted processes without a final usage event are excluded.'},
  published_bytes:Object.values(publication.files).reduce((n:number,bytes:any)=>n+bytes,0)};
mkdirSync(out,{recursive:true});writeFileSync(path.join(out,'verification.json'),JSON.stringify(result,null,2)+'\n');
console.log(JSON.stringify({...result,runs:result.runs.length,workers:result.workers.map(({segment_ids,...rest}:any)=>rest)}));
