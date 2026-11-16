<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref, watch } from 'vue';
import { History, MessageSquareText, PenLine, Sparkles } from '@lucide/vue';
import { noteRequest, notesChanged, openPersonalNotes, type NoteRef } from '../personal-notes';
import { continueUnderstanding, thinkingTime, type NoteThinkingView, type UnderstandingSummary, type ThinkingEntry } from '../note-thinking';
import type { LearningEvidenceView } from '../api';
import UnderstandingSummaryCard from './UnderstandingSummary.vue';
const props=defineProps<{noteId:string;revision:number;compact?:boolean}>();
const emit=defineEmits<{revision:[revision:number];discussion:[session:string,turn:string]}>();
const view=ref<NoteThinkingView|null>(null), loading=ref(false), busy=ref(false), error=ref(''), notice=ref('');
const evidence=ref<LearningEvidenceView|null>(null), evidenceScope=ref<string|null>(null), correcting=ref(false), correction=ref('');
const pending=ref<{operation_id:string;text:string}|null>(null);
let generation=0, timer:ReturnType<typeof setTimeout>|undefined;
async function load(more=false) {
  const request=++generation, noteId=props.noteId; clearTimeout(timer); loading.value=true; error.value='';
  try {
    const result=await noteRequest<NoteThinkingView>('thinking',{note_id:noteId,...(more?{after:view.value?.next_cursor}:{})});
    if(request!==generation)return;
    if(more && view.value) result.timeline=[...view.value.timeline,...result.timeline.filter(e=>!view.value!.timeline.some(old=>old.id===e.id))];
    view.value=result;
    if(result.update_status==='pending') timer=setTimeout(()=>void load(),4000);
  } catch(failure) {if(request===generation)error.value=(failure as Error).message;}
  finally {if(request===generation)loading.value=false;}
}
async function showEvidence(id:string,workspace:string|null,edit=false) {
  if(busy.value)return;
  const noteId=props.noteId; busy.value=true;error.value='';
  try {
    const result=await noteRequest<LearningEvidenceView>('thinking_evidence',{note_id:noteId,workspace_id:workspace,evidence_ref:id});
    if(noteId!==props.noteId)return;
    evidence.value=result;evidenceScope.value=workspace;correcting.value=edit;correction.value='';pending.value=null;
  }catch(failure){error.value=(failure as Error).message;}finally{busy.value=false;}
}
async function correct() {
  if(!evidence.value || busy.value || (!pending.value && !correction.value.trim()))return;
  pending.value??={operation_id:crypto.randomUUID(),text:correction.value};busy.value=true;error.value='';
  try {
    await noteRequest('thinking_correct',{note_id:props.noteId,workspace_id:evidenceScope.value,evidence_ref:evidence.value.evidence_id,...pending.value});
    pending.value=null;evidence.value=null;notice.value='已保存你的补充，当前判断已更新。';notesChanged.value++;
    window.dispatchEvent(new Event('tutor-state-changed'));await load();
  }catch(failure){error.value=(failure as Error).message;}finally{busy.value=false;}
}
async function resume(item:UnderstandingSummary,mode:'discuss'|'practice') {
  busy.value=true;error.value='';
  try{await continueUnderstanding(item,props.noteId,mode);}catch(failure){error.value=(failure as Error).message;}finally{busy.value=false;}
}
async function retry() {
  if(busy.value)return;busy.value=true;error.value='';
  try{for(const job of view.value?.updates??[])if(['failed','interrupted'].includes(job.status))await noteRequest('thinking_retry',{note_id:props.noteId,...job});await load();}
  catch(failure){error.value=(failure as Error).message;}finally{busy.value=false;}
}
function openNote(reference:NoteRef){if(reference.note_id===props.noteId && reference.revision)emit('revision',reference.revision);else openPersonalNotes(reference);}
function title(entry:ThinkingEntry){return entry.kind==='revision'?(entry.reference?.revision===1?'记下最初的想法':`保存第 ${entry.reference?.revision} 版`):entry.kind==='discussion'?'在讨论中继续想':entry.kind==='correction'?'纠正系统判断':entry.kind==='activity'?'尝试一个情境':entry.kind==='action'?({skip:'暂时跳过',request_hint:'请求提示',reveal:'查看讲解'}[entry.action??'']??'活动记录'):entry.nature==='performance'?'这次的实际表现':entry.assisted?'讲解后的表达':'对这次表达的理解';}
function changed(){if(!busy.value)void load();}
watch(()=>[props.noteId,props.revision],()=>{++generation;view.value=null;evidence.value=null;notice.value='';pending.value=null;void load();},{immediate:true});
onMounted(()=>window.addEventListener('tutor-state-changed',changed));
onBeforeUnmount(()=>{++generation;clearTimeout(timer);window.removeEventListener('tutor-state-changed',changed);});
</script>

