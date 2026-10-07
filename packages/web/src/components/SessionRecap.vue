<script setup lang="ts">
import { nextTick, onBeforeUnmount, ref, watch } from 'vue';
import { api as sharedApi } from '../api';
import { bindSceneApi } from '../network-context';
import { recapStatus, recapTime, type RecapTarget, type SessionRecap } from '../session-recap';
const api = bindSceneApi(sharedApi);
const props = defineProps<{ sessionId: string; navigate: (target: RecapTarget) => Promise<void> }>();
const emit = defineEmits<{ (e: 'close'): void }>();
const recap = ref<SessionRecap | null>(null), loading = ref(false), navigating = ref(false), error = ref('');
const closeButton = ref<HTMLButtonElement>();
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
watch(() => props.sessionId, () => { recap.value = null; navigating.value = false; void refresh(); }, { immediate: true });
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
function base(turn_id: string) {
  return { session_id: props.sessionId, through_seq: recap.value!.through_seq, turn_id };
}
function trapFocus(event: KeyboardEvent) {
  if (event.key !== 'Tab') return;
  const controls = Array.from((event.currentTarget as HTMLElement).querySelectorAll<HTMLElement>('button:not(:disabled), summary'));
  const first = controls[0], last = controls.at(-1);
  if (event.shiftKey && document.activeElement === first) { event.preventDefault(); last?.focus(); }
  else if (!event.shiftKey && document.activeElement === last) { event.preventDefault(); first?.focus(); }
}
</script>

<template>
  <Teleport to="body">
    <div class="recap-backdrop" @click.self="emit('close')">
      <section class="session-recap" role="dialog" aria-modal="true" aria-labelledby="recap-title"
        @keydown.esc.stop.prevent="emit('close')" @keydown="trapFocus">
        <header>
          <div><p class="recap-kicker">当前对话</p><h2 id="recap-title">本次阅读回顾</h2></div>
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
        <div v-if="recap" class="recap-sections">
          <section aria-labelledby="recap-questions"><h3 id="recap-questions">讨论过的问题 <span>{{ recap.questions.length }}</span></h3>
            <p v-if="!recap.questions.length" class="recap-empty">还没有提问。</p>
            <article v-for="item in recap.questions" :key="item.evidence[0].turn_id">
              <details><summary>{{ item.text.slice(0, 100) }}{{ item.text.length > 100 ? '…' : '' }}</summary><p class="recap-text">{{ item.text }}</p></details>
              <footer><span class="recap-status">{{ recapStatus(item.status) }}</span><button :disabled="navigating" @click="open({ ...base(item.evidence[0].turn_id), kind: 'turn' })">回到问题</button></footer>
            </article>
          </section>
          <section aria-labelledby="recap-sources"><h3 id="recap-sources">引用的原文 <span>{{ recap.sources.length }}</span></h3>
            <p v-if="!recap.sources.length" class="recap-empty">还没有已绑定的原文来源。</p>
            <article v-for="item in recap.sources" :key="`${item.evidence[0].turn_id}:${item.source_ref_id}`">
              <h4>{{ item.label }}</h4><blockquote v-if="item.quote">{{ item.quote }}</blockquote>
              <p v-if="item.unavailable_reason" class="recap-unavailable">{{ item.unavailable_reason }}</p>
              <footer><button :disabled="navigating" @click="open({ ...base(item.evidence[0].turn_id), kind: 'turn' })">关联问题</button>
                <button :disabled="navigating || !!item.unavailable_reason" @click="open({ ...base(item.evidence[0].turn_id), kind: 'source', source: item })">查看原文</button></footer>
            </article>
          </section>
          <section aria-labelledby="recap-effects"><h3 id="recap-effects">留下的成果 <span>{{ recap.effects.length }}</span></h3>
            <p v-if="!recap.effects.length" class="recap-empty">还没有阅读成果。</p>
            <article v-for="item in recap.effects" :key="`${item.evidence[0].turn_id}:${item.effect_id}`">
              <p class="recap-text">{{ item.label }}</p><span class="recap-status">{{ recapStatus(item.status) }}</span>
              <p v-if="item.unavailable_reason" class="recap-unavailable">{{ item.unavailable_reason }}</p>
              <footer><button :disabled="navigating" @click="open({ ...base(item.evidence[0].turn_id), kind: 'turn' })">关联回合</button>
                <button :disabled="navigating || !!item.unavailable_reason" @click="open({ ...base(item.evidence[0].turn_id), kind: 'effect', effect: item })">{{ item.effect.kind === 'presentation' ? '查看原版本' : item.object_id ? '定位成果' : '查看操作记录' }}</button></footer>
            </article>
          </section>
          <section aria-labelledby="recap-continuations"><h3 id="recap-continuations">待继续事项 <span>{{ recap.continuations.length }}</span></h3>
            <p v-if="!recap.continuations.length" class="recap-empty">没有明确待继续的任务或运行。</p>
            <article v-for="item in recap.continuations" :key="item.goal_id ?? item.evidence[0].turn_id">
              <p class="recap-text">{{ item.text }}</p><footer><span class="recap-status">{{ recapStatus(item.status) }}</span>
                <button :disabled="navigating" @click="open({ ...base(item.evidence[0].turn_id), kind: 'turn' })">回到{{ item.goal_id ? '任务' : '回合' }}</button></footer>
            </article>
          </section>
        </div>
      </section>
    </div>
  </Teleport>
