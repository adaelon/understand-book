import { cpSync, existsSync, mkdirSync, readFileSync, appendFileSync } from 'node:fs';
import path from 'node:path';
import { saveCorrectionJson as save } from '../../packages/core/src/video-correction';
import { prepareVideoGraph } from '../../packages/core/src/video-graph';
import { prepareVideoDiscourse } from '../../packages/core/src/video-discourse';
import { prepareVideoRevision } from '../../packages/core/src/video-revision';
import { runVideoGraph } from '../../skills/build/video-graph';
import { runVideoDiscourse } from '../../skills/build/video-discourse';
import { publishVideoRevision, runVideoRevision } from '../../skills/build/video-revision';
const read = (file:string)=>JSON.parse(readFileSync(file,'utf8'));
const root=path.resolve(process.env.VI17_OUTPUT||'evals/video/generated/vi17'),source=path.join(root,'source'),workspace=path.join(root,'workspace');
const original=path.resolve('evals/video/generated/vi11/workspace');
const executable=path.resolve(process.argv[2]),publisher=path.resolve(process.argv[3]),service=path.join(root,'service');
mkdirSync(root,{recursive:true});
if(!existsSync(path.join(source,'video_source_manifest.json'))) {
  const v=read('evals/video/generated/vi16/verification.json'),corrections=path.resolve(v.partial_export,'../../..');
  mkdirSync(source,{recursive:true});
  const manifest=read(path.join(original,'video_source_manifest.json'));
  for(const name of ['source.txt','base.json','video_import.json','transcript_map.json','video_enrichments.json','video_translation_progress.json',
    'transcript','media','video-enrichments','.build/video-translation','.build/video-visual','.build/video-graph','.build/video-discourse'])
    if(existsSync(path.join(original,name)))cpSync(path.join(original,name),path.join(source,name),{recursive:true});
  const translationWorkspace=path.resolve('evals/video/generated/vi10/workspace');
  const translationPlan=read(path.join(translationWorkspace,'.build/video-translation/plan.json'));
  if(JSON.stringify(translationPlan.material)!==JSON.stringify(manifest.material))throw new Error('VI10 translation material mismatch');
  cpSync(path.join(translationWorkspace,'.build/video-translation'),path.join(source,'.build/video-translation'),{recursive:true});
  cpSync(path.join(corrections,'transcript_corrections.json'),path.join(source,'transcript_corrections.json'));
  save(path.join(source,'video_source_manifest.json'),manifest);
}
// The original VI13/14 coverage is at einsum, outside the corrected word. Add
// actual pre-revision underflow work in this isolated copy so propagation can
// be observed against a real prior model result at the edited location.
await publishVideoRevision(source,publisher,service);
const selection={vu_ids:['lecture-v1/vu-0001/1'],segment_ids:['segment-81','segment-82','segment-83']};
prepareVideoGraph(source,'graph-vi17-tensor',selection);
const graph=await runVideoGraph(source,'graph-vi17-tensor',executable);
appendFileSync(path.join(root,'baseline-runs.jsonl'),JSON.stringify({stage:'graph',...graph})+'\n');
await publishVideoRevision(source,publisher,service);
prepareVideoDiscourse(source,'discourse-vi17-tensor',selection);
const discourse=await runVideoDiscourse(source,'discourse-vi17-tensor',executable);
appendFileSync(path.join(root,'baseline-runs.jsonl'),JSON.stringify({stage:'discourse',...discourse})+'\n');
await publishVideoRevision(source,publisher,service);
prepareVideoRevision(source,workspace,{corrections:source});
const state=await runVideoRevision(workspace,executable,publisher,service);
save(path.join(root,'run.json'),state);console.log(JSON.stringify(state,null,2));
