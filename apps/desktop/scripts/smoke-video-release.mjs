import assert from 'node:assert/strict';
import { execFileSync, spawnSync } from 'node:child_process';
import { copyFileSync, existsSync, mkdirSync, readFileSync, renameSync, writeFileSync } from 'node:fs';
import path from 'node:path';

const [binaryArg, publisherArg, mediaArg, rootArg, audioArg] = process.argv.slice(2);
if(!rootArg)throw new Error('Usage: smoke-video-release.mjs SIDECAR PUBLISHER ORIGINAL_MP4 OUTPUT_DIR [AUDIO_REVIEW]');
const root=path.resolve(rootArg);mkdirSync(root,{recursive:true});
const binary=path.join(root,'understand-book-build.exe'), publisher=path.join(root,'publish-book.exe');
copyFileSync(path.resolve(binaryArg),binary);copyFileSync(path.resolve(publisherArg),publisher);
const resolveCommand=(name)=>process.env[name.toUpperCase()]||execFileSync('where.exe',[name],{encoding:'utf8',windowsHide:true}).trim().split(/\r?\n/)[0];
const bundled=existsSync(path.join(root,'resources/video/bin/ffmpeg.exe'));
const decoder=bundled?path.join(root,'resources/video/bin/ffmpeg.exe'):resolveCommand('ffmpeg');
const env={...process.env,PATH:process.env.SystemRoot+'\\System32',FFMPEG:bundled?'':decoder,FFPROBE:bundled?'':resolveCommand('ffprobe')};
const checks=[];
function invoke(args,overrides={},expected=0){
  const r=spawnSync(binary,args,{cwd:root,env:{...env,...overrides},encoding:'utf8',windowsHide:true,maxBuffer:16*1024*1024});
  assert.equal(r.status,expected,`${args.join(' ')}\n${r.stderr}\n${r.stdout}`);return JSON.parse(r.stdout);
}
const missing=invoke(['video-doctor','--python',path.join(root,'missing-python'),'--model-dir',path.join(root,'missing-model')],
  {FFMPEG:path.join(root,'missing-ffmpeg'),FFPROBE:path.join(root,'missing-ffprobe')},1);
assert(missing.checks.every(c=>!c.ready&&c.recovery));checks.push('specific_missing_dependencies_and_recovery');
const configured=invoke(['video-doctor']);assert(configured.ready);checks.push(bundled?'bundled_dependencies_without_path_overrides':'configured_dependencies');
const input=path.join(root,'external.mp4'),workspace=path.join(root,'workspace');
if(!existsSync(input)&&!existsSync(path.join(workspace,'video_import.json')))
  execFileSync(decoder,['-v','error','-ss','547','-i',path.resolve(mediaArg),'-t','8','-c:v','libx264','-preset','ultrafast','-c:a','aac',input],{windowsHide:true});
if(!existsSync(path.join(workspace,'video_import.json'))){
  const failed=spawnSync(binary,['video-import',input,'--workspace',workspace],{cwd:root,env:{...env,FFPROBE:path.join(root,'missing-ffprobe')},encoding:'utf8',windowsHide:true});
  assert.notEqual(failed.status,0);
  const saved=JSON.parse(readFileSync(path.join(workspace,'video_import.json'),'utf8'));assert.equal(saved.stage,'failed');assert.equal(saved.copied_bytes,saved.total_bytes);
  renameSync(input,path.join(root,'external-unavailable.mp4'));
}
const imported=invoke(['video-resume','--workspace',workspace]);assert.equal(imported.stage,'awaiting_transcript');checks.push('resume_import_without_external_source');
const buildFile=path.join(workspace,'video_build.json');
if(!existsSync(buildFile)){
  const failed=invoke(['video-transcribe','--workspace',workspace],{VIDEO_WHISPER_MODEL_DIR:''},1);
  assert.equal(failed.chunks[0].status,'failed');assert.match(failed.chunks[0].failure,/VIDEO_WHISPER_MODEL_DIR/);
}
const transcript=invoke(['video-transcribe','--workspace',workspace]);assert.equal(transcript.stage,'ready');
assert(existsSync(path.join(workspace,'.build/video/whisper.py')));checks.push('embedded_whisper_real_audio_after_failed_attempt');
const source=readFileSync(path.join(workspace,'source.txt'),'utf8');assert.match(source,/tensor/i);
const recovered=invoke(['video-transcribe','--workspace',workspace],{FFMPEG:'missing',VIDEO_WHISPER_PYTHON:'missing',VIDEO_WHISPER_MODEL_DIR:''});
assert.deepEqual(recovered,transcript);checks.push('completed_resume_without_dependencies_or_new_recognition');
const linkFile=path.join(workspace,'video_publication.json');
const published=existsSync(linkFile)
  ? JSON.parse(readFileSync(path.resolve(workspace,JSON.parse(readFileSync(linkFile,'utf8')).directory,'publication.json'),'utf8'))
  : JSON.parse(execFileSync(publisher,[path.join(root,'service'),'import',workspace],{cwd:root,env,encoding:'utf8',windowsHide:true}));
const publishedDir=path.join(root,'service/library/published',published.reference.book_id,published.reference.publication_id);
assert(readFileSync(path.join(workspace,'media/original.mp4')).equals(readFileSync(path.join(publishedDir,'media/original.mp4'))));
checks.push('standalone_publisher_keeps_imported_bytes');
let audioReview;
if(audioArg){
  const audio=path.join(root,'video-audio-review.exe');copyFileSync(path.resolve(audioArg),audio);
  const manifest=JSON.parse(readFileSync(path.join(workspace,'video_source_manifest.json'),'utf8'));
  const request=path.join(root,'audio-request.json'),output=path.join(root,'audio-review.json');
  writeFileSync(request,JSON.stringify({location:{video:manifest.material.video,start_ms:0,end_ms:8000},question:'讲者如何描述小数张量？'}));
  execFileSync(audio,[publishedDir,request,output],{cwd:root,env,windowsHide:true});
  audioReview=JSON.parse(readFileSync(output,'utf8'));assert.match(audioReview.review.reviewed_text,/tensor/i);
  assert.deepEqual(audioReview.review.publication,published.reference);checks.push('installed_original_audio_review');
}
const result={status:'passed',platform:process.platform,system_only_path:env.PATH,checks,source_text:source,publication:published.reference,published_directory:publishedDir,
  dependency_report:configured,build:transcript,audio_review:audioReview,installed_binary:binary,bytes:Object.values(published.files).reduce((sum,bytes)=>sum+bytes,0),at:new Date().toISOString()};
writeFileSync(path.join(root,'verification.json'),JSON.stringify(result,null,2)+'\n');console.log(JSON.stringify(result,null,2));
