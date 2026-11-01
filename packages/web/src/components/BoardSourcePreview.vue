<script setup lang="ts">
import { applicationOverlayTarget } from "../application-overlay";
import { computed, nextTick, onBeforeUnmount, onMounted, ref, useId, watch } from 'vue';
import { ArrowUpRight, BookOpen, LoaderCircle, X } from '@lucide/vue';
import { t as uiText } from '../i18n';
import type { SourcePopupView } from '../api';
import type { BoardSourceRequest } from '../board-workspace';
import MediaSourcePreview from './MediaSourcePreview.vue';
import MusicSourcePreview from './MusicSourcePreview.vue';
import SourceExcerpt from './SourceExcerpt.vue';
import BoardSourceReader from './BoardSourceReader.vue';

const props = withDefaults(defineProps<{ source: SourcePopupView | null; sourceRequest?: BoardSourceRequest; loading?: boolean; opening?: boolean; error?: string; canOpenInReader?: boolean; context?: 'board' | 'note'; canOpen?: boolean }>(), { canOpenInReader: true, context: 'board' });
const emit = defineEmits<{ (event: 'close'): void; (event: 'open-reader', lid?: string): void; (event: 'retry'): void; (event: 'open'): void }>();
const dialog = ref<HTMLElement>(), closeButton = ref<HTMLButtonElement>();
const readingLid = ref<string>();
const titleId = useId();
let origin: HTMLElement | null = null;
const continuous = computed(() => !!props.sourceRequest && !!props.source && !props.source.stale && !props.source.media && !props.source.music);
watch(() => [props.sourceRequest?.board_id, props.sourceRequest?.item_id, props.sourceRequest?.source_ref_id], () => { readingLid.value = undefined; });
const readerCanOpen = computed(() => !!props.source && !props.source.stale && props.source.can_open_in_reader && props.canOpenInReader !== false);
const notice = computed(() => {
  if (!props.source) return '';
  // The existing media previews already render their own availability reason.
  if ((props.source.media && props.source.unavailable_reason) || (props.source.music && props.source.stale)) return '';
  return props.source.unavailable_reason || (props.source.stale ? uiText('原资料暂不可打开，保留引用时的内容。') : '');
});
function stopMedia() { dialog.value?.querySelectorAll('video, audio').forEach(player => (player as HTMLMediaElement).pause()); }
function close() { stopMedia(); emit('close'); }
function keydown(event: KeyboardEvent) {
  if (event.target instanceof Element && event.target.closest('[data-personal-notes]')) return;
  if (event.key === 'Escape') { event.preventDefault(); event.stopPropagation(); close(); return; }
  if (event.key !== 'Tab' || !dialog.value) return;
  const controls = [...dialog.value.querySelectorAll<HTMLElement>('button:not(:disabled),a[href],input:not(:disabled),select:not(:disabled),textarea:not(:disabled),audio[controls],video[controls],summary,[tabindex="0"]')]
    .filter(node => !node.closest('[hidden],[inert]') && getComputedStyle(node).display !== 'none' && getComputedStyle(node).visibility !== 'hidden');
  const first = controls[0], last = controls.at(-1);
  if (!first || !last) { event.preventDefault(); dialog.value.focus(); return; }
  if (event.shiftKey && (document.activeElement === first || !dialog.value.contains(document.activeElement))) { event.preventDefault(); last.focus(); }
  else if (!event.shiftKey && (document.activeElement === last || !dialog.value.contains(document.activeElement))) { event.preventDefault(); first.focus(); }
}
onMounted(async () => {
  origin = document.activeElement instanceof HTMLElement ? document.activeElement : null;
  // Loading can temporarily disable the focused control and return focus to body.
  // Keep modal keyboard controls available throughout a chapter change.
  document.addEventListener('keydown', keydown, true);
  await nextTick(); closeButton.value?.focus({ preventScroll: true });
});
onBeforeUnmount(() => { document.removeEventListener('keydown', keydown, true); stopMedia(); if (origin?.isConnected) origin.focus({ preventScroll: true }); });
</script>

