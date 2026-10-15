import { existsSync, mkdirSync, readFileSync, writeFileSync, copyFileSync } from 'node:fs';
import path from 'node:path';
import { prepareVideoCorrectionRequest, runVideoCorrection, formatVideoCorrectionReview } from '../../skills/build/video-correction';
import { saveCorrectionJson } from '../../packages/core/src/video-correction';

const original=path.resolve('evals/video/generated/vi11/workspace');
const root=path.resolve(process.env.VI16_OUTPUT || 'evals/video/generated/vi16');
const workspace=path.join(root,'workspace');mkdirSync(workspace,{recursive:true});
const read=(file:string)=>JSON.parse(readFileSync(file,'utf8'));
const manifest=read(path.join(original,'video_source_manifest.json'));
for (const file of ['source.txt','base.json','video_source_manifest.json',manifest.transcript_map_path]) {
  const target=path.join(workspace,file);mkdirSync(path.dirname(target),{recursive:true});
  if(!existsSync(target)) copyFileSync(path.join(original,file),target);
}
const link=read(path.join(original,'video_publication.json'));
if(!existsSync(path.join(workspace,'video_publication.json'))) saveCorrectionJson(path.join(workspace,'video_publication.json'),{...link,directory:path.resolve(original,link.directory)});
const reviews=readFileSync(path.resolve('evals/video/results/vi15-20261011/verification.json'),'utf8');
const prior=JSON.parse(reviews).cases;
const tensor=prior.find((c:any)=>c.review.location.start_ms===547000).review;
const uncertain=prior.find((c:any)=>c.review.location.start_ms===640000).review;
const tasks=[
  {id:'terminology',request:{segment_ids:['segment-82','segment-87'],vu_ids:['lecture-v1/vu-0001/1','lecture-v1/vu-0005/2'],
    question:'核对 denser 与 beef float 16 是否是识别错误。给出有材料依据的最小逐处校正，保留其余讲者表达；画面解释单列。',audio_reviews:[tensor],
    audio_requests:[{location:{video:manifest.material.video,start_ms:590000,end_ms:598000},question:'讲者在这里说的浮点格式名称是什么？'}]}},
  {id:'uncertain',request:{segment_ids:['segment-95','segment-96','segment-97'],vu_ids:['lecture-v1/vu-0008/1'],
    question:'复核 sarcastic 是否应改；现有短音频识别也返回 sarcastic。若不足以确定原话请保留疑点，不要依据常识或烧录字幕猜词。',audio_reviews:[uncertain]}},
];
const audioProbe=process.env.VI16_AUDIO_EXE || path.join(process.env.CARGO_TARGET_DIR || 'target','debug/examples/video_audio_probe.exe');
for (const task of tasks) {
  const input=await prepareVideoCorrectionRequest(workspace,task.id,task.request,audioProbe);
  saveCorrectionJson(path.join(root,`${task.id}-input.json`),{...input,media:{...input.media,images:input.media.images.map(({data:_,...i})=>i)}});
  if(process.argv[2]) {
    const receipt=await runVideoCorrection(workspace,task.id,path.resolve(process.argv[2]));
    if(!('reused' in receipt)) saveCorrectionJson(path.join(root,`${task.id}-run.json`),receipt);
    writeFileSync(path.join(root,`${task.id}-review.txt`),formatVideoCorrectionReview(workspace,task.id));
    console.log(JSON.stringify({task:task.id,...receipt}));
  }
}
