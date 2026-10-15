import { existsSync, readFileSync } from 'node:fs';
import { execFileSync } from 'node:child_process';
import path from 'node:path';
import assert from 'node:assert/strict';
import { prepareVideoRevision } from '../../packages/core/src/video-revision';
import { buildReplacementTranscript } from '../../skills/build/video-revision';
import { validateMediaLocation, validateTranscriptMap } from '../../packages/core/src/video-contract';
import { saveCorrectionJson as save } from '../../packages/core/src/video-correction';
const root=path.resolve(process.env.VI17_OUTPUT||'evals/video/generated/vi17'),source=path.join(root,'source'),workspace=path.join(root,'replacement');
const read=(f:string)=>JSON.parse(readFileSync(f,'utf8'));
const before=read(path.join(source,'video_source_manifest.json')),clip=path.join(root,'replacement-547-555.mp4');
if(!existsSync(clip))execFileSync(process.env.FFMPEG||'ffmpeg',['-v','error','-ss','547','-i',path.join(source,before.source.imported_path),
  '-t','8','-map','0:v:0','-map','0:a:0','-c:v','libx264','-preset','ultrafast','-crf','20','-c:a','aac',clip],{windowsHide:true});
prepareVideoRevision(source,workspace,{replacement:clip});
const started=Date.now();
const manifest=await buildReplacementTranscript(workspace,{python:process.env.VIDEO_WHISPER_PYTHON,modelDir:process.env.VIDEO_WHISPER_MODEL_DIR});
const map=read(path.join(workspace,'transcript_map.json')),text=readFileSync(path.join(workspace,'source.txt'),'utf8');
validateTranscriptMap(map,manifest,read(path.join(workspace,'base.json')),text);
const oldSegment=read(path.join(source,'transcript_map.json')).segments.find((s:any)=>s.segment_id==='segment-82');
assert.notEqual(manifest.material.video.media_id,before.material.video.media_id);
assert.equal(manifest.previous_book_id,before.material.book_id);
assert(manifest.source.duration_ms<9000);
assert(map.segments.length>0);assert(map.segments.every((s:any)=>s.location.start_ms>=0&&s.location.end_ms<9000));
assert.throws(()=>validateMediaLocation(oldSegment.location,manifest.source));
save(path.resolve('evals/video/results/vi17-20261011/replacement.json'),{status:'passed',source_clip:{start_ms:547000,end_ms:555000},
  previous_material:before.material,material:manifest.material,old_location_rejected:oldSegment.location,
  duration_ms:manifest.source.duration_ms,map,text,elapsed_ms:Date.now()-started,
  validation_scope:'Actual original-video clip, import, real Whisper transcript and old-location rejection; derived-stage replacement is covered by the implementation and bounded task plans.'});
console.log(JSON.stringify({status:'passed',material:manifest.material,text,elapsed_ms:Date.now()-started}));