<template>
  <Teleport :to="applicationOverlayTarget()">
    <div class="board-source-backdrop" @click.self="close">
      <section ref="dialog" class="board-source-dialog" :class="{ 'board-source-dialog-reader': continuous }" role="dialog" aria-modal="true" :aria-label="context === 'note' ? uiText('笔记原文') : uiText('画板原文')" tabindex="-1">
        <header class="board-source-heading"><div><span class="board-source-kicker"><BookOpen :size="14" aria-hidden="true" />{{ continuous ? uiText('临时阅读') : context === 'note' ? uiText('笔记来源') : uiText('画板来源') }}<span v-if="source?.stale" class="board-source-snapshot">{{ uiText('引用时的快照') }}</span></span><h2 :id="titleId">{{ (continuous ? source?.material_title || source?.label : source?.label) || uiText('原文来源') }}</h2><p v-if="!continuous && source?.material_title && source.material_title !== source.label">{{ source.material_title }}</p><p v-if="!continuous && source?.heading_path?.length" class="board-source-path">{{ source.heading_path.join(' › ') }}</p></div><button ref="closeButton" type="button" class="board-source-close" :aria-label="context === 'note' ? uiText('关闭来源预览，返回笔记') : uiText('关闭来源预览，返回画板')" @click="close"><X :size="19" aria-hidden="true" /></button></header>
        <div class="board-source-content" :class="{ 'board-source-content-reader': continuous }" :aria-busy="loading || opening">
          <p v-if="loading" class="board-source-loading" role="status"><LoaderCircle :size="18" aria-hidden="true" />{{ uiText('正在加载原文…') }}</p>
          <div v-if="error" class="board-source-error" role="alert"><p>{{ error }}</p><button type="button" :disabled="loading || opening" @click="emit('retry')">{{ uiText('重试') }}</button></div>
          <p v-if="notice" class="board-source-notice" role="status">{{ notice }}</p>
          <template v-if="source">
            <MusicSourcePreview v-if="source.music" :key="source.source_ref_id" :source="source" />
            <MediaSourcePreview v-else-if="source.media" :key="source.source_ref_id" :source="source" />
            <BoardSourceReader v-else-if="continuous && sourceRequest" :request="sourceRequest" :source="source" @position="readingLid = $event" />
            <SourceExcerpt v-else :source="source" />
          </template>
        </div>
        <footer><p v-if="continuous" class="board-source-return-note">{{ uiText('关闭后继续画板中的讨论') }}</p><button type="button" @click="close">{{ context === 'note' ? uiText('返回笔记') : uiText('返回画板') }}</button><button v-if="context === 'note' && canOpen" type="button" class="board-source-open" @click="stopMedia(); emit('open')">{{ uiText('打开原资料') }}</button><button v-if="context !== 'note' && readerCanOpen" type="button" class="board-source-open-reader" :disabled="opening" @click="stopMedia(); emit('open-reader', readingLid)">{{ opening ? uiText('正在打开…') : uiText('在阅读器中打开') }}<ArrowUpRight :size="14" aria-hidden="true" /></button></footer>
      </section>
    </div>
  </Teleport>
</template>

