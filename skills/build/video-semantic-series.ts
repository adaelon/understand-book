import {appendFileSync,existsSync,mkdirSync,readFileSync,writeFileSync} from 'node:fs';
import {spawn} from 'node:child_process';
import {randomBytes} from 'node:crypto';
import {createInterface} from 'node:readline';
import {fileURLToPath} from 'node:url';
import path from 'node:path';
import {prepareVideoGraph,createVideoGraphSession} from '../../packages/core/src/video-graph';
import {prepareVideoDiscourse,createVideoDiscourseSession} from '../../packages/core/src/video-discourse';
import type {AutomaticBuildExecutorSessionResponse as Response} from '../../packages/core/src/automatic-build-executor-session';
import {BUILD_EXECUTOR_BOOTSTRAP_CONTRACT_V3 as bootstrap} from '../../packages/core/src/build-executor-connection-capability';
import {createBuildExecutorMcpSession} from './build-executor-mcp';
import {videoExecutorArgs,videoExecutorPrompt,videoExecutorStorageArgs} from './video-executor-runtime';

type Session=(request:unknown)=>Response;
type Stage='graph'|'discourse';
interface Job {batch_id:string;selection:{vu_ids:string[];segment_ids:string[]}}
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

export async function runVideoSemanticSeriesCli(argv:string[]) {
  const [mode,workspaceArg,stageArg,jobsFile,argument]=argv,workspace=path.resolve(workspaceArg);
  if(stageArg!=='graph'&&stageArg!=='discourse')throw new Error('Expected graph or discourse stage');
  const stage:Stage=stageArg,jobs:Job[]=JSON.parse(readFileSync(jobsFile,'utf8'));
  const accepted=(job:Job)=>existsSync(path.join(workspace,`video-enrichments/${job.batch_id}/${stage}.json`));
  if(mode==='serve'){
    if(jobs.length>3)throw new Error('Semantic series supports at most three tasks per session');
    const handler=sequenceVideoSemanticSessions(argument,jobs.map(job=>()=>{
      if(stage==='graph'){prepareVideoGraph(workspace,job.batch_id,job.selection);return createVideoGraphSession(workspace,job.batch_id,argument);}
      prepareVideoDiscourse(workspace,job.batch_id,job.selection);return createVideoDiscourseSession(workspace,job.batch_id,argument);
    }));
    const privateRoot=path.join(workspace,'.build/video-semantic-series',stage);mkdirSync(privateRoot,{recursive:true});
    const session=createBuildExecutorMcpSession({bootstrap_version:bootstrap.version,protocol_generation:bootstrap.session_protocol,
      session_private_root:privateRoot,execute_request:handler});
    for await(const line of createInterface({input:process.stdin})){
      const request=JSON.parse(line),response=session.handle_message(request) as any;
      if(response!==undefined){
        appendFileSync(path.join(privateRoot,'mcp-events.jsonl'),JSON.stringify({at:new Date().toISOString(),method:request.method,
          tool:request.params?.name,isError:response.result?.isError??false,bytes:Buffer.byteLength(JSON.stringify(response)),
          images:response.result?.content?.filter((c:any)=>c.type==='image').length??0,
          ...(response.result?.isError?{diagnostic:response.result.content?.[0]?.text}:{})})+'\n');
        process.stdout.write(JSON.stringify(response)+'\n');
      }
    }
  }else if(mode==='run'){
    for(let start=0;start<jobs.length;start+=3){
      const group=jobs.slice(start,start+3).filter(job=>!accepted(job));
      if(!group.length)continue;
      const started=Date.now(),handoff='abhandoff1_'+randomBytes(32).toString('hex');
      const run=path.join(workspace,'.build/video-semantic-series',stage,'runs',String(started));mkdirSync(run,{recursive:true});
      const groupFile=path.join(run,'jobs.json');writeFileSync(groupFile,JSON.stringify(group));
      const args=['exec','--ignore-user-config','--skip-git-repo-check','--ephemeral','-C',run,...videoExecutorStorageArgs(workspace),
        '-c','model_reasoning_effort="low"','-c','features.shell_tool=false','-c','features.apps=false',
        '-c','developer_instructions='+JSON.stringify(videoExecutorPrompt()),
        '-c','mcp_servers.understand_book_build_executor.command='+JSON.stringify(process.execPath),
        '-c','mcp_servers.understand_book_build_executor.args='+JSON.stringify(videoExecutorArgs('video-semantic-series',script,['serve',workspace,stage,groupFile,handoff])),
        '-c','mcp_servers.understand_book_build_executor.required=true','-c','mcp_servers.understand_book_build_executor.default_tools_approval_mode="approve"',
        '--json','-o',path.join(run,'lifecycle.txt'),handoff];
      const child=spawn(path.resolve(argument),args,{windowsHide:true,stdio:['ignore','pipe','pipe']}),events:unknown[]=[];
      createInterface({input:child.stdout}).on('line',line=>{try{const e=JSON.parse(line);
        if(['turn.completed','turn.failed','error'].includes(e.type))events.push(e);
        else if(e.item)events.push({type:e.type,item_type:e.item.type,tool:e.item.tool,status:e.item.status});
      }catch{/* CLI diagnostic */}});
      child.stderr.on('data',bytes=>appendFileSync(path.join(run,'stderr.txt'),bytes));
      const exit=await new Promise<number|null>((resolve,reject)=>{child.on('close',resolve);child.on('error',reject);});
      const complete=group.every(accepted);
      writeFileSync(path.join(run,'run.json'),JSON.stringify({stage,batches:group.map(j=>j.batch_id),elapsed_ms:Date.now()-started,exit_code:exit,complete,events},null,2));
      if(!complete)throw new Error(`${stage} series incomplete; resume accepted contributions`);
      console.log(JSON.stringify({stage,accepted:Math.min(start+3,jobs.length),total:jobs.length}));
    }
  }else throw new Error('Expected run|serve WORKSPACE graph|discourse JOBS_JSON CODEX_EXE|HANDOFF');
}
if(process.argv[1]&&path.resolve(process.argv[1])===script)await runVideoSemanticSeriesCli(process.argv.slice(2));
