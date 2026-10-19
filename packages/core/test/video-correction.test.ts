import { afterEach, expect, it } from 'vitest';
import { mkdtempSync, mkdirSync, readFileSync, writeFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { admitVideoCorrection, decideVideoCorrection, editVideoTranscript, exportVideoCorrectionInput,
  inspectVideoCorrection, prepareVideoCorrection, readVideoCorrections, saveVideoCorrectionDraft, type VideoCorrectionInput } from '../src/video-correction';
import { createVideoCorrectionSession } from '../src/video-correction-executor';
import { createBuildExecutorMcpSession } from '../../../skills/build/build-executor-mcp';
import { formatVideoCorrectionReview, prepareVideoCorrectionRequest } from '../../../skills/build/video-correction';
import { BUILD_EXECUTOR_BOOTSTRAP_CONTRACT_V3 as bootstrap } from '../src/build-executor-connection-capability';

const dirs: string[] = [];
afterEach(() => {for(const d of dirs.splice(0)) rmSync(d,{recursive:true,force:true});});
function fixture() {
  const f=JSON.parse(readFileSync(new URL('../../../crates/base-schema/fixtures/video-contract.json',import.meta.url),'utf8')).cases[0];
  const dir=mkdtempSync(path.join(tmpdir(),'vi16-'));dirs.push(dir);
  const published=path.join(dir,'published');mkdirSync(published);
  for (const root of [dir,published]) for(const [file,value] of Object.entries({
    'video_source_manifest.json':f.manifest,'base.json':f.base,[f.manifest.transcript_map_path]:f.map,
    'source.txt':f.source_text,'publication.json':{reference:f.questions[0].publication},
  })) {const p=path.join(root,file);mkdirSync(path.dirname(p),{recursive:true});writeFileSync(p,typeof value==='string'?value:JSON.stringify(value));}
  writeFileSync(path.join(dir,'video_publication.json'),JSON.stringify({reference:f.questions[0].publication,directory:'published'}));
  const selection={segment_ids:f.map.segments.map((s:any)=>s.segment_id),vu_ids:[],question:'Fixture: inspect exact wording, keep uncertainty.'};
  const prepare=(draft='draft')=>prepareVideoCorrection(dir,draft,selection);
  return {dir,published,f,selection,prepare};
}
function candidate(input: VideoCorrectionInput) {
  const s=input.segments[0];
  const change=(id:string,before:string,after:string)=>({id,segment_id:s.segment_id,start:s.text.indexOf(before),before,after,
    reason:'可控合同夹具，非原片纠错真值',basis:'transcript_context',evidence:[{source_id:'text:'+s.segment_id,role:'原转写对照'}]});
  return {input_ref:input.input_ref,changes:[change('first','back','Back'),change('second','instability','instabilities')],
    notes:[{kind:'uncertain',text:'保留未确认听辨',evidence:[{source_id:'text:'+s.segment_id,role:'疑点语段'}]}]};
}
function manual(dir:string,id:string,after:string) {
  const s=readVideoCorrections(dir).segments.find(s=>s.segment_id===id)!;
  return editVideoTranscript(dir,{segment_id:id,expected_revision:s.edit_revision,before:s.text,after,reason:'人工文字编辑测试'});
}
it('manual edits and export preserve original publication, times and transcript identity',()=>{
  const {dir,published,f}=fixture(),before=readFileSync(path.join(dir,'source.txt')),originalMap=readFileSync(path.join(dir,f.manifest.transcript_map_path));
  const s=readVideoCorrections(dir).segments[0];
  manual(dir,s.segment_id,s.text.replace('back','Back'));
  const reopened=readVideoCorrections(dir);expect(reopened.revision).toBe(1);expect(reopened.segments[0].edit_revision).toBe(1);
  const out=exportVideoCorrectionInput(dir);expect(out.input.canonical_text).toContain('Back');expect(out.input.history[0].origin).toBe('manual');
  expect(out.input.source_material).toEqual(f.manifest.material);expect(out.input.segments[0].location).toEqual(s.location);
  expect(readFileSync(path.join(dir,'source.txt'))).toEqual(before);expect(readFileSync(path.join(published,'source.txt'))).toEqual(before);
  expect(readFileSync(path.join(dir,f.manifest.transcript_map_path))).toEqual(originalMap);
  expect(()=>editVideoTranscript(dir,{segment_id:s.segment_id,expected_revision:0,before:s.text,after:'stale editor',reason:'outdated input'})).toThrow('edit conflict');
  expect(readVideoCorrections(dir)).toEqual(reopened);
});
it('repeated invalid model candidates stop after three attempts without leaving a draft',()=>{
  const {dir,prepare}=fixture(),input=prepare(),c=candidate(input),handoff='abhandoff1_'+'b'.repeat(64);
  c.changes[0].start++;
  const execute=createVideoCorrectionSession(dir,'draft',handoff);
  const open=execute({version:'automatic_build_executor_open_request.v3',opaque_handoff_ref:handoff}).action;
  if(open.kind!=='DELIVER_INPUT') throw new Error('Expected input');
  const part=execute(open.next_request).action;if(part.kind!=='INPUT_BATCH') throw new Error('Expected batch');
  const b=part.batch;
  const grant=execute({version:'automatic_build_executor_generation_start_request.v3',opaque_session_ref:b.opaque_session_ref,generation_input_ref:b.generation_input_ref,confirmed_through_ordinal:b.last_ordinal}).action;
  if(grant.kind!=='GENERATE') throw new Error('Expected grant');
  const submit={version:'automatic_build_executor_candidate_submit.v3',opaque_session_ref:grant.opaque_session_ref,candidate_sink_ref:grant.candidate_sink_ref,candidate:c};
  expect(execute(submit).action.kind).toBe('GENERATE');expect(execute(submit).action.kind).toBe('GENERATE');
  expect(execute(submit).action).toEqual({kind:'DONE',status:'retryable_failure'});
  expect(readVideoCorrections(dir).drafts).toEqual([]);
});
it('candidate submission stays pending, partial adoption can continue in the same segment without losing prior choices',()=>{
  const {dir,prepare}=fixture(),input=prepare(),c=candidate(input),original=readVideoCorrections(dir).segments[0].text;
  saveVideoCorrectionDraft(dir,'draft',c);
  expect(readVideoCorrections(dir).segments[0].text).toBe(original);
  expect(inspectVideoCorrection(dir,'draft').changes.every(c=>c.status==='pending')).toBe(true);
  expect(decideVideoCorrection(dir,'draft','accept',['second']).applied).toEqual(['second']);
  expect(readVideoCorrections(dir).segments[0].text).toBe(original.replace('instability','instabilities'));
  expect(inspectVideoCorrection(dir,'draft').changes[0].conflict).toBe(false);
  expect(decideVideoCorrection(dir,'draft','accept',['first']).applied).toEqual(['first']);
  expect(readVideoCorrections(dir).segments[0].text).toBe(original.replace('back','Back').replace('instability','instabilities'));
  const state=readFileSync(path.join(dir,'transcript_corrections.json'));
  decideVideoCorrection(dir,'draft','accept','all');saveVideoCorrectionDraft(dir,'draft',c);
  expect(readFileSync(path.join(dir,'transcript_corrections.json'))).toEqual(state);
});
it('rejection preserves the original words and notes cannot enter exported speaker text',()=>{
  const {dir,prepare}=fixture(),input=prepare(),c=candidate(input);
  c.notes.push({kind:'background',text:'补充知识只作为注记',evidence:c.changes[0].evidence});
  saveVideoCorrectionDraft(dir,'draft',c);
  decideVideoCorrection(dir,'draft','reject',['first']);decideVideoCorrection(dir,'draft','accept','all');
  const out=exportVideoCorrectionInput(dir).input;
  expect(out.segments[0].text).toContain('back');expect(out.segments[0].text).toContain('instabilities');
  expect(out.canonical_text).not.toContain('补充知识');expect(out.adopted_drafts[0].changes.map(c=>c.id)).toEqual(['second']);
  expect(formatVideoCorrectionReview(dir,'draft')).toContain('rejected');
});
it('manual editing while a draft is being generated preserves its old input and leaves its suggestions pending',()=>{
  const {dir,prepare}=fixture(),input=prepare(),c=candidate(input);
  const replacement=input.segments[0].text+' Human edit.';manual(dir,input.segments[0].segment_id,replacement);
  expect(prepare().segments[0].text).toBe(input.segments[0].text);
  saveVideoCorrectionDraft(dir,'draft',c);
  expect(decideVideoCorrection(dir,'draft','accept','all').conflicts).toEqual(['first','second']);
  expect(readVideoCorrections(dir).segments[0].text).toBe(replacement);
  expect(inspectVideoCorrection(dir,'draft').changes.every(c=>c.status==='pending'&&c.conflict)).toBe(true);
  expect(formatVideoCorrectionReview(dir,'draft')).toContain(replacement);
  expect(prepare('fresh').segments[0].text).toBe(replacement);
});
it('an intervening manual edit remains a conflict even when the text is changed back',()=>{
  const {dir,prepare}=fixture(),input=prepare();saveVideoCorrectionDraft(dir,'draft',candidate(input));
  decideVideoCorrection(dir,'draft','accept',['first']);
  const s=readVideoCorrections(dir).segments[0];manual(dir,s.segment_id,s.text+' temporary');manual(dir,s.segment_id,s.text);
  expect(decideVideoCorrection(dir,'draft','accept',['second']).conflicts).toEqual(['second']);
  expect(inspectVideoCorrection(dir,'draft').changes[1].status).toBe('pending');
});
it('accept all applies unaffected segments but keeps conflicting segments pending',()=>{
  const {dir,prepare}=fixture(),input=prepare(),c=candidate(input),s=input.segments[1];
  c.changes.push({id:'other',segment_id:s.segment_id,start:0,before:s.text,after:s.text+' fixed',reason:'fixture',basis:'transcript_context',evidence:[{source_id:'text:'+s.segment_id,role:'context'}]});
  saveVideoCorrectionDraft(dir,'draft',c);manual(dir,input.segments[0].segment_id,input.segments[0].text+' manual');
  const result=decideVideoCorrection(dir,'draft','accept','all');expect(result.applied).toEqual(['other']);expect(result.conflicts).toEqual(['first','second']);
  const before=readFileSync(path.join(dir,'transcript_corrections.json'));
  expect(()=>decideVideoCorrection(dir,'draft','reject',['first','unknown'])).toThrow('Unknown');
  expect(readFileSync(path.join(dir,'transcript_corrections.json'))).toEqual(before);
});
it('rejects changed versions, source fabrication, wrong ranges, overlapping proposals and visual-only speech edits',()=>{
  const {dir,prepare,f}=fixture(),input=prepare(),c=candidate(input);
  for (const mutate of [
    (v:any)=>v.input_ref='wrong', (v:any)=>v.changes[0].before='wrong', (v:any)=>v.changes[0].start++,
    (v:any)=>v.changes[0].evidence[0].source_id='undelivered', (v:any)=>v.changes[0].basis='audio_review',
    (v:any)=>v.changes.push({...v.changes[0],id:'overlap'}), (v:any)=>v.changes[0].after=v.changes[0].before,
    (v:any)=>v.changes[0].evidence=[],
  ]) {const bad=structuredClone(c);mutate(bad);expect(()=>admitVideoCorrection(input,bad)).toThrow();}
  input.sources.push({source_id:'image_1',kind:'frame',target:{kind:'visual',publication:input.publication,frames:[],regions:[]}});
  const visual=structuredClone(c);visual.changes[0].evidence=[{source_id:'image_1',role:'visible only'}];
  expect(()=>admitVideoCorrection(input,visual)).toThrow('spoken wording');
  saveVideoCorrectionDraft(dir,'draft',c);const changed=structuredClone(c);changed.changes[0].after='changed';
  expect(()=>saveVideoCorrectionDraft(dir,'draft',changed)).toThrow('cannot be replaced');
  f.manifest.material.transcript.transcript_revision='new';writeFileSync(path.join(dir,'video_source_manifest.json'),JSON.stringify(f.manifest));
  expect(()=>decideVideoCorrection(dir,'draft','accept','all')).toThrow('material changed');
});
it('freezes authentic VI15 review metadata, keeps uncertainty, and rejects another publication or time range',()=>{
  const {dir,selection,f}=fixture();
  const s=f.map.segments[0],review={review_result_ref:'review-fixture',publication:f.questions[0].publication,location:s.location,
    question:'word?',reviewed_text:'Back in the day',reviewed_words:[],uncertainties:['Still uncertain'],provider:{engine:'test-fixture'},transcript:[s]};
  const input=prepareVideoCorrection(dir,'audio',{...selection,audio_reviews:[review]}),c=candidate(input);
  c.changes[0].basis='audio_review';c.changes[0].evidence=[{source_id:'audio:review-fixture',role:'machine result'}];
  saveVideoCorrectionDraft(dir,'audio',c);expect(formatVideoCorrectionReview(dir,'audio')).toContain('Still uncertain');
  for(const altered of [{...review,publication:{...review.publication,publication_id:'other'}},{...review,transcript:[]},
    {...review,location:{...review.location,start_ms:0,end_ms:1000}}]) {
    expect(()=>prepareVideoCorrection(dir,'bad'+Math.random().toString().slice(2),{...selection,audio_reviews:[altered]})).toThrow('does not match');
  }
});
it('four-tool delivery replays native images and saves only a pending draft after the final acknowledgement',()=>{
  const {dir,prepare}=fixture(),input=prepare(),c=candidate(input);
  const png=Buffer.alloc(24);Buffer.from([137,80,78,71,13,10,26,10]).copy(png);png.writeUInt32BE(1,16);png.writeUInt32BE(1,20);
  input.media.images=[{image_id:'image_1',requested_ms:578000,frame:{video:input.material.video,actual_pts_ms:578000,width:1,height:1},region:null,width:1,height:1,png_bytes:24,data:png.toString('base64')}];
  writeFileSync(path.join(dir,'.build/video-corrections/draft/input.json'),JSON.stringify(input));
  const handoff='abhandoff1_'+'a'.repeat(64);
  const make=()=>createBuildExecutorMcpSession({bootstrap_version:bootstrap.version,protocol_generation:bootstrap.session_protocol,
    session_private_root:dir,execute_request:createVideoCorrectionSession(dir,'draft',handoff)});
  let mcp=make(),id=0;
  const call=(name:string,args:unknown):any=>{const rpc=mcp.handle_message({jsonrpc:'2.0',id:++id,method:'tools/call',params:{name,arguments:args}}) as any;
    return {...JSON.parse(rpc.result.content[0].text),isError:rpc.result.isError,content:rpc.result.content};};
  let open=call('executor.open',{version:'automatic_build_executor_open_request.v3',opaque_handoff_ref:handoff});
  expect(call('executor.generation.start',{...open.action.next_request,version:'automatic_build_executor_generation_start_request.v3',confirmed_through_ordinal:-1}).isError).toBe(true);
  const first=call('executor.input.next',open.action.next_request);
  mcp=make();open=call('executor.open',{version:'automatic_build_executor_open_request.v3',opaque_handoff_ref:handoff});
  const part=call('executor.input.next',open.action.next_request);
  expect(part.content.filter((c:any)=>c.type==='image')).toEqual(first.content.filter((c:any)=>c.type==='image'));
  expect(part.content.filter((c:any)=>c.type==='image')[0].data).toBe(input.media.images[0].data);
  const b=part.action.batch,grant=call('executor.generation.start',{version:'automatic_build_executor_generation_start_request.v3',opaque_session_ref:b.opaque_session_ref,generation_input_ref:b.generation_input_ref,confirmed_through_ordinal:b.last_ordinal});
  const bad=structuredClone(c);bad.changes[0].start++;
  const retry=call('executor.submit_candidate',{version:'automatic_build_executor_candidate_submit.v3',opaque_session_ref:grant.action.opaque_session_ref,candidate_sink_ref:grant.action.candidate_sink_ref,candidate:bad});
  expect(retry.action.kind).toBe('GENERATE');expect(retry.action.retry_feedback.json_pointer).toBe('/changes/0');
  expect(readVideoCorrections(dir).drafts).toEqual([]);
  const done=call('executor.submit_candidate',{version:'automatic_build_executor_candidate_submit.v3',opaque_session_ref:grant.action.opaque_session_ref,candidate_sink_ref:grant.action.candidate_sink_ref,candidate:c});
  expect(done.isError).not.toBe(true);expect(done.action.status).toBe('committed');
  expect(readVideoCorrections(dir).revision).toBe(0);expect(inspectVideoCorrection(dir,'draft').changes.every(c=>c.status==='pending')).toBe(true);
  expect(createVideoCorrectionSession(dir,'draft',handoff)({version:'automatic_build_executor_open_request.v3',opaque_handoff_ref:handoff}).action.kind).toBe('DONE');
});
it('maintenance preparation resumes without redoing audio and rejects a changed correction request',async()=>{
  const {dir,selection}=fixture();
  const input=await prepareVideoCorrectionRequest(dir,'draft',selection);
  expect((await prepareVideoCorrectionRequest(dir,'draft',selection)).input_ref).toBe(input.input_ref);
  await expect(prepareVideoCorrectionRequest(dir,'draft',{...selection,question:'changed'})).rejects.toThrow('changed');
  await expect(prepareVideoCorrectionRequest(dir,'draft',{...selection,audio_requests:[{location:{},question:'new audio'}]})).rejects.toThrow('changed');
});
