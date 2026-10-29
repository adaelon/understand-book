import {appendFileSync,existsSync,mkdirSync,readFileSync,writeFileSync} from 'node:fs';
import {randomBytes} from 'node:crypto';
import {createInterface} from 'node:readline';
import {fileURLToPath} from 'node:url';
import path from 'node:path';
import {prepareVideoGraph,createVideoGraphSession} from '../../packages/core/src/video-graph';
import {prepareVideoDiscourse,createVideoDiscourseSession} from '../../packages/core/src/video-discourse';
import type {AutomaticBuildExecutorSessionResponse as Response} from '../../packages/core/src/automatic-build-executor-session';
import {BUILD_EXECUTOR_BOOTSTRAP_CONTRACT_V3 as bootstrap} from '../../packages/core/src/build-executor-connection-capability';
import {createBuildExecutorMcpSession} from './build-executor-mcp';
import {videoExecutorArgs,videoExecutorPrompt,videoExecutorStorageArgs,runVideoCodex,type VideoExecutorControl} from './video-executor-runtime';
import {createVideoExecutorCoordinator,proxyVideoExecutor,runVideoExecutorPool} from './video-executor-coordinator';

type Session=(request:unknown)=>Response;
export type VideoSemanticStage='graph'|'discourse';
export interface VideoSemanticJob {batch_id:string;selection:{vu_ids:string[];segment_ids:string[]}}
const script=fileURLToPath(import.meta.url);

/** Advance only after the current stage has durably admitted its candidate.
 * Each factory freezes its input after preceding contributions are available. */
export function sequenceVideoSemanticSessions(handoff:string,factories:Array<()=>Session>):Session {
  let current:Session|undefined,index=0;
  const open={version:'automatic_build_executor_open_request.v3',opaque_handoff_ref:handoff};
  function advance():Response {
    while(index<factories.length){
      current=factories[index++]();
      const response=current(open);
      if(response.action.kind!=='DONE'||response.action.status!=='committed')return response;
    }
    current=undefined;
    return {version:'automatic_build_executor_session.v3',action:{kind:'DONE',status:'committed'}};
  }
  return request=>{
    const r=request as Record<string,unknown>;
    if(r.version===open.version){
      if(r.opaque_handoff_ref!==handoff)throw new Error('Semantic series handoff mismatch');
      if(!current)return advance();
    }
    if(!current)throw new Error('Open the semantic series first');
    const response=current(request);
    return response.action.kind==='DONE'&&response.action.status==='committed'?advance():response;
  };
}

export async function runVideoSemanticSeries(workspace:string,stage:VideoSemanticStage,jobs:VideoSemanticJob[],executable:string,
  control:VideoExecutorControl={}) {
  workspace=path.resolve(workspace);
  const accepted=(job:VideoSemanticJob)=>existsSync(path.join(workspace,`video-enrichments/${job.batch_id}/${stage}.json`));
  const groups:Array<{index:number;jobs:VideoSemanticJob[]}>=[];
  for(let start=0;start<jobs.length;start+=3){
    const group=jobs.slice(start,start+3).filter(job=>!accepted(job));
    if(group.length)groups.push({index:start/3,jobs:group});
  }
  if(!groups.length)return;
  const privateRoot=path.join(workspace,'.build/video-semantic-series',stage);
  const launch=async(group:typeof groups[number],runControl:VideoExecutorControl,
    coordinator?:Awaited<ReturnType<typeof createVideoExecutorCoordinator>>)=>{
      runControl.signal?.throwIfAborted();
      const handoff='abhandoff1_'+randomBytes(32).toString('hex');
      const run=path.join(privateRoot,'runs',`${Date.now()}-${group.index}-${randomBytes(4).toString('hex')}`);mkdirSync(run,{recursive:true});
      const groupFile=path.join(run,'jobs.json');writeFileSync(groupFile,JSON.stringify(group.jobs));
      const endpoint=coordinator?.register(handoff,semanticSeriesSession(workspace,stage,group.jobs,handoff),
        (request,response)=>logExchange(path.join(run,'mcp-events.jsonl'),request,response));
      const args=['exec','--ignore-user-config','--skip-git-repo-check','--ephemeral','-C',run,...videoExecutorStorageArgs(workspace),
        '-c','model_reasoning_effort="low"','-c','features.shell_tool=false','-c','features.apps=false',
        '-c','developer_instructions='+JSON.stringify(videoExecutorPrompt()),
        '-c','mcp_servers.understand_book_build_executor.command='+JSON.stringify(process.execPath),
        '-c','mcp_servers.understand_book_build_executor.args='+JSON.stringify(videoExecutorArgs('video-semantic-series',script,
          ['serve',workspace,stage,groupFile,handoff,...(endpoint?[endpoint]:[])])),
        '-c','mcp_servers.understand_book_build_executor.required=true','-c','mcp_servers.understand_book_build_executor.default_tools_approval_mode="approve"',
        '--json','-o',path.join(run,'lifecycle.txt'),handoff];
      try {
        await runVideoCodex({executable,args,run_dir:run,
          logical_call_id:`video:${path.resolve(workspace)}:${stage}:${group.index}`,complete:()=>group.jobs.every(accepted)},runControl);
      } finally { coordinator?.unregister(handoff); }
      await runControl.onProgress?.();
      console.log(JSON.stringify({stage,accepted:jobs.filter(accepted).length,total:jobs.length}));
  };
  if(stage==='graph'){
    // Identity matching reads preceding graph contributions, including within a group.
    for(const group of groups)await launch(group,control);
    return;
  }
  // Discourse only needs local source evidence. Freeze all pending inputs from
  // this publication before either executor can admit a contribution.
  for(const group of groups)for(const job of group.jobs){
    control.signal?.throwIfAborted();
    prepareVideoDiscourse(workspace,job.batch_id,job.selection);
  }
  const coordinator=await createVideoExecutorCoordinator(privateRoot);
  try { await runVideoExecutorPool(groups,control.max_parallel??2,control,(group,runControl)=>launch(group,runControl,coordinator)); }
  finally { await coordinator.close(); }
}

