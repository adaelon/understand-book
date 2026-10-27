<script setup lang="ts">
import { t as uiText } from '../i18n';

import { onBeforeUnmount, ref } from 'vue';
import { Copy, ImagePlus, X } from '@lucide/vue';
import type { VideoQuestionImage } from '../../../core/src/generated/VideoQuestionImage';
import { copyQuestionImage, questionImageFromFile, questionImageUrl } from '../question-image';
const props = defineProps<{ image?: VideoQuestionImage | null; disabled?: boolean }>();
const emit = defineEmits<{ (e: 'change', image: VideoQuestionImage | null): void; (e: 'loading', active: boolean): void }>();
const picker = ref<HTMLInputElement>(), busy = ref(false), error = ref(''), copied = ref(false);
const preview = ref<HTMLDialogElement>();
let alive = true;
onBeforeUnmount(() => { alive = false; });
async function acceptFiles(files: File[]) {
  if (props.disabled || busy.value) return;
  if (files.length !== 1) { error.value = uiText('每次请添加一张图片'); return; }
  error.value = ''; copied.value = false; busy.value = true; emit('loading', true);
  try { const image = await questionImageFromFile(files[0]); if (alive) emit('change', image); }
  catch (failure) { if (alive) error.value = (failure as Error).message; }
  finally { busy.value = false; if (alive) emit('loading', false); }
}
function paste(event: ClipboardEvent) {
  const files = Array.from(event.clipboardData?.items ?? []).filter(i => i.kind === 'file' && i.type.startsWith('image/'))
    .flatMap(i => { const file = i.getAsFile(); return file ? [file] : []; });
  if (!files.length) return;
  event.preventDefault(); void acceptFiles(files);
}
async function copy() {
  if (!props.image) return;
  error.value = ''; copied.value = false;
  try { await copyQuestionImage(props.image); copied.value = true; }
  catch (failure) { error.value = (failure as Error).message; }
}
function chooseFile() { if (!props.disabled && !busy.value) picker.value?.click(); }
function remove() { copied.value = false; error.value = ''; emit('change', null); }
defineExpose({ paste, chooseFile });
</script>
<template>
  <div v-show="image || busy || error" class="question-image-input" :aria-label="uiText('问题图片附件')">
    <div v-if="image" class="attached-image">
      <button type="button" class="image-thumbnail" :aria-label="uiText('放大问题图片')" :title="uiText('点击放大')" @click="preview?.showModal()"><img :src="questionImageUrl(image)" :alt="uiText('待发送的问题图片')" /></button>
      <button type="button" class="remove-image" :aria-label="uiText('移除图片')" :title="uiText('移除图片')" :disabled="disabled || busy" @click="remove"><X :size="12" aria-hidden="true" /></button>
    </div>
    <span v-if="busy" class="image-loading" role="status">{{ uiText('正在添加图片…') }}</span>
    <input ref="picker" type="file" accept="image/png,image/jpeg,image/webp" :aria-label="uiText('选择问题图片')" hidden @change="acceptFiles(Array.from(($event.target as HTMLInputElement).files ?? [])); ($event.target as HTMLInputElement).value = ''" />
    <p v-if="error" role="alert">{{ error }}</p>
    <dialog v-if="image" ref="preview" class="question-image-preview" :aria-label="uiText('问题图片预览')" @click.self="preview?.close()">
      <div class="preview-content">
        <div class="preview-toolbar"><span>{{ image.origin === 'selection' ? uiText('框选截图') : uiText('问题图片') }}</span><button type="button" :disabled="busy" @click="copy"><Copy :size="15" aria-hidden="true" />{{ copied ? uiText('已复制') : uiText('复制图片') }}</button><button type="button" :disabled="disabled || busy" @click="chooseFile"><ImagePlus :size="15" aria-hidden="true" />{{ uiText('替换图片') }}</button><button type="button" class="close-preview" :aria-label="uiText('关闭图片预览')" @click="preview?.close()"><X :size="18" aria-hidden="true" /></button></div>
        <img class="expanded-image" :src="questionImageUrl(image)" :alt="uiText('问题图片放大预览')" />
        <p v-if="error" role="alert">{{ error }}</p>
      </div>
    </dialog>
  </div>
</template>
<style scoped>
.question-image-input { display: flex; align-items: center; flex-wrap: wrap; gap: 8px; padding: 4px 4px 0; font-size: 12px; }
.attached-image { position: relative; flex: 0 0 auto; width: 72px; height: 56px; }
button { display: inline-flex; align-items: center; justify-content: center; gap: 6px; font: inherit; color: var(--ink); border: 0; border-radius: 8px; background: transparent; padding: 6px 8px; cursor: pointer; }
button:hover:not(:disabled) { background: var(--surface-soft); }
button:focus-visible { outline: 2px solid var(--brand-accent); outline-offset: 2px; }
.image-thumbnail { width: 100%; height: 100%; padding: 0; overflow: hidden; border: 1px solid var(--line); background: var(--canvas); cursor: zoom-in; }
.image-thumbnail img { width: 100%; height: 100%; object-fit: contain; }
.remove-image { position: absolute; top: -5px; right: -5px; width: 22px; height: 22px; min-height: 22px; padding: 0; border: 1px solid var(--line); border-radius: 50%; background: var(--canvas); box-shadow: 0 1px 3px #0001; }
.image-loading { color: var(--steel); }
.question-image-preview { max-width: min(900px, calc(100vw - 32px)); max-height: calc(100dvh - 48px); box-sizing: border-box; padding: 14px; border: 1px solid var(--line); border-radius: 16px; background: var(--canvas); color: var(--ink); }
.question-image-preview::backdrop { background: #17130e99; }
.preview-content { display: flex; flex-direction: column; gap: 12px; }
.preview-toolbar { display: flex; flex-wrap: wrap; align-items: center; gap: 4px; }
.preview-toolbar > span { flex: 1; margin-right: 12px; font-size: 13px; color: var(--steel); }
.close-preview { margin-left: auto; }
.expanded-image { display: block; max-width: 100%; max-height: calc(100dvh - 160px); object-fit: contain; }
button:disabled { opacity: .5; cursor: default; }
[role=alert] { flex-basis: 100%; color: #a13c29; margin: 4px 0 0; }
</style>
