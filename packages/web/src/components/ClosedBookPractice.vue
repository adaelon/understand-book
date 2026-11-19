<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue';
import { ArrowUp, BookOpen, CircleHelp, Pause, RotateCcw } from '@lucide/vue';
import { renderMarkdown } from '../md';
import { t } from '../i18n';
import type { ClosedBookPractice } from '../generated/ClosedBookPractice';
import LoadingAnimation from './LoadingAnimation.vue';

const props = defineProps<{ state: ClosedBookPractice | null; restoring: boolean; busy: boolean; error: string;
  messages: { id: string; user: string; answer: string }[]; draft: string }>();
const emit = defineEmits<{ send: [text: string]; help: [kind: 'hint' | 'explain']; leave: [kind: 'pause' | 'exit'];
  retry: []; resume: []; 'update:draft': [text: string] }>();
const dialog = ref<HTMLDialogElement>();
const input = ref<HTMLTextAreaElement>();
const transcript = ref<HTMLElement>();
const complete = computed(() => props.state?.status === 'complete');
const paused = computed(() => props.state?.status === 'paused');
const practiceVisible = computed(() => props.state?.status === 'active' || paused.value || complete.value);
const shown = computed(() => props.restoring || practiceVisible.value);
// A closed-book answer can name a source, but cannot open links or embed an old page.
function markdown(text: string) {
  const html = document.createElement('div'); html.innerHTML = renderMarkdown(text);
  for (const link of html.querySelectorAll('a')) link.replaceWith(document.createTextNode(link.textContent ?? ''));
  for (const media of html.querySelectorAll('img,iframe,video,audio')) media.remove();
  return html.innerHTML;
}
async function updateDialog() {
  await nextTick();
  if (shown.value && !dialog.value?.open) dialog.value?.showModal();
  else if (!shown.value && dialog.value?.open) dialog.value.close();
}
watch(shown, updateDialog); onMounted(updateDialog);
onBeforeUnmount(() => dialog.value?.close());
watch(() => [props.messages.length, props.state?.attempt, props.busy], async () => {
  await nextTick(); if (transcript.value) transcript.value.scrollTop = transcript.value.scrollHeight;
  if (!props.busy && !complete.value) input.value?.focus({ preventScroll: true });
});
function submit() { if (!props.busy && props.draft.trim()) emit('send', props.draft.trim()); }
</script>