<template>
  <section class="note-thinking" :aria-busy="loading" aria-label="想法与理解路径">
    <header class="thinking-heading"><div><span class="thinking-eyebrow"><Sparkles :size="13"/>相关理解</span><h2 v-if="!compact">这条想法，走到了哪里</h2></div><button class="quiet" :disabled="loading" @click="load()">{{loading?'读取中…':'刷新'}}</button></header>
    <p v-if="view?.update_status==='pending'" class="thinking-status" role="status">新表达正在整理，先保留上一次判断。</p>
    <p v-if="view?.update_status==='failed'" class="thinking-status" role="status">新表达已保留，理解说明尚未整理完成。<button :disabled="busy" @click="retry">重试整理</button></p>
    <div class="thinking-summaries"><UnderstandingSummaryCard v-for="item in view?.current_understanding" :key="`${item.workspace_id}:${item.source_id}:${item.source_revision}:${item.evidence_ref}`" :item="item" :compact="compact" :busy="busy" :note-id="noteId" @evidence="showEvidence(item.evidence_ref,item.workspace_id)" @correct="showEvidence(item.evidence_ref,item.workspace_id,true)" @discuss="resume(item,'discuss')" @practice="resume(item,'practice')" @note="openNote" /></div>
    <p v-if="view && !view.current_understanding.length" class="thinking-empty">还没有相关理解观察。{{compact?'一起推敲后，这里会呈现有依据的变化。':'已保存的原话和讨论都在下方；新的表达和练习将帮助系统确认理解。'}}</p>
    <article v-if="evidence" class="thinking-evidence" aria-label="这条理解的依据">
      <header><h3>{{evidence.label}} · 判断依据</h3><button :disabled="!!pending || busy" @click="evidence=null">收起</button></header>
      <p v-if="evidence.superseded" class="thinking-status">这条判断已被纠正，保留在历史中。</p>
      <blockquote>{{evidence.learner_quote}}</blockquote><p>{{evidence.interpretation}}</p>
      <p v-if="evidence.correction">你的补充：{{evidence.correction}}</p>
      <details v-if="evidence.assistance?.length"><summary>当时已得到的帮助（{{evidence.assistance_count}}）</summary><p v-for="help in evidence.assistance" :key="help.fact_ref">{{help.text}}</p></details>
      <p v-else class="thinking-muted">这次表达前未记录到提示或讲解。</p>
      <p v-if="evidence.feedback_hidden" class="thinking-muted">本次活动的详细判定将在查看讲解后展示。</p>
      <details v-else-if="evidence.source_quotes?.length"><summary>具体判定依据</summary><div v-for="(item,index) in evidence.source_quotes" :key="index"><p>{{item.reason}}</p><blockquote v-for="(source,i) in item.sources" :key="i">{{source.quote??source.observation}}</blockquote></div></details>
      <button v-if="evidence.source_session_id && evidence.source_turn_id" @click="emit('discussion',evidence.source_session_id,evidence.source_turn_id)">查看当时的讨论</button>
      <button v-if="!correcting && !evidence.superseded" :disabled="busy" @click="correcting=true">纠正判断</button>
      <form v-if="correcting && !evidence.superseded" class="thinking-correction" @submit.prevent="correct"><label>补充你的实际情况<textarea v-model="correction" :disabled="!!pending" rows="3" placeholder="例如：刚才我是参考笔记才答出来的。" /></label><button :disabled="busy || (!correction.trim() && !pending)">{{busy?'保存中…':pending?'重试保存补充':'保存补充'}}</button></form>
    </article>
    <template v-if="!compact">
      <div class="thinking-path-heading"><History :size="15"/><h3>思考路径</h3><span>最近的变化在前</span></div>
      <ol class="thinking-path"><li v-for="entry in view?.timeline" :key="entry.id" :class="entry.kind"><span class="path-dot"><PenLine v-if="entry.kind==='revision'" :size="13"/><MessageSquareText v-else-if="entry.kind==='discussion'" :size="13"/><Sparkles v-else :size="13"/></span><div class="path-entry"><header><h4>{{title(entry)}}</h4><time>{{thinkingTime(entry.occurred_at)}}</time></header><p v-if="entry.reference" class="thinking-muted">围绕第 {{entry.reference.revision}} 版{{entry.authorship==='assistant_draft'?' · 助手摘录':entry.authorship==='adopted_assistant'?' · 已采纳的助手表达':entry.authorship==='material_excerpt'?' · 材料摘录':''}}</p><p v-if="entry.questionable" class="thinking-status">题目存疑，当前判断已排除此活动。</p><p v-if="entry.superseded" class="thinking-muted">当时的判断，后来已纠正</p><blockquote v-if="entry.text">{{entry.text}}</blockquote><p v-if="entry.interpretation" class="path-interpretation">{{entry.interpretation}}</p><p v-if="entry.correction">你的补充：{{entry.correction}}</p><p v-if="entry.change_note">修改原因：{{entry.change_note}}</p><p v-if="entry.restored_from" class="thinking-muted">从第 {{entry.restored_from}} 版恢复</p><div class="path-links"><button v-if="entry.reference && entry.kind==='revision'" @click="emit('revision',entry.reference.revision!)">阅读这一版 · 对照修改</button><button v-if="entry.based_on_turn" @click="emit('discussion',entry.based_on_turn.session_id,entry.based_on_turn.turn_id)">由这次讨论形成</button><button v-if="entry.kind==='understanding'||entry.kind==='correction'" :disabled="busy" @click="showEvidence(entry.id,entry.workspace_id??null)">查看判断依据</button><button v-if="entry.source_session_id && entry.source_turn_id" @click="emit('discussion',entry.source_session_id,entry.source_turn_id)">查看原讨论</button></div></div></li></ol>
      <button v-if="view?.next_cursor!=null" class="thinking-more" :disabled="loading" @click="load(true)">更早的思考</button>
    </template>
    <p v-if="notice" class="thinking-status" role="status">{{notice}}</p><p v-if="error" class="thinking-error" role="alert">{{error}} <button :disabled="loading || busy" @click="load()">重新读取</button></p>
  </section>
