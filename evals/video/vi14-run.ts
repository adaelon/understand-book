import { appendFileSync, mkdirSync, writeFileSync } from 'node:fs';
import path from 'node:path';
import { prepareVideoDiscourse } from '../../packages/core/src/video-discourse';
import { runVideoDiscourse } from '../../skills/build/video-discourse';
import { publishVideoVisualSnapshot } from '../../skills/build/video-visual-units';
const workspace=path.resolve('evals/video/generated/vi11/workspace');
const out=path.resolve('evals/video/generated/vi14');mkdirSync(out,{recursive:true});
const tasks=[
  {id:'discourse-vi14-einsum',vu_ids:['lecture-v1/vu-0009/1','lecture-v1/vu-0010/1'],segment_ids:['segment-188','segment-189','segment-190','segment-191']},
];
for(const task of tasks) {
  const input=prepareVideoDiscourse(workspace,task.id,task);
  writeFileSync(path.join(out,`${task.id}-input.json`),JSON.stringify({input_ref:input.input_ref,publication:input.publication,
    visual_units:input.visual_units,transcript:input.media.transcript,images:input.media.images.map(({data:_,...image})=>image)},null,2));
  if(process.argv[2]) {
    const receipt=await runVideoDiscourse(workspace,task.id,path.resolve(process.argv[2]));
    appendFileSync(path.join(out,'runs.jsonl'),JSON.stringify({task:task.id,...receipt})+'\n');
    console.log(JSON.stringify({task:task.id,...receipt}));
  }
}
if(process.argv[3] && process.argv[4]) {
  const receipt=await publishVideoVisualSnapshot(workspace,path.resolve(process.argv[3]),path.resolve(process.argv[4]));
  appendFileSync(path.join(out,'publications.jsonl'),JSON.stringify(receipt)+'\n');
  console.log(JSON.stringify({reused:receipt.reused,reference:receipt.reference,elapsed_ms:receipt.elapsed_ms}));
}
