<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue';
import { ArrowLeft, ArrowRight, BookOpen, Check, ChevronRight, FileText, History, Plus, RefreshCw, Route, SlidersHorizontal, X } from '@lucide/vue';
import type { TutorSession } from '../generated/TutorSession';
import type { TutorState } from '../generated/TutorState';
import type { TutorAction } from '../generated/TutorAction';
import type { TutorSessionMode } from '../generated/TutorSessionMode';
import UnderstandingSpace from './UnderstandingSpace.vue';

const props = defineProps<{ state: TutorState | null; busy: boolean; error: string; pending: boolean; sourceId?: string; label: string;
  readiness?: { status: string; limitations: string[]; reason: string } | null; readinessError?: string; initialView?: 'current' | 'understanding' | 'history' }>();
const emit = defineEmits<{ (e: 'action', action: TutorAction): void; (e: 'retry'): void; (e: 'close'): void; (e: 'refreshReadiness'): void }>();
type View = 'current' | 'understanding' | 'history' | 'new';
const tabs = [{ id: 'current', label: '当前学习' }, { id: 'understanding', label: '我的理解' }, { id: 'history', label: '历史会话' }] as const;
const modes: { value: TutorSessionMode | null; label: string; description: string; icon: typeof BookOpen }[] = [
  { value: null, label: '按需调整', description: '跟随当前问题', icon: SlidersHorizontal },
  { value: 'direct_explanation', label: '直接讲解', description: '完整说明当前问题', icon: BookOpen },
  { value: 'guided_inquiry', label: '引导思考', description: '尝试、示范与按需帮助', icon: Route },
];
const suggestions = [
  { label: '讲清这一节', intent: '我想把当前这一节的核心概念真正弄明白。' },
  { label: '带我推导一遍', intent: '请带我一步步推导当前内容，让我能自己解释其中的逻辑。' },
  { label: '换个例子解释', intent: '请用一个具体例子，帮我理解当前内容。' },
];
const statusLabels = { active: '进行中', paused: '已暂停', ended: '已结束' };
const sessions = computed(() => Object.values(props.state?.sessions ?? {}).filter((session): session is TutorSession => !!session));
const currentSession = computed(() => props.state?.sessions[props.state.control.current_tutor_session_id ?? ''] ?? null);
const selectedId = ref<string | null>(currentSession.value?.id ?? null);
const selectedSession = computed(() => props.state?.sessions[selectedId.value ?? ''] ?? currentSession.value);
const history = computed(() => sessions.value.filter(session => session.id !== currentSession.value?.id || session.status !== 'active'));
const view = ref<View>(props.initialView ?? (currentSession.value ? 'current' : 'new'));
const intent = ref('');
const startRequested = ref<string | null>(null);
const resumeRequested = ref<string | null>(null);
const confirmEnd = ref(false);
const body = ref<HTMLElement>();
const closeButton = ref<HTMLButtonElement>();
const goalHeading = ref<HTMLElement>();
const origin = document.activeElement instanceof HTMLElement ? document.activeElement : null;
const locked = computed(() => props.busy || props.pending);
const ready = computed(() => props.readiness?.status === 'ready' && !props.readinessError);
const belongsToSource = (session: TutorSession) => !!props.sourceId && session.material_scope.some(material => material.source_id === props.sourceId);
const canResume = computed(() => !locked.value && props.state?.control.enabled && ready.value && selectedSession.value && belongsToSource(selectedSession.value));
const canStart = computed(() => !locked.value && props.state && props.sourceId && intent.value.trim());
const statusText = computed(() => !props.state ? '正在读取学习状态' : props.readinessError ? '学习状态读取失败'
  : !props.state.control.enabled ? '学习助手已关闭' : ready.value ? '学习助手已就绪' : '学习基础准备中');

