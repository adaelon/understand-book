<script setup lang="ts">
import { t as uiText, errorMessage } from '@understand-book/web/reader/i18n';

import { computed, onBeforeUnmount, onMounted, ref, shallowRef } from 'vue';
import { localMaterials, materialSize, syncLocalMaterial, CloudSyncUnavailable, type LocalMaterial, type MaterialScope } from '../local-materials';
import type { PreparationUpload } from '../personal-preparations';
import LocalMaterialReader from './LocalMaterialReader.vue';
const props = defineProps<{ scope?: MaterialScope; workspaceName?: string; archived?: boolean }>();
const emit = defineEmits<{ synced: []; allowance: [] }>();
const items = ref<LocalMaterial[]>([]), selected = shallowRef<LocalMaterial | null>(null), busy = ref(false), loading = ref(true);
const error = ref(''), notice = ref(''), needsPlan = ref(false), progress = ref<number | null>(null), syncing = ref(''), dragging = ref(false);
const used = computed(() => items.value.reduce((total, item) => total + item.bytes, 0));
let alive = true, session: PreparationUpload | null = null, unlisten: (() => void) | undefined;
async function refresh() { const next = await localMaterials.list(props.scope ?? null); if (alive) items.value = next; }
function failure(reason: unknown) { if (alive) error.value = errorMessage(reason instanceof Error ? reason.message : String(reason)); }
async function importPaths(paths?: string[]) {
  if (busy.value || props.archived) return;
  busy.value = true; error.value = ''; notice.value = ''; needsPlan.value = false;
  try {
    const scope = props.scope ?? { account_id: '', workspace_id: '' };
    let imported: LocalMaterial | null = null;
    if (paths) {
      for (const path of paths) imported = await localMaterials.import(path, scope, props.workspaceName ?? uiText('本机书库'));
    } else {
      const item = await localMaterials.choose(scope, props.workspaceName ?? uiText('本机书库'));
      if (!item) return;
      imported = item;
    }
    await refresh(); if (alive) notice.value = uiText('已保存在这台电脑，不占用云端空间。');
    if (alive && imported?.filename.toLowerCase().endsWith('.pdf')) selected.value = imported;
  } catch (reason) { failure(reason); }
  finally { busy.value = false; }
}
async function sync(item: LocalMaterial) {
  if (!props.scope || busy.value) return;
  busy.value = true; error.value = ''; notice.value = ''; needsPlan.value = false; syncing.value = item.material_id;
  try {
    await syncLocalMaterial(item, props.scope, bytes => { if (alive) progress.value = Math.round(bytes / item.bytes * 100); }, value => { session = value; if (!alive) value.cancelled = true; });
    if (alive) { notice.value = uiText('云端副本已保存。可在云端准备区查看后续处理；本机副本继续保留。'); emit('synced'); await refresh(); }
  } catch (reason) { failure(reason); if (alive) needsPlan.value = reason instanceof CloudSyncUnavailable; }
  finally { busy.value = false; progress.value = null; syncing.value = ''; session = null; }
}
async function remove(item: LocalMaterial) {
  if (busy.value || !window.confirm(uiText('删除“{0}”的本机副本？原文件和已有云端副本不会删除。', [item.filename]))) return;
  busy.value = true; error.value = '';
  try { await localMaterials.remove(item.material_id); await refresh(); } catch (reason) { failure(reason); } finally { busy.value = false; }
}
onMounted(async () => {
  try {
    await refresh();
    const { getCurrentWebviewWindow } = await import('@tauri-apps/api/webviewWindow');
    const stop = await getCurrentWebviewWindow().onDragDropEvent(event => {
      if (!alive) return;
      dragging.value = event.payload.type === 'over' || event.payload.type === 'enter';
      if (event.payload.type === 'drop') { dragging.value = false; void importPaths(event.payload.paths); }
    });
    if (alive) unlisten = stop; else stop();
  } catch (reason) { failure(reason); }
  finally { loading.value = false; }
});
onBeforeUnmount(() => { alive = false; unlisten?.(); if (session) session.cancelled = true; });
</script>
<template>
  <section class="local-materials" :aria-label="uiText('本机资料')">
    <header><div><h3>{{ uiText('加入自己的资料') }}</h3><p>{{ uiText('默认保存在这台电脑。需要在其他设备使用时，再选择同步到云端。') }}</p></div><span class="local-badge">{{ uiText('仅本机') }}</span></header>
    <div class="local-space"><strong>{{ uiText('本机资料') }}</strong><span>{{ items.length }} {{ uiText('份 ·') }} {{ materialSize(used) }}</span><p>{{ uiText('不占云端空间，不需要云端存储套餐。') }}</p></div>
    <div v-if="!archived" class="local-import" :class="{ dragging }">
      <div><strong>{{ dragging ? uiText('松开，保存到本机') : uiText('选择资料，或拖入窗口') }}</strong><p>EPUB · Markdown · TXT · PDF · MP4 · PPTX · PPT</p></div>
      <button :disabled="busy" @click="importPaths()">{{ busy && !syncing ? uiText('正在保存…') : uiText('导入到本机') }}</button>
    </div>
    <p v-if="loading" role="status">{{ uiText('正在读取本机书库…') }}</p>
    <p v-if="notice" role="status">{{ notice }}</p>
    <p v-if="error" role="alert">{{ error }} <button v-if="needsPlan" @click="emit('allowance')">{{ uiText('查看云端套餐') }}</button></p>
    <article v-for="item in items" :key="item.material_id" class="local-item">
      <div><strong>{{ item.filename }}</strong><small>{{ materialSize(item.bytes) }} {{ uiText('· 已保存在本机') }}<template v-if="!scope"> · {{ item.workspace_name }}</template></small></div>
      <div class="actions"><button @click="selected = item">{{ uiText('打开资料') }}</button><button v-if="scope && !archived" :disabled="busy" @click="sync(item)">{{ syncing === item.material_id ? (progress === null ? uiText('正在检查云端空间…') : uiText('正在同步 {0}%', [progress])) : uiText('同步到云端') }}</button><button :disabled="busy" @click="remove(item)">{{ uiText('删除本机副本') }}</button></div>
    </article>
    <p v-if="!loading && !items.length">{{ uiText('还没有本机资料。导入后可直接打开原文。') }}</p>
    <p class="local-note">{{ uiText('本机原文可直接阅读。云端 AI 学习和深度资料准备使用单独的服务额度。') }}</p>
    <LocalMaterialReader v-if="selected" :key="selected.material_id" :material="selected" @close="selected = null" />
  </section>
