<script setup lang="ts">
import { computed, nextTick, ref, watch } from 'vue';
import { ChevronDown, ChevronRight, ListTree, Search } from '@lucide/vue';
import type { BoardItem } from '../generated/BoardItem';
import type { BoardMindMap } from '../generated/BoardMindMap';
import { topicChildren, branchIds } from '../mind-map-editor';
const props = defineProps<{ items: BoardItem[]; mindMaps: BoardMindMap[]; selectedIds: string[]; foldedIds: string[]; editingId: string | null; draftTitle: string; disabled?: boolean }>();
const emit = defineEmits<{
  (event: 'select', id: string, additive: boolean): void; (event: 'fold', id: string): void;
  (event: 'edit', id: string): void; (event: 'draft', text: string): void; (event: 'commit'): void; (event: 'cancel'): void;
  (event: 'keydown', keyboard: KeyboardEvent, id: string): void; (event: 'drop', target: { itemId: string; parentId: string; index: number }): void;
  (event: 'locate', id: string): void;
}>();
const root = ref<HTMLElement>(), query = ref(''), dragging = ref<string | null>(null), dropId = ref<string | null>(null), dropPosition = ref<'before' | 'after' | 'inside'>('inside');
const rows = computed(() => {
  const byId = new Map(props.items.map(item => [item.item_id, item])), result: { item: BoardItem; depth: number; parent: string | null; hidden: number }[] = [];
  const term = query.value.trim().toLocaleLowerCase();
  const matches = (id: string) => !term || branchIds(props.items, id).some(descendant => byId.get(descendant)?.title.toLocaleLowerCase().includes(term));
  function walk(id: string, depth: number, parent: string | null) { const item = byId.get(id); if (!item || !matches(id)) return;
    const folded = props.foldedIds.includes(id), hidden = folded ? branchIds(props.items, id).length - 1 : 0;
    result.push({ item, depth, parent, hidden }); if (!folded || term) topicChildren(item).forEach(child => walk(child, depth + 1, id)); }
  props.mindMaps.forEach(map => walk(map.root_item_id, 0, null)); return result;
});
watch(() => props.editingId, async id => { await nextTick(); if (id) { const input = root.value?.querySelector<HTMLTextAreaElement>('textarea'); input?.focus(); input?.select(); } else if (props.selectedIds[0]) root.value?.querySelector<HTMLElement>(`[data-outline-item="${CSS.escape(props.selectedIds[0])}"]`)?.focus(); }, { immediate: true });
function inputKey(event: KeyboardEvent) { event.stopPropagation(); if (event.isComposing || event.keyCode === 229) return; if (event.key === 'Enter' && !event.shiftKey) { event.preventDefault(); emit('commit'); } else if (event.key === 'Escape') { event.preventDefault(); emit('cancel'); } }
function dragOver(event: DragEvent, row: { item: BoardItem; parent: string | null }) {
  event.preventDefault(); if (!dragging.value || branchIds(props.items, dragging.value).includes(row.item.item_id)) { dropId.value = null; return; }
  const bounds = (event.currentTarget as HTMLElement).getBoundingClientRect(), relative = (event.clientY - bounds.top) / bounds.height;
  dropId.value = row.item.item_id; dropPosition.value = row.parent && relative < .25 ? 'before' : row.parent && relative > .75 ? 'after' : 'inside';
}
function drop(event: DragEvent, row: { item: BoardItem; parent: string | null }) {
  event.preventDefault();
  if (dragging.value && dropId.value === row.item.item_id && dragging.value !== row.item.item_id && !branchIds(props.items, dragging.value).includes(row.item.item_id)) {
    const parentId = dropPosition.value === 'inside' ? row.item.item_id : row.parent!;
    const ordered = topicChildren(props.items.find(item => item.item_id === parentId)).filter(id => id !== dragging.value);
    emit('drop', { itemId: dragging.value, parentId, index: dropPosition.value === 'inside' ? ordered.length : ordered.indexOf(row.item.item_id) + (dropPosition.value === 'after' ? 1 : 0) });
  }
  dragging.value = null; dropId.value = null;
}
</script>
<template>
  <section ref="root" class="mind-map-outline" aria-label="思维导图大纲">
    <header><ListTree :size="17" /><strong>主题大纲</strong><span>{{ items.filter(item => item.target.kind === 'topic').length }} 个主题</span></header>
    <label class="mind-map-outline-search"><Search :size="15" /><input v-model="query" type="search" placeholder="查找主题…" aria-label="查找大纲主题" /></label>
    <div class="outline-tree" role="tree" aria-label="主题层级">
      <div v-for="row in rows" :key="row.item.item_id" class="outline-row" :class="{ selected: selectedIds.includes(row.item.item_id), [`drop-${dropPosition}`]: dropId === row.item.item_id }" :style="{ '--depth': row.depth }" :data-outline-item="row.item.item_id" role="treeitem" :aria-level="row.depth + 1" :aria-selected="selectedIds.includes(row.item.item_id)" :aria-expanded="topicChildren(row.item).length ? !foldedIds.includes(row.item.item_id) : undefined" :tabindex="selectedIds.includes(row.item.item_id) ? 0 : -1" :draggable="!disabled && !!row.parent && editingId !== row.item.item_id" @dragstart="dragging = row.item.item_id" @dragend="dragging = null; dropId = null" @dragover="dragOver($event, row)" @dragleave="dropId = null" @drop="drop($event, row)" @click="emit('select', row.item.item_id, $event.shiftKey)" @dblclick="!disabled && emit('edit', row.item.item_id)" @keydown="emit('keydown', $event, row.item.item_id)">
        <button v-if="topicChildren(row.item).length" class="outline-fold" type="button" :aria-label="`${foldedIds.includes(row.item.item_id) ? '展开' : '折叠'} ${row.item.title}`" @click.stop="emit('fold', row.item.item_id)" @keydown.enter.stop @keydown.space.stop><ChevronRight v-if="foldedIds.includes(row.item.item_id)" :size="15" /><ChevronDown v-else :size="15" /></button><span v-else class="outline-leaf"></span>
        <textarea v-if="editingId === row.item.item_id" :value="draftTitle" aria-label="编辑主题标题" rows="2" @input="emit('draft', ($event.target as HTMLTextAreaElement).value)" @keydown="inputKey" @click.stop @pointerdown.stop></textarea>
        <button v-else class="outline-title" type="button" tabindex="-1" @dblclick.stop="emit('edit', row.item.item_id)">{{ row.item.title }}</button>
        <small v-if="row.hidden">+{{ row.hidden }}</small><button v-if="query" class="outline-locate" type="button" @click.stop="emit('locate', row.item.item_id)" @keydown.enter.stop @keydown.space.stop>定位</button>
      </div>
      <p v-if="!rows.length" class="outline-empty">没有找到匹配的主题。</p>
    </div>
    <footer>Tab 添加子主题 · Enter 添加同级 · 拖到主题上移动分支</footer>
  </section>