<template>
  <Teleport to="body">
    <dialog ref="dialog" class="closed-book-dialog" :aria-labelledby="practiceVisible ? 'closed-book-title' : undefined" :aria-label="practiceVisible ? undefined : t('正在加载')" @cancel.prevent="practiceVisible && !restoring && emit('leave', 'pause')">
      <div v-if="restoring && !practiceVisible" class="closed-book-loading">
        <LoadingAnimation v-if="!error" />
        <div v-else class="closed-book-error" role="alert"><p>{{ t('暂时无法恢复学习，请重试。') }}</p><p>{{ error }}</p><button :disabled="busy" @click="emit('retry')"><RotateCcw :size="14" />{{ t('重试') }}</button></div>
      </div>
      <div v-else-if="practiceVisible" class="closed-book-shell">
        <header class="closed-book-header">
          <div><p class="closed-book-eyebrow">{{ t('闭卷对话') }}</p><h1 id="closed-book-title">{{ t('练到会') }}</h1></div>
          <button v-if="!paused && !complete" class="closed-book-leave" :disabled="restoring" @click="emit('leave', 'pause')"><Pause :size="15" />{{ t('暂停') }}</button>
        </header>
        <main ref="transcript" class="closed-book-transcript" aria-live="polite">
          <p v-if="restoring" role="status" class="closed-book-empty">{{ t(error ? '暂时无法恢复练习，资料仍保持收起。' : '正在恢复练习…') }}</p>
          <template v-else-if="state">
            <div class="closed-book-focus"><span>{{ t('这一轮') }}</span><h2>{{ state.target }}</h2><p>{{ t('用自己的话说清楚。卡住时，可以向我求助。') }}</p></div>
            <div v-if="paused" class="closed-book-paused"><strong>{{ t('已暂停') }}</strong><p>{{ t(state.can_resume ? '问题和草稿已保留，准备好后可以接着回答。' : '学习目标已变化，可以为当前内容开始新练习。') }}</p><button :disabled="busy" @click="emit('resume')">{{ t(state.can_resume ? '继续练习' : '开始当前目标') }}</button><button :disabled="busy" @click="emit('leave', 'exit')">{{ t('返回看资料') }}</button></div>
            <p v-if="!messages.length && !busy && !error" class="closed-book-empty">{{ t('准备好后，开始第一个问题。') }}</p>
            <article v-for="(message, index) in messages" :key="message.id" class="closed-book-exchange">
              <div v-if="index > 0 && message.user" class="closed-book-response"><span>{{ t('你的回答') }}</span><div v-html="markdown(message.user)" /></div>
              <div class="closed-book-question" v-html="markdown(message.answer)" />
            </article>
            <div v-if="complete" class="closed-book-complete"><strong>{{ t('这一轮，已经能独立完成') }}</strong><p>{{ t(state.course ? '把这次作答交回本课，核对已完成的要求和接下来的一步。' : '当前目标已完成，可以回到刚才的学习位置。') }}</p><button :disabled="busy" @click="emit('leave', 'exit')">{{ t(state.course ? '回到本课' : '回到学习') }}</button></div>
            <p v-if="busy" class="closed-book-wait" role="status"><span />{{ t('正在准备回应…') }}</p>
          </template>
          <div v-if="error" class="closed-book-error" role="alert"><p>{{ error }}</p><button :disabled="busy" @click="emit('retry')"><RotateCcw :size="14" />{{ t('重试') }}</button></div>
        </main>
        <footer v-if="state && !complete && !paused && !restoring" class="closed-book-footer">
          <div class="closed-book-support"><button :disabled="busy" @click="emit('help', 'hint')"><CircleHelp :size="15" />{{ t('给一点提示') }}</button><button :disabled="busy" @click="emit('help', 'explain')">{{ t('这一步再讲讲') }}</button><button :disabled="busy" @click="emit('leave', 'exit')"><BookOpen :size="15" />{{ t('返回看资料') }}</button></div>
          <form class="closed-book-composer" @submit.prevent="submit">
            <textarea ref="input" :value="draft" :disabled="busy" :aria-label="t('你的回答')" :placeholder="t('写下你的理解、解题思路，或者卡住的地方…')" rows="3" @input="emit('update:draft', ($event.target as HTMLTextAreaElement).value)" @keydown.ctrl.enter.prevent="submit" @keydown.meta.enter.prevent="submit" />
            <button :disabled="busy || !draft.trim()" :aria-label="t('发送回答')"><ArrowUp :size="18" /></button>
          </form>
          <div class="closed-book-footnote"><span>{{ t(state.assisted ? '这次已获得帮助，之后再独立试一次。' : '这里只保留当前问题和对话。') }}</span><button v-if="!messages.length && !busy" @click="emit('retry')">{{ t('开始提问') }}</button><button v-else-if="state.assisted && !busy" @click="emit('send', t('换一个同目标的新问题，让我独立试一次。'))">{{ t('独立试一次 →') }}</button></div>
        </footer>
      </div>
    </dialog>
  </Teleport>
</template>