</template>

<style scoped>
.recap-backdrop { position: fixed; inset: 0; z-index: 160; background: #0006; display: grid; place-items: center; padding: 20px; }
.session-recap { background: var(--canvas, #fff); color: var(--ink, #222); width: min(760px, 100%); max-height: calc(100dvh - 40px); overflow: auto; border: 1px solid var(--hairline, #ddd); border-radius: 16px; padding: 24px; box-sizing: border-box; }
header, .recap-range, footer { display: flex; justify-content: space-between; gap: 12px; align-items: center; }
header { position: sticky; top: -24px; padding: 12px 0; background: var(--canvas, #fff); z-index: 1; }
h2 { font-size: 22px; margin: 4px 0; } h3 { font-size: 17px; margin: 26px 0 12px; } h3 span { opacity: .55; font-size: 13px; margin-left: 8px; } h4, p { margin: 8px 0; }
.recap-kicker, small, .recap-hint, .recap-empty { font-size: 13px; color: var(--ink-secondary, #666); }
article { border-top: 1px solid var(--hairline, #ddd); padding: 14px 0; overflow-wrap: anywhere; }
.recap-text, blockquote { white-space: pre-wrap; } blockquote { margin: 12px 0; padding-left: 14px; border-left: 2px solid var(--hairline, #ddd); }
summary { cursor: pointer; line-height: 1.6; } footer { justify-content: flex-end; margin-top: 10px; flex-wrap: wrap; } footer .recap-status { margin-right: auto; }
.recap-status { display: inline-block; font-size: 12px; padding: 3px 8px; border-radius: 5px; background: var(--surface, #f2f3f0); }
button { font: inherit; font-size: 13px; color: inherit; background: transparent; border: 1px solid var(--hairline, #ccc); padding: 7px 10px; border-radius: 7px; cursor: pointer; flex-shrink: 0; } button:disabled { opacity: .45; cursor: default; }
.recap-error, .recap-unavailable { color: var(--danger, #9c3c30); font-size: 13px; }
@media (max-width: 600px) { .recap-backdrop { padding: 0; } .session-recap { width: 100%; height: 100dvh; max-height: 100dvh; border-radius: 0; padding: 16px; } header { top: -16px; } .recap-range { align-items: flex-start; flex-wrap: wrap; } }
</style>
