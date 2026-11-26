<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue';
import { ArrowLeft, ArrowRight, ArrowUpRight, Check, Circle, CircleCheck, GitBranch, LocateFixed, Search, Sparkles } from '@lucide/vue';
import { api as sharedApi, ApiError } from '../api';
import { bindSceneApi, readerKey } from '../reader-context';
import { inquiryCourse, type CourseCatalog } from '../inquiry-course';
import { discussKnowledge, knowledgeRelations, openKnowledgeSource, pendingKnowledgeReference, type KnowledgeNodeView, type KnowledgeTreeRequest, type KnowledgeTreeView } from '../knowledge-tree';
import { continueUnderstanding, type UnderstandingSummary } from '../note-thinking';
import { openPersonalNotes } from '../personal-notes';
import UnderstandingSummaryCard from './UnderstandingSummary.vue';
import UnderstandingSpace from './UnderstandingSpace.vue';

const props=defineProps<{sourceId?:string;currentLid?:string;positionMs?:number;workspaceId?:string;title?:string}>();
const emit=defineEmits<{close:[]}>();
const api=bindSceneApi(sharedApi);
const view=ref<KnowledgeTreeView>(),selected=ref<KnowledgeNodeView>(),evidence=ref<string>(),records=ref(false);
const busy=ref(false),error=ref(''),query=ref(''),courseError=ref('');
const catalog=ref<CourseCatalog>(),detail=ref<HTMLElement>();
let controller:AbortController|undefined,timer:ReturnType<typeof setTimeout>|undefined,disposed=false;
let request:KnowledgeTreeRequest={node_id:'root'};
const compact=ref(window.matchMedia('(max-width:460px)').matches),expandedTheme=ref('');
const media=window.matchMedia('(max-width:460px)');
function resize(){compact.value=media.matches;}
function openTheme(node:KnowledgeNodeView){if(compact.value){expandedTheme.value=expandedTheme.value===node.id?'':node.id;}else{selected.value=undefined;void load({node_id:node.id});}}
const groups=computed(()=>view.value?.map_groups??[]);
const recent=computed(()=>view.value?.node.understanding?.[0]);
const lessons=computed(()=>catalog.value?.course?.lessons.filter(l=>!props.sourceId||l.source.publication.book_id===props.sourceId)??[]);
const completed=computed(()=>lessons.value.filter(l=>catalog.value?.progress.some(p=>p.lesson_id===l.id&&p.release_version===catalog.value?.release_version&&p.status==='completed')).length);
const activeLesson=computed(()=>lessons.value.find(l=>catalog.value?.progress.some(p=>p.lesson_id===l.id&&p.release_version===catalog.value?.release_version&&['active','paused'].includes(p.status))));
function status(node:KnowledgeNodeView){
  const items=node.understanding??[];
  if(items.some(i=>i.difficulties.length))return {label:items.some(i=>i.demonstrated.length)?'已有进展 · 有卡点':'待澄清',kind:'difficulty',icon:GitBranch};
  if(items.some(i=>i.nature==='performance'&&i.demonstrated.length&&i.assistance==='本次未记录提示或先前讲解'))return {label:'自己用过',kind:'observed',icon:CircleCheck};
  if(items.some(i=>i.demonstrated.length))return {label:items.some(i=>i.assistance==='在已提供帮助下')?'解释过 · 讨论后':'已有表达依据',kind:'observed',icon:Check};
  return {label:items.length?'还待观察':'还待探索',kind:'unknown',icon:Circle};
}
const current=(node:KnowledgeNodeView)=>view.value?.current_ids.includes(node.id);
async function load(next=request,quiet=false){
  controller?.abort();const call=new AbortController();controller=call;request=next;
  if(!quiet)busy.value=true;error.value='';
  try{
    const result=await api.knowledgeTree({...next,overview:true,current_lid:props.currentLid,position_ms:props.positionMs},call.signal);
    if(disposed||call.signal.aborted)return;
    if(!result.map_groups?.some(g=>g.topic.id===expandedTheme.value))expandedTheme.value=result.map_groups?.find(g=>result.current_ids.includes(g.topic.id))?.topic.id??result.map_groups?.[0]?.topic.id??'';
    view.value=result;request={...next,node_id:result.node.id,reference:undefined};
    const nodes=result.map_groups?.flatMap(g=>g.nodes)??[];
    selected.value=nodes.find(n=>n.id===selected.value?.id)??(result.node.child_count?undefined:result.node);
  }catch(e){if(!disposed&&!call.signal.aborted)error.value=e instanceof Error?e.message:String(e);}
  finally{if(controller===call){busy.value=false;controller=undefined;schedule();}}
}
function schedule(){clearTimeout(timer);if(!disposed)timer=setTimeout(()=>{if(document.visibilityState==='visible'&&!evidence.value&&!records.value)void load(request,true);else schedule();},15000);}
async function loadProgress(){
  if(!props.workspaceId)return;courseError.value='';
  try{const result=await inquiryCourse.catalog(props.workspaceId);if(!disposed)catalog.value=result;}
  catch(e){if(!disposed&&!(e instanceof ApiError&&e.status===404))courseError.value='课程进度暂时无法读取';}
}
async function choose(node:KnowledgeNodeView){selected.value=node;await nextTick();detail.value?.scrollIntoView({block:'nearest',behavior:'instant'});}
async function resume(item:UnderstandingSummary,mode:'discuss'|'practice',message?:string){try{await continueUnderstanding(item,undefined,mode,message);emit('close');}catch(e){error.value=(e as Error).message;}}
function explore(node:KnowledgeNodeView){discussKnowledge(node,readerKey());emit('close');}
function source(node:KnowledgeNodeView){if(node.sources[0]){openKnowledgeSource(node.sources[0],readerKey());emit('close');}}
function search(){selected.value=undefined;void load({node_id:'root',query:query.value.trim()});}
function locate(){selected.value=undefined;query.value='';void load({});}
function refresh(){void load(request,true);}
watch(pendingKnowledgeReference,reference=>{if(reference){pendingKnowledgeReference.value=null;selected.value=undefined;evidence.value=undefined;records.value=false;void load({reference});}});
onMounted(()=>{media.addEventListener('change',resize);const reference=pendingKnowledgeReference.value;pendingKnowledgeReference.value=null;void load(reference?{reference}:{node_id:'root'});void loadProgress();window.addEventListener('tutor-state-changed',refresh);});
onBeforeUnmount(()=>{media.removeEventListener('change',resize);disposed=true;controller?.abort();clearTimeout(timer);window.removeEventListener('tutor-state-changed',refresh);});
</script>