<style scoped>
.closed-book-dialog{position:fixed;inset:0;margin:0;width:100vw;height:100dvh;max-width:none;max-height:none;border:0;outline:none;padding:0;background:var(--canvas,#faf9f6);color:var(--ink,#292622);font-family:var(--sans,system-ui);overflow:hidden}
.closed-book-dialog::backdrop{background:var(--canvas,#faf9f6)}
.closed-book-loading{height:100%;display:grid;place-items:center;padding:24px;box-sizing:border-box}.closed-book-loading>.closed-book-error{max-width:480px;overflow-wrap:anywhere}
.closed-book-shell{height:100%;max-width:820px;margin:auto;display:flex;flex-direction:column;padding:0 32px;box-sizing:border-box}
.closed-book-header{display:flex;align-items:center;justify-content:space-between;padding:24px 0 18px;border-bottom:1px solid var(--hairline,#e5e2da)}
.closed-book-eyebrow{font-size:11px;letter-spacing:.12em;color:var(--steel,#77736b);margin:0 0 5px}.closed-book-header h1{font-size:20px;margin:0;font-weight:600}
button{display:inline-flex;gap:6px;align-items:center;justify-content:center;font:inherit;cursor:pointer;border:1px solid var(--hairline,#ddd8cf);border-radius:8px;padding:8px 12px;background:transparent;color:inherit}button:disabled{opacity:.45;cursor:default}button:hover:not(:disabled){background:var(--paper,#eeece6)}button:focus-visible,textarea:focus-visible{outline:2px solid var(--brand-accent,#6b8065);outline-offset:3px}
.closed-book-leave{font-size:13px}.closed-book-transcript{flex:1;min-height:0;overflow-y:auto;padding:28px 3px 20px;scrollbar-gutter:stable}.closed-book-focus{margin-bottom:30px}.closed-book-focus>span{font-size:11px;color:var(--steel,#77736b)}.closed-book-focus h2{font-size:21px;line-height:1.5;margin:8px 0;font-weight:550}.closed-book-focus p,.closed-book-empty{color:var(--steel,#77736b);font-size:13px;line-height:1.7}
.closed-book-exchange{font-size:16px;line-height:1.9;margin-bottom:28px;overflow-wrap:anywhere}.closed-book-response{background:var(--paper,#f0eee7);border-radius:12px;padding:14px 18px;margin:0 0 22px 40px}.closed-book-response>span{font-size:11px;color:var(--steel,#77736b)}.closed-book-exchange :deep(p){margin:8px 0}.closed-book-exchange :deep(pre){overflow:auto;padding:12px;background:var(--paper,#f0eee7);border-radius:8px}.closed-book-exchange :deep(.katex-display){overflow:auto}
.closed-book-footer{border-top:1px solid var(--hairline,#e5e2da);padding:14px 0 22px}.closed-book-support{display:flex;gap:8px;flex-wrap:wrap;margin-bottom:12px}.closed-book-support button{font-size:12px;padding:6px 9px;border:0}.closed-book-support button:last-child{margin-left:auto}
.closed-book-composer{position:relative;border:1px solid var(--hairline,#d9d4c9);border-radius:12px;background:var(--paper,#fff)}.closed-book-composer textarea{width:100%;box-sizing:border-box;resize:vertical;max-height:28vh;border:0;background:transparent;color:inherit;padding:15px 58px 15px 16px;line-height:1.7;font:inherit;outline:none}.closed-book-composer button{position:absolute;bottom:12px;right:12px;border:0;border-radius:9px;background:var(--ink,#292622);color:var(--canvas,#fff);padding:9px}
.closed-book-footnote{font-size:11px;color:var(--steel,#77736b);margin-top:10px;display:flex;justify-content:space-between;gap:10px;align-items:center}.closed-book-footnote button{flex-shrink:0;white-space:nowrap;font-size:12px;border:0;padding:2px}.closed-book-error{padding:12px 16px;border:1px solid #c97a62;border-radius:9px;font-size:13px}.closed-book-wait{font-size:13px;color:var(--steel,#77736b);display:flex;align-items:center;gap:9px}.closed-book-wait span{width:7px;height:7px;border-radius:50%;background:currentColor}.closed-book-complete{padding:20px;border-left:3px solid var(--brand-accent,#6b8065);background:var(--paper,#f0eee7);border-radius:4px;font-size:14px;line-height:1.8}
.closed-book-paused{margin-bottom:24px;padding:20px;background:var(--paper,#f0eee7);border-radius:12px;font-size:14px;line-height:1.8}.closed-book-paused button{margin-right:10px}
@media(max-width:600px){.closed-book-shell{padding:0 18px}.closed-book-header{padding:16px 0 12px}.closed-book-transcript{padding-top:20px}.closed-book-focus h2{font-size:19px}.closed-book-exchange{font-size:15px}.closed-book-response{margin-left:16px}.closed-book-footer{padding-bottom:max(12px,env(safe-area-inset-bottom))}.closed-book-support{gap:2px}.closed-book-support button{font-size:11px;padding:6px}.closed-book-composer textarea{font-size:16px}}
</style>
