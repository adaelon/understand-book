import { afterEach, expect, it } from 'vitest';
import { mkdtempSync, mkdirSync, readFileSync, writeFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { admitVideoGraphCandidate, acceptVideoGraphCandidate, createVideoGraphSession, selectVideoGraphContext, type VideoGraphInput } from '../src/video-graph';
import { estimateTokens } from '../src/window';
import { mergeAndGate } from '../src/merge';
import { ReadOnlyBaseZ } from '../src/zod';
import { validateEvidenceTarget } from '../src/video-contract';
import { createBuildExecutorMcpSession } from '../../../skills/build/build-executor-mcp';
import { BUILD_EXECUTOR_BOOTSTRAP_CONTRACT_V3 as bootstrap } from '../src/build-executor-connection-capability';
const dirs:string[]=[];
afterEach(()=>{for(const d of dirs.splice(0)) rmSync(d,{recursive:true,force:true});});
function fixture() {
  const f=JSON.parse(readFileSync(new URL('../../../crates/base-schema/fixtures/video-contract.json',import.meta.url),'utf8')).cases[0];
  const frame=f.questions[0].frame, png=Buffer.alloc(24); Buffer.from([137,80,78,71,13,10,26,10]).copy(png);png.writeUInt32BE(frame.width,16);png.writeUInt32BE(frame.height,20);
  const input:VideoGraphInput={version:'video_graph_input.v1',input_ref:'frozen',batch_id:'joint',publication:f.questions[0].publication,
    manifest:f.manifest,base:f.base,source:f.source_text,transcript:f.map,locations:[f.map.segments[0].location],vu_ids:['vu'],existing_nodes:[],existing_edges:[],
    media:{version:'executor_media_input.v1',media_input_ref:'frozen',work_unit_id:'joint',stage:'video_graph',input_hash:'frozen',material:f.manifest.material,
      transcript:[f.map.segments[0]],images:[{image_id:'image_1',requested_ms:frame.actual_pts_ms,frame,region:null,width:frame.width,height:frame.height,png_bytes:png.length,data:png.toString('base64')}]}};
  const text={kind:'text',segment_id:f.map.segments[0].segment_id,support_role:'转写解释计算规则'};
  const visual={kind:'visual',image_id:'image_1',support_role:'画面提供输入维度和结果'};
  const c:any={input_ref:input.input_ref,nodes:[
    {local_id:'einsum',type:'concept',name:'einsum',meaning:'具名维度收缩运算',same_as:null,support:{mode:'joint',members:[text,visual]}},
    {local_id:'x',type:'entity',name:'x',meaning:'矩阵乘法示例的输入矩阵',same_as:null,support:{mode:'visual',members:[visual]}},
    {local_id:'other-x',type:'entity',name:'x',meaning:'浮点下溢示例的标量',same_as:null,support:{mode:'visual',members:[visual]}},
  ],edges:[{source:'einsum',target:'x',type:'applies',direction:'directed',weight:1,support:{mode:'joint',members:[text,visual]}}]};
  return {input,c};
}
function workspace(input:VideoGraphInput) {
  const dir=mkdtempSync(path.join(tmpdir(),'vi13-'));dirs.push(dir);
  mkdirSync(path.join(dir,'.build/video-graph/joint'),{recursive:true});
  writeFileSync(path.join(dir,'.build/video-graph/joint/input.json'),JSON.stringify(input));
  writeFileSync(path.join(dir,'video_source_manifest.json'),JSON.stringify(input.manifest)); return dir;
}
it('keeps pure visual objects without fake LIDs, same labels distinct and joint roles through schema serialization',()=>{
  const {input,c}=fixture(),out=admitVideoGraphCandidate(input,c);
  expect(out.nodes[1].occurrences).toEqual([]);expect(out.nodes[1].source_lid).toBeNull();
  expect(out.nodes[1].id).not.toBe(out.nodes[2].id);
  const base=ReadOnlyBaseZ.parse({...input.base,graph_nodes:out.nodes,graph_edges:out.edges});
  expect(base.graph_nodes[0].evidence?.[0].target.kind).toBe('joint');
  expect(base.graph_edges[0].evidence).toEqual(out.edges[0].evidence);
});
it('rejects unprovided sources, false joint support, input changes and invented object matches',()=>{
  const {input,c}=fixture();
  for(const mutate of [
    (x:any)=>x.input_ref='changed', (x:any)=>x.nodes[0].support.members[0].segment_id='undelivered',
    (x:any)=>x.nodes[1].support.members[0].image_id='undelivered', (x:any)=>x.nodes[0].support.members.pop(),
    (x:any)=>x.nodes[0].support.members[0].support_role='', (x:any)=>x.edges[0].target='missing',
    (x:any)=>x.nodes[1].same_as={node_id:'invented',reason:'same label'},
  ]){const bad=structuredClone(c);mutate(bad);expect(()=>admitVideoGraphCandidate(input,bad)).toThrow();}
});
it('explicit same-object reuse accumulates language and visual occurrences without merging homonyms',()=>{
  const {input,c}=fixture(),first=admitVideoGraphCandidate(input,c);
  input.existing_nodes=first.nodes;
  const next=structuredClone(c);next.nodes=[{...c.nodes[1],same_as:{node_id:first.nodes[1].id,reason:'明确为同一矩阵'},support:{mode:'text',members:[c.nodes[0].support.members[0]]}}];next.edges=[];
  const second=admitVideoGraphCandidate(input,next);
  const merged=mergeAndGate([first,second],input.base.lid_nodes,e=>{validateEvidenceTarget(e.target,input.manifest,input.base,input.source);return true;});
  expect(merged.nodes).toHaveLength(3);expect(merged.nodes[1].evidence).toHaveLength(2);
  expect(merged.nodes[1].occurrences).toHaveLength(1);
  next.nodes[0].meaning='同名但含义不同';expect(()=>admitVideoGraphCandidate(input,next)).toThrow('exact meaning');
});
it('generic graph gate rejects unsupported media and retains only accepted contributions',()=>{
  const {input,c}=fixture(),out=admitVideoGraphCandidate(input,c);
  expect(mergeAndGate([out],input.base.lid_nodes).nodes).toHaveLength(0);
  const merged=mergeAndGate([out],input.base.lid_nodes,e=>e.target.kind!=='joint');
  expect(merged.nodes).toHaveLength(2);expect(merged.edges).toHaveLength(0);
});
it('explicitly attaches a video occurrence to an existing Pass1 text object',()=>{
  const {input,c}=fixture();
  const original={id:'entity:matrix',type:'entity' as const,name:'matrix',occurrences:[input.media.transcript[0].text_range.lid],source_lid:null};
  input.existing_nodes=[original];
  c.nodes=[{...c.nodes[1],same_as:{node_id:original.id,reason:'本段代码明确展示同一个矩阵'}}];c.edges=[];
  const added=admitVideoGraphCandidate(input,c);
  const merged=mergeAndGate([{nodes:[original],edges:[]},added],input.base.lid_nodes,()=>true);
  expect(merged.nodes).toHaveLength(1);expect(merged.nodes[0].id).toBe(original.id);
  expect(merged.nodes[0].occurrences).toEqual(original.occurrences);expect(merged.nodes[0].evidence?.[0].target.kind).toBe('visual');
});
it('bounds a growing lecture graph while retaining relevant exact identities and their current support',()=>{
  const {input,c}=fixture(), first=admitVideoGraphCandidate(input,c);
  const growing:VideoGraphInput['existing_nodes']=Array.from({length:300},(_,i)=>({...first.nodes[0],id:`old-${i}`,name:`unrelated ${i}`,occurrences:[],identity:{kind:'video_object' as const,meaning:`unrelated object ${i}`}}));
  growing.push(first.nodes[0]);
  const before=structuredClone(growing), selected=selectVideoGraphContext(growing,input.media.transcript,'einsum');
  expect(estimateTokens(JSON.stringify(selected,null,2)+'\n')).toBeLessThanOrEqual(4000);
  expect(selected[0].id).toBe(first.nodes[0].id);
  expect(selected[0].identity).toEqual(first.nodes[0].identity);
  expect(selected[0].evidence).toBeUndefined();expect(growing).toEqual(before);
  input.existing_nodes=selected;
  c.nodes=[{...c.nodes[0],same_as:{node_id:selected[0].id,reason:'本段仍为同一算子'}}];c.edges=[];
  expect(admitVideoGraphCandidate(input,c).nodes[0].id).toBe(selected[0].id);
  c.nodes[0].meaning='不同含义';expect(()=>admitVideoGraphCandidate(input,c)).toThrow('exact meaning');
});
it('four-tool native input replays images, gates final acknowledgement, and publishes immutable support',()=>{
  const {input,c}=fixture(),dir=workspace(input),handoff='abhandoff1_'+'a'.repeat(64);
  const make=()=>createBuildExecutorMcpSession({bootstrap_version:bootstrap.version,protocol_generation:bootstrap.session_protocol,
    session_private_root:dir,execute_request:createVideoGraphSession(dir,'joint',handoff)});
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
  const file=path.join(dir,'video-enrichments/joint/graph.json'),before=readFileSync(file);
  acceptVideoGraphCandidate(dir,'joint',c);expect(readFileSync(file)).toEqual(before);
  const changed=structuredClone(c);changed.nodes[0].name='changed';expect(()=>acceptVideoGraphCandidate(dir,'joint',changed)).toThrow('cannot be replaced');
  expect(createVideoGraphSession(dir,'joint',handoff)({version:'automatic_build_executor_open_request.v3',opaque_handoff_ref:handoff}).action.kind).toBe('DONE');
});

it('returns bounded candidate feedback, permits correction, and stops after three invalid attempts',()=>{
  const {input,c}=fixture(),dir=workspace(input),handoff='handoff';
  const start=()=>{
    const session=createVideoGraphSession(dir,'joint',handoff);
    const open:any=session({version:'automatic_build_executor_open_request.v3',opaque_handoff_ref:handoff});
    const batch:any=session(open.action.next_request);
    const b=batch.action.batch;
    const grant:any=session({version:'automatic_build_executor_generation_start_request.v3',opaque_session_ref:b.opaque_session_ref,generation_input_ref:b.generation_input_ref,confirmed_through_ordinal:b.last_ordinal});
    return {session,request:{version:'automatic_build_executor_candidate_submit.v3',opaque_session_ref:grant.action.opaque_session_ref,candidate_sink_ref:grant.action.candidate_sink_ref}};
  };
  const bad=structuredClone(c);bad.nodes[0].support.members[0].segment_id='not-delivered';
  let {session,request}=start();
  const retry:any=session({...request,candidate:bad});
  expect(retry.action.kind).toBe('GENERATE');expect(retry.action.semantic_attempt).toBe(2);
  expect(retry.action.retry_feedback.expected).toMatch(/support|source|delivered/i);
  expect(readFileSync(path.join(dir,'.build/video-graph/joint/last-rejected-candidate.json'),'utf8')).toContain('not-delivered');
  expect(session({...request,candidate:bad}).action.kind).toBe('GENERATE');
  expect(session({...request,candidate:bad}).action).toEqual({kind:'DONE',status:'retryable_failure'});
  ({session,request}=start());
  expect(session({...request,candidate:bad}).action.kind).toBe('GENERATE');
  expect(session({...request,candidate:c}).action).toEqual({kind:'DONE',status:'committed'});
});