</template>
<style scoped>
.local-materials{margin:1.2rem 0;padding:1.3rem;border:1px solid var(--line,#dfd8ce);border-radius:16px;background:var(--paper,#fffdf8);color:var(--ink,#302d27)}header{display:flex;justify-content:space-between;gap:1rem}h3{margin:0;font-size:1.1rem}p{line-height:1.7;color:var(--ink-muted,#786c5f);font-size:.9rem}.local-badge{align-self:flex-start;white-space:nowrap;border:1px solid #d6e2d4;border-radius:20px;padding:.25rem .7rem;color:#44683e;font-size:.8rem}.local-space{border:1px solid var(--line,#dfd8ce);padding:1rem;border-radius:12px;margin:.5rem 0 1rem;display:flex;flex-wrap:wrap;gap:.6rem;justify-content:space-between}.local-space p{width:100%;margin:0}.local-import{display:flex;align-items:center;justify-content:space-between;gap:1rem;padding:1.3rem;background:#f4e7d7;border:1px dashed #b88051;border-radius:12px}.local-import p{margin:.4rem 0 0}.dragging{background:#e5eedb}.local-item{display:flex;align-items:center;justify-content:space-between;gap:1rem;padding:1rem 0;border-bottom:1px solid var(--line,#dfd8ce)}.local-item strong{overflow-wrap:anywhere}.local-item small{display:block;color:var(--ink-muted,#786c5f);margin-top:.4rem}.actions{display:flex;gap:.5rem;flex-wrap:wrap}button{font:inherit;padding:.55rem .8rem;border:1px solid var(--line,#d8cbbd);background:var(--paper,#fffdf8);color:inherit;border-radius:8px;cursor:pointer}button:disabled{opacity:.55;cursor:default}.local-import button{color:#fff;background:#955021;white-space:nowrap}.local-note{font-size:.8rem}p[role=alert]{color:#a33825}@media(max-width:700px){.local-item,.local-import{align-items:flex-start;flex-direction:column}.local-materials{padding:1rem}}
</style>