</template>
<style scoped>
.mind-map-outline{display:flex;flex-direction:column;min-width:0;min-height:0;height:100%;background:var(--reader-canvas);color:var(--ink)}header{display:flex;align-items:center;gap:9px;padding:19px 22px 12px;color:var(--brand-accent)}header strong{font-size:14px;font-weight:550}header span{margin-left:auto;color:var(--stone);font-size:11px}.mind-map-outline-search{display:flex;flex:0 0 auto;min-width:0;min-height:44px;align-items:center;gap:8px;margin:0 20px 14px;padding:9px 11px;border:1px solid var(--hairline);border-radius:8px;background:var(--reader-card);color:var(--stone)}.mind-map-outline-search svg{flex-shrink:0}.mind-map-outline-search input{flex:1;min-width:0;min-height:0;width:0;padding:0;border:0;outline:0;background:transparent;color:var(--ink);font:inherit;font-size:12px}.outline-tree{flex:1;min-height:0;overflow:auto;padding:2px 16px 24px;overscroll-behavior:contain}.outline-row{display:flex;align-items:center;min-height:43px;padding:4px 10px 4px calc(7px + var(--depth) * 24px);border:1px solid transparent;border-radius:7px;gap:6px;cursor:pointer}.outline-row:hover{background:var(--surface-soft)}.outline-row.selected{background:color-mix(in srgb,var(--brand-accent) 8%,var(--reader-card));border-color:color-mix(in srgb,var(--brand-accent) 32%,transparent)}.outline-row.drop-target{border:1px dashed var(--brand-accent)}button{font:inherit;color:inherit;cursor:pointer}.outline-fold{display:grid;place-items:center;width:25px;height:28px;border:0;background:transparent;color:var(--steel);flex-shrink:0}.outline-leaf{width:25px;flex-shrink:0}.outline-title{flex:1;min-width:0;padding:6px 0;border:0;background:transparent;text-align:left;font-size:13px;line-height:1.6;white-space:pre-wrap;overflow-wrap:anywhere}.outline-row small{color:var(--stone);font-size:10px}.outline-locate{border:0;background:transparent;color:var(--brand-accent);font-size:11px}.outline-row textarea{flex:1;min-width:0;border:1px solid var(--brand-accent);border-radius:5px;padding:7px;background:var(--reader-card);color:var(--ink);font:inherit;font-size:14px;line-height:1.6;resize:vertical}.outline-empty{font-size:13px;color:var(--stone);padding:20px}footer{border-top:1px solid var(--hairline);padding:12px 20px;color:var(--stone);font-size:10px;line-height:1.7}button:focus-visible,.outline-row:focus-visible{outline:2px solid var(--brand-accent);outline-offset:2px}@media(max-width:700px){header{padding:14px 14px 10px}.mind-map-outline-search{margin:0 12px 10px}.outline-tree{padding:0 8px 20px}.outline-row{padding-left:calc(3px + var(--depth) * 18px);min-height:46px}footer{display:none}}
</style>
<style scoped>
.outline-row.drop-inside{border:1px dashed var(--brand-accent)}.outline-row.drop-before{border-top:2px solid var(--brand-accent);border-top-left-radius:0;border-top-right-radius:0}.outline-row.drop-after{border-bottom:2px solid var(--brand-accent);border-bottom-left-radius:0;border-bottom-right-radius:0}
</style>