function changeView(next: View) { view.value = next; confirmEnd.value = false; }
function openTab(next: View) {
  if (next === 'current') selectedId.value = currentSession.value?.id ?? null;
  changeView(next);
}
async function requestEnd() {
  confirmEnd.value = true;
  await nextTick();
  if (body.value) body.value.scrollTop = body.value.scrollHeight;
}
watch(view, async () => { await nextTick(); if (body.value) body.value.scrollTop = 0; });
watch(currentSession, (next, previous) => {
  if (next?.id !== previous?.id) selectedId.value = next?.id ?? null;
  // Navigate only after the server has confirmed the requested session.
  if (next && next.id !== previous?.id && next.user_intent === startRequested.value) {
    intent.value = ''; startRequested.value = null; changeView('current');
  }
  if (next?.id === resumeRequested.value && next?.status === 'active') {
    selectedId.value = next.id; resumeRequested.value = null; changeView('current');
  }
});
watch(() => props.state, (next, previous) => {
  if (!previous && next && currentSession.value && view.value === 'new' && !intent.value.trim()) changeView('current');
});
watch(selectedSession, () => { confirmEnd.value = false; });
function start() {
  if (!canStart.value) return;
  startRequested.value = intent.value.trim();
  emit('action', { kind: 'start', user_intent: startRequested.value, explicit_constraints: [],
    material_scope: [{ source_id: props.sourceId!, scope_refs: [], role: 'primary' }], default_teaching_intent: null });
}
function resume() {
  if (!canResume.value || !selectedSession.value) return;
  resumeRequested.value = selectedSession.value.id;
  emit('action', { kind: 'resume', session_id: selectedSession.value.id });
}
async function inspectSession(session: TutorSession) {
  selectedId.value = session.id; changeView('current');
  await nextTick(); goalHeading.value?.focus();
}
function tabKey(event: KeyboardEvent) {
  const index = tabs.findIndex(tab => tab.id === view.value);
  const offset = event.key === 'ArrowRight' ? 1 : event.key === 'ArrowLeft' ? -1 : 0;
  if (!offset && event.key !== 'Home' && event.key !== 'End') return;
  event.preventDefault();
  const next = event.key === 'Home' ? 0 : event.key === 'End' ? tabs.length - 1 : (index + offset + tabs.length) % tabs.length;
  openTab(tabs[next].id);
  (event.currentTarget as HTMLElement).querySelectorAll<HTMLButtonElement>('[role="tab"]')[next]?.focus();
}
function trapFocus(event: KeyboardEvent) {
  if (event.key !== 'Tab') return;
  const controls = [...(event.currentTarget as HTMLElement).querySelectorAll<HTMLElement>('button:not(:disabled), textarea:not(:disabled), input:not(:disabled), select:not(:disabled), summary, a[href], [tabindex="0"]')]
    .filter(element => element.tabIndex >= 0 && element.getClientRects().length);
  const first = controls[0], last = controls.at(-1);
  if (event.shiftKey && document.activeElement === first) { event.preventDefault(); last?.focus(); }
  else if (!event.shiftKey && document.activeElement === last) { event.preventDefault(); first?.focus(); }
}
onMounted(() => closeButton.value?.focus({ preventScroll: true }));
onBeforeUnmount(() => { if (origin?.isConnected) origin.focus({ preventScroll: true }); });
</script>