<template>
  <main class="understanding-map" aria-label="我的理解" :aria-busy="busy">
    <div class="map-page">
      <header class="map-heading"><div><span class="map-eyebrow">{{title || view?.breadcrumbs[0]?.label || '当前学习'}} / 理解与进展</span><h1>我的理解</h1><p>看见已经连起来的，也找到下一步。</p></div><button class="text-action" @click="emit('close')"><ArrowLeft :size="15"/>返回学习</button></header>
      <template v-if="evidence || records"><button class="text-action return-map" @click="evidence=undefined;records=false"><ArrowLeft :size="15"/>回到理解地图</button><UnderstandingSpace :evidence-ref="evidence"/></template>
      <template v-else>
        <section class="progress-strip" aria-label="学习进度和最近变化">
          <div v-if="lessons.length" class="course-progress"><div><span>课程进度</span><strong>{{completed}} / {{lessons.length}} 节已完成</strong></div><progress :value="completed" :max="lessons.length" aria-label="课程完成进度"/><small>{{activeLesson?`正在学习：${activeLesson.question}`:'可以继续探索感兴趣的问题'}}</small></div>
          <div v-else class="scope-progress"><span>当前学习范围</span><strong>{{view?.breadcrumbs[0]?.label || title || '正在读取…'}}</strong></div>
          <button v-if="recent" class="recent-change" @click="evidence=recent.evidence_ref"><span><Sparkles :size="13"/>最近的理解</span><strong>{{recent.learner_summary?.headline || recent.demonstrated[0] || recent.difficulties[0] || recent.label}}</strong></button>
          <p v-else class="first-expression">从一个问题开始，<br>你的表达会逐渐出现在这里。</p>
          <button class="primary-action" @click="emit('close')">继续学习<ArrowRight :size="15"/></button>
        </section>
        <p v-if="courseError" class="map-error">{{courseError}} <button @click="loadProgress">重试进度</button></p>
        <div class="map-toolbar"><div><h2>理解地图</h2><nav aria-label="知识位置"><button v-for="crumb in view?.breadcrumbs" :key="crumb.id" @click="selected=undefined;query='';load({node_id:crumb.id})">{{crumb.id==='root'?'全貌':crumb.label}}</button></nav></div><div class="map-tools"><button class="text-action" @click="locate"><LocateFixed :size="15"/>我学到这里</button><form @submit.prevent="search"><label class="map-search"><Search :size="15"/><input v-model="query" aria-label="寻找理解问题" placeholder="找一个问题或概念"/></label></form></div></div>
        <p v-if="error" role="alert" class="map-error">{{error}} <button @click="load()">重试</button></p>
        <p v-if="busy&&!view" role="status">正在展开你的理解地图…</p>
        <div class="topic-grid">
          <section v-for="(group,index) in groups" :key="group.topic.id" class="topic" :class="{'current-topic':current(group.topic),'expanded-topic':expandedTheme===group.topic.id}">
            <button class="topic-heading" :aria-expanded="compact ? expandedTheme===group.topic.id : undefined" @click="openTheme(group.topic)"><span class="topic-number">{{String(index+1+(request.after??0)).padStart(2,'0')}}</span><span><strong>{{group.topic.label}}</strong><small>{{current(group.topic)?'正在读这里':group.topic.understanding?.length?'已有理解可以接着看':'展开这个主题'}}</small></span><ArrowUpRight :size="14"/></button>
            <div class="topic-questions"><button v-for="node in group.nodes" :key="node.id" class="question-node" :aria-pressed="selected?.id===node.id" @click="choose(node)"><strong>{{node.label}}</strong><span :data-state="status(node).kind"><component :is="status(node).icon" :size="13"/>{{status(node).label}}</span></button><button v-if="compact" class="text-action" @click="selected=undefined;load({node_id:group.topic.id})">展开整个主题<ArrowRight :size="13"/></button></div>
          </section>
        </div>
        <p v-if="view&&!groups.length&&!selected" class="empty-map">{{query?'没有找到相符的内容，换一个关键词试试。':'这里还没有展开的知识条目，可以从原文或提问开始。'}}</p>
        <button v-if="request.after" class="text-action more-topics" @click="selected=undefined;load({...request,after:Math.max(0,request.after-12)})"><ArrowLeft :size="15"/>前面的主题</button>
        <button v-if="view?.next!=null" class="text-action more-topics" @click="selected=undefined;load({...request,after:view.next})">查看更多主题<ArrowRight :size="15"/></button>
        <div class="map-legend"><span><CircleCheck :size="13"/>自己用过</span><span><Check :size="13"/>已有表达</span><span><GitBranch :size="13"/>待澄清</span><span><Circle :size="13"/>还待探索</span></div>
        <section v-if="selected" ref="detail" class="selected-understanding" :aria-label="`${selected.label}的理解`">
          <header><span class="map-eyebrow">{{selected.label}}</span><button class="text-action" @click="selected=undefined">收起</button></header>
          <template v-if="selected.understanding?.length"><UnderstandingSummaryCard v-for="item in selected.understanding" :key="item.evidence_ref" :item="item" @evidence="evidence=item.evidence_ref" @correct="evidence=item.evidence_ref" @discuss="resume(item,'discuss',$event)" @practice="resume(item,'practice')" @note="openPersonalNotes"/></template>
          <div v-else class="unexplored"><h3>从这里开始弄清一个问题。</h3><p>{{selected.summary || '可以说说你对这一点的猜想，也可以先让学习助手解释。'}}</p><div><button class="primary-action" @click="explore(selected)">从这里探索<ArrowRight :size="15"/></button><button v-if="selected.sources.length" class="text-action" @click="source(selected)">看看原文</button></div></div>
          <div v-if="selected.relations.length" class="related-understanding"><span>与这一点有关</span><button v-for="relation in selected.relations" :key="`${relation.kind}:${relation.target_id}`" :disabled="!relation.available" @click="selected=undefined;load({node_id:relation.target_id})"><small>{{knowledgeRelations[relation.kind]||relation.kind}}</small>{{relation.label}}<ArrowUpRight :size="12"/></button></div>
          <button v-if="selected.child_count" class="text-action" @click="load({node_id:selected!.id});selected=undefined">展开这部分内容<ArrowRight :size="15"/></button>
        </section>
        <footer class="map-footer"><span>理解状态来自你的实际表达与尝试。</span><button class="text-action" @click="records=true">查看完整记录<ArrowUpRight :size="14"/></button></footer>
      </template>
    </div>
  </main>
