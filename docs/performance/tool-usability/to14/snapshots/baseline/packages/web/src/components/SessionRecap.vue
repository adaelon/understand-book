<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, ref, watch } from 'vue';
import { api as sharedApi } from '../api';
import { bindSceneApi, readerKey } from '../network-context';
import { recapEntries, recapGroups, recapShareSource, recapTime, type RecapTarget, type SessionRecap } from '../session-recap';
import type { ShareSource } from '../reading-share';
import ShareImagePanel from './ShareImagePanel.vue';
const api = bindSceneApi(sharedApi);
const props = defineProps<{ sessionId: string; sessionTitle?: string; navigate: (target: RecapTarget) => Promise<void> }>();
const emit = defineEmits<{ (e: 'close'): void }>();
const recap = ref<SessionRecap | null>(null), loading = ref(false), navigating = ref(false), error = ref('');
const closeButton = ref<HTMLButtonElement>();
const shareButton = ref<HTMLButtonElement>();
const focusedId = ref(''), selected = ref<string[]>([]), shareSource = ref<ShareSource | null>(null);
const entries = computed(() => recap.value ? recapEntries(recap.value) : []);
const focused = computed(() => entries.value.find(item => item.id === focusedId.value) ?? entries.value[0]);
const groups = computed(() => recapGroups.map(group => ({ ...group, entries: entries.value.filter(item => item.group === group.kind) })));
watch(entries, items => { selected.value = selected.value.filter(id => items.some(item => item.id === id)); });
function share() {
  if (recap.value && selected.value.length) shareSource.value = recapShareSource(recap.value, selected.value, props.sessionTitle);
}
async function closeShare() {
  shareSource.value = null;
  await nextTick(); shareButton.value?.focus({ preventScroll: true });
}
let request = 0;
async function refresh() {
  const sequence = ++request, session = props.sessionId;
  loading.value = true; error.value = '';
  try {
    const result = await api.sessionRecap(session);
    if (sequence === request) recap.value = result;
  } catch (failure) {
    if (sequence === request) error.value = failure instanceof Error ? failure.message : String(failure);
  } finally { if (sequence === request) loading.value = false; }
}
watch([() => props.sessionId, readerKey], () => {
  recap.value = null; navigating.value = false; focusedId.value = ''; selected.value = []; shareSource.value = null;
  void refresh();
}, { immediate: true });
onBeforeUnmount(() => { request++; });
void nextTick(() => closeButton.value?.focus());
async function open(target: RecapTarget) {
  if (navigating.value) return;
  const sequence = request;
  navigating.value = true; error.value = '';
  try {
    await props.navigate(target);
    if (sequence === request) emit('close');
  } catch (failure) {
    if (sequence === request) error.value = failure instanceof Error ? failure.message : String(failure);
  } finally { if (sequence === request) navigating.value = false; }
}
function trapFocus(event: KeyboardEvent) {
  if (event.key !== 'Tab') return;
  const controls = Array.from((event.currentTarget as HTMLElement).querySelectorAll<HTMLElement>('button:not(:disabled), input:not(:disabled), summary'));
  const first = controls[0], last = controls.at(-1);
  if (event.shiftKey && document.activeElement === first) { event.preventDefault(); last?.focus(); }
  else if (!event.shiftKey && document.activeElement === last) { event.preventDefault(); first?.focus(); }
}
</script>

<template>
  <Teleport to="body">
    <div class="recap-backdrop" :inert="!!shareSource" :aria-hidden="shareSource ? 'true' : undefined" @click.self="emit('close')">
      <section class="session-recap" role="dialog" aria-modal="true" aria-labelledby="recap-title"
        @keydown.esc.stop.prevent="emit('close')" @keydown="trapFocus">
        <header>
          <div><p class="recap-kicker">{{ sessionTitle || '当前对话' }}</p><h2 id="recap-title">本次阅读回顾</h2></div>
          <button ref="closeButton" class="recap-close" @click="emit('close')">返回聊天</button>
        </header>
        <div class="recap-range">
          <div v-if="recap"><p>从对话开始 · 截至 {{ recapTime(recap.through_at) }}</p><small>更新于 {{ recapTime(recap.generated_at) }}</small></div>
          <p v-else>正在读取已保存记录</p>
          <button class="recap-refresh" :disabled="loading || navigating" @click="refresh">{{ loading ? '读取中…' : '刷新回顾' }}</button>
        </div>
        <p class="recap-hint">新记录会在刷新后加入。已回答表示回答已交付。</p>
        <p v-if="error" role="alert" class="recap-error">{{ error }}</p>
        <p v-if="loading && !recap" role="status">正在整理本次阅读…</p>
        <section v-if="focused" class="recap-focus" aria-label="当前查看重点">
          <p class="recap-kicker">当前查看 · {{ recapGroups.find(group => group.kind === focused!.group)?.label }}</p>
          <h3 v-if="focused.label">{{ focused.label }}</h3>
          <p class="recap-text">{{ focused.text }}</p>
          <span v-if="focused.status" class="recap-status">{{ focused.status }}</span>
          <p v-if="focused.unavailable" class="recap-unavailable">{{ focused.unavailable }}</p>
        </section>
        <div v-if="recap" class="recap-sections">
          <section v-for="group in groups" :key="group.kind" :aria-labelledby="`recap-${group.kind}`">
            <h3 :id="`recap-${group.kind}`">{{ group.label }} <span>{{ group.entries.length }}</span></h3>
            <p v-if="!group.entries.length" class="recap-empty">{{ group.empty }}</p>
            <article v-for="(item, index) in group.entries" :key="item.id" :class="{ 'is-focused': focused?.id === item.id }" @click="focusedId = item.id">
              <div class="recap-item-tools">
                <label><input v-model="selected" type="checkbox" :value="item.id" :aria-label="`分享${group.label}第${index + 1}项`" @click.stop />选入分享</label>
                <button class="recap-focus-button" :aria-pressed="focused?.id === item.id" @click="focusedId = item.id">重点查看</button>
              </div>
              <details v-if="item.group === 'questions'"><summary>{{ item.text.slice(0, 100) }}{{ item.text.length > 100 ? '…' : '' }}</summary><p class="recap-text">{{ item.text }}</p></details>
              <template v-else><h4 v-if="item.label">{{ item.label }}</h4><p class="recap-text">{{ item.text }}</p></template>
              <p v-if="item.unavailable" class="recap-unavailable">{{ item.unavailable }}</p>
              <footer><span v-if="item.status" class="recap-status">{{ item.status }}</span>
                <button v-if="item.turnAction" :disabled="navigating" @click.stop="open({ session_id: item.target.session_id, through_seq: item.target.through_seq, turn_id: item.target.turn_id, kind: 'turn' })">{{ item.turnAction }}</button>
                <button :disabled="navigating || !!item.unavailable" @click.stop="open(item.target)">{{ item.action }}</button>
              </footer>
            </article>
          </section>
        </div>
        <div class="recap-share-bar"><p>已选 {{ selected.length }} 项<span> · 仅分享勾选的记录</span></p><button ref="shareButton" :disabled="!selected.length || loading || navigating" @click="share">生成阅读回顾卡</button></div>
      </section>
    </div>
    <ShareImagePanel v-if="shareSource" :source="shareSource" @close="closeShare" @reselect="closeShare" />
  </Teleport>
