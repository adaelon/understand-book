<script setup lang="ts">
import { t as uiText, formatLocale } from '../i18n';

import { computed, nextTick, onBeforeUnmount, ref, watch } from 'vue';
import { calibrationRevision, calibrationTime, createCalibrationDraft, type MusicAlignmentRevision, type MusicCalibrationContext } from '../music-calibration';

const props = defineProps<{ context: MusicCalibrationContext; saving?: boolean; saveError?: string }>();
const emit = defineEmits<{ save: [revision: MusicAlignmentRevision]; close: [] }>();
const first = ref(0), last = ref(0), selectedOccurrence = ref(''), selectedNote = ref('');
const draft = ref(createCalibrationDraft(props.context.manifest, 0, 0));
const baseline = ref(''), dirty = computed(() => JSON.stringify(draft.value) !== baseline.value);
const localError = ref(''), savedNotice = ref(''), audio = ref<HTMLAudioElement | null>(null);
const currentMs = ref(0), playing = ref(false), looping = ref(false), audition = ref<'original' | 'draft'>('draft'), rate = ref(1);
const scoreHost = ref<HTMLDivElement | null>(null), scoreSvg = ref(''), scoreError = ref(''), scoreLoading = ref(false), pageId = ref('');
const objects = computed(() => new Map(props.context.manifest.score_objects.map(object => [object.source_id, object])));
const partNames = computed(() => new Map(props.context.manifest.parts.map(part => [part.part_id, part.name])));
const occurrence = computed(() => draft.value.occurrences.find(item => item.occurrence_id === selectedOccurrence.value) ?? draft.value.occurrences[0]);
const original = computed(() => props.context.manifest.occurrences.find(item => item.occurrence_id === occurrence.value.occurrence_id)!);
const visibleNotes = computed(() => draft.value.notes.filter(note => note.occurrence_id === occurrence.value.occurrence_id));
const missingNotes = computed(() => draft.value.notes.filter(note => note.start_ms === null || note.end_ms === null).length);
const activeNote = computed(() => visibleNotes.value.find(note => note.score_object_id === selectedNote.value));
const availablePages = computed(() => [...new Set(occurrence.value.score_object_ids.map(id => objects.value.get(id)?.page_id).filter(Boolean))] as string[]);
const range = computed(() => audition.value === 'original' ? original.value : occurrence.value);
let scoreRequest = 0, playbackFrame = 0;

