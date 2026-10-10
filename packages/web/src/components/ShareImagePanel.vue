<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue';
import { renderShare, saveShare, type ShareDraft, type ShareSource } from '../reading-share';

const props = defineProps<{ source: ShareSource; loading?: boolean }>();
const emit = defineEmits<{ (event: 'close' | 'reselect'): void }>();
const selected = ref(props.source.parts.map(p => p.id));
const answerText = ref(props.source.parts[0]?.text ?? '');
const answerRange = ref<HTMLTextAreaElement | null>(null);
const choosingAnswer = ref(false);
const answerRangeError = ref('');
const selectedSource = computed<ShareSource>(() => props.source.answer ? { ...props.source,
  parts: [{ ...props.source.parts[0], text: answerText.value }],
  association: answerText.value === props.source.answer.fullText ? '回答文字全文 · 本回答来源' : '回答节选 · 本回答来源',
} : props.source);
const title = ref(props.source.diagram ? props.source.diagram.title : props.source.recap ? '本次阅读回顾' : '阅读随记'), reflection = ref('');
const layout = ref<ShareDraft['layout']>(props.source.diagram ? 'diagram' : props.source.recap ? 'recap' : props.source.parts.every(p => p.id === 'excerpt') ? 'excerpt' : 'understanding'), palette = ref<ShareDraft['palette']>('blue');
const orientation = ref<'portrait' | 'landscape'>(props.source.diagram ? 'landscape' : 'portrait');
const draft = computed<ShareDraft>(() => ({ source: selectedSource.value, selected: selected.value,
  title: title.value, reflection: reflection.value, layout: layout.value, palette: palette.value, orientation: orientation.value }));
const pages = ref<{ blob: Blob; url: string }[]>([]), currentPage = ref(0);
const url = computed(() => pages.value[currentPage.value]?.url ?? '');
const error = ref(''), status = ref(''), busy = ref(false), saving = ref(false);
const dialog = ref<HTMLDialogElement | null>(null), selection = ref<HTMLElement | null>(null);
let generation = 0;
function invalidate() {
  generation++; busy.value = false; error.value = ''; status.value = '';
  pages.value.forEach(page => URL.revokeObjectURL(page.url));
  pages.value = []; currentPage.value = 0;
}
watch(draft, invalidate, { deep: true, flush: 'sync' });
async function preview() {
  invalidate(); const request = generation;
  busy.value = true;
  try {
    const result = await renderShare(draft.value);
    if (request !== generation) return;
    pages.value = result.map(blob => ({ blob, url: URL.createObjectURL(blob) }));
  } catch (failure) { if (request === generation) error.value = failure instanceof Error ? failure.message : String(failure); }
  finally { if (request === generation) busy.value = false; }
}
async function download() {
  const page = pages.value[currentPage.value];
  if (!page || saving.value) return;
  const request = generation; saving.value = true; error.value = '';
  try {
    const saved = await saveShare(page.blob, page.url, () => request === generation, pages.value.length > 1 ? currentPage.value + 1 : undefined);
    if (request === generation && saved) status.value = '已交给保存入口。也可以在下方打开图片后保存。';
  } catch { if (request === generation) error.value = '保存失败，分享编辑与预览仍保留，请重试下载。'; }
  finally { saving.value = false; }
}
async function reselect() {
  invalidate();
  if (props.source.diagram) { emit('close'); return; }
  if (props.source.recap) { emit('reselect'); return; }
  if (props.source.answer) { choosingAnswer.value = true; await nextTick(); answerRange.value?.focus(); return; }
  await nextTick(); selection.value?.focus({ preventScroll: true });
}
function selectAnswerRange(all = false) {
  const full = props.source.answer?.fullText;
  if (!full) return;
  const field = answerRange.value;
  const text = all ? full : full.slice(field?.selectionStart ?? 0, field?.selectionEnd ?? 0);
  if (!text.trim()) { answerRangeError.value = '请先在回答中选择连续文字。'; return; }
  answerText.value = text; selected.value = ['body']; answerRangeError.value = ''; choosingAnswer.value = false;
}
onMounted(() => { dialog.value?.showModal(); });
onBeforeUnmount(() => { invalidate(); dialog.value?.close(); });
</script>

