<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref } from 'vue';
import { ArrowRight, BookOpen, GitCompareArrows, LayoutDashboard, MessageSquareText, Plus, Search, Workflow } from '@lucide/vue';
import { t as uiText } from '../i18n';
import { hostFetch } from '../reader-host';
import { boardDate } from '../board-workspace';
import { bookTitle } from '../book-title';
import type { BoardSummary } from '../generated/BoardSummary';
import type { LearningWorkspace, PublishedBookRef } from '../reader-context';
import type { SharedBook } from '../shared-workspaces';

const props = defineProps<{ materials: SharedBook[]; workspaces: LearningWorkspace[]; local?: boolean; busy?: boolean }>();
const emit = defineEmits<{
  (event: 'open', board: BoardSummary, publication: PublishedBookRef): void;
  (event: 'create', title: string, publication: PublishedBookRef): void;
  (event: 'materials'): void;
}>();
const items = ref<BoardSummary[]>([]), loading = ref(true), error = ref(''), query = ref('');
const creating = ref(false), title = ref(''), materialKey = ref('');
const titleInput = ref<HTMLInputElement>();
let request: AbortController | null = null;
const key = (reference: PublishedBookRef) => JSON.stringify(reference);
const matches = (left: PublishedBookRef, right: PublishedBookRef) => left.book_id === right.book_id && left.publication_id === right.publication_id;
function materialLabel(reference: PublishedBookRef) {
  const material = props.materials.find(item => matches(item.published_book_ref, reference));
  return material ? bookTitle({ title: material.title, book_id: reference.book_id }) : uiText('原资料暂不可用');
}
const rows = computed(() => items.value.map(board => {
  const publicationId = board.publication_id ?? (props.local ? 'local' : null);
  const original = publicationId ? { book_id: board.book_id, publication_id: publicationId } : null;
  const workspace = props.workspaces.find(item => item.learning_workspace_id === board.learning_workspace_id);
  const readableMembers = workspace?.members.filter(item => item.readable && props.materials.some(material => matches(material.published_book_ref, item.published_book_ref))) ?? [];
  const publication = board.learning_workspace_id
    ? readableMembers.find(item => original && matches(item.published_book_ref, original))?.published_book_ref ?? readableMembers[0]?.published_book_ref
    : original && props.materials.some(item => matches(item.published_book_ref, original)) ? original : undefined;
  const material = original ? materialLabel(original) : uiText('原资料暂不可用');
  const workspaceTitle = workspace?.goal.text ?? uiText('个人画板');
  const unavailable = workspace?.archived ? uiText('恢复所属工作区后可继续打开') : !publication ? uiText('所属资料暂不可打开，画板内容仍保留') : '';
  return { board, publication, material, workspaceTitle, unavailable };
}));
const filtered = computed(() => {
  const text = query.value.trim().toLocaleLowerCase();
  return text ? rows.value.filter(row => `${row.board.title}\n${row.material}\n${row.workspaceTitle}`.toLocaleLowerCase().includes(text)) : rows.value;
});
const selectedMaterial = computed(() => props.materials.find(item => key(item.published_book_ref) === materialKey.value));
async function load() {
  request?.abort(); const current = new AbortController(); request = current;
  loading.value = true; error.value = '';
  try { const result = await hostFetch<{ items: BoardSummary[] }>('GET', props.local ? '/book/library/boards' : '/me/boards', undefined, current.signal); if (!current.signal.aborted) items.value = result.items; }
  catch (failure) { if (!current.signal.aborted) error.value = failure instanceof Error ? failure.message : String(failure); }
  finally { if (!current.signal.aborted) loading.value = false; }
}
async function start(suggestion = '') {
  title.value = suggestion; creating.value = true;
  if (!selectedMaterial.value) materialKey.value = props.materials[0] ? key(props.materials[0].published_book_ref) : '';
  await nextTick(); titleInput.value?.focus();
}
function create() {
  if (!title.value.trim() || !selectedMaterial.value || props.busy) return;
  emit('create', title.value.trim(), selectedMaterial.value.published_book_ref);
}
onMounted(load);
onBeforeUnmount(() => request?.abort());
</script>

