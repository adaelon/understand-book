import { mkdirSync, readFileSync, readdirSync, writeFileSync, copyFileSync, cpSync } from 'node:fs';
import path from 'node:path';
import assert from 'node:assert/strict';
import { decideVideoCorrection, editVideoTranscript, exportVideoCorrectionInput, inspectVideoCorrection,
  readVideoCorrections, saveCorrectionJson } from '../../packages/core/src/video-correction';
import { formatVideoCorrectionReview } from '../../skills/build/video-correction';

const root=path.resolve(process.env.VI16_OUTPUT || 'evals/video/generated/vi16');
const workspace=path.join(root,'workspace'),read=(file:string)=>JSON.parse(readFileSync(file,'utf8'));
const state=readVideoCorrections(workspace),draft=inspectVideoCorrection(workspace,'terminology');
assert(draft.changes.every(c=>c.status==='pending'),'Production draft remains unadopted during evaluation');
const tensor=draft.changes.find(c=>c.before.includes('denser') && c.after.includes('tensor'));
const format=draft.changes.find(c=>c.before.includes('beef') && /bfloat|bf16/i.test(c.after));
assert(tensor,'Real Harness should propose the audio-supported denser → tensor correction');
assert(format,'Real Harness should propose a grounded name correction for beef float 16');
const unresolved=inspectVideoCorrection(workspace,'uncertain');
assert(unresolved.notes.some(n=>n.kind==='uncertain' && n.text.includes('sarcastic')));
assert(!unresolved.changes.some(c=>c.before.includes('sarcastic')),'Insufficient evidence must remain uncertain');
const manifest=read(path.join(workspace,'video_source_manifest.json')),link=read(path.join(workspace,'video_publication.json'));
const original=new Map(['source.txt','base.json',manifest.transcript_map_path,'video_source_manifest.json','video_publication.json'].map(file=>[file,readFileSync(path.join(workspace,file))]));
const publishedSource=readFileSync(path.join(link.directory,'source.txt'));
const publishedMap=readFileSync(path.join(link.directory,manifest.transcript_map_path));
const scenarios=path.join(root,'scenarios',new Date().toISOString().replaceAll(':','-'));mkdirSync(scenarios,{recursive:true});
function fork(name:string) {
  const dest=path.join(scenarios,name);mkdirSync(dest);
  for(const [file,bytes] of original) {mkdirSync(path.dirname(path.join(dest,file)),{recursive:true});writeFileSync(path.join(dest,file),bytes);}
  copyFileSync(path.join(workspace,'transcript_corrections.json'),path.join(dest,'transcript_corrections.json'));
  cpSync(path.join(workspace,'.build/video-corrections'),path.join(dest,'.build/video-corrections'),{recursive:true});
  return dest;
}
const partial=fork('partial');
assert.deepEqual(decideVideoCorrection(partial,'terminology','accept',[tensor.id]).applied,[tensor.id]);
assert.equal(readVideoCorrections(partial).segments.find(s=>s.segment_id===format.segment_id)!.text,state.segments.find(s=>s.segment_id===format.segment_id)!.text);
assert.deepEqual(decideVideoCorrection(partial,'terminology','reject',[format.id]).rejected,[format.id]);
const partialExport=exportVideoCorrectionInput(partial);
assert.equal(partialExport.input.segments.find(s=>s.segment_id===tensor.segment_id)!.text,
  state.segments.find(s=>s.segment_id===tensor.segment_id)!.text.replace(tensor.before,tensor.after));
assert.equal(partialExport.input.segments.find(s=>s.segment_id===format.segment_id)!.text,
  state.segments.find(s=>s.segment_id===format.segment_id)!.text);
assert(!partialExport.input.adopted_drafts.flatMap(d=>d.changes).some(c=>c.id===format.id));
for (const note of draft.notes) assert(!partialExport.input.canonical_text.includes(note.text));

const refused=fork('rejected');decideVideoCorrection(refused,'terminology','reject','all');
assert.deepEqual(readVideoCorrections(refused).segments,state.segments);
const accepted=fork('accepted');assert.equal(decideVideoCorrection(accepted,'terminology','accept','all').conflicts.length,0);
assert(inspectVideoCorrection(accepted,'terminology').changes.every(c=>c.status==='accepted'));
exportVideoCorrectionInput(accepted);

const conflict=fork('manual-after-draft'),current=readVideoCorrections(conflict).segments.find(s=>s.segment_id===tensor.segment_id)!;
const manualText=current.text.replace('denser','tensor');
editVideoTranscript(conflict,{segment_id:current.segment_id,expected_revision:current.edit_revision,before:current.text,after:manualText,reason:'验收副本：模拟人工先行校正同一语段'});
const decision=decideVideoCorrection(conflict,'terminology','accept','all');
assert(decision.conflicts.includes(tensor.id));assert(decision.draft.changes.find(c=>c.id===tensor.id)!.status==='pending');
assert.equal(readVideoCorrections(conflict).segments.find(s=>s.segment_id===current.segment_id)!.text,manualText);
assert(inspectVideoCorrection(conflict,'terminology').changes.find(c=>c.id===tensor.id)!.conflict);
writeFileSync(path.join(conflict,'review.txt'),formatVideoCorrectionReview(conflict,'terminology'));
for(const directory of [workspace,partial,refused,accepted,conflict]) for(const [file,bytes] of original)
  assert.deepEqual(readFileSync(path.join(directory,file)),bytes,`${directory}/${file} must stay unchanged`);
assert.deepEqual(readFileSync(path.join(link.directory,'source.txt')),publishedSource);
assert.deepEqual(readFileSync(path.join(link.directory,manifest.transcript_map_path)),publishedMap);
const mcp=readFileSync(path.join(workspace,'.build/video-corrections/terminology/mcp-events.jsonl'),'utf8').trim().split('\n').map(line=>JSON.parse(line));
assert(mcp.some(e=>e.images>0 && !e.isError),'Native images must actually reach the Harness');
const result={status:'passed',publication:link.reference,material:state.material,workspace,scenarios,
  evidence:{terminology:{changes:draft.changes,notes:draft.notes,source_ids:draft.sources.map(s=>s.source_id)},
    uncertain:{changes:unresolved.changes,notes:unresolved.notes},
    native_image_deliveries:mcp.filter(e=>e.images>0),
    runs:['terminology','uncertain'].flatMap(id=>readdirSync(path.join(workspace,'.build/video-corrections',id,'runs'))
      .map(run=>({draft:id,...read(path.join(workspace,'.build/video-corrections',id,'runs',run,'run.json'))})))},
  checks:{pending_before_user_choice:true,partial_accept_only_selected:true,rejected_text_unchanged:true,batch_accept:true,
    manual_edit_keeps_old_suggestion_pending:true,original_source_map_and_publication_unchanged:true,notes_separate_from_speaker_text:true},
  partial_export:partialExport.file,
  decision_provenance:'All accept/reject/manual decisions are deterministic evaluation actions in isolated copies; the real model drafts in workspace remain pending.',
  limitations:['Whisper words/timing are machine candidates, not full human verbatim truth.','Source revision publication and derived updates belong to VI17.']};
saveCorrectionJson(path.join(root,'verification.json'),result);
console.log(JSON.stringify({status:result.status,changes:draft.changes.length,uncertainty_notes:unresolved.notes.length,partial_export:partialExport.file}));