function resetDraft() {
  draft.value = createCalibrationDraft(props.context.manifest, first.value, last.value);
  baseline.value = JSON.stringify(draft.value);
  selectedOccurrence.value = draft.value.occurrences[0].occurrence_id;
  selectedNote.value = ''; localError.value = '';
}
resetDraft();
watch(() => props.context, () => { resetDraft(); savedNotice.value = uiText('已保存到本地工作区，修订历史已更新。'); });
function changeRange() { if (last.value < first.value) last.value = first.value; resetDraft(); savedNotice.value = ''; }
function selectOccurrence(id: string) { pause(); selectedOccurrence.value = id; selectedNote.value = ''; }
watch(availablePages, pages => { if (!pages.includes(pageId.value)) pageId.value = pages[0] ?? ''; }, { immediate: true });
watch([pageId, () => props.context], async () => {
  const request = ++scoreRequest;
  scoreSvg.value = ''; scoreError.value = '';
  const page = props.context.score_pages.find(item => item.page_id === pageId.value);
  if (!page) return;
  scoreLoading.value = true;
  try {
    const response = await fetch(page.url);
    if (!response.ok) throw new Error(uiText('谱面载入失败（{0}）', [response.status]));
    const source = await response.text();
    if (request === scoreRequest) { scoreSvg.value = source; await nextTick(); paintSelection(); }
  } catch (error) { if (request === scoreRequest) scoreError.value = (error as Error).message; }
  finally { if (request === scoreRequest) scoreLoading.value = false; }
}, { immediate: true });
function paintSelection() {
  scoreHost.value?.querySelectorAll('.calibration-selected').forEach(element => element.classList.remove('calibration-selected'));
  const id = objects.value.get(selectedNote.value)?.svg_element_id;
  if (id) scoreHost.value?.querySelector(`[id="${CSS.escape(id)}"]`)?.classList.add('calibration-selected');
}
watch(selectedNote, async id => {
  const object = objects.value.get(id);
  if (object) pageId.value = object.page_id;
  await nextTick(); paintSelection();
});
function selectScore(event: MouseEvent) {
  let target = event.target as Element | null;
  while (target && target !== scoreHost.value) {
    if (target.id) {
      const object = props.context.manifest.score_objects.find(item => item.svg_element_id === target!.id);
      if (object && occurrence.value.score_object_ids.includes(object.source_id) && !object.rest) { selectedNote.value = object.source_id; return; }
    }
    target = target.parentElement;
  }
}
function seek(ms: number) {
  if (!audio.value) return;
  try { audio.value.currentTime = Math.max(0, Math.min(props.context.manifest.recording.duration_ms, ms)) / 1000; currentMs.value = audio.value.currentTime * 1000; }
  catch (error) { localError.value = (error as Error).message; }
}
function stopPlaybackClock() { playing.value = false; cancelAnimationFrame(playbackFrame); playbackFrame = 0; }
function startPlaybackClock() {
  cancelAnimationFrame(playbackFrame); playing.value = true;
  const tick = () => { updateTime(); if (playing.value && audio.value && !audio.value.paused) playbackFrame = requestAnimationFrame(tick); };
  playbackFrame = requestAnimationFrame(tick);
}
function pause() { audio.value?.pause(); stopPlaybackClock(); }
async function play(which: 'original' | 'draft') {
  if (!audio.value) return;
  audition.value = which; localError.value = '';
  const interval = which === 'original' ? original.value : occurrence.value;
  if (!Number.isFinite(interval.start_ms) || interval.start_ms >= interval.end_ms) { localError.value = uiText('请先填写有效的复听区间。'); return; }
  seek(interval.start_ms); audio.value.playbackRate = rate.value;
  try { await audio.value.play(); playing.value = !audio.value.paused; } catch (error) { localError.value = uiText('无法播放：{0}', [(error as Error).message]); }
}
function updateTime() {
  if (!audio.value) return;
  currentMs.value = audio.value.currentTime * 1000;
  if (playing.value && currentMs.value >= range.value.end_ms) { if (looping.value) seek(range.value.start_ms); else pause(); }
}
function changeRate() { if (audio.value) audio.value.playbackRate = rate.value; }
function inputMs(event: Event): number | null { const value = (event.target as HTMLInputElement).value; return value.trim() === '' ? null : Number(value) * 1000; }
function setBoundary(field: 'start_ms' | 'end_ms', value: number | null) {
  if (value === null) return;
  const index = draft.value.occurrences.indexOf(occurrence.value), previous = occurrence.value[field];
  occurrence.value[field] = value;
  // A shared bar line moves once, keeping the two selected measures adjacent.
  if (field === 'end_ms' && draft.value.occurrences[index + 1]?.start_ms === previous) draft.value.occurrences[index + 1].start_ms = value;
  if (field === 'start_ms' && draft.value.occurrences[index - 1]?.end_ms === previous) draft.value.occurrences[index - 1].end_ms = value;
}
function save() {
  localError.value = ''; savedNotice.value = '';
  try { emit('save', calibrationRevision(props.context.manifest, draft.value, `calibration-${crypto.randomUUID()}`)); }
  catch (error) { localError.value = (error as Error).message; }
}
function occurrenceLabel(index: number) { const item = props.context.manifest.occurrences[index]; return uiText('{0} · 第 {1} 小节 · {2}', [index + 1, item.measure, calibrationTime(item.start_ms)]); }
function noteLabel(id: string) {
  const item = objects.value.get(id)!;
  const pitch = item.midi_pitch == null ? uiText('未定音高') : uiText('{0}{1}（实音）', [['C', 'C♯', 'D', 'D♯', 'E', 'F', 'F♯', 'G', 'G♯', 'A', 'A♯', 'B'][item.midi_pitch % 12], Math.floor(item.midi_pitch / 12) - 1]);
  return uiText('{0} · {1} · 声部 {2} · 偏移 {3} 四分拍', [partNames.value.get(item.part_id) ?? item.part_id, pitch, item.voice, item.beat]);
}
onBeforeUnmount(() => { scoreRequest++; pause(); });
</script>

