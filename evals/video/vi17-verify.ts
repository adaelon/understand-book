import { readFileSync, existsSync, readdirSync } from 'node:fs';
import { execFileSync } from 'node:child_process';
import assert from 'node:assert/strict';
import path from 'node:path';
import { saveCorrectionJson as save } from '../../packages/core/src/video-correction';
import { readVideoRevision } from '../../packages/core/src/video-revision';
import { validateTranscriptMap } from '../../packages/core/src/video-contract';
import { runVideoRevision } from '../../skills/build/video-revision';
const read=(f:string)=>JSON.parse(readFileSync(f,'utf8'));
const root=path.resolve(process.env.VI17_OUTPUT||'evals/video/generated/vi17'),workspace=path.join(root,'workspace'),source=path.join(root,'source');
const out=path.resolve('evals/video/results/vi17-20261011');
const state=readVideoRevision(workspace);assert.equal(state.stage,'ready');
const oldLink=read(path.join(source,'video_publication.json')),newLink=read(path.join(workspace,'video_publication.json'));
const oldDir=path.resolve(source,oldLink.directory),newDir=path.resolve(workspace,newLink.directory);
const oldManifest=read(path.join(oldDir,'video_source_manifest.json')),newManifest=read(path.join(newDir,'video_source_manifest.json'));
assert.notEqual(oldLink.reference.book_id,newLink.reference.book_id);
assert.equal(newManifest.previous_book_id,oldLink.reference.book_id);
assert.deepEqual(newManifest.material.video,oldManifest.material.video);
assert.notEqual(newManifest.material.transcript.transcript_revision,oldManifest.material.transcript.transcript_revision);
const oldMap=read(path.join(oldDir,'transcript_map.json')),newMap=read(path.join(newDir,'transcript_map.json'));
const oldText=readFileSync(path.join(oldDir,'source.txt'),'utf8'),newText=readFileSync(path.join(newDir,'source.txt'),'utf8');
validateTranscriptMap(newMap,newManifest,read(path.join(newDir,'base.json')),newText);
assert.equal(newMap.segments.length,720);
const before=oldMap.segments.find((s:any)=>s.segment_id==='segment-82'),after=newMap.segments.find((s:any)=>s.segment_id==='segment-82');
assert(before.text.includes('denser'));assert(after.text.includes('tensor'));assert(!after.text.includes('denser'));
assert.equal(newText,oldText.replace(before.text,after.text));
assert.deepEqual(newMap.segments.map((s:any)=>s.location),oldMap.segments.map((s:any)=>s.location));
assert(newMap.segments.every((s:any)=>s.words.length===0));
const oldZh=read(path.join(oldDir,'transcript_translations.json')),newZh=read(path.join(newDir,'transcript_translations.json'));
assert.equal(newZh.length,720);const chinese=newZh.find((t:any)=>t.segment_id==='segment-82');
assert(chinese.zh_text.includes('张量'));assert(!chinese.zh_text.includes('denser'));
const plan=read(path.join(workspace,'.build/video-translation/plan.json'));
for(const decision of state.decisions.filter(d=>d.stage==='translation'&&d.action==='reused')) {
  const task=plan.tasks.find((t:any)=>t.id===decision.task);
  for(const id of task.segment_ids)assert.equal(newZh.find((t:any)=>t.segment_id===id).zh_text,oldZh.find((t:any)=>t.segment_id===id).zh_text);
}
const semantics:any={};
for(const stage of ['graph','discourse']) {
  const batch=`${stage}-vi17-tensor`,decision=state.decisions.find(d=>d.stage===stage&&d.task===batch);
  assert.equal(decision?.action,'recompute');
  const oldInput=read(path.join(source,`.build/video-${stage}/${batch}/input.json`));
  const newInput=read(path.join(workspace,`.build/video-${stage}/${batch}/input.json`));
  assert(oldInput.media.transcript.some((s:any)=>s.text.includes('denser')));
  assert(newInput.media.transcript.some((s:any)=>s.text.includes('tensor')));
  const oldResult=read(path.join(oldDir,`video-enrichments/${batch}/${stage}.json`)),newResult=read(path.join(newDir,`video-enrichments/${batch}/${stage}.json`));
  assert.equal(newResult.material.book_id,state.book_id);assert.equal(newResult.input_publication.book_id,state.book_id);
  assert(/张量|tensor/i.test(JSON.stringify(newResult)));
  semantics[stage]={before:oldResult,after:newResult,input_changed:true};
}
const frame=(dir:string)=>execFileSync(process.env.FFMPEG||'ffmpeg',['-v','error','-ss','550','-i',path.join(dir,newManifest.source.imported_path),
  '-frames:v','1','-f','rawvideo','-pix_fmt','rgb24','-'],{windowsHide:true,maxBuffer:16*1024*1024});
