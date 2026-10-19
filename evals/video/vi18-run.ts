/** Full-lecture acceptance production. Four isolated workspaces keep the existing
 * single-writer stage stores independent; only the parent assembles publications. */
import { cpSync, copyFileSync, existsSync, mkdirSync, readFileSync, writeFileSync, appendFileSync } from 'node:fs';
import { execFile } from 'node:child_process';
import { promisify } from 'node:util';
import path from 'node:path';
import { publishVideoEnrichmentBatch, type VideoEnrichments } from '../../packages/core/src/video-enrichment';
import { publishVideoVisualSnapshot } from '../../skills/build/video-visual-units';
import { planVideoLectureSemantics } from '../../packages/core/src/video-lecture';
import type { VisualUnit } from '../../packages/core/src/video-visual-units';

const read = (file: string) => JSON.parse(readFileSync(file, 'utf8'));
const save = (file: string, value: unknown) => { mkdirSync(path.dirname(file), {recursive:true}); writeFileSync(file, JSON.stringify(value,null,2)+'\n'); };
const run = promisify(execFile);
const root = path.resolve(process.env.VI18_OUTPUT || 'evals/video/generated/vi18');
const origin = path.resolve('evals/video/generated/vi11/workspace');
const [sidecar, codex, publisher, phase = 'visual'] = process.argv.slice(2).map((v,i) => i < 3 ? path.resolve(v) : v);
if (!sidecar || !codex || !publisher) throw new Error('Usage: vi18-run.ts SIDECAR CODEX PUBLISHER [visual|semantic]');
mkdirSync(root,{recursive:true});
const workspace = path.join(root,'workspace'), service = path.join(root,'service');
function initialize(destination: string) {
  if (existsSync(path.join(destination,'video_source_manifest.json'))) return;
  mkdirSync(destination,{recursive:true});
  for (const name of ['source.txt','base.json','video_import.json','transcript_map.json','video_enrichments.json','video_translation_progress.json','transcript','media','video-enrichments','video_source_manifest.json'])
    cpSync(path.join(origin,name),path.join(destination,name),{recursive:true,force:false});
}
initialize(workspace);
const manifest = read(path.join(workspace,'video_source_manifest.json'));
const duration = manifest.source.duration_ms;
const workers = Array.from({length:4},(_,i)=>({ id: i, workspace:path.join(root,`part-${i+1}`), analysis:`lecture18-part${i+1}`,
  start_ms:i*1200000, end_ms:Math.min((i+1)*1200000,duration) })).filter(w=>w.start_ms<duration);