function semanticSeriesSession(workspace:string,stage:VideoSemanticStage,jobs:VideoSemanticJob[],handoff:string):Session {
  return sequenceVideoSemanticSessions(handoff,jobs.map(job=>()=>{
    if(stage==='graph'){prepareVideoGraph(workspace,job.batch_id,job.selection);return createVideoGraphSession(workspace,job.batch_id,handoff);}
    return createVideoDiscourseSession(workspace,job.batch_id,handoff);
  }));
}

function logExchange(file:string,request:unknown,response:unknown) {
  const r=request as any,s=response as any;
  if(s===undefined)return;
  appendFileSync(file,JSON.stringify({at:new Date().toISOString(),method:r.method,
    tool:r.params?.name,isError:s.result?.isError??false,bytes:Buffer.byteLength(JSON.stringify(s)),
    images:s.result?.content?.filter((c:any)=>c.type==='image').length??0,
    ...(s.result?.isError?{diagnostic:s.result.content?.[0]?.text}:{})})+'\n');
}

export async function runVideoSemanticSeriesCli(argv:string[]) {
  const [mode,workspaceArg,stageArg,jobsFile,argument,endpoint]=argv,workspace=path.resolve(workspaceArg);
  if(stageArg!=='graph'&&stageArg!=='discourse')throw new Error('Expected graph or discourse stage');
  if(mode==='serve'&&endpoint)return proxyVideoExecutor(endpoint);
  const stage:VideoSemanticStage=stageArg,jobs:VideoSemanticJob[]=JSON.parse(readFileSync(jobsFile,'utf8'));
  if(mode==='serve'){
    if(jobs.length>3)throw new Error('Semantic series supports at most three tasks per session');
    if(stage==='discourse')for(const job of jobs)prepareVideoDiscourse(workspace,job.batch_id,job.selection);
    const handler=semanticSeriesSession(workspace,stage,jobs,argument);
    const privateRoot=path.join(workspace,'.build/video-semantic-series',stage);mkdirSync(privateRoot,{recursive:true});
    const session=createBuildExecutorMcpSession({bootstrap_version:bootstrap.version,protocol_generation:bootstrap.session_protocol,
      session_private_root:privateRoot,execute_request:handler});
    for await(const line of createInterface({input:process.stdin})){
      const request=JSON.parse(line),response=await session.handle_message(request);
      if(response!==undefined){
        logExchange(path.join(privateRoot,'mcp-events.jsonl'),request,response);
        process.stdout.write(JSON.stringify(response)+'\n');
      }
    }
  }else if(mode==='run'){
    await runVideoSemanticSeries(workspace,stage,jobs,path.resolve(argument));
  }else throw new Error('Expected run|serve WORKSPACE graph|discourse JOBS_JSON CODEX_EXE|HANDOFF');
}
if(process.argv[1]&&path.resolve(process.argv[1])===script)await runVideoSemanticSeriesCli(process.argv.slice(2));
