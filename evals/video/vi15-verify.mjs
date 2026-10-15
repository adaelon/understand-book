import { readFileSync, writeFileSync, mkdirSync } from 'node:fs';
import { resolve, join } from 'node:path';
import { spawnSync } from 'node:child_process';
import assert from 'node:assert/strict';

const workspace=resolve('evals/video/generated/vi11/workspace');
const link=JSON.parse(readFileSync(join(workspace,'video_publication.json'),'utf8'));
const directory=link.directory ?? link.publication_dir;
const root=resolve('evals/video/generated/vi15');mkdirSync(root,{recursive:true});
const manifest=JSON.parse(readFileSync(join(directory,'video_source_manifest.json'),'utf8'));
const before=readFileSync(join(directory,manifest.canonical_text_path));
const mapBefore=readFileSync(join(directory,manifest.transcript_map_path));
const exe=process.env.VI15_AUDIO_EXE || join(process.env.CARGO_TARGET_DIR || 'target','debug/examples/video_audio_probe.exe');
const env={...process.env,VIDEO_WHISPER_PYTHON:process.env.VIDEO_WHISPER_PYTHON || resolve('.worktree-local/whisper-venv/Scripts/python.exe'),
  VIDEO_WHISPER_MODEL_DIR:process.env.VIDEO_WHISPER_MODEL_DIR || resolve('.worktree-local/whisper-models/small.en')};
const cases=[
  {id:'v05',start:578000,end:586000,question:'讲者说的是 underflow 还是 overflow？'},
  {id:'v07',start:648000,end:656000,question:'讲者具体说了什么？听不清的词请保留疑点。'},
  {id:'terminology',start:640000,end:650000,question:'保存正文中的 sarcastic 是否是原话？'},
  {id:'tensor',start:547000,end:555000,question:'保存正文中的 denser 是否是原话？'},
];
const results=[];
for(const c of cases){
  const input=join(root,`${c.id}-input.json`),output=join(root,`${c.id}.json`);
  writeFileSync(input,JSON.stringify({location:{video:manifest.source.video,start_ms:c.start,end_ms:c.end},question:c.question},null,2));
  const run=spawnSync(exe,[directory,input,output],{env,encoding:'utf8',windowsHide:true,timeout:150000});
  assert.equal(run.status,0,`${c.id}: ${run.stderr} ${run.error||''}`);
  const r=JSON.parse(readFileSync(output,'utf8'));results.push({id:c.id,...r});
  assert.deepEqual(r.review.publication,link.reference);assert.equal(r.review.location.start_ms,c.start);assert.equal(r.review.location.end_ms,c.end);
  assert(r.review.uncertainties.length>0);assert.equal(r.review.provider.audio_input,'original_pcm_s16le_16000_mono');
  assert.equal(r.review.provider.recognition_prompt,null);
  assert(r.review.reviewed_words.every(w=>w.start_ms>=c.start && w.end_ms<=c.end));
  console.log(c.id,r.review.reviewed_text,JSON.stringify(r.review.uncertainties));
}
assert.match(results[0].review.reviewed_text,/underflow/i);assert.match(results[0].review.reviewed_text,/overflow/i);
assert(results[1].review.reviewed_text.trim());
assert.match(results[3].review.reviewed_text,/tensor/i);
assert(results[3].review.transcript.some(s=>s.text.includes('denser')));
assert.deepEqual(readFileSync(join(directory,manifest.canonical_text_path)),before);
assert.deepEqual(readFileSync(join(directory,manifest.transcript_map_path)),mapBefore);
const result={status:'passed',publication:link.reference,original_english_unchanged:true,time_map_unchanged:true,
  cases:results,comparison:'V05 two terms agree with the existing user listening record. V07 exact wording remains uncertain. The denser passage now returns tensor at an estimated 552000–552820 ms; sarcastic remains unresolved. Terminology differences are machine review candidates, not user-approved edits.',
  model:'faster-whisper small.en CPU int8; no chat-model inference',output:root};
writeFileSync(join(root,'verification.json'),JSON.stringify(result,null,2));