<template>
  <Teleport to="body">
    <div class="tutor-backdrop" @click.self="emit('close')">
      <section role="dialog" aria-modal="true" aria-label="学习详情" class="tutor-panel" @keydown.esc.stop.prevent="emit('close')" @keydown="trapFocus">
        <header class="tutor-header">
          <span class="tutor-brand"><BookOpen aria-hidden="true" /></span>
          <div><h2>学习详情</h2><p>从当前问题出发，按你的节奏学。</p></div>
          <div class="tutor-header-tools">
            <span class="tutor-status" role="status" :aria-label="label" data-testid="teaching-readiness" :data-ready="ready" :class="{ 'is-ready': ready && state?.control.enabled }"><span class="status-dot" />{{ statusText }}</span>
            <button class="icon-button refresh-button" aria-label="刷新学习状态" @click="emit('refreshReadiness')"><RefreshCw aria-hidden="true" /></button>
            <button ref="closeButton" class="icon-button" aria-label="关闭" @click="emit('close')"><X aria-hidden="true" /></button>
          </div>
        </header>
        <nav class="tutor-navigation" aria-label="学习会话导航">
          <div class="tutor-tabs" role="tablist" aria-label="学习内容" @keydown="tabKey">
            <button v-for="tab in tabs" :id="'tutor-tab-' + tab.id" :key="tab.id" class="tutor-tab" role="tab" :aria-selected="view === tab.id" :aria-controls="'tutor-' + tab.id" :tabindex="view === tab.id || (view === 'new' && tab.id === 'current') ? 0 : -1" @click="openTab(tab.id)">{{ tab.label }}</button>
          </div>
          <button class="new-learning" :aria-pressed="view === 'new'" @click="changeView('new')"><Plus aria-hidden="true" />新学习</button>
        </nav>
        <div ref="body" class="tutor-body">
          <div v-if="error || readinessError || !ready || readiness?.limitations?.length || (state && !state.control.enabled)" class="tutor-notices">
            <p v-if="error" role="alert">{{ error }} <button :disabled="busy" @click="emit('retry')">重试</button></p>
            <p v-if="readinessError" role="alert">{{ readinessError }} <button @click="emit('refreshReadiness')">重试学习状态</button></p>
            <p v-else-if="!ready">{{ readiness?.reason || '正在读取学习就绪状态' }}。等待期间可以先保存学习目标。</p>
            <p v-if="state && !state.control.enabled">学习助手已关闭。可以先保存目标，开启后继续学习。</p>
            <ul v-if="readiness?.limitations?.length"><li v-for="limitation in readiness.limitations" :key="limitation">{{ limitation }}</li></ul>
          </div>
          <section v-show="view === 'current'" id="tutor-current" role="tabpanel" aria-labelledby="tutor-tab-current" class="tutor-content">
            <template v-if="selectedSession">
              <div class="goal-eyebrow"><span>{{ selectedSession.id === currentSession?.id ? '当前目标' : '历史目标' }}</span><span class="session-status" :data-status="selectedSession.status"><span class="status-dot" />{{ statusLabels[selectedSession.status] }}</span></div>
              <div class="session-goal"><p class="material-label"><BookOpen aria-hidden="true" />{{ belongsToSource(selectedSession) ? '当前书籍' : '其他书籍' }}</p><h3 ref="goalHeading" tabindex="-1">{{ selectedSession.user_intent }}</h3><p>接着当前问题继续，也可以随时换一种讲法。</p></div>
              <p v-if="!belongsToSource(selectedSession)" class="scope-notice">请回到本次学习的书籍继续，或为当前书籍开始新目标。</p>
              <div class="teaching-section"><div class="section-label">本次学习的默认教法<span>随时可以调整</span></div><div class="teaching-modes" role="group" aria-label="默认教法">
                <button v-for="mode in modes" :key="mode.label" class="teaching-mode" :aria-pressed="selectedSession.default_teaching_intent === mode.value" :disabled="locked" @click="emit('action', { kind: 'set_mode', session_id: selectedSession.id, mode: mode.value })"><component :is="mode.icon" class="mode-icon" aria-hidden="true" /><strong>{{ mode.label }}</strong><small>{{ mode.description }}</small><span v-if="selectedSession.default_teaching_intent === mode.value" class="mode-check"><Check aria-hidden="true" /></span></button>
              </div><p class="teaching-note">引导思考时，你可以自己尝试，也可以请我示范一小步或补充解释。想先看完整说明，可以在对话中说“这次直接讲解”，只调整本轮。</p></div>
              <p class="source-note"><FileText aria-hidden="true" />从原文出发，结合你的回答继续讲解。</p>
            </template>
            <div v-else class="tutor-empty"><BookOpen aria-hidden="true" /><h3>从一个问题，开始新的理解</h3><p>写下这次想弄懂的内容，也可以回到阅读页直接提问。</p><button class="primary-action" @click="changeView('new')">新建学习目标<ArrowRight aria-hidden="true" /></button></div>
          </section>
          <section v-show="view === 'new'" id="tutor-new" class="tutor-content" aria-label="新的学习目标">
            <button v-if="currentSession" class="text-button back-button" @click="openTab('current')"><ArrowLeft aria-hidden="true" />返回当前学习</button>
            <div class="new-heading"><h3>这次，想弄懂什么？</h3><p>从一个问题开始，也可以描述你想达到的目标。</p></div>
            <form id="tutor-start-form" @submit.prevent="start"><label for="tutor-intent">你的学习目标</label><textarea id="tutor-intent" v-model="intent" rows="4" :disabled="locked" placeholder="例如：我想弄懂这一节，能用自己的话解释，再独立做一个例子。" /></form>
            <div class="intent-suggestions" aria-label="学习目标示例"><button v-for="suggestion in suggestions" :key="suggestion.label" :disabled="locked" @click="intent = suggestion.intent">{{ suggestion.label }}</button></div>
            <div class="learning-scope"><BookOpen aria-hidden="true" />学习范围<strong>{{ sourceId ? '当前书籍' : '请先打开一本书' }}</strong></div>
          </section>
          <section v-show="view === 'understanding'" id="tutor-understanding" role="tabpanel" aria-labelledby="tutor-tab-understanding" class="tutor-content"><UnderstandingSpace :key="sourceId" /></section>
          <section v-show="view === 'history'" id="tutor-history" role="tabpanel" aria-labelledby="tutor-tab-history" class="tutor-content">
            <div class="new-heading"><h3>每一次学习，都有来处</h3><p>回到之前的问题，继续还没结束的思考。</p></div>
            <div v-if="!history.length" class="tutor-empty"><History aria-hidden="true" /><h3>还没有历史会话</h3><p>暂停、结束或切换过的学习，会留在这里。</p></div>
            <button v-for="session in history" :key="session.id" class="history-session" @click="inspectSession(session)"><span class="history-icon"><BookOpen aria-hidden="true" /></span><span class="history-copy"><strong>{{ session.user_intent }}</strong><small>{{ statusLabels[session.status] }} · {{ belongsToSource(session) ? '当前书籍' : '其他书籍' }}<span v-if="session.id === currentSession?.id"> · 当前</span></small></span><ChevronRight aria-hidden="true" /></button>
          </section>
          <div v-if="confirmEnd && selectedSession" class="end-confirmation" role="group" aria-label="确认结束学习"><p>结束这次学习？记录仍会保留。</p><div><button :disabled="locked" @click="confirmEnd = false">取消</button><button class="primary-action" :disabled="locked" @click="emit('action', { kind: 'end', session_id: selectedSession.id })">结束学习</button></div></div>
        </div>
        <footer v-if="view === 'current' && selectedSession" class="tutor-footer"><div class="footer-copy">随时回来，接着往下学。<button v-if="selectedSession.status !== 'ended'" class="end-session" :disabled="locked" @click="requestEnd">结束这次学习</button></div><div class="footer-actions"><button v-if="selectedSession.status === 'active'" class="secondary-action" :disabled="locked" @click="emit('action', { kind: 'pause', session_id: selectedSession.id })">暂停</button><button class="primary-action" :disabled="!canResume" @click="resume">继续学习<ArrowRight aria-hidden="true" /></button></div></footer>
        <footer v-else-if="view === 'new'" class="tutor-footer"><span class="footer-copy">学习中可以随时调整讲法。</span><button class="primary-action" form="tutor-start-form" type="submit" :disabled="!canStart">{{ ready && state?.control.enabled ? '开始学习' : '保存学习意图' }}<ArrowRight aria-hidden="true" /></button></footer>
      </section>
    </div>
  </Teleport>
