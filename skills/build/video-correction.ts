import { appendFileSync, existsSync, mkdirSync, readFileSync } from 'node:fs';
import { spawn, execFile } from 'node:child_process';
import { isDeepStrictEqual, promisify } from 'node:util';
import { randomBytes } from 'node:crypto';
import { createInterface } from 'node:readline';
import { fileURLToPath } from 'node:url';
import path from 'node:path';
import { videoExecutorArgs, videoExecutorPrompt, videoExecutorStorageArgs } from './video-executor-runtime';
import { createBuildExecutorMcpSession } from './build-executor-mcp';
import { BUILD_EXECUTOR_BOOTSTRAP_CONTRACT_V3 as bootstrap } from '../../packages/core/src/build-executor-connection-capability';
import { createVideoCorrectionSession } from '../../packages/core/src/video-correction-executor';
import { correctionRoot, readVideoCorrectionInput, readVideoCorrections, prepareVideoCorrection, editVideoTranscript,
  inspectVideoCorrection, decideVideoCorrection, exportVideoCorrectionInput, saveCorrectionJson, type CorrectionSelection } from '../../packages/core/src/video-correction';

const script = fileURLToPath(import.meta.url);
const read = (file: string) => JSON.parse(readFileSync(file,'utf8'));
export async function runVideoCorrection(workspace: string, draft: string, executable: string) {
  workspace = path.resolve(workspace); readVideoCorrectionInput(workspace,draft);
  if (readVideoCorrections(workspace).drafts.some(d => d.draft_id === draft)) return { reused: true };
  const started = Date.now(), handoff = 'abhandoff1_' + randomBytes(32).toString('hex');
  const run = path.join(correctionRoot(workspace,draft),'runs',String(started)); mkdirSync(run,{recursive:true});
  const role = videoExecutorPrompt();
  const args = ['exec','--ignore-user-config','--skip-git-repo-check','--ephemeral','-C',run, ...videoExecutorStorageArgs(workspace),
    '-c','model_reasoning_effort="low"','-c','features.shell_tool=false','-c','features.apps=false',
    '-c','developer_instructions='+JSON.stringify(role),
    '-c','mcp_servers.understand_book_build_executor.command='+JSON.stringify(process.execPath),
    '-c','mcp_servers.understand_book_build_executor.args='+JSON.stringify(videoExecutorArgs('video-correction', script, ['serve',workspace,draft,handoff])),
    '-c','mcp_servers.understand_book_build_executor.required=true',
    '-c','mcp_servers.understand_book_build_executor.default_tools_approval_mode="approve"',
    '--json','-o',path.join(run,'lifecycle.txt'),handoff];
  const child = spawn(executable,args,{windowsHide:true,stdio:['ignore','pipe','pipe']});
  const events: unknown[] = [];
  createInterface({input:child.stdout}).on('line',line => { try { const e=JSON.parse(line);
    if (['turn.completed','turn.failed','error'].includes(e.type)) events.push(e);
    else if(e.item) events.push({type:e.type,item_type:e.item.type,tool:e.item.tool,status:e.item.status});
  } catch { /* host diagnostics */ } });
  child.stderr.on('data',bytes => appendFileSync(path.join(run,'stderr.txt'),bytes));
  const exit = await new Promise<number|null>((resolve,reject) => {child.on('close',resolve);child.on('error',reject);});
  const complete = readVideoCorrections(workspace).drafts.some(d => d.draft_id === draft);
  const receipt = { elapsed_ms: Date.now()-started, exit_code:exit, complete, events };
  saveCorrectionJson(path.join(run,'run.json'),receipt);
  if (!complete) throw new Error('Correction executor incomplete; resume the same run command');
  return receipt;
}