<template>
  <dialog ref="dialog" class="share-image-panel" aria-labelledby="share-heading" @keydown.esc.stop.prevent="emit('close')" @cancel.prevent="emit('close')">
    <header><div><h2 id="share-heading">生成分享图</h2><p>把这次阅读，留成一张卡片。</p></div><button class="share-close" aria-label="关闭分享" @click="emit('close')">×</button></header>
    <div class="share-workspace">
      <div class="share-controls">
        <fieldset ref="selection" tabindex="-1"><legend>选择内容</legend>
          <label v-for="part in selectedSource.parts" :key="part.id" class="share-choice">
            <span><input v-model="selected" type="checkbox" :value="part.id" :aria-label="part.label" :disabled="saving || !!source.diagram" />{{ part.label }}</span>
            <details class="share-original"><summary>查看内容</summary><pre>{{ part.text }}</pre></details>
          </label>
        </fieldset>
        <template v-if="source.answer">
          <p class="share-range-label">{{ selectedSource.association.split(' · ')[0] }}</p>
          <button :disabled="saving" class="share-reselect" @click="choosingAnswer = !choosingAnswer">调整回答范围</button>
          <div v-if="choosingAnswer" class="share-answer-range">
            <p>选择连续文字，保留说明中的适用条件。也可包含全部回答文字。</p>
            <textarea ref="answerRange" :value="source.answer.fullText" readonly rows="8" aria-label="原回答文字" />
            <button :disabled="saving" @click="selectAnswerRange()">使用选中文字</button>
            <button :disabled="saving" @click="selectAnswerRange(true)">包含全部回答文字</button>
            <p v-if="answerRangeError" role="alert">{{ answerRangeError }}</p>
          </div>
        </template>
        <button v-if="source.recap" :disabled="saving" class="share-reselect" @click="reselect">重新选材</button>
        <label>标题<input v-model="title" :disabled="saving" /></label>
        <label>我的感想<textarea v-model="reflection" rows="3" :disabled="saving" placeholder="写一句你的想法（可选）"></textarea></label>
        <div class="share-style-options"><label>排版<select v-model="layout" :disabled="saving"><option v-if="source.diagram" value="diagram">图解卡</option><option v-else-if="source.recap" value="recap">阅读回顾卡</option><template v-else><option value="understanding">理解卡</option><option v-if="!source.answer" value="excerpt">摘录卡</option></template></select></label>
        <label v-if="source.diagram">方向<select v-model="orientation" :disabled="saving"><option value="landscape">横版</option><option value="portrait">竖版</option></select></label>
        <label>配色<select v-model="palette" :disabled="saving"><option value="paper">暖纸珊瑚</option><option value="blue">奶油蓝</option></select></label></div>
        <p v-if="loading" role="status">{{ source.answer ? '正在读取原回答来源…' : '正在读取材料名称…' }}</p>
        <details v-else class="share-source-details"><summary>出处</summary><p v-if="selectedSource.association">{{ selectedSource.association }}</p><p v-for="sourceLabel in source.sources" :key="sourceLabel" class="share-source">{{ sourceLabel }}</p></details>
      </div>
      <div class="share-preview">
        <nav v-if="pages.length > 1" aria-label="分享图分页"><button :disabled="currentPage === 0 || saving" @click="currentPage--">上一页</button><span>{{ currentPage + 1 }} / {{ pages.length }}</span><button :disabled="currentPage === pages.length - 1 || saving" @click="currentPage++">下一页</button></nav>
        <img v-if="url" :src="url" alt="分享图预览，与下载的 PNG 相同" :width="orientation === 'landscape' ? 1440 : 1080" :height="orientation === 'landscape' ? 1080 : 1440" :class="{ landscape: orientation === 'landscape' }" />
        <div v-else class="share-empty"><span aria-hidden="true">“</span><p>{{ busy ? '正在排好这一页…' : '给阅读留一点余白' }}</p><small>{{ busy ? '长内容会自动分页' : '写下感想，预览你的分享图' }}</small></div>
        <template v-if="url"><a :href="url" target="_blank" rel="noopener">打开图片</a><p>手机浏览器若未显示下载，可打开图片后长按保存。</p></template>
      </div>
    </div>
    <footer class="share-bottom"><div class="share-feedback"><p v-if="error" role="alert">{{ error }}</p><button v-if="error" @click="reselect">{{ source.diagram ? "返回图解" : "返回选材" }}</button><p v-if="status" role="status">{{ status }}</p><p v-if="!error && !status" class="share-hint">{{ pages.length > 1 ? `共 ${pages.length} 页，逐页保存` : source.recap ? '分享所选记录，不改动阅读事实' : source.diagram ? '图解与现场说明一同保留' : source.answer ? '助手解释与我的感想分别呈现' : '只为这次分享，不改动原笔记' }}</p></div><div class="share-actions"><button :disabled="loading || busy || saving || !selected.length" @click="preview">{{ busy ? '正在生成…' : '预览图片' }}</button><button class="share-save" :disabled="!url || saving" @click="download">{{ saving ? '正在保存…' : '下载 PNG' }}</button></div></footer>
  </dialog>
