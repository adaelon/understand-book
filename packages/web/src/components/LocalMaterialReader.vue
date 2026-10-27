<script setup lang="ts">
import { t as uiText } from '../i18n';

import { computed, defineAsyncComponent, onBeforeUnmount, onMounted, ref } from 'vue';
import { localMaterials, localMaterialUrl, type LocalMaterial } from '../local-materials';
import { openLocalEpub, readerDocument, type EpubChapter } from '../local-epub';
import { renderMarkdown } from '../md';
const LocalPdfReader = defineAsyncComponent(() => import('./LocalPdfReader.vue'));
const props = defineProps<{ material: LocalMaterial }>();
defineEmits<{ close: [] }>();
const url = localMaterialUrl(props.material), format = props.material.filename.split('.').pop()?.toLowerCase();
const error = ref(''), loading = ref(false), text = ref(''), chapters = ref<EpubChapter[]>([]), chapter = ref(0);
const html = computed(() => format === 'epub' ? chapters.value[chapter.value]?.html : readerDocument(renderMarkdown(text.value)));
const controller = new AbortController(); let dispose: (() => void) | undefined;
async function original() { try { await localMaterials.openOriginal(props.material.material_id); } catch (reason) { error.value = String(reason); } }
onMounted(async () => {
  if (!['txt', 'md', 'epub'].includes(format ?? '')) return;
  loading.value = true;
  try {
    const response = await fetch(url, { signal: controller.signal });
    if (!response.ok) throw new Error(uiText('本机文件暂时无法读取。'));
    if (format === 'epub') { const epub = openLocalEpub(await response.arrayBuffer()); chapters.value = epub.chapters; dispose = epub.dispose; }
    else { const buffer = await response.arrayBuffer(); try { text.value = new TextDecoder('utf-8', { fatal: true }).decode(buffer); } catch { text.value = new TextDecoder('gb18030').decode(buffer); } }
  } catch (reason) { if (!controller.signal.aborted) error.value = reason instanceof Error ? reason.message : String(reason); }
  finally { loading.value = false; }
});
onBeforeUnmount(() => { controller.abort(); dispose?.(); });
</script>
<template>
  <section class="local-reader" role="dialog" aria-modal="true" :aria-label="uiText('本机阅读：{0}', [material.filename])">
    <header><div><strong>{{ material.filename }}</strong><span>{{ uiText('本机原文 · 无需云端空间') }}</span></div><div><button @click="original">{{ uiText('用系统应用打开') }}</button><button @click="$emit('close')">{{ uiText('返回资料') }}</button></div></header>
    <p v-if="loading" role="status">{{ uiText('正在打开本机资料…') }}</p><p v-if="error" role="alert">{{ error }}</p>
    <LocalPdfReader v-if="format === 'pdf'" :url="url" :material-id="material.material_id" />
    <video v-else-if="format === 'mp4'" :src="url" controls preload="metadata" @error="error = uiText('这个视频暂时无法在应用内播放，可使用系统应用打开原文件。')" />
    <template v-else-if="format === 'md' || format === 'epub'">
      <nav v-if="format === 'epub' && chapters.length"><button :disabled="chapter === 0" @click="chapter--">{{ uiText('上一节') }}</button><select v-model="chapter" :aria-label="uiText('EPUB 章节')"><option v-for="(entry, index) in chapters" :key="index" :value="index">{{ entry.title }}</option></select><button :disabled="chapter === chapters.length - 1" @click="chapter++">{{ uiText('下一节') }}</button></nav>
      <iframe v-if="!loading && !error" :srcdoc="html" sandbox="" :title="material.filename" />
    </template>
    <pre v-else-if="format === 'txt'">{{ text }}</pre>
    <div v-else class="office-original"><h2>{{ uiText('课件已保存在本机') }}</h2><p>{{ uiText('使用本机 PowerPoint 或其他演示应用打开原件。需要在这里阅读时，可将课件另存为 PDF 后导入。') }}</p><button @click="original">{{ uiText('打开课件原件') }}</button></div>
  </section>
</template>
<style scoped>
.local-reader{position:fixed;inset:0;z-index:1100;background:#fcfaf6;display:flex;flex-direction:column;color:#302d27}.local-reader>header{padding:.8rem 1.3rem;border-bottom:1px solid #dfd8ce;display:flex;justify-content:space-between;align-items:center;gap:1rem}.local-reader header span{display:block;font-size:.75rem;color:#786c5f;margin-top:.3rem}.local-reader button,.local-reader select{font:inherit;padding:.45rem .75rem;border:1px solid #dbd1c2;background:#fffdf8;border-radius:7px;margin:.2rem;cursor:pointer}.local-reader nav{display:flex;justify-content:center;padding:.4rem}.local-reader select{max-width:60%}.local-reader iframe{width:100%;flex:1;border:0;background:white}.local-reader pre{white-space:pre-wrap;overflow:auto;line-height:1.9;font-family:inherit;max-width:860px;margin:1.5rem auto;padding:0 2rem;flex:1}.local-reader video{min-height:0;flex:1;background:#141414}.local-pdf{flex:1;min-height:0}.local-reader>p{padding:0 1rem}.office-original{padding:3rem;max-width:680px;margin:auto;line-height:1.8}
</style>
