import { appendFileSync, mkdirSync, writeFileSync } from 'node:fs';
import path from 'node:path';
import { prepareVideoGraph } from '../../packages/core/src/video-graph';
import { runVideoGraph } from '../../skills/build/video-graph';
const workspace=path.resolve('evals/video/generated/vi11/workspace');
const out=path.resolve('evals/video/generated/vi13');mkdirSync(out,{recursive:true});
const tasks=[
  {id:'graph-vi13-einsum',vu_ids:['lecture-v1/vu-0009/1'],segment_ids:['segment-188','segment-189']},
  {id:'graph-vi13-visual',vu_ids:['lecture-v1/vu-0008/1'],segment_ids:[]},
  {id:'graph-vi13-text',vu_ids:[],segment_ids:['segment-191','segment-192']},
  {id:'graph-vi13-confirm',vu_ids:['lecture-v1/vu-0010/1'],segment_ids:['segment-191','segment-192']},
];
for(const task of tasks) {
  const input=prepareVideoGraph(workspace,task.id,{vu_ids:task.vu_ids,segment_ids:task.segment_ids});
  writeFileSync(path.join(out,`${task.id}-input.json`),JSON.stringify({input_ref:input.input_ref,publication:input.publication,
    vu_ids:input.vu_ids,transcript:input.media.transcript,images:input.media.images.map(({data:_,...image})=>image),existing_nodes:input.existing_nodes.map(n=>({id:n.id,name:n.name,identity:n.identity}))},null,2));
  if(process.argv[2]) {
    const receipt=await runVideoGraph(workspace,task.id,path.resolve(process.argv[2]));
    appendFileSync(path.join(out,'runs.jsonl'),JSON.stringify({task:task.id,...receipt})+'\n');
    console.log(JSON.stringify({task:task.id,...receipt}));
  }
}
