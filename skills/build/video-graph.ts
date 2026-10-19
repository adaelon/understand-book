import { appendFileSync, existsSync, mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { spawn } from 'node:child_process';
import { randomBytes } from 'node:crypto';
import { createInterface } from 'node:readline';
import { fileURLToPath } from 'node:url';
import path from 'node:path';
import { videoExecutorArgs, videoExecutorPrompt, videoExecutorStorageArgs } from './video-executor-runtime';
import { createBuildExecutorMcpSession } from './build-executor-mcp';
import { BUILD_EXECUTOR_BOOTSTRAP_CONTRACT_V3 as bootstrap } from '../../packages/core/src/build-executor-connection-capability';
import { createVideoGraphSession, prepareVideoGraph, readVideoGraphInput } from '../../packages/core/src/video-graph';
import { publishVideoVisualSnapshot } from './video-visual-units';

const script=fileURLToPath(import.meta.url);
export async function runVideoGraph(workspace:string,batch:string,executable:string) {
  workspace=path.resolve(workspace); readVideoGraphInput(workspace,batch);
  if(existsSync(path.join(workspace,`video-enrichments/${batch}/graph.json`))) return {reused:true};
  const started=Date.now(), handoff='abhandoff1_'+randomBytes(32).toString('hex');
  const run=path.join(workspace,'.build/video-graph',batch,'runs',String(started)); mkdirSync(run,{recursive:true});
  const role=videoExecutorPrompt();
  const args=['exec','--ignore-user-config','--skip-git-repo-check','--ephemeral','-C',run, ...videoExecutorStorageArgs(workspace),
    '-c','model_reasoning_effort="low"','-c','features.shell_tool=false','-c','features.apps=false',
    '-c','developer_instructions='+JSON.stringify(role),
    '-c','mcp_servers.understand_book_build_executor.command='+JSON.stringify(process.execPath),
    '-c','mcp_servers.understand_book_build_executor.args='+JSON.stringify(videoExecutorArgs('video-graph', script, ['serve',workspace,batch,handoff])),
    '-c','mcp_servers.understand_book_build_executor.required=true',
    '-c','mcp_servers.understand_book_build_executor.default_tools_approval_mode="approve"',
    '--json','-o',path.join(run,'lifecycle.txt'),handoff];
  const child=spawn(executable,args,{windowsHide:true,stdio:['ignore','pipe','pipe']});
  const events:unknown[]=[];
  createInterface({input:child.stdout}).on('line',line=>{try{const e=JSON.parse(line);
    if(['turn.completed','turn.failed','error'].includes(e.type)) events.push(e);
    else if(e.item) events.push({type:e.type,item_type:e.item.type,tool:e.item.tool,status:e.item.status});
  }catch{/* diagnostic */}});
  child.stderr.on('data',bytes=>appendFileSync(path.join(run,'stderr.txt'),bytes));
  const exit=await new Promise<number|null>((resolve,reject)=>{child.on('close',resolve);child.on('error',reject);});
  const complete=existsSync(path.join(workspace,`video-enrichments/${batch}/graph.json`));
  const receipt={elapsed_ms:Date.now()-started,exit_code:exit,complete,events};
  writeFileSync(path.join(run,'run.json'),JSON.stringify(receipt,null,2));
  if(!complete) throw new Error('Graph executor incomplete; resume the same command');
  return receipt;
}
export async function runVideoGraphCli(argv: string[]) {
  const [mode,workspaceArg,batch,argument,publisher,service]=argv,workspace=path.resolve(workspaceArg);
  if(mode==='serve') {
    const session=createBuildExecutorMcpSession({bootstrap_version:bootstrap.version,protocol_generation:bootstrap.session_protocol,
      session_private_root:path.join(workspace,'.build/video-graph',batch),execute_request:createVideoGraphSession(workspace,batch,argument)});
    for await(const line of createInterface({input:process.stdin})) {
      const request=JSON.parse(line),response=session.handle_message(request) as any;
      if(response!==undefined) {
        appendFileSync(path.join(workspace,'.build/video-graph',batch,'mcp-events.jsonl'),JSON.stringify({at:new Date().toISOString(),method:request.method,
          tool:request.params?.name,isError:response.result?.isError??false,bytes:Buffer.byteLength(JSON.stringify(response)),
          images:response.result?.content?.filter((c:any)=>c.type==='image').length??0,
          ...(response.result?.isError?{diagnostic:response.result.content?.[0]?.text}:{})})+'\n');
        process.stdout.write(JSON.stringify(response)+'\n');
      }
    }
  } else if(mode==='prepare') {
    const input=prepareVideoGraph(workspace,batch,JSON.parse(readFileSync(argument,'utf8')));
    console.log(JSON.stringify({batch,input_ref:input.input_ref,images:input.media.images.length,segments:input.media.transcript.length}));
  } else if(mode==='run') {
    console.log(JSON.stringify(await runVideoGraph(workspace,batch,path.resolve(argument))));
    if(publisher&&service) console.log(JSON.stringify(await publishVideoVisualSnapshot(workspace,path.resolve(publisher),service)));
  } else throw new Error('Expected prepare WORKSPACE BATCH SELECTION_JSON or run WORKSPACE BATCH CODEX_EXE [PUBLISHER SERVICE_ROOT]');
}
if(process.argv[1] && path.resolve(process.argv[1])===script) await runVideoGraphCli(process.argv.slice(2));
