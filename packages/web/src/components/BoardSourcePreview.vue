<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, useId } from 'vue';
import { BookOpen, X } from '@lucide/vue';
import { t as uiText } from '../i18n';
import type { SourcePopupView } from '../api';
import MediaSourcePreview from './MediaSourcePreview.vue';
import MusicSourcePreview from './MusicSourcePreview.vue';
import SourceExcerpt from './SourceExcerpt.vue';

const props = defineProps<{ source: SourcePopupView }>();
const emit = defineEmits<{ (event: 'close'): void }>();
const dialog = ref<HTMLElement>(), closeButton = ref<HTMLButtonElement>();
const titleId = useId();
let origin: HTMLElement | null = null;
const notice = computed(() => {
  // The existing media previews already render their own availability reason.
  if ((props.source.media && props.source.unavailable_reason) || (props.source.music && props.source.stale)) return '';
  return props.source.unavailable_reason || (props.source.stale ? uiText('原资料暂不可打开，保留引用时的内容。') : '');
});
function stopMedia() { dialog.value?.querySelectorAll('video, audio').forEach(player => (player as HTMLMediaElement).pause()); }
function close() { stopMedia(); emit('close'); }
function keydown(event: KeyboardEvent) {
  if (event.key === 'Escape') { event.preventDefault(); event.stopPropagation(); close(); return; }
  if (event.key !== 'Tab' || !dialog.value) return;
  const controls = [...dialog.value.querySelectorAll<HTMLElement>('button:not(:disabled),a[href],input:not(:disabled),select:not(:disabled),textarea:not(:disabled),audio[controls],video[controls],summary,[tabindex="0"]')]
    .filter(node => !node.closest('[hidden],[inert]') && getComputedStyle(node).display !== 'none' && getComputedStyle(node).visibility !== 'hidden');
  const first = controls[0], last = controls.at(-1);
  if (!first || !last) { event.preventDefault(); dialog.value.focus(); return; }
  if (event.shiftKey && (document.activeElement === first || !dialog.value.contains(document.activeElement))) { event.preventDefault(); last.focus(); }
  else if (!event.shiftKey && (document.activeElement === last || !dialog.value.contains(document.activeElement))) { event.preventDefault(); first.focus(); }
}
onMounted(async () => { origin = document.activeElement instanceof HTMLElement ? document.activeElement : null; await nextTick(); closeButton.value?.focus({ preventScroll: true }); });
onBeforeUnmount(() => { stopMedia(); if (origin?.isConnected) origin.focus({ preventScroll: true }); });
</script>

<template>
  <Teleport to="body">
    <div class="board-source-backdrop" @click.self="close" @keydown="keydown">
      <section ref="dialog" class="board-source-dialog" role="dialog" aria-modal="true" :aria-label="uiText('画板原文')" tabindex="-1">
        <header class="board-source-heading"><div><span class="board-source-kicker"><BookOpen :size="14" aria-hidden="true" />{{ uiText('画板来源') }}<span v-if="source.stale" class="board-source-snapshot">{{ uiText('引用时的快照') }}</span></span><h2 :id="titleId">{{ source.label || uiText('原文来源') }}</h2><p v-if="source.material_title && source.material_title !== source.label">{{ source.material_title }}</p></div><button ref="closeButton" type="button" class="board-source-close" :aria-label="uiText('关闭来源预览，返回画板')" @click="close"><X :size="19" aria-hidden="true" /></button></header>
        <div class="board-source-content">
          <p v-if="notice" class="board-source-notice" role="status">{{ notice }}</p>
          <MusicSourcePreview v-if="source.music" :key="source.source_ref_id" :source="source" />
          <MediaSourcePreview v-else-if="source.media" :key="source.source_ref_id" :source="source" />
          <SourceExcerpt v-else :source="source" />
        </div>
        <footer><button type="button" @click="close">{{ uiText('返回画板') }}</button></footer>
      </section>
    </div>
  </Teleport>
</template>

<style scoped>
.board-source-backdrop{position:fixed;z-index:220;inset:0;display:flex;align-items:center;justify-content:center;padding:28px;background:#25232050;backdrop-filter:blur(3px)}.board-source-dialog{display:flex;flex-direction:column;width:min(780px,100%);max-height:calc(100dvh - 56px);min-height:0;border:1px solid var(--hairline);border-radius:16px;background:var(--reader-card);color:var(--ink);box-shadow:0 18px 70px #25232030;font-family:var(--sans);outline:none}.board-source-heading{display:flex;align-items:flex-start;justify-content:space-between;gap:20px;flex-shrink:0;padding:24px 27px 20px;border-bottom:1px solid var(--hairline)}.board-source-heading>div{min-width:0}.board-source-kicker{display:flex;align-items:center;gap:6px;color:var(--steel);font-size:11px}.board-source-snapshot{margin-left:6px;padding:2px 7px;border-radius:5px;background:var(--surface-soft);color:var(--stone);font-size:10px}.board-source-heading h2{margin:12px 0 0;font-size:21px;font-weight:550;line-height:1.6;overflow-wrap:anywhere}.board-source-heading p{margin:7px 0 0;color:var(--stone);font-size:12px;line-height:1.7;overflow-wrap:anywhere}.board-source-close{display:grid;place-items:center;flex-shrink:0;width:34px;height:34px;min-height:0;padding:0;border:0;border-radius:7px;background:transparent;color:var(--steel);cursor:pointer}.board-source-close:hover{background:var(--surface-soft)}.board-source-content{min-height:0;overflow:auto;overscroll-behavior:contain;padding:23px 27px;font-size:14px;line-height:1.8}.board-source-notice{margin:0 0 18px;padding:11px 13px;border-radius:7px;background:var(--surface-soft);color:var(--steel);font-size:12px;line-height:1.8}.board-source-dialog>footer{display:flex;justify-content:flex-end;flex-shrink:0;padding:15px 27px;border-top:1px solid var(--hairline)}.board-source-dialog>footer button{padding:9px 15px;border:1px solid var(--hairline);border-radius:7px;background:var(--surface-soft);color:var(--slate);font-size:12px;cursor:pointer}.board-source-dialog button:focus-visible{outline:2px solid var(--brand-accent);outline-offset:3px}@media(max-width:600px){.board-source-backdrop{padding:14px}.board-source-dialog{max-height:calc(100dvh - 28px);border-radius:13px}.board-source-heading{padding:20px;gap:12px}.board-source-heading h2{font-size:18px}.board-source-content{padding:20px}.board-source-dialog>footer{padding:13px 20px}}
</style>
