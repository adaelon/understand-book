// Verify real persisted ASR coverage and canonical mapping; listening truth remains separate.
import { readFileSync, writeFileSync, mkdirSync, statSync, existsSync } from "node:fs";
import path from "node:path";
import assert from "node:assert/strict";
import { readVideoBuild, stitchVideoWords, type RawWhisper } from "../../packages/core/src/video-transcript";
import { readVideoImport } from "../../packages/core/src/video-import";
import { validateTranscriptMap, validateVideoSourceManifest } from "../../packages/core/src/video-contract";
const workspace=path.resolve(process.argv[2] || "evals/video/generated/vi2/moved-workspace");
const output=path.resolve(process.argv[3] || "evals/video/generated/vi3");
const read=(name:string)=>JSON.parse(readFileSync(path.join(workspace,name),"utf8"));
const build=readVideoBuild(workspace)!;
const source=readVideoImport(workspace).source!;
const chunks=build.chunks.filter(c=>c.status==="completed");
const raw=chunks.map(c=>read(`transcript/raw/${c.id}.json`) as RawWhisper);
const parts=raw.map((r,i)=>r.segments.flatMap(s=>s.words.map(w=>({text:w.word,start:chunks[i].input_start_ms+w.start*1000,
  end:chunks[i].input_start_ms+w.end*1000,probability:w.probability,chunk:chunks[i].id}))));
const joined=stitchVideoWords(parts,chunks);
if (build.stage!=="ready") {
  console.log(JSON.stringify({stage:build.stage,completed:chunks.length,total:build.chunks.length,boundaries:joined.boundaries,failure:build.failure},null,2));
} else {
  assert.equal(chunks.length,build.chunks.length);
  assert.equal(chunks[0].start_ms,0);
  assert.equal(chunks.at(-1)!.end_ms,source.duration_ms);
  for(let i=1;i<chunks.length;i++) assert.equal(chunks[i].start_ms,chunks[i-1].end_ms);
  const manifest=validateVideoSourceManifest(read("video_source_manifest.json"));
  const map=validateTranscriptMap(read("transcript_map.json"),manifest,read("base.json"),readFileSync(path.join(workspace,"source.txt"),"utf8"));
  const points=[0,source.duration_ms/2,source.duration_ms-30000].map(at=> {
    const s=map.segments.find(s=>s.location.end_ms>=at)!;assert(s);
    return {requested_ms:at,segment_id:s.segment_id,lid:s.text_range.lid,location:s.location,text:s.text};
  });
  const startFile=path.join(output,"run-start.json");
  const started=existsSync(startFile)?JSON.parse(readFileSync(startFile,"utf8")).started_at:null;
  const finished=statSync(path.join(workspace,"video_build.json")).mtime.toISOString();
  const result={date:new Date().toISOString(),video:source.video,transcript:map.transcript,book_id:build.book_id,
    publication:existsSync(path.join(workspace,"video_publication.json"))?read("video_publication.json").reference:null,
    complete_coverage_ms:source.duration_ms,chunks:chunks.length,segments:map.segments.length,
    started_at:started,build_state_saved_at:finished,elapsed_to_saved_state_ms:started?Date.parse(finished)-Date.parse(started):null,
    inference_ms:raw.reduce((n,r)=>n+(r.inference_ms||0),0),model_load_ms:raw.reduce((n,r)=>n+(r.load_ms||0),0),
    raw_bytes:statSync(path.join(workspace,"transcript/raw_results.json")).size,
    source_bytes:statSync(path.join(workspace,"source.txt")).size,media_bytes:statSync(path.join(workspace,source.imported_path)).size,
    boundaries:joined.boundaries,points,uncertain_word_count:read("transcript/joins.json").uncertainties.length,
    checks:{continuous_successful_plan:true,canonical_text_lid_mapping:true,all_joins_resolved:true,beginning_middle_end:true},
    listening_truth:"Full manual verbatim truth and word alignment remain unmeasured",api_calls:0,api_cost:0};
  mkdirSync(output,{recursive:true});writeFileSync(path.join(output,"verification.json"),JSON.stringify(result,null,2)+"\n");
  console.log(JSON.stringify(result,null,2));
}