/** Explicit maintenance request; the original-audio probe calls the production VI15 service. */
export async function prepareVideoCorrectionRequest(workspace: string, draft: string, request: CorrectionSelection & {
  audio_review_files?: string[]; audio_requests?: Array<{location: unknown; question: string}>;
}, audioProbe?: string) {
  const file = path.join(correctionRoot(workspace,draft),'input.json');
  const requestFile = path.join(correctionRoot(workspace,draft),'request.json');
  if (existsSync(requestFile) && !isDeepStrictEqual(read(requestFile), request))
    throw new Error('Correction request changed; use a new draft ID');
  if (!existsSync(requestFile)) saveCorrectionJson(requestFile,request);
  if (existsSync(file)) {
    const prior = readVideoCorrectionInput(workspace,draft);
    // Resume the frozen request without running Whisper again. Changed selections
    // or questions must explicitly use another draft ID.
    if (JSON.stringify(prior.segments.map(s=>s.segment_id)) !== JSON.stringify(request.segment_ids)
      || prior.question !== request.question) throw new Error('Correction request changed; use a new draft ID');
    return prepareVideoCorrection(workspace,draft,{...request,audio_reviews:prior.sources.flatMap(s=>s.review?[s.review]:[])});
  }
  const reviews = [...(request.audio_reviews ?? []), ...(request.audio_review_files ?? []).map(file => { const value=read(file);return value.review ?? value; })];
  if (request.audio_requests?.length) {
    if (!audioProbe) throw new Error('Provide the VI15 video_audio_probe executable for requested audio reviews');
    const link = read(path.join(workspace,'video_publication.json')), published=path.resolve(workspace,link.directory);
    const dir=path.join(correctionRoot(workspace,draft),'audio'); mkdirSync(dir,{recursive:true});
    for (const [i, audio] of request.audio_requests.entries()) {
      const input=path.join(dir,`${i}-request.json`),output=path.join(dir,`${i}-review.json`);
      if (existsSync(output) && existsSync(input) && JSON.stringify(read(input)) === JSON.stringify(audio)) reviews.push(read(output).review);
      else {
        saveCorrectionJson(input,audio);
        await promisify(execFile)(audioProbe,[published,input,output],{windowsHide:true,timeout:150000,maxBuffer:1024*1024});
        reviews.push(read(output).review);
      }
    }
  }
  return prepareVideoCorrection(workspace,draft,{...request,audio_reviews:reviews});
}
export function formatVideoCorrectionReview(workspace: string, draft: string) {
  const d = inspectVideoCorrection(workspace,draft);
  const lines = [`校正草稿 ${draft} · 转写版本 ${d.material.transcript.transcript_revision}`,
    `来源发布 ${d.publication.publication_id}`, ''];
  for (const c of d.changes) {
    lines.push(`${c.id} · ${c.segment_id} · ${c.status}${c.conflict ? ' · 内容已变化' : ''}`,
      `修改前：${c.before}`,`修改后：${c.after}`,`理由：${c.reason}`,
      ...c.evidence.map(e => `依据：${e.source_id} — ${e.role}`));
    if (c.conflict) lines.push(c.conflict_reason!,`当前语段：${c.current_text}`);
    lines.push('');
  }
  for (const note of d.notes) lines.push(`${note.kind}：${note.text}`,...note.evidence.map(e=>`依据：${e.source_id} — ${e.role}`),'');
  for (const s of d.sources) {
    if (s.review) lines.push(`原音频 ${s.source_id} · ${s.review.location.start_ms/1000}–${s.review.location.end_ms/1000} 秒`,
      s.review.reviewed_text,...s.review.uncertainties,'');
    if (s.transcript) lines.push(`原转写 ${s.source_id} · ${s.transcript.location.start_ms/1000}–${s.transcript.location.end_ms/1000} 秒`,s.transcript.text,'');
    if (s.kind === 'frame') lines.push(`原画面 ${s.source_id}：${JSON.stringify(s.target)}`,'');
  }
  return lines.join('\n');
}
export async function runVideoCorrectionCli(argv: string[]) {
  const [mode,workspaceArg,draft,argument,extra] = argv;
  if (!workspaceArg) throw new Error('Expected status|edit|prepare|run|review|inspect|accept|reject|export WORKSPACE ...');
  const workspace = path.resolve(workspaceArg);
  if (mode === 'serve') {
    const session = createBuildExecutorMcpSession({bootstrap_version:bootstrap.version,protocol_generation:bootstrap.session_protocol,
      session_private_root:correctionRoot(workspace,draft),execute_request:createVideoCorrectionSession(workspace,draft,argument)});
    for await (const line of createInterface({input:process.stdin})) {
      const request=JSON.parse(line),response=session.handle_message(request) as any;
      if (response !== undefined) {
        appendFileSync(path.join(correctionRoot(workspace,draft),'mcp-events.jsonl'),JSON.stringify({at:new Date().toISOString(),method:request.method,
          tool:request.params?.name,isError:response.result?.isError??false,bytes:Buffer.byteLength(JSON.stringify(response)),
          images:response.result?.content?.filter((c:any)=>c.type==='image').length??0,
          ...(response.result?.isError?{diagnostic:response.result.content?.[0]?.text}:{})})+'\n');
        process.stdout.write(JSON.stringify(response)+'\n');
      }
    }
    return;
  }
  let result: unknown;
  if (mode === 'status') result=readVideoCorrections(workspace);
  else if (mode === 'edit') result=editVideoTranscript(workspace,read(draft));
  else if (mode === 'prepare') {
    const input=await prepareVideoCorrectionRequest(workspace,draft,read(argument),extra && path.resolve(extra));
    result={draft,input_ref:input.input_ref,segments:input.segments.length,images:input.media.images.length,audio_reviews:input.sources.filter(s=>s.review).length};
  } else if (mode === 'run') result=await runVideoCorrection(workspace,draft,path.resolve(argument));
  else if (mode === 'review') { console.log(formatVideoCorrectionReview(workspace,draft)); return; }
  else if (mode === 'inspect') result=inspectVideoCorrection(workspace,draft);
  else if (mode === 'accept' || mode === 'reject') {
    if (!argument) throw new Error('Specify all or comma-separated change IDs');
    result=decideVideoCorrection(workspace,draft,mode,argument==='all'?'all':argument.split(','));
  } else if (mode === 'export') {const out=exportVideoCorrectionInput(workspace);result={file:out.file,correction_revision:out.input.correction_revision};}
  else throw new Error('Unknown correction operation');
  console.log(JSON.stringify(result,null,2));
}
if (process.argv[1] && path.resolve(process.argv[1]) === script) await runVideoCorrectionCli(process.argv.slice(2));