</template>

<style scoped>
.recap-backdrop { position: fixed; inset: 0; z-index: 160; background: #0006; display: grid; place-items: center; padding: 20px; }
.session-recap { background: var(--canvas, #fffaf2); color: var(--ink, #222); width: min(820px, 100%); max-height: calc(100dvh - 40px); overflow: auto; border: 1px solid var(--hairline, #ddd); border-radius: 24px; padding: 24px; box-sizing: border-box; }
header, .recap-range, footer { display: flex; justify-content: space-between; gap: 12px; align-items: center; }
header { position: sticky; top: -24px; padding: 12px 0; background: var(--canvas, #fff); z-index: 1; }
h2 { font-size: 22px; margin: 4px 0; } h3 { font-size: 17px; margin: 26px 0 12px; } h3 span { opacity: .55; font-size: 13px; margin-left: 8px; } h4, p { margin: 8px 0; }
.recap-kicker, small, .recap-hint, .recap-empty { font-size: 13px; color: var(--ink-secondary, #666); }
article { border: 1px solid var(--hairline, #ddd); border-radius: 14px; padding: 16px; margin: 12px 0; overflow-wrap: anywhere; }
article.is-focused { border-color: var(--accent, #ba6451); }
.recap-focus { background: var(--surface, #f2f3f0); border-radius: 20px; padding: 20px 24px; margin: 24px 0 32px; overflow-wrap: anywhere; }
.recap-focus .recap-text { font-size: 18px; line-height: 1.8; max-height: 280px; overflow: auto; }
.recap-focus h3 { margin: 12px 0; }
.recap-item-tools { display: flex; justify-content: space-between; align-items: center; gap: 12px; margin-bottom: 12px; }
.recap-item-tools label { display: flex; align-items: center; gap: 8px; font-size: 13px; cursor: pointer; min-height: 40px; }
input { accent-color: var(--accent, #ba6451); }
.recap-focus-button { border: 0; }
.recap-share-bar { position: sticky; bottom: -24px; display: flex; justify-content: space-between; align-items: center; gap: 12px; background: var(--canvas, #fffaf2); border-top: 1px solid var(--hairline, #ddd); padding: 16px 0; margin-top: 24px; }
.recap-share-bar p { font-size: 13px; }.recap-share-bar span { color: var(--ink-secondary, #666); }
.recap-share-bar button { color: var(--accent, #a34e3c); }
.recap-kicker { overflow-wrap: anywhere; }
.recap-text, blockquote { white-space: pre-wrap; } blockquote { margin: 12px 0; padding-left: 14px; border-left: 2px solid var(--hairline, #ddd); }
summary { cursor: pointer; line-height: 1.6; } footer { justify-content: flex-end; margin-top: 10px; flex-wrap: wrap; } footer .recap-status { margin-right: auto; }
.recap-status { display: inline-block; font-size: 12px; padding: 3px 8px; border-radius: 5px; background: var(--surface, #f2f3f0); }
button { font: inherit; font-size: 13px; color: inherit; background: transparent; border: 1px solid var(--hairline, #ccc); padding: 7px 10px; border-radius: 7px; cursor: pointer; flex-shrink: 0; } button:disabled { opacity: .45; cursor: default; }
.recap-error, .recap-unavailable { color: var(--danger, #9c3c30); font-size: 13px; }
@media (max-width: 600px) { .recap-backdrop { padding: 0; } .session-recap { width: 100%; height: 100dvh; max-height: 100dvh; border-radius: 0; padding: 16px; } header { top: -16px; } .recap-range { align-items: flex-start; flex-wrap: wrap; } .recap-focus { padding: 16px; } .recap-share-bar { bottom: -16px; } .recap-share-bar span { display: none; } }
</style>