</template>

<style scoped>
.understanding-map{height:100%;min-height:0;overflow:auto;background:var(--canvas,#faf8f4);color:var(--ink,#302e29);font:14px/1.7 var(--sans);overscroll-behavior:contain}.map-page{max-width:1200px;margin:auto;padding:35px 42px 60px}button,input{font:inherit;color:inherit}button{cursor:pointer}button:disabled{opacity:.5;cursor:default}button:focus-visible,input:focus-visible{outline:2px solid var(--brand-accent);outline-offset:4px}.map-heading{display:flex;align-items:center;justify-content:space-between;gap:24px;margin-bottom:26px}.map-eyebrow{font-size:11px;letter-spacing:.05em;color:var(--steel)}h1{font:500 34px/1.4 var(--font-display, var(--sans));margin:6px 0 7px}h2{font-size:16px;font-weight:600;margin:0}h3{font-size:19px;font-weight:500;margin:0 0 12px}.map-heading p{font-size:13px;color:var(--steel);margin:0}.text-action{display:inline-flex;align-items:center;justify-content:center;gap:7px;background:transparent;border:0;padding:8px 3px;min-height:40px;color:var(--steel);font-size:12px}.text-action:hover{color:var(--brand-accent)}.primary-action{display:inline-flex;align-items:center;justify-content:center;gap:10px;background:var(--brand-accent);color:var(--brand-on-accent);border:0;border-radius:8px;min-height:44px;padding:10px 16px;font-size:13px;white-space:nowrap}.progress-strip{display:grid;grid-template-columns:minmax(180px,1fr) minmax(200px,1.3fr) auto;gap:26px;align-items:center;padding:23px 0;border-block:1px solid var(--line);margin-bottom:32px}.course-progress>div{display:flex;align-items:baseline;justify-content:space-between;gap:12px}.course-progress span,.scope-progress>span{font-size:12px;color:var(--steel)}.course-progress strong{font-size:16px;font-weight:500}.course-progress small{display:block;font-size:11px;color:var(--steel);margin-top:6px}.course-progress progress{display:block;width:100%;height:5px;margin-top:10px;appearance:none;border:0;background:var(--surface-soft);border-radius:3px;overflow:hidden}.course-progress progress::-webkit-progress-bar{background:var(--surface-soft)}.course-progress progress::-webkit-progress-value{background:var(--brand-accent);border-radius:3px}.scope-progress>strong{display:block;font-weight:500;margin-top:5px}.recent-change{background:transparent;text-align:left;border:0;border-radius:0;border-left:1px solid var(--line);padding:4px 0 4px 22px}.recent-change>span{display:flex;gap:7px;align-items:center;color:var(--brand-accent);font-size:11px;margin-bottom:5px}.recent-change strong{font-size:13px;font-weight:400;line-height:1.8}.first-expression{font-size:13px;color:var(--steel);margin:0}.map-toolbar{display:flex;align-items:center;justify-content:space-between;flex-wrap:wrap;gap:12px;margin-bottom:20px}.map-toolbar nav{display:flex;flex-wrap:wrap;gap:7px;margin-top:5px}.map-toolbar nav button{background:transparent;border:0;color:var(--steel);font-size:11px;padding:3px 0}.map-toolbar nav button+button:before{content:'/';margin-right:7px;color:var(--steel)}.map-tools{display:flex;flex-wrap:wrap;align-items:center;gap:16px}.map-search{display:flex;align-items:center;gap:7px;border-bottom:1px solid var(--line);color:var(--steel);padding:4px 0}.map-search input{border:0;background:transparent;min-width:0;width:145px;font-size:12px;padding:7px 0;outline-offset:1px}.topic-grid{display:grid;grid-template-columns:repeat(auto-fit,minmax(min(225px,100%),1fr));gap:26px}.topic{min-width:0}.topic-heading{display:flex;align-items:center;text-align:left;width:100%;gap:11px;border:0;background:transparent;padding:10px 0 17px;min-height:76px}.topic-heading>span:nth-child(2){min-width:0;flex:1}.topic-number{font:400 31px/1 Georgia,serif;color:var(--steel);opacity:.7}.current-topic .topic-number,.current-topic .topic-heading small{color:var(--brand-accent);opacity:1}.topic-heading strong{display:block;font-size:14px;font-weight:500;line-height:1.7;overflow-wrap:anywhere}.topic-heading small{display:block;font-size:11px;color:var(--steel);margin-top:3px}.topic-heading>svg{color:var(--steel);flex-shrink:0}.topic-questions{border-left:1px solid var(--line);padding-left:16px;margin-left:9px;display:grid;gap:12px}.question-node{position:relative;display:flex;flex-direction:column;justify-content:space-between;gap:15px;text-align:left;min-height:110px;width:100%;padding:15px 14px;background:var(--reader-card,#fffefa);border:1px solid var(--line);border-radius:10px}.question-node:before{content:'';position:absolute;width:15px;height:1px;background:var(--line);top:26px;left:-16px}.question-node:after{content:'';position:absolute;width:5px;height:5px;border-radius:50%;background:var(--line);top:24px;left:-19px}.question-node[aria-pressed=true]{border-color:var(--brand-accent);background:var(--brand-accent-soft,var(--surface-soft))}.question-node[aria-pressed=true]:after{background:var(--brand-accent)}.question-node:hover{border-color:var(--brand-accent)}.question-node>strong{font-size:14px;font-weight:500;line-height:1.8;overflow-wrap:anywhere}.question-node>span{display:flex;gap:6px;align-items:center;font-size:11px;color:var(--steel)}.question-node>span[data-state=observed]{color:var(--success,#4b735b)}.question-node>span[data-state=difficulty]{color:var(--brand-accent)}.map-legend{display:flex;gap:18px;flex-wrap:wrap;font-size:11px;color:var(--steel);margin:22px 0}.map-legend>span{display:flex;gap:5px;align-items:center}.selected-understanding{scroll-margin-top:18px;margin:25px 0;border-top:1px solid var(--line);padding-top:15px}.selected-understanding>header{display:flex;align-items:center;justify-content:space-between;gap:16px}.selected-understanding .understanding-summary{margin:8px 0}.related-understanding{display:flex;align-items:center;gap:8px;flex-wrap:wrap;margin-top:16px;font-size:12px}.related-understanding>span{color:var(--steel);margin-right:5px}.related-understanding button{display:flex;align-items:center;gap:6px;background:var(--surface-soft);border:0;border-radius:6px;padding:7px 10px;font-size:12px}.related-understanding small{color:var(--steel)}.unexplored{padding:20px 0}.unexplored p{max-width:780px;white-space:pre-line;color:var(--steel)}.unexplored>div{display:flex;align-items:center;gap:18px;margin-top:20px}.map-footer{border-top:1px solid var(--line);display:flex;align-items:center;justify-content:space-between;gap:12px;margin-top:30px;padding-top:16px;font-size:11px;color:var(--steel)}.map-error{font-size:13px;color:var(--brand-accent)}.map-error button{border:0;background:transparent;color:inherit;text-decoration:underline}.empty-map{color:var(--steel);font-size:13px;padding:25px 0}.return-map{margin-bottom:20px}
@media(max-width:700px){.map-page{padding:25px 20px 45px}.progress-strip{grid-template-columns:1fr auto;gap:18px}.recent-change,.first-expression{grid-column:1/-1;grid-row:2;border:0;padding:0}.topic-grid{grid-template-columns:repeat(2,minmax(0,1fr));gap:20px}.map-heading{align-items:flex-start}.map-heading h1{font-size:29px}.map-heading>.text-action{white-space:nowrap}.topic-questions{padding-left:12px}.question-node:before{left:-12px;width:11px}.question-node:after{left:-15px}}
@media(max-width:460px){.topic:not(.expanded-topic) .topic-questions{display:none}.topic:not(.expanded-topic){border-bottom:1px solid var(--line)}.map-page{padding:23px 17px 40px}.progress-strip{grid-template-columns:1fr}.progress-strip>.primary-action{justify-self:start;grid-row:3}.course-progress strong{font-size:15px}.topic-grid{grid-template-columns:1fr;gap:18px}.topic-heading{min-height:64px;padding-bottom:12px}.topic-questions{grid-template-columns:1fr;gap:10px}.question-node{min-height:90px;padding:13px 14px;gap:9px}.map-tools{width:100%;justify-content:space-between;gap:8px}.map-search input{width:135px;font-size:16px}.map-toolbar nav button{min-height:34px}.map-footer{align-items:flex-start}.map-footer span{max-width:55%}.map-legend{gap:11px}.map-heading>.text-action{font-size:11px}.related-understanding button{min-height:40px}}
</style>