<template>
  <main class="board-library" :aria-label="uiText('学习画板')" :aria-busy="loading || busy">
    <header class="board-library-heading">
      <div><p class="board-library-eyebrow">{{ uiText('把资料变成自己的理解') }}</p><h1>{{ uiText('学习画板') }}</h1><p>{{ uiText('把书中的概念、视频里的过程画清楚，指着不懂的地方继续问。') }}</p></div>
      <button type="button" class="board-library-primary" data-tour="board-create" :disabled="busy" @click="start()"><Plus :size="17" aria-hidden="true" />{{ uiText('新建画板') }}</button>
    </header>
    <details class="board-library-intro" :open="!items.length">
      <summary>{{ uiText('画板可以帮我做什么？') }}</summary>
      <div class="board-library-uses" data-tour="board-uses" :aria-label="uiText('用画板开始理解')">
      <button type="button" :disabled="busy" @click="start(uiText('把这个过程画清楚'))"><Workflow :size="22" aria-hidden="true" /><span><strong>{{ uiText('梳理过程与关系') }}</strong><small>{{ uiText('看清每一步，以及它们为什么相连') }}</small></span><ArrowRight :size="16" aria-hidden="true" /></button>
      <button type="button" :disabled="busy" @click="start(uiText('比较这两个概念'))"><GitCompareArrows :size="22" aria-hidden="true" /><span><strong>{{ uiText('比较容易混淆的概念') }}</strong><small>{{ uiText('把相同点、区别和条件摆在一起') }}</small></span><ArrowRight :size="16" aria-hidden="true" /></button>
      <button type="button" :disabled="busy" @click="start(uiText('检查我对这里的理解'))"><MessageSquareText :size="22" aria-hidden="true" /><span><strong>{{ uiText('检查自己的理解') }}</strong><small>{{ uiText('留下你的想法，对照原资料继续追问') }}</small></span><ArrowRight :size="16" aria-hidden="true" /></button>
      </div>
    </details>
    <form v-if="creating" class="board-library-create" @submit.prevent="create">
      <template v-if="materials.length">
        <label>{{ uiText('想弄懂什么？') }}<input ref="titleInput" v-model="title" :disabled="busy" maxlength="160" required :placeholder="uiText('比如：为什么需要缓存？')" /></label>
        <label>{{ uiText('从哪份资料开始？') }}<select v-model="materialKey" :disabled="busy" required><option v-for="material in materials" :key="key(material.published_book_ref)" :value="key(material.published_book_ref)">{{ materialLabel(material.published_book_ref) }}</option></select></label>
        <div class="board-library-create-actions"><button type="submit" class="board-library-primary" :disabled="busy || !title.trim() || !selectedMaterial">{{ busy ? uiText('正在打开…') : uiText('创建并打开画板') }}<ArrowRight :size="16" aria-hidden="true" /></button><button type="button" :disabled="busy" @click="creating = false">{{ uiText('取消') }}</button></div>
      </template>
      <template v-else><p>{{ uiText('先添加一本书或一段视频，就能围绕资料建立画板。') }}</p><button type="button" class="board-library-primary" @click="emit('materials')">{{ uiText('查看学习资料') }}<ArrowRight :size="16" aria-hidden="true" /></button></template>
    </form>
    <section class="board-library-saved" :aria-label="uiText('我的画板')">
      <div class="board-library-list-heading" data-tour="board-library"><h2>{{ uiText('我的画板') }}<span v-if="!loading">{{ items.length }}</span></h2><label class="board-library-search"><Search :size="16" aria-hidden="true" /><input v-model="query" type="search" :aria-label="uiText('搜索画板或所属资料')" :placeholder="uiText('搜索画板或所属资料')" /></label></div>
      <p v-if="loading" class="board-library-state" role="status">{{ uiText('正在读取你的画板…') }}</p>
      <div v-else-if="error" class="board-library-error" role="alert"><p>{{ uiText('画板暂时无法读取：{0}', [error]) }}</p><button type="button" @click="load">{{ uiText('重试') }}</button></div>
      <div v-else-if="!items.length" class="board-library-empty"><LayoutDashboard :size="30" aria-hidden="true" /><h3>{{ uiText('从一个想弄懂的问题开始') }}</h3><p>{{ uiText('选一本书或一段视频，建立第一张画板。之后可以从这里直接继续。') }}</p><button type="button" class="board-library-primary" :disabled="busy" @click="start()">{{ uiText('创建第一张画板') }}<ArrowRight :size="16" aria-hidden="true" /></button></div>
      <p v-else-if="!filtered.length" class="board-library-state">{{ uiText('没有找到相关画板，试试问题名称或资料标题。') }}</p>
      <div v-else class="board-library-grid">
        <article v-for="row in filtered" :key="row.board.board_id" class="board-library-card">
          <div class="board-library-card-top"><LayoutDashboard :size="20" aria-hidden="true" /><span>{{ uiText('{0} 项内容', [row.board.item_count]) }}</span></div>
          <h3>{{ row.board.title }}</h3><p class="board-library-material"><BookOpen :size="14" aria-hidden="true" />{{ row.material }}</p><p class="board-library-workspace">{{ row.workspaceTitle }}</p>
          <p v-if="row.unavailable" class="board-library-unavailable">{{ row.unavailable }}</p>
          <footer><span>{{ boardDate(row.board.updated_at) }}</span><button type="button" :disabled="busy || !!row.unavailable" @click="row.publication && emit('open', row.board, row.publication)">{{ uiText('打开画板') }}<ArrowRight :size="15" aria-hidden="true" /></button></footer>
        </article>
      </div>
    </section>
  </main>