<template>
  <main class="calibration-workbench">
    <header class="calibration-header">
      <div><span class="calibration-eyebrow">{{ uiText('本地材料 · 时间校准') }}</span><h1>{{ context.manifest.work.title }}</h1><p>{{ uiText('对照录音和乐谱，逐段修正候选时间，再决定接纳到哪一层精度。') }}</p></div>
      <button type="button" :disabled="dirty || saving" @click="emit('close')">{{ uiText('返回工作区') }}</button>
    </header>
    <audio ref="audio" :src="context.audio_url" preload="metadata" @timeupdate="updateTime" @pause="stopPlaybackClock" @ended="stopPlaybackClock" @play="startPlaybackClock" @error="localError = uiText('录音载入失败，请检查本地音频文件。'); stopPlaybackClock()" />
    <section class="calibration-transport" :aria-label="uiText('校准录音播放器')">
      <div class="calibration-play-row"><button class="calibration-primary" @click="playing ? pause() : play('draft')">{{ playing ? uiText('暂停') : uiText('复听草稿') }}</button><button @click="play('original')">{{ uiText('复听原时间') }}</button><label><input v-model="looping" type="checkbox" /> {{ uiText('循环本小节') }}</label><label>{{ uiText('速度') }} <select v-model="rate" :aria-label="uiText('复听速度')" @change="changeRate"><option :value="0.5">0.5×</option><option :value="0.75">0.75×</option><option :value="1">1×</option></select></label><output class="calibration-clock">{{ calibrationTime(currentMs) }}</output></div>
      <input type="range" :aria-label="uiText('录音位置')" min="0" :max="context.manifest.recording.duration_ms" step="10" :value="currentMs" @input="seek(Number(($event.target as HTMLInputElement).value))" />
      <small>{{ uiText('正在使用') }}{{ audition === 'original' ? uiText('原时间') : uiText('草稿时间') }} · {{ context.manifest.recording.title }}<template v-if="context.manifest.recording.kind === 'score_synthesis'"> {{ uiText('· 谱面合成示范') }}</template></small>
    </section>
    <div v-if="localError || saveError" class="calibration-message error" role="alert">{{ localError || saveError }}</div>
    <div v-if="savedNotice" class="calibration-message success" role="status">{{ savedNotice }}</div>
    <section class="calibration-window" :aria-label="uiText('校准范围')">
      <div><strong>{{ uiText('选择连续出现的范围') }}</strong><p>{{ uiText('反复段落按演奏顺序单独列出。要移动窗口外缘，请先把相邻小节一起选入。') }}</p></div>
      <label>{{ uiText('从') }}<select v-model="first" :disabled="dirty || saving" :aria-label="uiText('窗口起始小节')" @change="changeRange"><option v-for="(item, index) in context.manifest.occurrences" :key="item.occurrence_id" :value="index">{{ occurrenceLabel(index) }}</option></select></label>
      <label>{{ uiText('到') }}<select v-model="last" :disabled="dirty || saving" :aria-label="uiText('窗口结束小节')" @change="changeRange"><option v-for="(item, index) in context.manifest.occurrences" :key="item.occurrence_id" :value="index" :disabled="index < first">{{ occurrenceLabel(index) }}</option></select></label>
      <button :disabled="!dirty || saving" @click="resetDraft">{{ uiText('放弃草稿修改') }}</button>
      <span v-if="dirty" class="calibration-unsaved">{{ uiText('有未保存修改') }}</span>
    </section>
    <div class="calibration-columns">
      <section class="calibration-score-panel" :aria-label="uiText('谱面对照')">
        <div class="calibration-section-head"><h2>{{ uiText('谱面对照') }}</h2><label>{{ uiText('谱页') }}<select v-model="pageId" :aria-label="uiText('校准谱页')"><option v-for="id in availablePages" :key="id" :value="id">{{ id }}</option></select></label></div>
        <p class="calibration-muted">{{ uiText('点击本小节的音符可选中对应时间；这里只标出选择，不将候选时间显示为精确跟谱。') }}</p>
        <p v-if="scoreLoading" role="status">{{ uiText('正在载入谱面…') }}</p><p v-if="scoreError" role="alert">{{ scoreError }}</p>
        <div ref="scoreHost" class="calibration-svg" @click="selectScore" v-html="scoreSvg" />
      </section>
      <section class="calibration-editor" :aria-label="uiText('时间草稿')">
        <div class="calibration-section-head"><h2>{{ uiText('时间草稿') }}</h2><span class="calibration-badge">{{ missingNotes ? uiText('{0} 个音符待补', [missingNotes]) : uiText('音符时间已填齐') }}</span></div>
        <label class="calibration-occurrence-label">{{ uiText('当前小节出现') }}<select :value="selectedOccurrence" :aria-label="uiText('当前小节出现')" @change="selectOccurrence(($event.target as HTMLSelectElement).value)"><option v-for="item in draft.occurrences" :key="item.occurrence_id" :value="item.occurrence_id">{{ uiText('第') }} {{ item.measure }} {{ uiText('小节 · 演奏序号') }} {{ item.order + 1 }}</option></select></label>
        <div class="calibration-original">{{ uiText('原时间') }} {{ calibrationTime(original.start_ms) }} — {{ calibrationTime(original.end_ms) }}</div>
        <div class="calibration-boundaries">
          <label>{{ uiText('小节起点（秒）') }}<input type="number" step="0.001" :aria-label="uiText('小节起点秒')" :value="occurrence.start_ms / 1000" @input="setBoundary('start_ms', inputMs($event))" /><button @click="setBoundary('start_ms', Math.round(currentMs))">{{ uiText('取当前时间') }}</button></label>
          <label>{{ uiText('小节终点（秒）') }}<input type="number" step="0.001" :aria-label="uiText('小节终点秒')" :value="occurrence.end_ms / 1000" @input="setBoundary('end_ms', inputMs($event))" /><button @click="setBoundary('end_ms', Math.round(currentMs))">{{ uiText('取当前时间') }}</button></label>
        </div>
        <p class="calibration-muted">{{ uiText('修改相邻小节共用的边界时，两侧同时移动。音符起点仍需单独核对。') }}</p>
        <div class="calibration-note-picker"><label>{{ uiText('音符') }}<select v-model="selectedNote" :aria-label="uiText('选择校准音符')"><option value="">{{ uiText('选择音符，或点选谱面') }}</option><option v-for="note in visibleNotes" :key="note.score_object_id" :value="note.score_object_id">{{ noteLabel(note.score_object_id) }}{{ note.start_ms === null ? uiText(' · 待补') : '' }}</option></select></label></div>
        <div v-if="activeNote" class="calibration-note-editor">
          <strong>{{ noteLabel(activeNote.score_object_id) }}</strong><small>{{ activeNote.score_object_id }}</small>
          <div class="calibration-boundaries"><label>{{ uiText('音符起点（秒）') }}<input type="number" step="0.001" :aria-label="uiText('音符起点秒')" :value="activeNote.start_ms === null ? '' : activeNote.start_ms / 1000" @input="activeNote.start_ms = inputMs($event)" /><button @click="activeNote.start_ms = Math.round(currentMs)">{{ uiText('取当前时间') }}</button></label><label>{{ uiText('音符终点（秒）') }}<input type="number" step="0.001" :aria-label="uiText('音符终点秒')" :value="activeNote.end_ms === null ? '' : activeNote.end_ms / 1000" @input="activeNote.end_ms = inputMs($event)" /><button @click="activeNote.end_ms = Math.round(currentMs)">{{ uiText('取当前时间') }}</button></label></div>
          <label><input v-model="activeNote.attack" type="checkbox" /> {{ uiText('此处重新起音') }}</label><p class="calibration-muted">{{ uiText('延音线接续音不应再次起跳；当前谱面延音标记：') }}{{ objects.get(activeNote.score_object_id)?.tie }}。</p>
          <button @click="activeNote.start_ms = null; activeNote.end_ms = null">{{ uiText('清除此音时间，保留待补') }}</button>
        </div>
        <p v-else class="calibration-empty">{{ uiText('当前小节有') }} {{ visibleNotes.length }} {{ uiText('个非休止音符。选择一个音符可编辑它的起止和是否重新起音。') }}</p>
        <details class="calibration-note-table"><summary>{{ uiText('查看本小节全部音符时间') }}</summary><div class="calibration-table-scroll"><table><thead><tr><th>{{ uiText('音符 / 声部') }}</th><th>{{ uiText('起点') }}</th><th>{{ uiText('终点') }}</th></tr></thead><tbody><tr v-for="note in visibleNotes" :key="note.score_object_id"><td><button @click="selectedNote = note.score_object_id">{{ noteLabel(note.score_object_id) }}</button></td><td>{{ note.start_ms === null ? uiText('待补') : calibrationTime(note.start_ms) }}</td><td>{{ note.end_ms === null ? uiText('待补') : calibrationTime(note.end_ms) }}</td></tr></tbody></table></div></details>
      </section>
    </div>
    <section class="calibration-save-panel" :aria-label="uiText('保存校准修订')">
      <div><h2>{{ uiText('保存这次判断') }}</h2><p>{{ uiText('范围') }} {{ calibrationTime(draft.start_ms) }} — {{ calibrationTime(draft.end_ms) }}{{ uiText('。保存会记录前后差异和依据。') }}</p></div>
      <div class="calibration-save-fields"><label>{{ uiText('修改原因') }}<input v-model="draft.reason" :aria-label="uiText('修改原因')" :placeholder="uiText('例如：第二次出现的收尾比候选时间更晚')" /></label><label>{{ uiText('时间判断的依据') }}<textarea v-model="draft.evidence" :aria-label="uiText('时间判断的依据')" rows="2" :placeholder="uiText('说明依据；尚未复听时如实写为待核候选')" /></label></div>
      <div class="calibration-save-controls"><label>{{ uiText('保存精度') }}<select v-model="draft.acceptance" :aria-label="uiText('保存精度')"><option value="pending">{{ uiText('保存待核候选') }}</option><option value="measure">{{ uiText('接纳小节定位，音符仍待核') }}</option><option value="note">{{ uiText('接纳逐音符定位') }}</option></select></label><label><input v-model="draft.reviewed" type="checkbox" /> {{ uiText('我已复听并核对本窗口') }}</label><button class="calibration-primary" :disabled="saving" @click="save">{{ saving ? uiText('正在保存…') : uiText('保存本地修订') }}</button></div>
      <p class="calibration-muted">{{ uiText('接纳逐音符需要本窗口所有非休止音符都有时间。仅保存候选不会启用精确跟谱。') }}</p>
    </section>
    <section class="calibration-history" :aria-label="uiText('校准历史')"><h2>{{ uiText('修订历史') }} <span>{{ context.history.length }}</span></h2><p v-if="!context.history.length" class="calibration-muted">{{ uiText('尚无保存记录。') }}</p><details v-for="(item, index) in [...context.history].reverse()" :key="`${item.receipt.revised_at}-${index}`"><summary>{{ item.receipt.reason }} <small>{{ new Date(item.receipt.revised_at).toLocaleString(formatLocale()) }}</small></summary><p>{{ calibrationTime(item.receipt.start_ms) }} — {{ calibrationTime(item.receipt.end_ms) }} · {{ item.receipt.after.occurrences.length }} {{ uiText('次小节出现 ·') }} {{ item.receipt.after.note_alignments.length }} {{ uiText('个音符时间') }}</p><ul><li v-for="coverage in item.receipt.after.coverage" :key="coverage.granularity">{{ coverage.granularity === 'note' ? uiText('音符') : coverage.granularity === 'measure' ? uiText('小节') : uiText('段落') }}：{{ coverage.status === 'accepted' ? uiText('已接纳') : uiText('待核') }} · {{ coverage.detail }}</li></ul></details></section>
  </main>