</template>

<style scoped>
.tutor-backdrop { position: fixed; inset: 0; z-index: 220; background: #24221d66; display: grid; place-items: center; padding: 24px; }
.tutor-panel { --tutor-muted: #77786e; --tutor-green: #56725a; --tutor-green-soft: #edf2e9; width: min(746px, 100%); max-height: min(850px, calc(100dvh - 48px)); min-height: 0; display: flex; flex-direction: column; border: 1px solid var(--line); border-radius: 22px; background: var(--reader-canvas, var(--canvas)); color: var(--reader-ink, var(--ink)); box-shadow: 0 24px 80px #241b1929, 0 2px 8px #241b1912; font: 14px/1.6 var(--sans); overflow: hidden; }
h2, h3, p { margin: 0; } svg { width: 18px; height: 18px; flex-shrink: 0; stroke-width: 1.7; }
button { display: inline-flex; align-items: center; justify-content: center; gap: 8px; color: inherit; border: 0; border-radius: 8px; background: transparent; padding: 0; min-height: 36px; font: inherit; cursor: pointer; }
button:disabled { opacity: .48; cursor: default; } button:focus-visible, textarea:focus-visible { outline: 2px solid var(--reader-coral); outline-offset: 3px; }
.tutor-header { display: flex; align-items: center; gap: 13px; padding: 25px 30px 22px; flex-shrink: 0; }
.tutor-brand { width: 44px; height: 44px; display: grid; place-items: center; background: var(--surface-soft); border: 1px solid var(--line); border-radius: 12px; color: var(--brand-green-deep); flex-shrink: 0; }.tutor-brand svg { width: 23px; height: 23px; }
h2 { font-size: 23px; line-height: 1.4; font-weight: 650; letter-spacing: -.5px; }.tutor-header p { font-size: 12px; color: var(--tutor-muted); margin-top: 3px; }
.tutor-header-tools { display: flex; align-items: center; gap: 5px; margin-left: auto; }.tutor-status { display: inline-flex; align-items: center; gap: 6px; color: var(--tutor-muted); font-size: 12px; margin-right: 5px; }.tutor-status.is-ready { color: var(--tutor-green); }.status-dot { width: 6px; height: 6px; border-radius: 50%; background: currentColor; flex-shrink: 0; }
.icon-button { width: 32px; height: 36px; color: var(--tutor-muted); }.icon-button:hover, .text-button:hover { background: var(--surface-soft); }.refresh-button svg { width: 15px; height: 15px; }
.tutor-navigation { display: flex; align-items: center; gap: 12px; margin: 0 30px; border-bottom: 1px solid var(--line); flex-shrink: 0; }.tutor-tabs { display: flex; gap: 27px; }.tutor-tab { position: relative; min-height: 49px; border-radius: 0; font-size: 13px; color: var(--tutor-muted); white-space: nowrap; }.tutor-tab[aria-selected=true] { color: var(--reader-ink); font-weight: 650; }.tutor-tab[aria-selected=true]::after { content: ''; position: absolute; bottom: -1px; left: 0; right: 0; height: 2px; background: var(--brand-green-deep); }
.new-learning { color: var(--brand-green-deep); font-size: 12px; font-weight: 600; margin-left: auto; padding: 5px 9px; white-space: nowrap; }.new-learning svg { width: 15px; height: 15px; }.new-learning:hover, .new-learning[aria-pressed=true] { background: var(--reader-coral-soft); }
.tutor-body { min-height: 0; overflow: auto; overscroll-behavior: contain; scrollbar-width: thin; }.tutor-content { padding: 27px 30px 29px; min-width: 0; overflow-wrap: anywhere; }
.tutor-notices { margin: 20px 30px 0; padding: 12px 15px; border: 1px solid var(--line); border-radius: 10px; background: var(--surface-soft); font-size: 12px; }.tutor-notices p + p, .tutor-notices ul { margin-top: 8px; }.tutor-notices ul { padding-left: 18px; margin-bottom: 0; }.tutor-notices button { color: var(--brand-green-deep); margin-left: 8px; font-size: 12px; }.tutor-notices [role=alert] { color: var(--brand-error); }
.goal-eyebrow { display: flex; align-items: center; justify-content: space-between; gap: 16px; color: var(--brand-green-deep); font-size: 11px; font-weight: 650; letter-spacing: 1px; }.session-status { display: inline-flex; align-items: center; gap: 5px; padding: 4px 9px; border-radius: 6px; color: var(--tutor-muted); background: var(--surface-soft); font-weight: 400; letter-spacing: 0; }.session-status[data-status=active] { color: var(--tutor-green); background: var(--tutor-green-soft); }.session-status .status-dot { width: 5px; height: 5px; }
.session-goal { padding: 20px 0 23px; }.session-goal .material-label { display: flex; align-items: center; gap: 7px; font-size: 12px; color: var(--tutor-muted); }.material-label svg { width: 14px; height: 14px; }.session-goal h3 { font-size: 30px; line-height: 1.45; font-weight: 650; margin: 12px 0; white-space: pre-wrap; }.session-goal h3:focus { outline: none; }.session-goal p { font-size: 13px; color: var(--tutor-muted); }
.scope-notice { background: var(--surface-soft); padding: 12px; margin-bottom: 18px; border-radius: 8px; font-size: 13px; }
.teaching-section { border-top: 1px solid var(--line); padding-top: 22px; }.section-label { display: flex; align-items: center; justify-content: space-between; font-size: 13px; font-weight: 600; margin-bottom: 12px; }.section-label span { color: var(--tutor-muted); font-size: 11px; font-weight: 400; }.teaching-modes { display: grid; grid-template-columns: repeat(3, minmax(0, 1fr)); gap: 10px; }.teaching-mode { position: relative; min-width: 0; flex-direction: column; align-items: flex-start; gap: 3px; padding: 15px 13px 13px; border: 1px solid var(--line); border-radius: 11px; text-align: left; background: var(--reader-card); }.teaching-mode .mode-icon { color: var(--tutor-muted); margin-bottom: 8px; }.teaching-mode strong { font-size: 13px; font-weight: 600; }.teaching-mode small { font-size: 11px; color: var(--tutor-muted); }.teaching-mode[aria-pressed=true] { background: var(--reader-coral-soft); border-color: var(--brand-green-deep); }.teaching-mode[aria-pressed=true] .mode-icon { color: var(--brand-green-deep); }.mode-check { position: absolute; right: 11px; top: 13px; display: grid; place-items: center; width: 15px; height: 15px; background: var(--brand-green-deep); color: var(--reader-card); border-radius: 50%; }.mode-check svg { width: 10px; height: 10px; }
.source-note { display: flex; align-items: center; gap: 7px; margin-top: 19px; color: var(--tutor-muted); font-size: 11px; }.source-note svg { width: 14px; height: 14px; }
.teaching-note { margin-top: 12px; color: var(--tutor-muted); font-size: 12px; }
.tutor-footer { display: flex; align-items: center; justify-content: space-between; gap: 16px; padding: 20px 30px; border-top: 1px solid var(--line); background: var(--reader-card); flex-shrink: 0; }.footer-copy { color: var(--tutor-muted); font-size: 11px; }.end-session { display: flex; justify-content: flex-start; color: var(--tutor-muted); font-size: 11px; min-height: 27px; }.end-session:hover { color: var(--brand-green-deep); }.footer-actions { display: flex; gap: 10px; }.primary-action { min-height: 44px; padding: 10px 23px; background: var(--brand-green-deep); color: var(--reader-card); border: 1px solid var(--brand-green-deep); border-radius: 9px; white-space: nowrap; font-size: 13px; font-weight: 600; }.primary-action:hover:not(:disabled) { filter: brightness(.95); }.secondary-action { min-height: 44px; padding: 9px 18px; border: 1px solid var(--line); background: var(--reader-card); font-size: 13px; }.end-confirmation { margin: 0 30px 20px; padding: 14px; background: var(--reader-coral-soft); border-radius: 10px; display: flex; flex-wrap: wrap; align-items: center; justify-content: space-between; gap: 12px; font-size: 13px; }.end-confirmation > div { display: flex; gap: 16px; }
.text-button { min-height: 32px; gap: 6px; font-size: 12px; color: var(--tutor-muted); }.text-button svg { width: 15px; height: 15px; }.new-heading { margin: 8px 0 26px; }.new-heading h3 { font-size: 24px; line-height: 1.45; font-weight: 650; margin-bottom: 7px; }.new-heading p { color: var(--tutor-muted); font-size: 13px; }
form label { display: block; font-size: 13px; font-weight: 600; }textarea { display: block; width: 100%; padding: 17px; margin-top: 10px; min-height: 137px; background: var(--reader-card); color: var(--reader-ink); border: 1px solid var(--line); border-radius: 12px; font: inherit; font-size: 14px; line-height: 1.8; resize: vertical; }textarea::placeholder { color: var(--tutor-muted); }.intent-suggestions { display: flex; flex-wrap: wrap; gap: 8px; margin-top: 12px; }.intent-suggestions button { border: 1px solid var(--line); background: var(--reader-card); border-radius: 7px; padding: 6px 10px; color: var(--tutor-muted); font-size: 11px; min-height: 32px; }.intent-suggestions button:hover:not(:disabled) { color: var(--brand-green-deep); border-color: var(--brand-green-deep); }.learning-scope { display: flex; align-items: center; gap: 9px; margin-top: 27px; padding: 13px 0; border-top: 1px solid var(--line); color: var(--tutor-muted); font-size: 12px; }.learning-scope strong { font-weight: 500; margin-left: auto; color: var(--reader-ink); }
.history-session { width: 100%; justify-content: flex-start; text-align: left; gap: 14px; padding: 18px 0; border-top: 1px solid var(--line); border-radius: 0; }.history-icon { width: 36px; height: 40px; display: grid; place-items: center; background: var(--surface-soft); color: var(--brand-green-deep); border: 1px solid var(--line); border-radius: 9px; flex-shrink: 0; }.history-copy { flex: 1; min-width: 0; }.history-copy strong { display: block; font-size: 14px; font-weight: 600; white-space: pre-wrap; }.history-copy small { display: block; font-size: 12px; color: var(--tutor-muted); margin-top: 5px; }.history-session > svg { color: var(--tutor-muted); width: 15px; }.history-session:hover strong { color: var(--brand-green-deep); }
.tutor-empty { text-align: center; padding: 36px 8px; }.tutor-empty > svg { width: 32px; height: 32px; color: var(--brand-green-deep); margin-bottom: 12px; }.tutor-empty h3 { font-size: 19px; font-weight: 600; margin-bottom: 8px; }.tutor-empty p { font-size: 13px; color: var(--tutor-muted); }.tutor-empty button { margin-top: 20px; }
@media (max-width: 600px) {
  .tutor-backdrop { padding: 10px; }.tutor-panel { max-height: calc(100dvh - 20px); border-radius: 18px; }.tutor-header { padding: 20px 20px 16px; gap: 10px; }.tutor-brand { width: 38px; height: 40px; }h2 { font-size: 21px; }.tutor-header p { font-size: 11px; }.tutor-status { max-width: 76px; font-size: 11px; }.tutor-header-tools { gap: 0; }.tutor-navigation { margin: 0 20px; gap: 5px; }.tutor-tabs { gap: 18px; }.new-learning { padding: 5px; gap: 3px; }.tutor-content { padding: 22px 20px 25px; }.tutor-notices { margin: 16px 20px 0; }.session-goal h3 { font-size: 27px; }.teaching-mode { padding: 13px 10px 11px; }.tutor-footer { padding: 17px 20px; flex-wrap: wrap; gap: 12px; }.primary-action { padding: 10px 17px; }.end-confirmation { margin-inline: 20px; }textarea { font-size: 16px; }
}
@media (max-width: 420px) {
  .tutor-header { flex-wrap: wrap; }.tutor-header-tools { margin-left: auto; }.tutor-status { display: none; }.tutor-header:has(.tutor-status:not(.is-ready)) .tutor-status { display: inline-flex; }.tutor-header:has(.tutor-status:not(.is-ready)) .tutor-header-tools { width: 100%; }.tutor-header:has(.tutor-status:not(.is-ready)) .tutor-status { max-width: none; margin-right: auto; }.tutor-header:has(.tutor-status:not(.is-ready)) .refresh-button { margin-left: auto; }.tutor-tabs { gap: 15px; }.tutor-tab { font-size: 12px; }.new-learning { font-size: 11px; }.tutor-content { padding-inline: 16px; }.tutor-navigation { margin-inline: 16px; }.tutor-footer { padding-inline: 16px; }.tutor-notices { margin-inline: 16px; }.teaching-modes { grid-template-columns: 1fr; }.teaching-mode { display: grid; grid-template-columns: 23px 1fr auto; align-items: center; gap: 8px; padding: 13px 12px; }.teaching-mode .mode-icon { margin: 0; }.mode-check { display: none; }.footer-actions { margin-left: auto; }.tutor-footer .footer-copy { width: 100%; }.tutor-footer .footer-actions { width: 100%; }.footer-actions .primary-action { flex: 1; }.end-session { min-height: 30px; }.section-label span { display: none; }
}
@media (pointer: coarse) { .icon-button { min-width: 36px; min-height: 44px; }.text-button, .end-session, .intent-suggestions button { min-height: 44px; } }
</style>