assert(frame(oldDir).equals(frame(newDir)));
const original=path.resolve('evals/video/generated/vi11/workspace');
for(const file of ['source.txt','base.json','transcript_map.json'])assert(readFileSync(path.join(original,file)).equals(readFileSync(path.join(source,file))));
const originalPublication=read(path.join(original,'video_publication.json'));
assert.equal(originalPublication.reference.publication_id,'01a125c2-840a-76d2-948c-0f427dd1ba88');
const saved=readFileSync(path.join(workspace,'video_publication.json'));
await runVideoRevision(workspace,'must-not-launch','must-not-publish',path.join(root,'service'));
assert(readFileSync(path.join(workspace,'video_publication.json')).equals(saved));
const runs:any[]=[];
function walk(dir:string,baseline:boolean) {
  if(!existsSync(dir))return;
  for(const e of readdirSync(dir,{withFileTypes:true})) {
    const file=path.join(dir,e.name);
    if(e.isDirectory())walk(file,baseline);
    else if(e.name==='run.json' && file.includes(`${path.sep}runs${path.sep}`))runs.push({baseline,file:path.relative(root,file).replaceAll('\\','/'),...read(file)});
  }
}
for(const stage of ['translation','visual','graph','discourse'])walk(path.join(workspace,'.build/video-'+stage),false);
for(const stage of ['graph','discourse'])for(const batch of [`${stage}-vi17-tensor`])walk(path.join(source,`.build/video-${stage}/${batch}/runs`),true);
const usage=runs.flatMap(r=>r.events??[]).filter(e=>e.type==='turn.completed').map(e=>e.usage);
save(path.join(out,'verification.json'),{status:'passed',old_publication:oldLink.reference,new_publication:newLink.reference,original_publication:originalPublication.reference,
  previous_book_id:newManifest.previous_book_id,material:newManifest.material,changed_segment:{before,after,old_chinese:oldZh.find((t:any)=>t.segment_id==='segment-82'),chinese},
  checks:{rebound_character_map:true,sentence_times_preserved:true,translation_context_invalidation:true,graph_and_discourse_recomputed:true,
    unchanged_complete_inputs_reused:true,old_publication_readable:true,original_canonical_files_unchanged:true,frame_pixels_equal:true,resume_without_model_or_publication:true},
  decisions:state.decisions,semantic_results:semantics,runs,model_elapsed_ms:runs.reduce((n,r)=>n+(r.elapsed_ms??0),0),
  usage:{input_tokens:usage.reduce((n,u)=>n+(u.input_tokens??0),0),cached_input_tokens:usage.reduce((n,u)=>n+(u.cached_input_tokens??0),0),output_tokens:usage.reduce((n,u)=>n+(u.output_tokens??0),0),api_cost:null},
  published_bytes:Object.values(read(path.join(newDir,'publication.json')).files).reduce((n:number,v:any)=>n+(typeof v==='number'?v:v.bytes??v.size_bytes??0),0),
  limitations:['Original model correction drafts remain pending; adopted wording is from the isolated VI16 evaluation copy.','Full lecture semantics and installation/DSH/Linux acceptance remain VI18 work.']});
console.log(JSON.stringify({status:'passed',old:oldLink.reference,new:newLink.reference,chinese:chinese.zh_text,model_runs:runs.length}));
