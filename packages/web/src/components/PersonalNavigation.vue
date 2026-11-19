<script setup lang="ts">
import { nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue';
import { X } from '@lucide/vue';
import { t } from '../i18n';
import { localRequest } from '../reader-host';

export interface ExternalPassage {
  source_ref: {book_id: string; source_revision: string; lid: string; range?: {start: number; end: number}};
  text: string;
  book_dir: string;
  title?: string;
}
interface ExternalNavigation {
  id: string;
  kind: 'goto' | 'compare';
  sources: ExternalPassage[];
}
const props = defineProps<{ ready: boolean; covered: boolean; navigate: (source: ExternalPassage) => Promise<boolean> }>();
const emit = defineEmits<{ changed: []; error: [message: string] }>();
const comparison = ref<ExternalNavigation | null>(null);
const dialog = ref<HTMLDialogElement | null>(null);
let disposed = false;
let handled: { id: string; displayed: boolean } | null = null;
let previousChange: string | number | undefined;
let timer: ReturnType<typeof setTimeout> | undefined;

async function poll() {
  try {
    if (disposed || !props.ready || document.visibilityState === 'hidden') return;
    const result = await localRequest<{pending: ExternalNavigation | null; change_id?: number | string}>('GET', '/personal/navigation');
    if (disposed) return;
    if (result.change_id !== previousChange && previousChange !== undefined) emit('changed');
    previousChange = result.change_id;
    const request = result.pending;
    if (!request) { handled = null; return; }
    if (handled?.id !== request.id) {
      let displayed = false;
      if (!props.covered && request.kind === 'compare' && request.sources.length === 2) {
        comparison.value = request;
        await nextTick();
        if (!props.covered && dialog.value && !dialog.value.open) dialog.value.showModal();
        displayed = dialog.value?.open === true && !props.covered;
      } else if (!props.covered && request.kind === 'goto' && request.sources.length === 1) {
        comparison.value = null;
        try { displayed = await props.navigate(request.sources[0]); }
        catch { emit('error', t('来源定位未完成，请重试。')); }
      }
      handled = { id: request.id, displayed };
    }
    if (!disposed) await localRequest('POST', '/personal/navigation', handled);
  } catch {
    // An interrupted connection is polled again. A rendered request only retries its receipt.
  } finally {
    if (!disposed) timer = setTimeout(poll, 1800);
  }
}
async function openSource(source: ExternalPassage) {
  if (!await props.navigate(source)) { emit('error', t('来源定位未完成，请重试。')); return; }
  comparison.value = null;
}
watch(() => props.covered, covered => { if (covered) comparison.value = null; });
onMounted(poll);
onBeforeUnmount(() => { disposed = true; clearTimeout(timer); });
</script>
<template>
  <dialog v-if="comparison && !covered" ref="dialog" class="passage-comparison" :aria-label="t('并排原文')" @close="comparison = null" @click.self="comparison = null">
    <section><header><h3>{{ t('并排原文') }}</h3><button :aria-label="t('关闭')" @click="comparison = null"><X :size="16"/></button></header>
      <div class="passages"><article v-for="(source,index) in comparison.sources" :key="index">
        <h4>{{ source.title || source.source_ref.book_id }}</h4><p>{{ source.text }}</p>
        <button @click="openSource(source)">{{ t('打开这段原文') }}</button>
      </article></div>
    </section>
  </dialog>
</template>
<style scoped>
.passage-comparison { border:1px solid var(--line); border-radius:14px; background:var(--paper,#fff); color:var(--ink); width:min(54rem,90vw); max-height:82vh; overflow:auto; padding:0; box-shadow:0 12px 48px #211d1926; }
.passage-comparison::backdrop { background:#24211e55; }
section { padding:18px; } header { display:flex; align-items:center; justify-content:space-between; margin-bottom:14px; }
h3,h4,p { margin:0; } h4 { font-size:.88rem; overflow-wrap:anywhere; color:var(--steel); }
.passages { display:grid; grid-template-columns:repeat(2,minmax(0,1fr)); gap:14px; }
article { border:1px solid var(--line); border-radius:8px; padding:14px; display:flex; flex-direction:column; align-items:flex-start; gap:12px; }
article p { line-height:1.75; white-space:pre-wrap; overflow-wrap:anywhere; flex:1; }
button { border:1px solid var(--line); background:transparent; color:inherit; padding:5px 9px; border-radius:6px; font:inherit; cursor:pointer; }
@media(max-width:620px){ .passages { grid-template-columns:1fr; } }
</style>
