<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue';
import { ArrowLeft, ArrowRight, ArrowUpRight, BookOpen, ChevronRight, MapPin, RefreshCw, Search, Sprout, X } from '@lucide/vue';
import { api as sharedApi } from '../api';
import { bindSceneApi, readerKey } from '../reader-context';
import { notesOwner, openPersonalNotes } from '../personal-notes';
import { continueUnderstanding, type UnderstandingSummary } from '../note-thinking';
import { discussKnowledge, pendingKnowledgeReference, knowledgeKinds, knowledgeRelations, openKnowledgeSource, type KnowledgeNodeView, type KnowledgeTreeRequest, type KnowledgeTreeView } from '../knowledge-tree';
import UnderstandingSummaryCard from './UnderstandingSummary.vue';
import UnderstandingSpace from './UnderstandingSpace.vue';

const props = defineProps<{ sourceId?: string; currentLid?: string; positionMs?: number }>();
const api = bindSceneApi(sharedApi);
const view = ref<KnowledgeTreeView | null>(null), busy = ref(false), error = ref('');
const query = ref(''), viewQuery = ref(''), searchOpen = ref(false), evidence = ref<string>();
const heading = ref<HTMLElement>(), fruitArea = ref<HTMLElement>(), searchInput = ref<HTMLInputElement>();
const treeKey = `knowledge-tree:${notesOwner() ?? 'local'}:${readerKey()}:${props.sourceId ?? ''}`;
let controller: AbortController | undefined, timer: ReturnType<typeof setTimeout> | undefined, disposed = false;
let lastRequest: KnowledgeTreeRequest = {};
const compact = computed(() => (view.value?.children.length ?? 0) === 3 && !viewQuery.value);
const single = computed(() => view.value?.children.length === 1);
const height = computed(() => compact.value ? 412 : single.value ? 260 : Math.ceil((view.value?.children.length ?? 0) / 2) * 146 + 125);
const branches = computed(() => (view.value?.children ?? []).map((node, i) => {
  const center = single.value || compact.value && i === 0;
  const left = compact.value ? i === 1 : i % 2 === 0;
  const y = compact.value ? i === 0 ? 16 : 170 : 16 + Math.floor(i / 2) * 146;
  const endX = center ? 300 : left ? 160 : 440;
  return { node, center, left, y, curve: `M 300 ${height.value - 44} C ${center ? 286 : left ? 265 : 335} ${y + 95}, ${endX} ${y + 105}, ${endX} ${y + 58}` };
}));
const entry = computed(() => viewQuery.value ? null : view.value?.entry_point);
const continuing = computed(() => !viewQuery.value && view.value?.node.child_count ? view.value.fruits.find(item => !item.later_evidence_ref) : undefined);
const introduction = computed(() => view.value?.introduction?.text || view.value?.node.summary);
const entryHint = computed(() => {
  if (entry.value?.kind === 'key_stop') return '材料已有的关键内容';
  if (entry.value?.kind === 'claim') return '按原文顺序，先看第一条论断';
  if (entry.value?.kind === 'music_event') return '从这一段的第一个音乐事件听起';
  return '顺着内容，向下展开';
});
function current(node: KnowledgeNodeView) {
  if (props.currentLid && node.reference.artifact === 'text') return props.currentLid === node.reference.entry_id || props.currentLid.startsWith(`${node.reference.entry_id}.`);
  if (props.positionMs != null) return node.sources.some(s => s.kind === 'music' && s.target.audio.start_ms <= props.positionMs! && props.positionMs! < s.target.audio.end_ms
    || s.kind === 'media' && s.start_ms != null && s.start_ms <= props.positionMs! && props.positionMs! < (s.end_ms ?? s.start_ms + 1000));
  return view.value?.current_ids.includes(node.id) ?? false;
}
async function load(request: KnowledgeTreeRequest = {}, quiet = false): Promise<boolean> {
  controller?.abort();
  const call = new AbortController(); controller = call; lastRequest = request;
  if (!quiet) busy.value = true;
  error.value = '';
  try {
    const result = await api.knowledgeTree({ ...request, current_lid: props.currentLid, position_ms: props.positionMs }, call.signal);
    if (disposed || call.signal.aborted) return false;
    view.value = result; viewQuery.value = request.query ?? '';
    // Resolve an initial reading location once. Background refresh keeps the
    // chosen branch even when playback or the reading position moves on.
    const { reference: _reference, ...resolved } = request;
    lastRequest = { ...resolved, node_id: result.node.id };
    try { localStorage.setItem(treeKey, result.node.id); } catch { /* Optional navigation memory. */ }
    return true;
  } catch (failure) {
    if (!disposed && !call.signal.aborted) error.value = failure instanceof Error ? failure.message : String(failure);
    return false;
  } finally {
    if (controller === call) { busy.value = false; controller = undefined; schedule(); }
  }
}
function schedule() {
  clearTimeout(timer);
  if (!disposed) timer = setTimeout(() => { if (document.visibilityState === 'visible' && !controller) void load(lastRequest, true); else schedule(); }, 10000);
}
async function open(id: string, fruit = false) {
  query.value = ''; searchOpen.value = false; evidence.value = undefined;
  if (!await load({ node_id: id })) return;
  await nextTick();
  const target = fruit ? fruitArea.value : heading.value;
  target?.focus({ preventScroll: true });
  target?.scrollIntoView({ block: 'start', behavior: 'smooth' });
}
function search() { void load({ node_id: view.value?.node.id, query: query.value }); }
async function toggleSearch() {
  searchOpen.value = !searchOpen.value;
  if (searchOpen.value) { await nextTick(); searchInput.value?.focus(); }
  else if (viewQuery.value) { query.value = ''; void load({ node_id: view.value?.node.id }); }
}
async function locate() {
  query.value = ''; searchOpen.value = false;
  if (await load({})) { await nextTick(); heading.value?.scrollIntoView({ block: 'start', behavior: 'smooth' }); }
}
async function resume(item: UnderstandingSummary, mode: 'discuss' | 'practice', message?: string) {
  try { await continueUnderstanding(item, undefined, mode, message); } catch (failure) { error.value = (failure as Error).message; }
}
function refresh() { if (!controller) void load(lastRequest, true); }
async function retryObservation(id: string) {
  try { await api.knowledgeRetry(id); await load(lastRequest, true); } catch (failure) { error.value = (failure as Error).message; }
}
onMounted(() => {
  let id: string | undefined;
  if (!props.currentLid && props.positionMs == null) { try { id = localStorage.getItem(treeKey) ?? undefined; } catch {} }
  const reference = pendingKnowledgeReference.value; pendingKnowledgeReference.value = null;
  void load(reference ? { reference } : id ? { node_id: id } : {});
  window.addEventListener('tutor-state-changed', refresh);
});
watch(pendingKnowledgeReference, reference => {
  if (reference) { pendingKnowledgeReference.value = null; evidence.value = undefined; query.value = ''; searchOpen.value = false; void load({ reference }); }
});
onBeforeUnmount(() => { disposed = true; controller?.abort(); clearTimeout(timer); window.removeEventListener('tutor-state-changed', refresh); });
</script>

