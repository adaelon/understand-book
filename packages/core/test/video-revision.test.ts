import { afterEach, expect, it } from 'vitest';
import { execFileSync } from 'node:child_process';
import { existsSync, mkdtempSync, readFileSync, renameSync, rmSync, writeFileSync, mkdirSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { segment } from '../src/segment';
import { editVideoTranscript, saveCorrectionJson as save } from '../src/video-correction';
import { prepareVideoRevision, readVideoRevision, reuseRevisionTranslations, prepareRevisionVisual,
  revisionSemanticInput, prepareRevisionSemantic, revisionSemanticBatches } from '../src/video-revision';
import { prepareVideoTranslation, acceptVideoTranslation } from '../src/video-translation';
import { prepareVideoGraph, acceptVideoGraphCandidate } from '../src/video-graph';
import { validateTranscriptMap, validateMediaLocation } from '../src/video-contract';
import { buildReplacementTranscript, runVideoRevision } from '../../../skills/build/video-revision';
const roots: string[] = [];
afterEach(() => { for (const dir of roots.splice(0)) rmSync(dir, {recursive:true,force:true}); });
const read = (file: string) => JSON.parse(readFileSync(file,'utf8'));
function fixture(count = 70) {
  const root = mkdtempSync(path.join(tmpdir(),'vi17-')); roots.push(root);
  const old = path.join(root,'old'), next = path.join(root,'new'); mkdirSync(old);
  const f = read(fileURLToPath(new URL('../../../crates/base-schema/fixtures/video-contract.json',import.meta.url))).cases[0];
  let text = '';
  const blocks = Array.from({length:count},(_,i) => { const start = text.length, value = `A denser at step ${i}.`;
    text += value + '\n\n'; return {kind:'leaf' as const,text:value,span:{start,end:text.length}}; });
  f.base = {book_id:f.manifest.material.book_id,lid_nodes:segment(blocks),graph_nodes:[],graph_edges:[]};
  f.manifest.source.duration_ms = count * 1000;
  f.manifest.transcript_map_path = 'transcript_map.json'; f.manifest.canonical_text_path = 'source.txt';
  f.manifest.coverage = ['transcript','translation','visual_index','graph','discourse'].map(capability => ({capability,
    accepted:capability === 'transcript' ? [{video:f.manifest.material.video,start_ms:0,end_ms:count*1000}] : [],pending:[],failed:[]}));
  f.map = {transcript:f.manifest.material.transcript,raw_result_path:'transcript/raw_results.json',
    machine_transcription:{engine:'faster-whisper',model:'small.en',language:'en'},
    segments:blocks.map((b,i) => ({segment_id:'s'+i,text:b.text,text_range:{lid:f.base.lid_nodes[i].lid,range:{start:0,end:b.text.length}},
      location:{video:f.manifest.material.video,start_ms:i*1000,end_ms:i*1000+900},words:[]}))};
  save(path.join(old,'base.json'),f.base); save(path.join(old,'video_source_manifest.json'),f.manifest);
  save(path.join(old,'transcript_map.json'),f.map); save(path.join(old,'transcript/raw_results.json'),{original:true});
  mkdirSync(path.dirname(path.join(old,f.manifest.source.imported_path)),{recursive:true});
  writeFileSync(path.join(old,f.manifest.source.imported_path),'imported media'); writeFileSync(path.join(old,'source.txt'),text);
  const reference = {book_id:f.base.book_id,publication_id:'prior-publication'};
  save(path.join(old,'publication.json'),{reference}); save(path.join(old,'video_publication.json'),{reference,directory:'.'});
  const plan = prepareVideoTranslation(old);
  for (const t of plan.tasks) acceptVideoTranslation(old,t.id,{transcript:plan.material.transcript,
    translations:t.segment_ids.map(segment_id=>({segment_id,zh_text:'原译文 '+segment_id}))});
  function edit(index:number,after='A corrected tensor with changed length.') {
    const s=f.map.segments[index];editVideoTranscript(old,{segment_id:s.segment_id,expected_revision:0,before:s.text,after,reason:'test correction'});
  }
  return {root,old,next,f,plan,edit};
}
it('creates an independent source revision, rebuilds offsets, keeps times and resumes with the same identity',()=>{
  const {old,next,f,edit}=fixture();edit(0);
  const bytes=readFileSync(path.join(old,'source.txt')),prior=readFileSync(path.join(old,'transcript_map.json'));
  const state=prepareVideoRevision(old,next,{corrections:old});
  expect(state.book_id).not.toBe(f.base.book_id);
  const manifest=read(path.join(next,'video_source_manifest.json')),map=read(path.join(next,'transcript_map.json')),base=read(path.join(next,'base.json'));
  expect(manifest.previous_book_id).toBe(f.base.book_id);expect(manifest.material.video).toEqual(f.manifest.material.video);
  expect(map.transcript.transcript_revision).not.toBe(f.map.transcript.transcript_revision);
  expect(base.lid_nodes[1].span.start).not.toBe(f.base.lid_nodes[1].span.start);
  expect(map.segments.map((s:any)=>s.location)).toEqual(f.map.segments.map((s:any)=>s.location));
  expect(map.segments[0].words).toEqual([]);
  validateTranscriptMap(map,manifest,base,readFileSync(path.join(next,'source.txt'),'utf8'));
  expect(prepareVideoRevision(old,next,{corrections:old}).book_id).toBe(state.book_id);
  expect(readFileSync(path.join(old,'source.txt'))).toEqual(bytes);expect(readFileSync(path.join(old,'transcript_map.json'))).toEqual(prior);
});
it('recomputes an unedited translation task when its context-only neighbour changed and reuses the independent task',()=>{
  const {old,next,edit}=fixture();edit(32);
  prepareVideoRevision(old,next,{corrections:old});
  const plan=reuseRevisionTranslations(next);
  expect(plan.tasks.map(t=>t.status)).toEqual(['pending','pending','accepted']);
  expect(readVideoRevision(next).decisions.filter(d=>d.action==='reused').map(d=>d.task)).toEqual(['translation-003']);
});
it('resumes from its portable frozen inputs even after the original workspace moved',()=>{
  const {old,next,root,edit}=fixture();edit(0);prepareVideoRevision(old,next,{corrections:old});
  const moved=path.join(root,'moved');renameSync(old,path.join(root,'old-moved'));renameSync(next,moved);
  expect(reuseRevisionTranslations(moved).tasks.map(t=>t.status)).toEqual(['pending','accepted','accepted']);
  const state=readVideoRevision(moved);expect(state.previous_material.book_id).not.toBe(state.book_id);
});
it('inherits prior smaller translation cuts and keeps unfinished semantic tasks in the revision plan',()=>{
  const {old,next,edit}=fixture();
  const file=path.join(old,'.build/video-translation/plan.json'),plan=read(file),first=plan.tasks.shift();
  plan.tasks.unshift(...[first.segment_ids.slice(0,16),first.segment_ids.slice(16)].map((segment_ids,i)=>({
    ...first,id:'split-'+i,segment_ids,status:'pending'})));
  save(file,plan);
  for(const task of plan.tasks.slice(0,2))acceptVideoTranslation(old,task.id,{transcript:plan.material.transcript,
    translations:task.segment_ids.map((segment_id:string)=>({segment_id,zh_text:'原译文 '+segment_id}))});
  prepareVideoGraph(old,'unfinished',{vu_ids:[],segment_ids:['s1']});
  edit(0);prepareVideoRevision(old,next,{corrections:old});
  const current=reuseRevisionTranslations(next);
  expect(current.tasks.map(t=>t.id)).toEqual(['split-0','split-1','translation-002','translation-003']);
  expect(current.tasks.map(t=>t.status)).toEqual(['pending','accepted','accepted','accepted']);
  expect(revisionSemanticBatches(next,'graph')).toContain('unfinished');
});
it('rejects unadopted/no-op updates and changed requests to an existing revision workspace',()=>{
  const {old,next,edit}=fixture();expect(()=>prepareVideoRevision(old,next,{corrections:old})).toThrow('No adopted');
  edit(0);prepareVideoRevision(old,next,{corrections:old});edit(1);
  expect(()=>prepareVideoRevision(old,next,{corrections:old})).toThrow('input changed');
});
it('updates visual associations and recomputes a task whose nearby context changed',()=>{
  const {old,next,f,edit}=fixture();
  f.manifest.source.width=2;f.manifest.source.height=2;save(path.join(old,'video_source_manifest.json'),f.manifest);
  const frames=[10,50].map((s,i)=>({id:'f'+i,requested_ms:s*1000,frame:{video:f.manifest.material.video,actual_pts_ms:s*1000,width:2,height:2},
    path:`.build/video-visual/test/frames/f${i}.png`,png_bytes:24,content_id:'f'+i}));
  for(const frame of frames){const png=Buffer.alloc(24);Buffer.from([137,80,78,71,13,10,26,10]).copy(png);png.writeUInt32BE(2,16);png.writeUInt32BE(2,20);
    mkdirSync(path.dirname(path.join(old,frame.path)),{recursive:true});writeFileSync(path.join(old,frame.path),png);}
  const tasks=frames.map((frame,i)=>({id:'vu-'+i,location:{video:f.manifest.material.video,start_ms:frame.requested_ms,end_ms:frame.requested_ms+1000},
    frame_ids:[frame.id],context_ids:[],status:'accepted',attempts:1,failure:null}));
  const plan={version:'video_visual_plan.v1',analysis_id:'test',input_ref:'old',created_at:'2026-10-11',manifest:f.manifest,transcript:f.map,
    options:{content_rect:[0,0,1,1]},ranges:tasks.map(t=>t.location),scan:{},frames,tasks};
  save(path.join(old,'.build/video-visual/test/plan.json'),plan);
  for(const [i,t] of tasks.entries())save(path.join(old,`.build/video-visual/test/accepted/${t.id}.json`),[{
    vu_id:`test/${t.id}/1`,analysis_id:'test',location:t.location,representative_frames:[frames[i]],screen_text:'tensor',description:'张量',change:'示例',
    content_rect:[0,0,1,1],transcript_segment_ids:['s'+(i?50:10)],description_reused_from:null}]);
  edit(8);prepareVideoRevision(old,next,{corrections:old});
  expect(prepareRevisionVisual(next,'test').tasks.map(t=>t.status)).toEqual(['pending','accepted']);
  expect(read(path.join(next,'.build/video-visual/test/accepted/vu-1.json'))[0].transcript_segment_ids).toEqual(['s50']);
});
it('compares all semantic context including unreferenced transcript, nodes and exact frame bytes',()=>{
  const {old}=fixture();const input=prepareVideoGraph(old,'semantic',{vu_ids:[],segment_ids:['s1','s2']});
  const next=structuredClone(input);next.publication={book_id:'new',publication_id:'new-pub'};
  expect(revisionSemanticInput(input,'graph')).toEqual(revisionSemanticInput(next,'graph'));
  next.media.transcript[1].text='Context changed even if candidate never quoted it';
  expect(revisionSemanticInput(input,'graph')).not.toEqual(revisionSemanticInput(next,'graph'));
  next.media.transcript=structuredClone(input.media.transcript);next.existing_nodes.push({id:'new',type:'concept',name:'x',occurrences:[],source_lid:null});
  expect(revisionSemanticInput(input,'graph')).not.toEqual(revisionSemanticInput(next,'graph'));
  next.existing_nodes=[];
  input.media.images=[{image_id:'i',data:'original'} as any];next.media.images=[{image_id:'i',data:'changed'} as any];
  expect(revisionSemanticInput(input,'graph')).not.toEqual(revisionSemanticInput(next,'graph'));
});
it('readmits equivalent semantic results into the new publication and retains their reuse origin',()=>{
  const {old,next,edit}=fixture();const input=prepareVideoGraph(old,'semantic',{vu_ids:[],segment_ids:['s1']});
  acceptVideoGraphCandidate(old,'semantic',{input_ref:input.input_ref,nodes:[{local_id:'x',type:'concept',name:'x',meaning:'tensor',same_as:null,
    support:{mode:'text',members:[{kind:'text',segment_id:'s1',support_role:'定义'}]}}],edges:[]});
  edit(0);prepareVideoRevision(old,next,{corrections:old});
  const reference={book_id:readVideoRevision(next).book_id,publication_id:'new-pub'};
  save(path.join(next,'publication.json'),{reference});save(path.join(next,'video_publication.json'),{reference,directory:'.'});
  prepareRevisionSemantic(next,'graph','semantic');
  const graph=read(path.join(next,'video-enrichments/semantic/graph.json'));
  expect(graph.input_publication).toEqual(reference);expect(graph.nodes[0].evidence[0].target.publication).toEqual(reference);
  expect(graph.nodes[0].evidence[0].target.transcript.transcript_revision).toBe(readVideoRevision(next).transcript_revision);
  expect(readVideoRevision(next).decisions[0].action).toBe('reused');
});
it('replacement media is reimported/retranscribed, cannot use old timecodes, and a failed build preserves the old workspace',async()=>{
  const {old,next,root,f}=fixture();const media=path.join(root,'clip.mp4');
  execFileSync(process.env.FFMPEG||'ffmpeg',['-v','error','-f','lavfi','-i','color=s=160x90:r=10:d=3','-f','lavfi','-i','anullsrc=r=16000:cl=mono',
    '-t','3','-c:v','libx264','-pix_fmt','yuv420p','-c:a','aac',media],{windowsHide:true});
  prepareVideoRevision(old,next,{replacement:media});
  const manifest=await buildReplacementTranscript(next,{recognize:async(_audio,output)=>writeFileSync(output,JSON.stringify({
    engine:'faster-whisper',model:'small.en',language:'en',segments:[{text:'Tensor.',start:.5,end:1,words:[{word:'Tensor.',start:.5,end:1,probability:.9}]}]}))});
  expect(manifest.material.video.media_id).not.toBe(f.manifest.material.video.media_id);
  expect(manifest.previous_book_id).toBe(f.manifest.material.book_id);
  expect(read(path.join(next,'transcript_map.json')).segments[0].location.start_ms).toBe(500);
  expect(()=>validateMediaLocation(f.map.segments[0].location,manifest.source)).toThrow();
  expect(reuseRevisionTranslations(next).tasks.every(t=>t.status==='pending')).toBe(true);
  const before=readFileSync(path.join(old,'source.txt'));
  await expect(runVideoRevision(next,path.join(root,'missing-model.exe'),'missing-publisher',path.join(root,'service'))).rejects.toThrow();
  expect(readVideoRevision(next).stage).toBe('failed');expect(readFileSync(path.join(old,'source.txt'))).toEqual(before);
  expect(existsSync(path.join(next,'video_publication.json'))).toBe(false);
},30000);