save(path.join(root,'production.json'), {version:'vi18_production.v1',material:manifest.material,source:origin,duration_ms:duration,workers});
async function command(w: typeof workers[number], args: string[]) {
  const started = Date.now();
  try {
    const sourceEntry=sidecar.endsWith('.ts');
    const result = await run(sourceEntry?process.execPath:sidecar, sourceEntry?['--import','tsx',sidecar,...args]:args,
      { windowsHide:true, maxBuffer:16*1024*1024 });
    appendFileSync(path.join(root,'commands.jsonl'),JSON.stringify({worker:w.id,args,elapsed_ms:Date.now()-started,ok:true,stdout:result.stdout})+'\n');
  } catch (error) {
    appendFileSync(path.join(root,'commands.jsonl'),JSON.stringify({worker:w.id,args,elapsed_ms:Date.now()-started,ok:false,error:(error as Error).message})+'\n');
    throw error;
  }
}
function collect(w: typeof workers[number], capability: string) {
  const catalog:VideoEnrichments=read(path.join(w.workspace,'video_enrichments.json'));
  for (const batch of catalog.batches.filter(b=>b.batch_id.includes('lecture18-') && b.coverage.some(c=>c.capability===capability))) {
    const prefix=`video-enrichments/${batch.batch_id}/`;
    publishVideoEnrichmentBatch({workspace_dir:workspace,batch_id:batch.batch_id,material:catalog.material,coverage:batch.coverage,
      artifacts:Object.fromEntries(batch.files.map(file=>[file.slice(prefix.length),readFileSync(path.join(w.workspace,file))]))});
  }
}
if (phase==='visual') {
  save(path.join(root,'visual-result.json'), {status:'running'});
  // Decode serially: several full-resolution pixel caches plus native decoders
  // can exhaust this Windows host before model generation even starts.
  for (const w of workers) {
    initialize(w.workspace);
    const scan = path.join(w.workspace,'.build/video-visual',w.analysis,'scan.json');
    if (!existsSync(scan)) { mkdirSync(path.dirname(scan),{recursive:true}); copyFileSync(path.join(origin,'.build/video-visual/lecture-v1/scan.json'),scan); }
    const options=read(scan).scan.options, ranges=[];
    for(let start_ms=w.start_ms;start_ms<w.end_ms;start_ms+=30000) ranges.push({video:manifest.material.video,start_ms,end_ms:Math.min(start_ms+30000,w.end_ms)});
    const settings=path.join(root,`${w.analysis}.json`); save(settings,{options,ranges});
    await command(w,['video-visual-units','prepare',w.workspace,w.analysis,settings]);
    console.log(JSON.stringify({worker:w.id,prepared:true,tasks:read(path.join(w.workspace,'.build/video-visual',w.analysis,'plan.json')).tasks.length}));
  }
  const results=await Promise.allSettled(workers.map(async w=>{
    await command(w,['video-visual-units','run',w.workspace,w.analysis,codex]);
    const finalPlan=read(path.join(w.workspace,'.build/video-visual',w.analysis,'plan.json'));
    if(finalPlan.tasks.some((task:any)=>task.status!=='accepted')) throw new Error(`${w.analysis}: durable tasks remain incomplete`);
    console.log(JSON.stringify({worker:w.id,visual:'complete'}));
  }));
  for(const w of workers) if(existsSync(path.join(w.workspace,'video_enrichments.json'))) collect(w,'visual_index');
  const receipt=await publishVideoVisualSnapshot(workspace,publisher,service); save(path.join(root,'visual-publication.json'),receipt);
  const failed=results.filter(r=>r.status==='rejected');
  save(path.join(root,'visual-result.json'),{status:failed.length?'incomplete':'complete',failures:failed.map(r=>String((r as PromiseRejectedResult).reason)),publication:receipt.reference});
  if(failed.length) throw new Error('Full visual production incomplete; repeat the same command to resume accepted tasks');
} else if(phase==='semantic') {
  const visual=read(path.join(root,'visual-result.json'));
  if(visual.status!=='complete')throw new Error('Complete the visual phase first');
  for(const w of workers){
    const plan=read(path.join(w.workspace,'.build/video-visual',w.analysis,'plan.json'));
    if(plan.tasks.some((task:any)=>task.status!=='accepted'))throw new Error(`${w.analysis}: visual completion receipt disagrees with durable tasks`);
  }
  const link=read(path.join(workspace,'video_publication.json'));
  const published=path.resolve(workspace,link.directory);
  const catalog:VideoEnrichments=read(path.join(published,'video_enrichments.json'));
  const units:VisualUnit[]=catalog.batches.flatMap(b=>b.files.filter(f=>f.endsWith('/visual_units.json'))
    .flatMap(f=>read(path.join(published,f)).units)).filter(u=>u.analysis_id.startsWith('lecture18-'));
  const segments=read(path.join(workspace,'transcript_map.json')).segments;
  for(const w of workers){
    const selections=planVideoLectureSemantics(units.filter(u=>u.analysis_id===w.analysis),segments.filter((s:any)=>s.location.start_ms>=w.start_ms&&s.location.start_ms<w.end_ms));
    const planFile=path.join(w.workspace,'.build/lecture18-semantics.json');
    if(!existsSync(planFile))save(planFile,selections);
    save(path.join(w.workspace,'video_publication.json'),{...link,directory:published});
  }
  // A fully generated recovery needs one final snapshot, rather than materializing
  // another whole-media graph snapshot immediately before the discourse snapshot.
  const fullyAccepted=workers.every(w=>{
    const selections=read(path.join(w.workspace,'.build/lecture18-semantics.json'));
    const accepted:VideoEnrichments=read(path.join(w.workspace,'video_enrichments.json'));
    return ['graph','discourse'].every(stage=>selections.every((_:unknown,i:number)=>{
      const id=`lecture18-${stage}-p${w.id+1}-${String(i+1).padStart(3,'0')}`;
      return accepted.batches.some(b=>b.batch_id===id&&b.files.some(f=>f.endsWith(`/${stage}.json`)));
    }));
  });
  if(fullyAccepted){
    for(const stage of ['graph','discourse'])for(const w of workers)collect(w,stage);
    const receipt=await publishVideoVisualSnapshot(workspace,publisher,service);
    for(const stage of ['graph','discourse']){
      save(path.join(root,`${stage}-publication.json`),receipt);
      save(path.join(root,`${stage}-result.json`),{status:'complete',failures:[],publication:receipt.reference});
    }
    console.log(JSON.stringify({semantic:'complete',reused_all_contributions:true,publication:receipt.reference}));
  }else{
  for(const stage of ['graph','discourse']){
    const results=await Promise.allSettled(workers.map(async w=>{
      const selections=read(path.join(w.workspace,'.build/lecture18-semantics.json'));
      const jobs=selections.map((selection:any,i:number)=>({batch_id:`lecture18-${stage}-p${w.id+1}-${String(i+1).padStart(3,'0')}`,selection}));
      const request=path.join(w.workspace,'.build',`lecture18-${stage}-jobs.json`);save(request,jobs);
      await command(w,['video-semantic-series','run',w.workspace,stage,request,codex]);
      const accepted:VideoEnrichments=read(path.join(w.workspace,'video_enrichments.json'));
      for(const {batch_id:batch} of jobs){
        if(!accepted.batches.some(b=>b.batch_id===batch&&b.files.some(f=>f.endsWith(`/${stage}.json`))))
          throw new Error(`${batch}: process exited without an accepted contribution`);
      }
      console.log(JSON.stringify({worker:w.id,stage,accepted:jobs.length,total:jobs.length}));
    }));
    for(const w of workers)collect(w,stage);
    const receipt=await publishVideoVisualSnapshot(workspace,publisher,service);save(path.join(root,`${stage}-publication.json`),receipt);
    const failed=results.filter(r=>r.status==='rejected');
    save(path.join(root,`${stage}-result.json`),{status:failed.length?'incomplete':'complete',failures:failed.map(r=>String((r as PromiseRejectedResult).reason)),publication:receipt.reference});
    if(failed.length)throw new Error(`Full ${stage} production incomplete; repeat semantic to resume`);
  }
  }
} else throw new Error('Unknown production phase');