<template>
  <section class="knowledge-space" aria-label="知识树" :aria-busy="busy">
    <template v-if="evidence">
      <button class="tree-back" @click="evidence=undefined"><ArrowLeft :size="15"/>回到这颗果实</button>
      <UnderstandingSpace :evidence-ref="evidence"/>
    </template>
    <template v-else>
      <div class="tree-masthead">
        <span class="tree-eyebrow"><Sprout :size="15"/>我的知识树</span>
        <div class="tree-tools">
          <button :disabled="!currentLid && positionMs == null" @click="locate"><MapPin :size="13"/>我在这里</button>
          <button aria-label="搜索知识树" :aria-expanded="searchOpen" @click="toggleSearch"><Search :size="15"/></button>
          <button aria-label="刷新知识树" :disabled="busy" @click="load(lastRequest)"><RefreshCw :size="13"/></button>
        </div>
      </div>
      <form v-if="searchOpen" class="tree-search" @submit.prevent="search">
        <input ref="searchInput" v-model="query" aria-label="寻找知识内容" placeholder="寻找概念、论断或片段"/>
        <button :disabled="busy">寻找</button><button type="button" aria-label="关闭搜索" @click="toggleSearch"><X :size="14"/></button>
      </form>
      <p v-if="error" class="tree-error" role="alert">{{error}} <button @click="load(lastRequest)">重试</button><button @click="open('root')">回到全貌</button></p>
      <p v-if="busy&&!view" class="tree-loading" role="status">正在展开已有内容…</p>
      <template v-if="view">
        <nav class="tree-path" aria-label="知识位置">
          <template v-for="(crumb,i) in view.breadcrumbs" :key="crumb.id"><ChevronRight v-if="i" :size="12"/><button :aria-current="crumb.id===view.node.id?'location':undefined" @click="open(crumb.id)">{{crumb.id==='root'?'内容全貌':crumb.label}}</button></template>
        </nav>
        <header ref="heading" tabindex="-1" class="knowledge-heading detail-title">
          <span class="location-caption">{{viewQuery?'搜索结果':knowledgeKinds[view.node.kind]??'当前内容'}}<span v-if="!viewQuery && current(view.node)" class="here-label"><MapPin :size="11"/>正在{{view.modality==='music'?'听':view.modality==='video'?'看':'读'}}这里</span></span>
          <h3>{{viewQuery?`“${viewQuery}”`:view.node.label}}</h3>
          <template v-if="!viewQuery && introduction">
            <span v-if="view.node.child_count && view.introduction?.is_opening" class="intro-caption">开篇线索</span>
            <p class="knowledge-summary">{{introduction}}</p>
          </template>
          <p v-else-if="viewQuery" class="scope-hint">找到 {{view.total}} 项内容，点开后可以查看它在树上的位置。</p>
          <p v-else-if="view.node.child_count" class="scope-hint">{{view.node.kind==='material'?'沿着材料展开，找到想理解的内容。':`这里有 ${view.node.child_count} 个分枝，可以逐步展开。`}}</p>
          <div v-if="!viewQuery && !view.node.child_count" class="knowledge-actions">
            <button v-if="view.node.id!=='root' && view.node.reference.artifact!=='navigation'" class="discuss-branch" @click="discussKnowledge(view.node,readerKey())">从这里聊起<ArrowUpRight :size="14"/></button>
            <button v-for="(source,i) in view.node.sources" :key="i" class="source-action" @click="openKnowledgeSource(source,readerKey())"><BookOpen :size="13"/>{{source.kind==='music'?(source.target.score?'回听与看谱':'回听原声'):source.kind==='media'?'回看来源':'打开原文'}}{{view.node.sources.length>1?` ${i+1}`:''}}</button>
          </div>
        </header>

        <div v-if="continuing" class="starting-point continuing-point">
          <span class="start-caption"><i class="fruit-dot"/>接着你的理解</span>
          <blockquote>{{continuing.learner_quote || continuing.label}}</blockquote>
          <p v-if="continuing.next_step">{{continuing.next_step}}</p>
          <button @click="resume(continuing,'discuss')">继续推敲<ArrowRight :size="15"/></button>
        </div>
        <div v-else-if="entry" class="starting-point">
          <span class="start-caption">从这里开始<span>{{entryHint}}</span></span>
          <strong>{{entry.label}}</strong>
          <button @click="open(entry.id)">{{entry.child_count?'展开这一枝':'看看这一点'}}<ArrowRight :size="15"/></button>
        </div>

        <template v-if="view.children.length">
          <div class="branches-caption"><span>{{viewQuery?'匹配的内容':'沿枝条展开'}}</span><small>{{view.total}} {{viewQuery?'项内容':'个分枝'}}<span v-if="view.node.fruit_count"> · {{view.node.fruit_count}} 颗理解果实</span></small></div>
          <div class="living-tree" :style="{height:`${height}px`}" :class="{'tree-busy':busy,'compact-tree':compact}" aria-label="内容枝条">
            <svg class="tree-branches" :viewBox="`0 0 600 ${height}`" preserveAspectRatio="none" aria-hidden="true">
              <path :d="`M 300 ${height-2} Q 287 ${height-68} 300 ${height-146}`" stroke="#a38c68" stroke-width="12" fill="none" stroke-linecap="round"/>
              <path v-for="branch in branches" :key="branch.node.id" :d="branch.curve" stroke="#b4a580" stroke-width="3" fill="none" stroke-linecap="round"/>
              <path :d="`M 300 ${height-18} Q 260 ${height-3} 230 ${height-2} M 300 ${height-18} Q 341 ${height-5} 370 ${height-2}`" fill="none" stroke="#a38c68" stroke-width="3" stroke-linecap="round"/>
            </svg>
            <div v-for="branch in branches" :key="branch.node.id" class="tree-branch" :class="{'branch-center':branch.center,'branch-left':!branch.center&&branch.left,'branch-right':!branch.center&&!branch.left,'is-current':current(branch.node),'branch-primary':compact&&branch.center}" :style="{top:`${branch.y}px`}">
              <button class="branch-content" @click="open(branch.node.id)">
                <span class="branch-kind"><MapPin v-if="current(branch.node)" :size="11"/>{{knowledgeKinds[branch.node.kind]??'内容'}}<span v-if="branch.node.child_count"> · {{branch.node.child_count}} 项</span></span>
                <strong>{{branch.node.label}}</strong>
                <span v-if="branch.node.preview?.length" class="branch-preview">{{branch.node.preview.join(' · ')}}</span>
                <span class="branch-open">{{branch.node.child_count?'展开':'查看'}}<ArrowUpRight :size="11"/></span>
              </button>
              <button v-if="branch.node.fruit_count" class="understanding-fruit" :aria-label="`${branch.node.label}，${branch.node.fruit_count} 颗理解果实`" @click="open(branch.node.id,true)">{{branch.node.fruit_count}}</button>
            </div>
            <div class="tree-root-label"><Sprout :size="13"/><strong>{{view.node.label}}</strong></div>
          </div>
          <div v-if="view.total>12" class="tree-pagination"><button :disabled="busy||!(lastRequest.after??0)" @click="load({...lastRequest,after:Math.max(0,(lastRequest.after??0)-12)})">前一组枝条</button><span>{{(lastRequest.after??0)+1}}–{{Math.min((lastRequest.after??0)+12,view.total)}} / {{view.total}}</span><button :disabled="busy||view.next==null" @click="load({...lastRequest,after:view.next??0})">更多枝条</button></div>
        </template>
        <p v-else-if="viewQuery" class="fruit-empty">没有找到这项内容，可以换个说法或沿枝条浏览。</p>
        <div v-if="!viewQuery && view.node.child_count && view.node.sources.length" class="knowledge-actions scope-actions"><button v-for="(source,i) in view.node.sources" :key="i" class="source-action" @click="openKnowledgeSource(source,readerKey())"><BookOpen :size="13"/>{{source.kind==='music'?'回听这一段':source.kind==='media'?'回看这一段':'读这一节原文'}}{{view.node.sources.length>1?` ${i+1}`:''}}</button></div>
        <div v-if="!viewQuery && view.node.relations.length" class="knowledge-relations"><span>相关联系</span><button v-for="(relation,i) in view.node.relations" :key="i" :disabled="!relation.available" @click="open(relation.target_id)"><b>{{knowledgeRelations[relation.kind]??relation.kind}}</b>{{relation.label}}<ChevronRight :size="13"/></button></div>

        <section v-if="!viewQuery" ref="fruitArea" tabindex="-1" class="tree-fruits" aria-label="这处内容的理解果实">
          <h4><i class="fruit-dot"/>这里的理解果实 <span>{{view.node.fruit_count}}</span></h4>
          <p v-if="!view.fruits.length" class="fruit-empty">{{view.node.fruit_count?'理解记录在更细的枝条上，点橙色果实继续寻找。':'这里还没有你的理解记录。聊聊你怎么看，新的表达会留在对应枝条上。'}}</p>
          <UnderstandingSummaryCard v-for="fruit in view.fruits" :key="fruit.evidence_ref" :item="fruit" show-quote @discuss="resume(fruit,'discuss',$event)" @practice="resume(fruit,'practice')" @evidence="evidence=fruit.evidence_ref" @correct="evidence=fruit.evidence_ref" @note="openPersonalNotes"/>
          <div v-if="view.next_fruit!=null || lastRequest.fruit_after" class="tree-pagination"><button :disabled="!lastRequest.fruit_after" @click="load({...lastRequest,fruit_after:Math.max(0,(lastRequest.fruit_after??0)-12)})">前一组理解</button><button :disabled="view.next_fruit==null" @click="load({...lastRequest,fruit_after:view.next_fruit??0})">更多理解</button></div>
        </section>
        <p v-for="update in view.updates" :key="update.job_ref" class="tree-update" role="status">{{['failed','interrupted'].includes(update.status)?'有一条表达尚未整理完成，原话和已有理解都已保留。':'新的表达正在整理，已有果实仍可查看。'}} <button v-if="['failed','interrupted'].includes(update.status)" @click="retryObservation(update.job_ref)">重试整理</button></p>
      </template>
    </template>
  </section>
