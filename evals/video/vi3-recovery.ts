import { execFile } from "node:child_process";
import { promisify } from "node:util";
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import path from "node:path";
import assert from "node:assert/strict";
import { importVideo } from "../../packages/core/src/video-import";
import { buildVideoTranscript, type VideoChunk } from "../../packages/core/src/video-transcript";
const run=promisify(execFile);
const root=path.resolve(process.argv[2]||"evals/video/generated/vi3-recovery");mkdirSync(root,{recursive:true});
const original=path.resolve("evals/video/generated/vi2/moved-workspace/media/original.mp4");
const clip=path.join(root,"clip.mp4"),workspace=path.join(root,"workspace");
await run(process.env.FFMPEG||"ffmpeg",["-v","error","-y","-ss","578","-i",original,"-t","8","-map","0:v:0","-map","0:a:0","-c:v","libx264","-preset","ultrafast","-c:a","aac",clip],{windowsHide:true});
await importVideo(clip,workspace);
const python=path.resolve(".worktree-local/whisper-venv/Scripts/python.exe");
let inject=true;const calls:string[]=[];
async function recognize(audio:string,output:string,chunk:VideoChunk) {
  calls.push(chunk.id);
  const request=path.join(root,"request.json");
  writeFileSync(request,JSON.stringify({audio:inject&&chunk.id==="chunk-001"?path.join(root,"missing.wav"):audio,
    model_dir:path.resolve(".worktree-local/whisper-models/small.en")}));
  await run(python,[path.resolve("packages/core/src/video-whisper.py"),request,output],{windowsHide:true});
}
const failed=await buildVideoTranscript(workspace,{recognize,chunkMs:4000,contextMs:2000});
assert.equal(failed.stage,"failed");assert.equal(failed.chunks[0].status,"completed");assert.equal(failed.chunks[1].status,"failed");
writeFileSync(path.join(root,"failed-state.json"),JSON.stringify(failed,null,2));
const preserved=readFileSync(path.join(workspace,"transcript/raw/chunk-000.json"));
inject=false;
const restored=await buildVideoTranscript(workspace,{recognize});
assert.equal(restored.stage,"ready",restored.failure||"");
assert.deepEqual(readFileSync(path.join(workspace,"transcript/raw/chunk-000.json")),preserved);
assert.deepEqual(calls,["chunk-000","chunk-001","chunk-001"]);
const result={date:new Date().toISOString(),source:{original,source_start_ms:578000,source_end_ms:586000},calls,
  actual_failure:failed.chunks[1].failure,restored_stage:restored.stage,preserved_success_response:true,
  failed_attempts:restored.chunks[1].attempts,engine:"faster-whisper 1.2.1 small.en CPU int8"};
writeFileSync(path.join(root,"verification.json"),JSON.stringify(result,null,2));console.log(JSON.stringify(result,null,2));