</template>

<style scoped>
.board-library{width:100%;max-width:1380px;margin:0 auto;padding:34px 40px 50px;box-sizing:border-box;color:var(--ink);font-family:var(--sans);overflow:auto;min-height:0;flex:1}.board-library *{box-sizing:border-box}.board-library button,.board-library input,.board-library select{font:inherit}.board-library button{display:inline-flex;align-items:center;justify-content:center;gap:8px;min-height:40px;padding:8px 13px;border:1px solid var(--line);border-radius:8px;background:var(--paper,var(--canvas));color:var(--ink);cursor:pointer}.board-library button:disabled{opacity:.5;cursor:default}.board-library button:focus-visible,.board-library input:focus-visible,.board-library select:focus-visible{outline:2px solid var(--brand-accent);outline-offset:3px}.board-library button.board-library-primary{background:var(--brand-accent);border-color:var(--brand-accent);color:var(--brand-on-accent,#fff)}.board-library-heading{display:flex;align-items:center;justify-content:space-between;gap:24px;margin-bottom:28px}.board-library-heading>div{min-width:0}.board-library h1{margin:5px 0 10px;font-family:var(--font-display);font-size:34px;font-weight:400;line-height:1.4}.board-library-heading p{margin:0;color:var(--steel);line-height:1.8;font-size:14px}.board-library-heading .board-library-eyebrow{color:var(--brand-accent);font-size:12px}.board-library-heading>button{flex-shrink:0}.board-library-uses{display:grid;grid-template-columns:repeat(3,minmax(0,1fr));gap:12px;margin-bottom:30px}.board-library-uses button{justify-content:flex-start;text-align:left;padding:18px 15px;gap:12px;background:var(--surface-soft);border-color:var(--hairline)}.board-library-uses button>svg{flex-shrink:0;color:var(--brand-accent)}.board-library-uses button>svg:last-child{margin-left:auto;width:16px}.board-library-uses span{display:grid;gap:5px;min-width:0}.board-library-uses strong{font-size:14px;font-weight:500;line-height:1.5}.board-library-uses small{font-size:12px;color:var(--steel);line-height:1.7}.board-library-create{display:grid;grid-template-columns:minmax(0,1fr) minmax(0,1fr);gap:18px;padding:22px;margin-bottom:30px;border:1px solid var(--line);border-radius:12px;background:var(--surface-soft)}.board-library-create label{display:flex;flex-direction:column;gap:8px;font-size:13px;min-width:0}.board-library-create input,.board-library-create select{width:100%;min-width:0;padding:10px 12px;border:1px solid var(--line);border-radius:7px;background:var(--paper,var(--canvas));color:var(--ink)}.board-library-create-actions{grid-column:1/-1;display:flex;flex-wrap:wrap;gap:10px}.board-library-create>p{grid-column:1/-1;margin:0;line-height:1.8}.board-library-list-heading{display:flex;align-items:center;justify-content:space-between;gap:20px;margin-bottom:18px}.board-library h2{font-size:20px;font-weight:500;margin:0}.board-library h2>span{font:13px var(--sans);margin-left:10px;color:var(--steel)}.board-library-search{display:flex;align-items:center;gap:8px;max-width:100%;width:280px;border:1px solid var(--line);border-radius:8px;padding:9px 12px;color:var(--steel)}.board-library-search input{min-width:0;width:100%;border:0;background:transparent;color:var(--ink);padding:0;font-size:13px}.board-library-grid{display:grid;grid-template-columns:repeat(auto-fill,minmax(265px,1fr));gap:18px}.board-library-card{display:flex;flex-direction:column;min-width:0;padding:22px;border:1px solid var(--line);border-radius:12px;background:var(--paper,var(--canvas))}.board-library-card-top{display:flex;align-items:center;justify-content:space-between;color:var(--brand-accent);gap:12px}.board-library-card-top span{font-size:12px;color:var(--steel)}.board-library-card h3{font-size:18px;font-weight:500;line-height:1.65;margin:18px 0 12px;overflow-wrap:anywhere}.board-library-material{display:flex;align-items:flex-start;gap:7px;font-size:12px;line-height:1.8;margin:0 0 5px;color:var(--steel);overflow-wrap:anywhere}.board-library-material>svg{flex-shrink:0;margin-top:3px}.board-library-workspace{font-size:12px;line-height:1.8;margin:0 0 20px;color:var(--steel);overflow-wrap:anywhere}.board-library-card footer{display:flex;align-items:center;justify-content:space-between;gap:8px;margin-top:auto;padding-top:16px;border-top:1px solid var(--hairline)}.board-library-card footer>span{font-size:11px;color:var(--steel)}.board-library-card footer button{color:var(--brand-accent);font-size:13px;border-color:transparent;padding:6px 0}.board-library-unavailable{font-size:12px;color:var(--steel);line-height:1.8;margin:0 0 12px}.board-library-state{padding:28px 0;color:var(--steel);font-size:14px;line-height:1.8}.board-library-error{display:flex;align-items:center;gap:15px;flex-wrap:wrap;color:var(--brand-error);padding:20px 0}.board-library-error p{margin:0;overflow-wrap:anywhere}.board-library-empty{display:flex;flex-direction:column;align-items:center;text-align:center;padding:44px 24px;background:var(--surface-soft);border-radius:12px}.board-library-empty>svg{color:var(--brand-accent)}.board-library-empty h3{font-size:20px;font-weight:500;margin:18px 0 0}.board-library-empty p{color:var(--steel);font-size:14px;line-height:1.8;max-width:450px;margin:12px 0 22px}
.board-library-intro{margin-bottom:28px}.board-library-intro summary{width:fit-content;color:var(--steel);font-size:13px;line-height:1.8;cursor:pointer;padding:4px 0}.board-library-intro[open] summary{margin-bottom:14px}.board-library-intro .board-library-uses{margin-bottom:0}
@media(max-width:1000px){.board-library{padding:28px 24px 40px}.board-library-uses{grid-template-columns:minmax(0,1fr)}.board-library-uses button{padding:14px 18px}.board-library-uses span{display:flex;gap:14px;align-items:baseline;flex-wrap:wrap}}
@media(max-width:600px){.board-library{padding:24px 16px 34px}.board-library-heading{align-items:flex-start;gap:14px;flex-direction:column}.board-library h1{font-size:29px}.board-library-heading>button{width:100%}.board-library-uses span{display:grid;gap:4px}.board-library-list-heading{align-items:stretch;flex-direction:column;gap:13px}.board-library-search{width:100%}.board-library-grid{grid-template-columns:minmax(0,1fr)}.board-library-create{grid-template-columns:minmax(0,1fr);padding:17px}.board-library-create input,.board-library-create select{font-size:16px}.board-library-create-actions{align-items:stretch}.board-library-create-actions>button{flex:1}.board-library button{min-height:44px}.board-library-empty{padding:30px 17px}}
</style>