</template>

<style scoped>
.calibration-workbench{scroll-padding-top:170px}.calibration-header>div{min-width:0}.calibration-header>button{flex-shrink:0}.calibration-svg :deep(svg){pointer-events:none}.calibration-svg :deep(.note){pointer-events:bounding-box}
.calibration-workbench{height:100dvh;overflow:auto;background:#f5f2ec;color:#292c31;padding:28px clamp(16px,3vw,48px) 64px;font-family:var(--font-sans,system-ui);box-sizing:border-box}.calibration-workbench *{box-sizing:border-box}.calibration-header{display:flex;justify-content:space-between;align-items:flex-start;gap:20px;margin-bottom:22px}.calibration-eyebrow{font-size:12px;letter-spacing:.12em;color:#68745d}.calibration-workbench h1{font-size:25px;margin:7px 0}.calibration-workbench h2{font-size:16px;margin:0}.calibration-workbench p{font-size:13px;line-height:1.7;margin:7px 0}.calibration-workbench button,.calibration-workbench select,.calibration-workbench input:not([type=range]):not([type=checkbox]),.calibration-workbench textarea{font:inherit;font-size:13px;border:1px solid #d6d5cd;border-radius:7px;background:#fff;padding:8px 10px;color:inherit;min-width:0}.calibration-workbench button{cursor:pointer;white-space:nowrap}.calibration-workbench button:disabled,.calibration-workbench select:disabled{opacity:.45;cursor:default}.calibration-workbench button:hover:not(:disabled){border-color:#687b69;background:#f1f5ef}.calibration-workbench button.calibration-primary{background:#345b4c;color:#fff;border-color:#345b4c}.calibration-workbench label{font-size:12px;display:flex;gap:7px;align-items:center}.calibration-workbench input[type=checkbox]{accent-color:#345b4c}.calibration-transport{position:sticky;top:-28px;z-index:3;background:#fffefaf2;backdrop-filter:blur(10px);border:1px solid #dbddd2;border-radius:12px;padding:14px 18px;margin-bottom:18px;box-shadow:0 4px 20px #26312b08}.calibration-play-row{display:flex;align-items:center;gap:12px;flex-wrap:wrap}.calibration-clock{margin-left:auto;font-size:22px;font-variant-numeric:tabular-nums}.calibration-transport input[type=range]{display:block;width:100%;accent-color:#345b4c;margin:14px 0 8px}.calibration-transport small{font-size:11px;color:#6f746d}.calibration-window{display:flex;align-items:center;gap:12px;flex-wrap:wrap;padding:10px 0 22px}.calibration-window>div{flex:1 1 300px}.calibration-window strong{font-size:14px}.calibration-window p{color:#73776e;font-size:12px}.calibration-window select{max-width:245px}.calibration-unsaved{font-size:12px;color:#966d29}.calibration-columns{display:grid;grid-template-columns:minmax(0,1.25fr) minmax(360px,1fr);gap:20px}.calibration-score-panel,.calibration-editor,.calibration-save-panel,.calibration-history{background:#fffefa;border:1px solid #deddd5;border-radius:12px;padding:20px;min-width:0}.calibration-section-head{display:flex;align-items:center;justify-content:space-between;gap:10px}.calibration-muted{color:#777c74!important;font-size:12px!important}.calibration-svg{overflow:auto;max-height:770px;background:#fff;min-height:240px}.calibration-svg :deep(svg){display:block;width:100%;height:auto}.calibration-svg :deep(.note){cursor:pointer}.calibration-svg :deep(.calibration-selected),.calibration-svg :deep(.calibration-selected *){fill:#c76b34!important;color:#c76b34!important;stroke:#c76b34!important}.calibration-badge{font-size:11px;background:#f2eadb;color:#876729;border-radius:20px;padding:5px 8px}.calibration-occurrence-label{margin-top:20px;display:grid!important;gap:6px}.calibration-occurrence-label select{width:100%;max-width:100%}.calibration-original{background:#f3f3ed;border-radius:7px;padding:10px;margin:14px 0;font-size:12px;font-variant-numeric:tabular-nums;color:#686e65}.calibration-boundaries{display:grid;grid-template-columns:1fr 1fr;gap:12px}.calibration-boundaries label{display:grid;grid-template-columns:1fr;gap:6px}.calibration-boundaries input{width:100%;font-variant-numeric:tabular-nums}.calibration-boundaries button{font-size:11px}.calibration-note-picker{border-top:1px solid #e4e3dc;padding-top:16px;margin-top:18px}.calibration-note-picker label{display:grid;grid-template-columns:1fr}.calibration-note-picker select{width:100%}.calibration-note-editor{margin-top:16px;padding:14px;background:#faf5ec;border:1px solid #e9dcc9;border-radius:8px}.calibration-note-editor>strong{display:block;font-size:12px}.calibration-note-editor>small{display:block;font-size:10px;color:#8a8173;margin:6px 0 14px;overflow-wrap:anywhere}.calibration-note-editor>.calibration-boundaries{margin-bottom:12px}.calibration-empty{padding:20px 0;color:#7b8177}.calibration-note-table{margin-top:20px;font-size:12px}.calibration-table-scroll{max-height:250px;overflow:auto}.calibration-note-table table{width:100%;border-collapse:collapse}.calibration-note-table td,.calibration-note-table th{text-align:left;padding:8px 4px;border-bottom:1px solid #eee;white-space:nowrap;font-size:11px}.calibration-note-table td button{font-size:11px;padding:4px;white-space:normal}.calibration-save-panel{margin-top:20px}.calibration-save-panel>div>p{color:#74796f}.calibration-save-fields{display:grid;grid-template-columns:1fr 1.4fr;gap:20px;margin:16px 0}.calibration-save-fields label{display:grid;align-content:start;gap:7px}.calibration-save-fields textarea{width:100%;resize:vertical}.calibration-save-controls{display:flex;flex-wrap:wrap;gap:16px;align-items:center}.calibration-save-controls>.calibration-primary{margin-left:auto}.calibration-history{margin-top:20px}.calibration-history h2 span{font-size:12px;color:#888;margin-left:6px}.calibration-history details{margin-top:14px;border-top:1px solid #e9e7df;padding-top:14px;font-size:13px}.calibration-history summary{cursor:pointer}.calibration-history small{color:#818679;margin-left:12px}.calibration-history ul{font-size:12px;padding-left:20px;line-height:1.7}.calibration-message{padding:12px 16px;border-radius:8px;margin:14px 0;font-size:13px}.calibration-message.error{background:#faeae4;color:#9c4c35}.calibration-message.success{background:#eaf2e8;color:#3c6c4c}
@media(max-width:850px){.calibration-columns{grid-template-columns:1fr}.calibration-svg{max-height:460px}.calibration-header h1{font-size:21px}.calibration-save-fields{grid-template-columns:1fr}.calibration-transport{top:-28px}.calibration-window select{max-width:260px}.calibration-score-panel{order:2}.calibration-editor{order:1}}@media(max-width:480px){.calibration-workbench{padding:20px 12px 40px}.calibration-header{gap:8px}.calibration-header>button{font-size:11px;padding:7px}.calibration-header h1{font-size:18px}.calibration-header p{font-size:12px}.calibration-transport{top:-20px;padding:12px}.calibration-play-row{gap:8px}.calibration-clock{font-size:19px}.calibration-play-row label{font-size:11px}.calibration-window>label{width:100%;display:grid;grid-template-columns:24px 1fr}.calibration-window select{max-width:none;width:100%}.calibration-editor,.calibration-score-panel,.calibration-save-panel,.calibration-history{padding:15px}.calibration-section-head{flex-wrap:wrap}.calibration-save-controls{align-items:flex-start;flex-direction:column}.calibration-save-controls>.calibration-primary{margin-left:0;width:100%}.calibration-save-controls>label:first-child{width:100%;flex-direction:column;align-items:flex-start}.calibration-save-controls select{width:100%}.calibration-history small{display:block;margin:6px 0}}
@media(max-width:480px){.calibration-workbench{scroll-padding-top:230px}}
</style>
