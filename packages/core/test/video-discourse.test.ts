import { afterEach, expect, it } from 'vitest';
import { mkdtempSync, mkdirSync, readFileSync, writeFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { admitVideoDiscourseCandidate, acceptVideoDiscourseCandidate, createVideoDiscourseSession, prepareVideoDiscourse, type VideoDiscourseInput } from '../src/video-discourse';
import { createBuildExecutorMcpSession } from '../../../skills/build/build-executor-mcp';
import { BUILD_EXECUTOR_BOOTSTRAP_CONTRACT_V3 as bootstrap } from '../src/build-executor-connection-capability';
const dirs:string[]=[];
afterEach(()=>{for(const d of dirs.splice(0)) rmSync(d,{recursive:true,force:true});});
function fixture() {
  const f=JSON.parse(readFileSync(new URL('../../../crates/base-schema/fixtures/video-contract.json',import.meta.url),'utf8')).cases[0];
  const frame=f.questions[0].frame, png=Buffer.alloc(24); Buffer.from([137,80,78,71,13,10,26,10]).copy(png);png.writeUInt32BE(frame.width,16);png.writeUInt32BE(frame.height,20);
  const input:VideoDiscourseInput={version:'video_graph_input.v1',input_ref:'frozen',batch_id:'joint',publication:f.questions[0].publication,
    manifest:f.manifest,base:f.base,source:f.source_text,transcript:f.map,locations:[f.map.segments[0].location],vu_ids:['vu'],existing_nodes:[],existing_edges:[],visual_units:[{vu_id:'vu',image_ids:['image_1']}],
    media:{version:'executor_media_input.v1',media_input_ref:'frozen',work_unit_id:'joint',stage:'video_graph',input_hash:'frozen',material:f.manifest.material,
      transcript:[f.map.segments[0]],images:[{image_id:'image_1',requested_ms:frame.actual_pts_ms,frame,region:null,width:frame.width,height:frame.height,png_bytes:png.length,data:png.toString('base64')}]}};
  const text={kind:'text',segment_id:f.map.segments[0].segment_id,support_role:'转写解释计算规则'};
  const visual={kind:'visual',image_id:'image_1',support_role:'画面提供输入维度和结果'};
  const c:any={input_ref:input.input_ref,items:[
    {local_id:'rule',mode:'informative',local_function:'explanation',local_summary:'hidden 求和规则',vu_ids:[],support:{mode:'text',members:[text]},relations:[]},
    {local_id:'example',mode:'informative',local_function:'example',rhetorical_move:'worked_example',local_summary:'输出矩阵实例',vu_ids:['vu'],support:{mode:'visual',members:[visual]},
      relations:[{target_id:'rule',type:'exemplifies',direction:'backward',confidence:0.9,rationale:'结果展示被消去的维度',support:{mode:'joint',members:[text,visual]}}]},
    {local_id:'limit',mode:'informative',local_function:'limitation',local_summary:'输出只保留显式标签',vu_ids:['vu'],support:{mode:'visual',members:[visual]},relations:[]},
  ]};
  return {input,c};
}
function workspace(input:VideoDiscourseInput) {
  const dir=mkdtempSync(path.join(tmpdir(),'vi13-'));dirs.push(dir);
  mkdirSync(path.join(dir,'.build/video-graph/joint'),{recursive:true});
  mkdirSync(path.join(dir,'.build/video-discourse/joint'),{recursive:true});
  writeFileSync(path.join(dir,'.build/video-discourse/joint/input.json'),JSON.stringify(input));
  writeFileSync(path.join(dir,'.build/video-graph/joint/input.json'),JSON.stringify(input));
  writeFileSync(path.join(dir,'video_source_manifest.json'),JSON.stringify(input.manifest)); return dir;
}

it('prepares and resumes a pure transcript task before a visual catalog exists',()=>{
  const {input,c}=fixture(),dir=workspace(input),published=path.join(dir,'published');
  mkdirSync(published,{recursive:true});
  for(const [file,value] of Object.entries({'publication.json':{reference:input.publication},'base.json':input.base,
    'video_source_manifest.json':input.manifest,[input.manifest.transcript_map_path]:input.transcript})) {
    const target=path.join(published,file);mkdirSync(path.dirname(target),{recursive:true});writeFileSync(target,JSON.stringify(value));
  }
  writeFileSync(path.join(published,'source.txt'),input.source);
  writeFileSync(path.join(dir,'video_publication.json'),JSON.stringify({reference:input.publication,directory:'published'}));
  const selection={vu_ids:[],segment_ids:[input.media.transcript[0].segment_id]};
  const prepared=prepareVideoDiscourse(dir,'text',selection);
  expect(prepared.media.images).toEqual([]);expect(prepared.visual_units).toEqual([]);
  expect(prepareVideoDiscourse(dir,'text',selection).input_ref).toBe(prepared.input_ref);
  expect(admitVideoDiscourseCandidate(prepared,{input_ref:prepared.input_ref,items:[c.items[0]]})).toHaveLength(1);
});

it('preserves independent teaching roles, many-to-many VUs and both endpoint sources',()=>{
  const {input,c}=fixture();
  input.visual_units.push({vu_id:'second',image_ids:['image_1']}); c.items[1].vu_ids.push('second');
  const items=admitVideoDiscourseCandidate(input,c);
  expect(items[1].vu_ids).toEqual(['vu','second']); expect(items[2].vu_ids).toEqual(['vu']);
  expect(items[1]).not.toHaveProperty('lid');
  expect(items[1].relations[0].target_id).toBe(items[0].id);
  expect(items[1].relations[0].evidence[0].target.kind).toBe('joint');
});
it('rejects missing endpoint modalities, undelivered media, invalid semantics and unsupported VU links',()=>{
  const {input,c}=fixture();
  for(const mutate of [
    (x:any)=>x.input_ref='changed',
    (x:any)=>x.items[1].relations[0].support={mode:'visual',members:[c.items[1].support.members[0]]},
    (x:any)=>x.items[1].relations[0].target_id='missing',
    (x:any)=>x.items[1].relations[0].rationale='',
    (x:any)=>x.items[1].relations[0].confidence=0.2,
    (x:any)=>x.items[1].relations[0].type='nearby',
    (x:any)=>x.items[1].support.members[0].image_id='undelivered',
    (x:any)=>x.items[1].vu_ids=['unknown'],
    (x:any)=>x.items[0].vu_ids=['vu'],
    (x:any)=>x.items[0].local_function='invented',
    (x:any)=>x.items[0].support.members[0].segment_id='missing',
  ]) {const bad=structuredClone(c);mutate(bad);expect(()=>admitVideoDiscourseCandidate(input,bad)).toThrow();}
});
it('four-tool native input replays images, gates final acknowledgement, and publishes immutable support',()=>{
  const {input,c}=fixture(),dir=workspace(input),handoff='abhandoff1_'+'a'.repeat(64);
  const make=()=>createBuildExecutorMcpSession({bootstrap_version:bootstrap.version,protocol_generation:bootstrap.session_protocol,
    session_private_root:dir,execute_request:createVideoDiscourseSession(dir,'joint',handoff)});
  let mcp=make(),id=0;
  const call=(name:string,args:unknown):any=>{const rpc=mcp.handle_message({jsonrpc:'2.0',id:++id,method:'tools/call',params:{name,arguments:args}}) as any;
    return {...JSON.parse(rpc.result.content[0].text),isError:rpc.result.isError,content:rpc.result.content};};
  let open=call('executor.open',{version:'automatic_build_executor_open_request.v3',opaque_handoff_ref:handoff});
  const premature=call('executor.generation.start',{...open.action.next_request,version:'automatic_build_executor_generation_start_request.v3',confirmed_through_ordinal:-1});
  expect(premature.isError).toBe(true);
  let part=call('executor.input.next',open.action.next_request);
  expect(part.content.filter((c:any)=>c.type==='image')[0].data).toBe(input.media.images[0].data);
  mcp=make();open=call('executor.open',{version:'automatic_build_executor_open_request.v3',opaque_handoff_ref:handoff});
  part=call('executor.input.next',open.action.next_request);
  expect(part.content.filter((c:any)=>c.type==='image')[0].data).toBe(input.media.images[0].data);
  const b=part.action.batch;
  const grant=call('executor.generation.start',{version:'automatic_build_executor_generation_start_request.v3',opaque_session_ref:b.opaque_session_ref,generation_input_ref:b.generation_input_ref,confirmed_through_ordinal:b.last_ordinal});
  const done=call('executor.submit_candidate',{version:'automatic_build_executor_candidate_submit.v3',opaque_session_ref:grant.action.opaque_session_ref,candidate_sink_ref:grant.action.candidate_sink_ref,candidate:c});
  expect(done.isError).not.toBe(true);expect(done.action.status).toBe('committed');
  const file=path.join(dir,'video-enrichments/joint/discourse.json'),before=readFileSync(file);
  acceptVideoDiscourseCandidate(dir,'joint',c);expect(readFileSync(file)).toEqual(before);
  const changed=structuredClone(c);changed.items[0].local_summary='changed';expect(()=>acceptVideoDiscourseCandidate(dir,'joint',changed)).toThrow('cannot be replaced');
  expect(createVideoDiscourseSession(dir,'joint',handoff)({version:'automatic_build_executor_open_request.v3',opaque_handoff_ref:handoff}).action.kind).toBe('DONE');
});

it('returns bounded candidate feedback, permits correction, and stops after three invalid attempts',()=>{
  const {input,c}=fixture(),dir=workspace(input),handoff='handoff';
  const start=()=>{
    const session=createVideoDiscourseSession(dir,'joint',handoff);
    const open:any=session({version:'automatic_build_executor_open_request.v3',opaque_handoff_ref:handoff});
    const batch:any=session(open.action.next_request);
    const b=batch.action.batch;
    const grant:any=session({version:'automatic_build_executor_generation_start_request.v3',opaque_session_ref:b.opaque_session_ref,generation_input_ref:b.generation_input_ref,confirmed_through_ordinal:b.last_ordinal});
    return {session,request:{version:'automatic_build_executor_candidate_submit.v3',opaque_session_ref:grant.action.opaque_session_ref,candidate_sink_ref:grant.action.candidate_sink_ref}};
  };
  const bad=structuredClone(c);bad.items[0].support.members[0].segment_id='not-delivered';
  let {session,request}=start();
  const retry:any=session({...request,candidate:bad});
  expect(retry.action.kind).toBe('GENERATE');expect(retry.action.semantic_attempt).toBe(2);
  expect(retry.action.retry_feedback.expected).toMatch(/support|source|delivered/i);
  expect(readFileSync(path.join(dir,'.build/video-discourse/joint/last-rejected-candidate.json'),'utf8')).toContain('not-delivered');
  expect(session({...request,candidate:bad}).action.kind).toBe('GENERATE');
  expect(session({...request,candidate:bad}).action).toEqual({kind:'DONE',status:'retryable_failure'});
  ({session,request}=start());
  expect(session({...request,candidate:bad}).action.kind).toBe('GENERATE');
  expect(session({...request,candidate:c}).action).toEqual({kind:'DONE',status:'committed'});
});