</template>

<style scoped>
.share-image-panel { --share-blue:#326bad; color:#37483f; background:#fcfbf6; border:1px solid #e1e4db; border-radius:22px; padding:0; width:min(1040px,calc(100vw - 48px)); max-width:calc(100vw - 16px); height:min(760px,calc(100dvh - 48px)); max-height:calc(100dvh - 16px); box-sizing:border-box; overflow:hidden; font:400 14px/1.6 'Noto Sans SC Variable',sans-serif; box-shadow:0 24px 100px #1b2d2830; }
.share-image-panel[open] { display:flex; flex-direction:column; }
.share-image-panel::backdrop { background:#27372e59; }
header { display:flex; justify-content:space-between; align-items:center; gap:16px; padding:20px 28px; border-bottom:1px solid #e8e9e2; flex-shrink:0; } h2 { margin:0; font-size:19px; font-weight:550; letter-spacing:.04em; } header p { margin:3px 0 0; font-size:12px; color:#859084; }
button { font:inherit; min-height:44px; border:1px solid #dce2d9; background:transparent; color:var(--share-blue); border-radius:10px; cursor:pointer; padding:8px 18px; } button:disabled { cursor:default; opacity:.42; } button:not(:disabled):hover { background:#eef2e9; } .share-close { width:44px; padding:0; border:0; font-size:26px; color:#7b8579; border-radius:50%; }
.share-workspace { display:grid; grid-template-columns:300px minmax(0,1fr); min-height:0; flex:1; }
.share-controls { padding:24px; min-width:0; overflow:auto; border-right:1px solid #e8e9e2; } fieldset { min-width:0; padding:0 0 20px; margin:0 0 20px; border:0; border-bottom:1px solid #e8e9e2; } legend { padding:0; font-size:12px; color:#8a9387; margin-bottom:12px; }
label { display:grid; gap:7px; margin-bottom:18px; font-size:13px; } .share-choice { margin-bottom:8px; } .share-choice span { display:flex; align-items:center; gap:8px; } input[type=checkbox] { accent-color:var(--share-blue); }
summary { cursor:pointer; font-size:12px; color:#859084; } .share-original { padding-left:26px; } .share-choice pre { max-height:120px; overflow:auto; white-space:pre-wrap; overflow-wrap:anywhere; font:400 12px/1.8 'Noto Sans SC Variable',sans-serif; margin:10px 0 0; color:#74806f; }
input:not([type=checkbox]),textarea,select { box-sizing:border-box; width:100%; min-width:0; padding:9px 11px; font:inherit; color:#465442; background:#fffefa; border:1px solid #dfe3d9; border-radius:8px; outline-offset:2px; } textarea { resize:vertical; min-height:88px; } input::placeholder,textarea::placeholder { color:#a0a798; }
.share-style-options { display:grid; grid-template-columns:1fr 1fr; gap:12px; } .share-source-details { border-top:1px solid #e8e9e2; padding-top:14px; } .share-source-details p { font-size:12px; overflow-wrap:anywhere; color:#859084; }
.share-reselect { margin-bottom:18px; width:100%; }
.share-answer-range { margin-bottom:18px; } .share-answer-range p,.share-range-label { font-size:12px; color:#697d68; } .share-answer-range button { width:100%; margin-top:8px; }
.share-preview { min-width:0; overflow:auto; padding:24px 32px; background:#f0f2eb; text-align:center; } .share-preview nav { display:flex; justify-content:center; align-items:center; gap:16px; margin-bottom:16px; font-size:12px; } .share-preview nav button { min-height:36px; padding:4px 12px; border:0; }
.share-preview img { display:block; width:min(100%,355px,max(180px,calc((100dvh - 340px) * .75))); height:auto; margin:0 auto; box-shadow:0 8px 30px #2c42311a; border-radius:2px; } .share-preview img.landscape { width:min(100%,600px); } .share-preview a { display:inline-flex; align-items:center; min-height:44px; font-size:12px; color:#697d68; text-decoration:none; } .share-preview p { font-size:11px; line-height:1.6; color:#8b9585; margin:0; }
.share-empty { min-height:100%; display:flex; flex-direction:column; justify-content:center; align-items:center; color:#95a58a; padding:30px 0; } .share-empty span { font:90px/1 Georgia,serif; color:#b5c5a7; } .share-empty p { font:400 22px/1.5 'Noto Serif SC Variable',serif; color:#74896b; } .share-empty small { margin-top:10px; font-size:12px; color:#9aa58f; }
.share-bottom { display:flex; justify-content:space-between; align-items:center; gap:16px; padding:16px 24px; border-top:1px solid #e3e7dd; flex-shrink:0; background:#fcfbf6; } .share-feedback { min-width:0; } .share-feedback p { margin:0; font-size:12px; overflow-wrap:anywhere; } .share-hint { color:#929b8b; } .share-actions { display:flex; gap:10px; flex-shrink:0; } .share-save { background:var(--share-blue); border-color:var(--share-blue); color:#fff; } .share-save:not(:disabled):hover { background:#265b97; } [role=alert] { color:#aa382b; }
.share-controls,.share-preview,.share-workspace,.share-choice pre { scrollbar-width:thin; scrollbar-color:#cbd3c3 transparent; } ::-webkit-scrollbar { width:5px; height:5px; } ::-webkit-scrollbar-track { background:transparent; } ::-webkit-scrollbar-thumb { background:#cbd3c3; border-radius:6px; }
@media(max-width:680px) { .share-image-panel { width:calc(100vw - 16px); height:calc(100dvh - 16px); border-radius:16px; } header { padding:14px 18px; } header p { display:none; } .share-workspace { display:block; overflow:auto; } .share-controls { overflow:visible; border-right:0; padding:18px; } fieldset { padding-bottom:12px; margin-bottom:14px; } label { margin-bottom:14px; } .share-preview { overflow:visible; padding:20px; } .share-preview img { width:min(100%,355px); } .share-empty { min-height:200px; } .share-bottom { padding:12px; display:block; } .share-feedback:has([role=alert]),.share-feedback:has([role=status]) { margin-bottom:8px; max-height:72px; overflow:auto; } .share-hint { display:none; } .share-actions { justify-content:flex-end; } .share-actions button { flex:1; } }
</style>