</template>

<style scoped>
.note-thinking{margin:14px 0 20px;min-width:0;color:var(--text,#302d29);font-size:13px;line-height:1.65}.thinking-heading,.thinking-evidence header,.path-entry header{display:flex;gap:12px;justify-content:space-between;align-items:flex-start}.thinking-heading{align-items:center;margin-bottom:12px}.thinking-heading h2{font-size:19px;line-height:1.4;margin:4px 0 0;letter-spacing:-.3px}.thinking-eyebrow{display:flex;gap:6px;align-items:center;color:var(--accent,#a95028);font-size:11px}.thinking-summaries{display:grid;gap:12px}.thinking-empty{padding:14px 16px;background:var(--bg,#f7f4ee);border-radius:10px;color:var(--steel,#79716a)}.thinking-status,.thinking-muted{font-size:12px;color:var(--steel,#79716a)}.thinking-error{color:var(--danger,#a43f31)}
button{font:inherit;font-size:12px;border:1px solid var(--border,#e5dfd7);border-radius:8px;padding:5px 9px;background:transparent;color:inherit;cursor:pointer}button:hover{background:var(--bg,#f7f4ee)}button:disabled{opacity:.5;cursor:wait}.quiet{border:0;color:var(--steel,#79716a);white-space:nowrap}.thinking-evidence{border:1px solid var(--border,#e5dfd7);padding:16px;border-radius:12px;margin-top:12px;overflow-wrap:anywhere}.thinking-evidence h3{font-size:14px;margin:0}.thinking-evidence>button{margin-right:8px}.thinking-evidence details{margin:12px 0}.thinking-evidence summary{cursor:pointer}.thinking-correction{display:grid;gap:8px;margin-top:14px}.thinking-correction label{display:grid;gap:7px}textarea{font:inherit;background:var(--surface,#fffdf9);color:inherit;border:1px solid var(--border,#e5dfd7);border-radius:8px;padding:10px;width:100%;box-sizing:border-box;resize:vertical}.thinking-correction button{justify-self:start;color:var(--accent,#a95028)}blockquote{margin:8px 0;padding:10px 12px;background:var(--bg,#f7f4ee);border-left:2px solid var(--border,#e5dfd7);white-space:pre-wrap;overflow-wrap:anywhere}
.thinking-path-heading{display:flex;gap:7px;align-items:center;margin:25px 0 14px}.thinking-path-heading h3{font-size:14px;margin:0}.thinking-path-heading span{font-size:11px;color:var(--steel,#79716a);margin-left:auto}.thinking-path{list-style:none;margin:0;padding:0}.thinking-path li{position:relative;display:grid;grid-template-columns:26px minmax(0,1fr);gap:9px;padding-bottom:22px}.thinking-path li:not(:last-child):before{content:'';position:absolute;left:12px;top:25px;bottom:0;border-left:1px solid var(--border,#e5dfd7)}.path-dot{width:25px;height:25px;display:flex;align-items:center;justify-content:center;border-radius:50%;background:var(--bg,#f7f4ee);color:var(--steel,#79716a)}.revision .path-dot{color:var(--accent,#a95028)}.path-entry h4{font-size:13px;margin:1px 0 0;font-weight:600}.path-entry time{font-size:10px;color:var(--steel,#79716a);white-space:nowrap}.path-entry p{margin:7px 0}.path-interpretation{color:var(--steel,#79716a)}.path-links{display:flex;gap:7px;flex-wrap:wrap}.path-links button{font-size:11px;padding:4px 7px}.thinking-more{margin-left:35px}
@media(max-width:400px){.thinking-heading h2{font-size:17px}.path-entry header{display:block}.path-entry time{display:block;margin-top:2px}.thinking-path-heading span{font-size:10px}.thinking-evidence{padding:12px}}
</style>