</template>

<style scoped>
.knowledge-space{container-type:inline-size;color:var(--ink,#342f29);font-size:13px;line-height:1.65;min-width:0}
.knowledge-space button{font:inherit;color:inherit;cursor:pointer}.knowledge-space button:disabled{opacity:.45;cursor:default}.knowledge-space button:focus-visible{outline:2px solid #927b54;outline-offset:3px}
.tree-masthead,.tree-tools{display:flex;align-items:center;gap:8px}.tree-masthead{justify-content:space-between;margin-bottom:18px}.tree-eyebrow{display:flex;align-items:center;gap:6px;color:#7b8566;font-size:11px;letter-spacing:.08em}.tree-tools{gap:3px;color:var(--steel,#80786d)}
.tree-tools button,.tree-back,.tree-pagination button,.tree-search button{border:0;background:transparent;padding:6px;display:inline-flex;gap:4px;align-items:center;font-size:11px}
.tree-path{display:flex;flex-wrap:wrap;align-items:center;gap:2px;font-size:10px;margin-bottom:18px;color:var(--steel,#80786d)}.tree-path button{max-width:220px;overflow:hidden;text-overflow:ellipsis;white-space:nowrap;background:transparent;border:0;padding:2px;color:inherit}.tree-path button[aria-current]{color:var(--ink,#342f29)}
.knowledge-heading{outline-offset:5px;margin-bottom:22px;scroll-margin-top:20px}.location-caption{display:flex;align-items:center;gap:10px;color:#7b8566;font-size:10px}.here-label{display:inline-flex;align-items:center;gap:3px;color:#627e63}.knowledge-heading h3{font-family:var(--font-serif,serif);font-size:25px;line-height:1.4;font-weight:500;margin:7px 0 13px;overflow-wrap:anywhere}.intro-caption{font-size:10px;color:#8a806e;letter-spacing:.08em}.knowledge-summary{margin:5px 0 12px;font-size:13px;line-height:1.85;white-space:pre-wrap;overflow-wrap:anywhere;color:var(--steel,#756e63)}.scope-hint{font-size:12px;color:var(--steel,#80786d);margin:0}
.starting-point{padding:16px 18px;margin:0 0 28px;background:#f2f4e9;border-left:3px solid #83946b;border-radius:0 12px 12px 0}.start-caption{display:flex;flex-wrap:wrap;gap:4px 12px;align-items:center;font-size:11px;color:#64764f}.start-caption>span{font-size:10px;color:#8c917f}.starting-point>strong{display:block;font-size:16px;line-height:1.6;font-weight:500;margin:7px 0 11px;overflow-wrap:anywhere}.starting-point>button{display:inline-flex;align-items:center;gap:12px;padding:7px 12px;border:0;border-radius:6px;background:#657750;color:#fffdf8;font-size:12px}.continuing-point{background:#faf0e4;border-color:#c88747}.continuing-point .start-caption{color:#a56532}.continuing-point>button{background:#a96a36}.starting-point blockquote{margin:8px 0;font-family:var(--font-serif,serif);font-size:16px;overflow-wrap:anywhere}.starting-point>p{font-size:12px;color:var(--steel,#80786d)}
.branches-caption{display:flex;align-items:baseline;justify-content:space-between;gap:8px;font-size:12px;margin-bottom:4px}.branches-caption small{font-size:10px;color:var(--steel,#80786d)}
.living-tree{position:relative;isolation:isolate;background:radial-gradient(ellipse at 50% 45%,#edf0df80,transparent 70%);margin:0 -6px;transition:opacity .2s}.tree-busy{opacity:.6}.tree-branches{position:absolute;inset:0;width:100%;height:100%;z-index:-1}.tree-branch{position:absolute;width:44%}.branch-left{left:1%}.branch-right{right:1%}.branch-center{left:10%;width:80%}.branch-content{position:relative;text-align:left;display:flex;flex-direction:column;width:100%;min-height:114px;border:1px solid #dcdccb;background:var(--surface,#fffdf9);border-radius:18px 18px 18px 4px;padding:11px 14px;box-shadow:0 4px 14px #535c3505}.branch-right .branch-content{border-radius:18px 18px 4px 18px}.branch-center .branch-content{border-radius:20px 20px 20px 6px}.branch-content:hover{border-color:#8c9872;background:#f7f8f0}.branch-kind{display:flex;align-items:center;gap:4px;color:#7d846c;font-size:10px}.branch-content strong{font-weight:500;font-size:13px;line-height:1.5;display:-webkit-box;-webkit-line-clamp:2;-webkit-box-orient:vertical;overflow:hidden;overflow-wrap:anywhere;margin:3px 0 5px}.branch-preview{display:-webkit-box;-webkit-line-clamp:2;-webkit-box-orient:vertical;overflow:hidden;overflow-wrap:anywhere;font-size:11px;line-height:1.6;color:#8c8c7a;margin-bottom:6px}.branch-open{font-size:10px;color:#7f886e;display:flex;gap:4px;align-items:center;margin-top:auto}.compact-tree .branch-content{min-height:138px}.branch-primary .branch-content{border-color:#9ba881;background:#f5f6ec;min-height:126px}.branch-primary strong{font-size:16px}.branch-primary .branch-preview{-webkit-line-clamp:1}.is-current .branch-content{border-color:#75967a;box-shadow:0 0 0 2px #7895781a}
.understanding-fruit{position:absolute;right:-5px;bottom:-9px;border:2px solid var(--surface,#fffdf9);width:31px;height:34px;border-radius:48% 48% 50% 50%;background:linear-gradient(135deg,#e3a15e,#bb652f);color:#fffaf3!important;font-size:11px!important;box-shadow:0 3px 8px #91522225}.understanding-fruit:before{content:'';position:absolute;width:12px;height:6px;left:12px;top:-5px;background:#819062;border-radius:100% 0 100% 0;transform:rotate(-20deg)}.fruit-dot{display:inline-block;width:9px;height:11px;border-radius:50%;background:#c87538;margin-right:5px;vertical-align:middle}
.tree-root-label{position:absolute;bottom:24px;left:18%;width:64%;padding:9px 12px;background:var(--surface,#fffdf9);text-align:center;border:1px solid #ddd4c2;border-radius:12px;box-sizing:border-box;display:flex;align-items:center;justify-content:center;gap:7px;color:#8b8166}.tree-root-label svg{flex-shrink:0}.tree-root-label strong{font-family:var(--font-serif,serif);font-size:13px;font-weight:500;display:-webkit-box;-webkit-line-clamp:2;-webkit-box-orient:vertical;overflow:hidden;color:var(--ink,#342f29)}
.tree-pagination{display:flex;align-items:center;justify-content:space-between;color:var(--steel,#80786d);font-size:11px;border-top:1px solid var(--border,#ded6c9);padding-top:8px}.knowledge-actions{display:flex;flex-wrap:wrap;gap:7px;margin-top:14px}.knowledge-actions button{display:inline-flex;align-items:center;gap:4px;border:1px solid var(--border,#ded6c9);border-radius:7px;background:transparent;padding:7px 10px;font-size:11px}.knowledge-actions .discuss-branch{color:#fffaf2;background:#a76b40;border-color:#a76b40}.scope-actions{margin:5px 0 22px}.knowledge-relations{margin:22px 0;font-size:11px}.knowledge-relations>span{color:var(--steel,#80786d)}.knowledge-relations button{display:flex;align-items:center;gap:9px;width:100%;border:0;border-bottom:1px solid var(--border,#ded6c9);padding:8px 0;text-align:left;background:transparent;overflow-wrap:anywhere}.knowledge-relations b{font-size:10px;white-space:nowrap;color:#7a8864;font-weight:500}.knowledge-relations svg{flex-shrink:0;margin-left:auto}
.tree-fruits{outline-offset:4px;scroll-margin-top:20px;border-top:1px solid var(--border,#ded6c9);padding-top:12px;margin-top:24px}.tree-fruits h4{font-size:13px;font-weight:500;margin:5px 0 10px}.tree-fruits h4 span{color:#a8532d;margin-left:4px}.tree-fruits :deep(.understanding-summary){margin:10px 0}.fruit-empty{font-size:12px;color:var(--steel,#80786d);line-height:1.8}.tree-back{margin-bottom:15px}.tree-search{display:flex;align-items:center;gap:8px;border-bottom:1px solid var(--border,#ded6c9);padding:6px 0 10px;margin-bottom:18px}.tree-search input{flex:1;min-width:0;border:0;background:transparent;color:inherit;font:inherit;outline-offset:4px}.tree-search button{color:var(--accent,#a8532d)}.tree-error{border-left:2px solid #b96639;padding:8px 10px;background:#f6eee3}.tree-error button{margin-left:8px}.tree-loading{padding:40px 0;color:var(--steel,#80786d)}.tree-update{font-size:11px;color:var(--steel,#80786d);margin-top:20px}
@container (max-width:440px){.tree-path button{max-width:155px}.knowledge-heading h3{font-size:22px}.tree-masthead{margin-bottom:13px}.tree-path{margin-bottom:14px}.starting-point{padding:13px 14px;margin-bottom:24px}.starting-point>strong{font-size:15px}.branch-content{padding:10px;min-height:118px}.branch-content strong{font-size:12px}.branch-primary strong{font-size:15px}.compact-tree .branch-content{min-height:138px}.branch-center{left:5%;width:90%}.branch-primary .branch-preview{-webkit-line-clamp:2}.tree-root-label{left:12%;width:76%}.branches-caption small{font-size:9px}}
</style>