<style scoped>
.board-source-dialog>footer{gap:10px}.board-source-dialog>footer .board-source-open{background:var(--brand-accent);border-color:var(--brand-accent);color:white}
.board-source-loading{display:flex;align-items:center;justify-content:center;gap:10px;min-height:110px;color:var(--stone);font-size:13px}.board-source-loading svg{animation:board-source-spin 1s linear infinite}.board-source-error{display:flex;align-items:center;gap:14px;margin-bottom:18px;padding:12px 14px;border:1px solid color-mix(in srgb,var(--brand-error) 25%,var(--hairline));border-radius:8px;color:var(--brand-error);background:var(--surface-soft);font-size:12px}.board-source-error p{margin:0;flex:1}.board-source-error button{flex-shrink:0;padding:5px 10px;border:1px solid currentColor;border-radius:5px;color:inherit;background:transparent;cursor:pointer}.board-source-dialog>footer{gap:10px}.board-source-dialog>footer .board-source-open-reader{display:flex;align-items:center;gap:7px;background:var(--brand-accent);border-color:var(--brand-accent);color:#fff}.board-source-open-reader:disabled{opacity:.6;cursor:wait}@keyframes board-source-spin{to{transform:rotate(360deg)}}@media(prefers-reduced-motion:reduce){.board-source-loading svg{animation:none}}
.board-source-backdrop{position:fixed;z-index:220;inset:0;display:flex;align-items:center;justify-content:center;padding:28px;background:#25232050;backdrop-filter:blur(3px)}.board-source-dialog{display:flex;flex-direction:column;width:min(780px,100%);max-height:calc(var(--app-content-height, 100dvh) - 56px);min-height:0;border:1px solid var(--hairline);border-radius:16px;background:var(--reader-card);color:var(--ink);box-shadow:0 18px 70px #25232030;font-family:var(--sans);outline:none}.board-source-heading{display:flex;align-items:flex-start;justify-content:space-between;gap:20px;flex-shrink:0;padding:24px 27px 20px;border-bottom:1px solid var(--hairline)}.board-source-heading>div{min-width:0}.board-source-kicker{display:flex;align-items:center;gap:6px;color:var(--steel);font-size:11px}.board-source-snapshot{margin-left:6px;padding:2px 7px;border-radius:5px;background:var(--surface-soft);color:var(--stone);font-size:10px}.board-source-heading h2{margin:12px 0 0;font-size:21px;font-weight:550;line-height:1.6;overflow-wrap:anywhere}.board-source-heading p{margin:7px 0 0;color:var(--stone);font-size:12px;line-height:1.7;overflow-wrap:anywhere}.board-source-close{display:grid;place-items:center;flex-shrink:0;width:34px;height:34px;min-height:0;padding:0;border:0;border-radius:7px;background:transparent;color:var(--steel);cursor:pointer}.board-source-close:hover{background:var(--surface-soft)}.board-source-content{min-height:0;overflow:auto;overscroll-behavior:contain;padding:23px 27px;font-size:14px;line-height:1.8}.board-source-notice{margin:0 0 18px;padding:11px 13px;border-radius:7px;background:var(--surface-soft);color:var(--steel);font-size:12px;line-height:1.8}.board-source-dialog>footer{display:flex;justify-content:flex-end;flex-shrink:0;padding:15px 27px;border-top:1px solid var(--hairline)}.board-source-dialog>footer button{padding:9px 15px;border:1px solid var(--hairline);border-radius:7px;background:var(--surface-soft);color:var(--slate);font-size:12px;cursor:pointer}.board-source-dialog button:focus-visible{outline:2px solid var(--brand-accent);outline-offset:3px}@media(max-width:600px){.board-source-backdrop{padding:14px}.board-source-dialog{max-height:calc(var(--app-content-height, 100dvh) - 28px);border-radius:13px}.board-source-heading{padding:20px;gap:12px}.board-source-heading h2{font-size:18px}.board-source-content{padding:20px}.board-source-dialog>footer{padding:13px 20px}}
</style>

<style scoped>
.board-source-dialog-reader{width:min(1800px,100%);height:calc(var(--app-content-height, 100dvh) - 56px)}
.board-source-dialog-reader .board-source-heading{align-items:center;gap:16px;padding:17px 26px 15px}
.board-source-dialog-reader .board-source-heading h2{margin-top:5px;font-size:18px;line-height:1.45}
.board-source-content-reader{display:flex;flex:1;flex-direction:column;overflow:hidden;padding:0}
.board-source-content-reader>.board-source-error,.board-source-content-reader>.board-source-notice{flex-shrink:0;margin:12px 24px}
.board-source-return-note{flex:1;margin:0;align-self:center;color:var(--stone);font-size:11px;line-height:1.7}
.board-source-dialog-reader>footer{align-items:center;padding:12px 24px}
@media(max-width:850px){.board-source-dialog-reader{height:calc(var(--app-content-height, 100dvh) - 28px)}.board-source-backdrop:has(.board-source-dialog-reader){padding:14px}}
@media(max-width:600px){.board-source-backdrop:has(.board-source-dialog-reader){padding:0}.board-source-dialog-reader{width:100%;height:var(--app-content-height, 100dvh);max-height:var(--app-content-height, 100dvh);border-radius:0;border:0}.board-source-dialog-reader .board-source-heading{padding:14px 17px 12px}.board-source-dialog-reader .board-source-heading h2{font-size:16px}.board-source-return-note{display:none}.board-source-dialog-reader>footer{padding:11px 15px max(11px,env(safe-area-inset-bottom));gap:8px}.board-source-dialog-reader>footer button{padding:9px 11px;font-size:11px}.board-source-content-reader>.board-source-error,.board-source-content-reader>.board-source-notice{margin:10px 15px}}
</style>
