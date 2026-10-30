import { afterEach, expect, it } from "vitest";
import { execFileSync } from "node:child_process";
import { existsSync, mkdtempSync, readFileSync, renameSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { importVideo } from "../src/video-import";
import { assembleVideoTranscript, buildVideoTranscript, planVideoChunks, stitchVideoWords, type RawWhisper } from "../src/video-transcript";
const roots: string[]=[];
afterEach(()=> { for(const root of roots.splice(0)) rmSync(root,{recursive:true,force:true}); });
it("covers the entire timeline including a partial last chunk",()=> {
  const plan=planVideoChunks(6410,3000,500);
  expect(plan.map(c=>[c.start_ms,c.end_ms,c.input_start_ms,c.input_end_ms])).toEqual([[0,3000,0,3500],[3000,6000,2500,6410],[6000,6410,5500,6410]]);
  expect(()=>planVideoChunks(0)).toThrow();
});
it("joins shared words once despite shifted estimates and rejects speech with no matching seam",()=> {
  const w=(text:string,start:number)=>({text,start,end:start+150,probability:.9,chunk:"fixture"});
  const chunks=planVideoChunks(6000,3000,1000);
  const result=stitchVideoWords([[w(" Start",1000),w(" shared",2800),w(" words",3100)],
    [w(" shared",2850),w(" words",3150),w(" end.",4000)]],chunks);
  expect(result.words.map(x=>x.text).join("")).toBe(" Start shared words end.");
  expect(result.boundaries[0].kind).toBe("shared_words");
  expect(()=>stitchVideoWords([[w(" wrong",2950)],[w(" different",3050)]],chunks)).toThrow("Unresolved transcript boundary");
});
it("keeps failed coverage distinct from silence, retries only missing recognition, and publishes valid LIDs after moving",async()=> {
  const root=mkdtempSync(path.join(tmpdir(),"vi3-")); roots.push(root);
  const input=path.join(root,"input.mp4"),workspace=path.join(root,"workspace");
  execFileSync(process.env.FFMPEG||"ffmpeg",["-v","error","-f","lavfi","-i","color=s=160x90:r=10:d=9",
    "-f","lavfi","-i","anullsrc=r=16000:cl=mono","-t","9","-c:v","libx264","-pix_fmt","yuv420p","-c:a","aac",input],{windowsHide:true});
  const imported=await importVideo(input,workspace);
  const calls:string[]=[]; let fail=true;
  const recognize=async(_audio:string,output:string,chunk:{id:string})=> {
    calls.push(chunk.id);
    if (chunk.id==="chunk-001" && fail) throw new Error("injected service failure");
    const raw:RawWhisper={engine:"faster-whisper",model:"small.en",language:"en",segments:chunk.id==="chunk-001"?[]:
      [{text:" Speech.",start:1,end:1.8,words:[{word:" Speech.",start:1,end:1.5,probability:.3},
        ...(chunk.id==="chunk-000" ? [{word:" here.",start:1.5,end:1.5,probability:.09},
          {word:" Then.",start:1.45,end:1.8,probability:.99}] : [])]}]};
    writeFileSync(output,JSON.stringify(raw));
  };
  const first=await buildVideoTranscript(workspace,{recognize,chunkMs:3000,contextMs:0});
  expect(first.stage).toBe("failed"); expect(first.chunks.map(c=>c.status)).toEqual(["completed","failed","completed"]);
  expect(existsSync(path.join(workspace,"base.json"))).toBe(false);
  const saved=readFileSync(path.join(workspace,"transcript/raw/chunk-000.json"));
  const moved=path.join(root,"moved");renameSync(workspace,moved); fail=false;
  const second=await buildVideoTranscript(moved,{recognize});
  expect(second.failure).toBeNull(); expect(second.stage).toBe("ready");
  expect(calls).toEqual(["chunk-000","chunk-001","chunk-002","chunk-001"]);
  expect(readFileSync(path.join(moved,"transcript/raw/chunk-000.json"))).toEqual(saved);
  const manifest=JSON.parse(readFileSync(path.join(moved,"video_source_manifest.json"),"utf8"));
  expect(manifest.source.video).toEqual(imported.video);
  expect(manifest.coverage[0].accepted).toEqual(second.chunks.map(c=>({video:imported.video,start_ms:c.start_ms,end_ms:c.end_ms})));
  expect(()=>assembleVideoTranscript(moved,{...second,chunks:second.chunks.slice(1)},imported.source!)).toThrow("plan has gaps");
  expect(JSON.parse(readFileSync(path.join(moved,"transcript_map.json"),"utf8")).segments).toHaveLength(2);
  expect(JSON.parse(readFileSync(path.join(moved,"transcript/joins.json"),"utf8")).uncertainties).toHaveLength(3);
  expect(readFileSync(path.join(moved,"source.txt"),"utf8")).toContain("Speech. here. Then.");
  await buildVideoTranscript(moved,{recognize}); expect(calls).toHaveLength(4);
},30000);
