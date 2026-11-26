<script setup lang="ts">
import { ArrowUpRight, CircleHelp, Check, PencilLine } from '@lucide/vue';
import { computed, nextTick, ref } from 'vue';
import { thinkingTime, type UnderstandingSummary } from '../note-thinking';
import { readerKey } from '../reader-context';
import UnderstandingQuote from './UnderstandingQuote.vue';
import { t as uiText } from '../i18n';
const props = defineProps<{ item: UnderstandingSummary; compact?: boolean; busy?: boolean; noteId?: string; showQuote?: boolean }>();
const summary = computed(() => props.item.correction ? null : props.item.learner_summary);
function showOnTree(){const reference=props.item.knowledge_links?.[0]?.reference;if(reference)window.dispatchEvent(new CustomEvent('knowledge-show',{detail:{reference,reader_key:readerKey()}}));}
const emit = defineEmits<{ evidence: []; correct: []; discuss: [message: string]; practice: []; note: [reference: import('../generated/NoteRef').NoteRef] }>();
const editing = ref(false), message = ref('');
const editor = ref<HTMLTextAreaElement>(), discussButton = ref<HTMLButtonElement>();
async function editDiscussion() {
  editing.value = true;
  await nextTick();
  editor.value?.focus();
}
async function cancelDiscussion() {
  editing.value = false;
  message.value = '';
  await nextTick();
  discussButton.value?.focus();
}
function sendDiscussion() {
  if (!editing.value || props.busy) return;
  editing.value = false;
  emit('discuss', message.value.trim());
}
const capability: Record<string, string> = { explanation: '解释', application: '应用', relation: '关系理解', transfer: '迁移', evaluation: '来源辨析', production: '构造', recall: '回忆', recognition: '识别', calculation: '计算' };
</script>

<template>
  <article class="understanding-summary" :class="{ compact }" :aria-label="item.label">
    <header><div><span class="summary-eyebrow">{{ item.later_evidence_ref ? '先前记录' : summary && item.previous ? '这次理解的变化' : '这一步，你的理解' }}</span><h3>{{ summary?.headline || item.label }}</h3></div><span v-if="item.correction" class="summary-corrected">已纠正</span></header>
    <p v-if="item.later_evidence_ref" class="summary-conditions">这项理解已有新的表达。这里保留当时的原话与帮助，最新判断见“我的理解”。</p>
    <p v-if="summary" class="summary-explanation">{{ summary.explanation }}</p>
    <div class="summary-parts">
      <section v-if="!summary && item.demonstrated.length" class="demonstrated"><h4><Check :size="14" />{{ item.later_evidence_ref ? '当时已表达' : '已经说清楚' }}</h4><p>{{ item.demonstrated[0] }}</p></section>
      <section v-if="item.difficulties.length" class="difficulties"><h4><PencilLine :size="14" />{{ item.later_evidence_ref ? '当时卡点' : '当前卡点' }}</h4><p>{{ item.difficulties[0] }}</p></section>
      <section v-if="!summary && !item.demonstrated.length && !item.difficulties.length && item.unconfirmed.length" class="unconfirmed"><h4><CircleHelp :size="14" />{{ item.later_evidence_ref ? '当时待确认' : '还待观察' }}</h4><p>{{ item.unconfirmed[0] }}</p></section>
    </div>
    <p v-if="compact && item.note_refs.some(r=>r.note_id===noteId)" class="summary-conditions">依据这条想法第 {{item.note_refs.filter(r=>r.note_id===noteId).map(r=>r.revision).join('、')}} 版的相关表达</p>
    <p v-if="compact" class="summary-conditions">{{item.assistance}}</p>
    <p v-if="item.source_session_title" class="summary-conditions">{{ uiText('关联对话') }} · {{item.source_session_title}}</p>
    <template v-if="!compact">
      <p v-if="item.correction" class="summary-correction">你的补充：{{ item.correction }}</p>
      <p class="summary-conditions">{{ item.assistance }} · {{ thinkingTime(item.occurred_at) }}</p>
      <details class="summary-basis" :open="showQuote">
        <summary>{{ item.previous ? '查看前后原话与依据' : '查看理解依据' }}</summary>
        <div class="summary-comparison">
          <div v-if="item.previous"><span>之前的表达</span><UnderstandingQuote :quote="item.previous.learner_quote" :choice="item.previous.choice_answer" /><small>{{ item.previous.assistance }}</small></div>
          <div><span>{{ item.previous ? '这次的表达' : '你的原话' }}</span><UnderstandingQuote :quote="item.learner_quote" :choice="item.choice_answer" /><small>{{ item.assistance }}</small></div>
        </div>
        <div v-if="item.unconfirmed.length" class="summary-unconfirmed"><strong>还没有观察到的表现</strong><p v-for="text in item.unconfirmed" :key="text">{{ text }}</p></div><button :disabled="busy" @click="$emit('evidence')">查看完整记录与帮助</button><button v-if="item.knowledge_links?.length" @click="showOnTree">在理解地图中查看</button><button v-if="item.target" :disabled="busy" @click="$emit('practice')">{{ uiText('换个例子练习') }}</button>
      </details>
      <p v-if="item.next_step" class="summary-next">{{ item.later_evidence_ref ? '当时的建议：' : '接下来：' }}{{ item.next_step }}</p>
      <div v-if="item.note_refs.length" class="summary-notes"><span>关联想法</span><button v-for="(reference,index) in item.note_refs" :key="`${reference.note_id}:${reference.revision}`" @click="$emit('note',reference)">{{ reference.note_id === noteId ? '这条想法' : `想法 ${index+1}` }} · 第 {{ reference.revision }} 版</button></div>
    </template>
    <form v-if="editing" class="discussion-draft" @submit.prevent="sendDiscussion">
      <label>{{ uiText('从这条理解继续讨论') }}<textarea ref="editor" v-model="message" rows="2" :placeholder="uiText('可以补充想问的问题')" @keydown.esc.stop.prevent="cancelDiscussion" /></label>
      <div><button type="submit" class="summary-primary" :disabled="busy">{{ uiText('发送到原讨论') }}</button><button type="button" :disabled="busy" @click="cancelDiscussion">{{ uiText('取消') }}</button></div>
    </form>
    <footer><button v-if="item.target && !editing" ref="discussButton" class="summary-primary" :disabled="busy" @click="editDiscussion">{{ uiText('接着这一点') }}<ArrowUpRight :size="14" /></button><button v-if="compact" :disabled="busy" @click="$emit('evidence')">查看依据</button><button v-if="!compact" :disabled="busy" @click="$emit('correct')">我的意思不是这样</button></footer>
  </article>
</template>

<style scoped>
.discussion-draft { margin-top: 14px; }
.discussion-draft label { display: grid; gap: 6px; font-weight: 600; }
.discussion-draft textarea { box-sizing: border-box; width: 100%; padding: 9px; border: 1px solid var(--line); border-radius: 8px; background: var(--canvas); color: inherit; font: inherit; resize: vertical; }
.discussion-draft > div { display: flex; gap: 7px; margin-top: 8px; }
.summary-explanation{font-size:15px;line-height:1.85;margin:0 0 14px}.summary-comparison{display:grid;grid-template-columns:repeat(auto-fit,minmax(min(220px,100%),1fr));gap:18px;margin:14px 0}.summary-comparison span,.summary-comparison small{font-size:12px;color:var(--muted,#79716a)}.summary-unconfirmed{margin:12px 0;padding-top:12px;border-top:1px solid var(--line,#e5dfd7);font-size:12px}.summary-unconfirmed p{margin:5px 0}
.understanding-summary{border:1px solid var(--border,#e5dfd7);border-radius:14px;background:var(--surface,#fffdf9);padding:18px;color:var(--text,#302d29);font-size:13px;line-height:1.65;overflow-wrap:anywhere}
header{display:flex;align-items:flex-start;justify-content:space-between;gap:12px}h3{font-size:15px;line-height:1.5;margin:5px 0 14px;font-weight:650}.summary-eyebrow,.summary-conditions,.summary-notes{font-size:11px;color:var(--steel,#79716a)}.summary-corrected{font-size:11px;white-space:nowrap;color:var(--accent,#a95028)}
.summary-parts{display:grid;gap:10px}.summary-parts section{padding-left:12px;border-left:2px solid #d9d3c9}.summary-parts h4{display:flex;align-items:center;gap:6px;margin:0 0 3px;font-size:12px;font-weight:600}.summary-parts p{margin:0}.demonstrated h4{color:#4e7867}.difficulties h4{color:var(--accent,#a95028)}.summary-parts .demonstrated{border-color:#a7c5b4}.summary-parts .difficulties{border-color:#d7a184}.unconfirmed h4{color:var(--steel,#79716a)}
.summary-conditions{margin:12px 0 8px}blockquote{margin:8px 0;padding:10px 12px;border-left:2px solid var(--border,#e5dfd7);white-space:pre-wrap;background:var(--bg,#f7f4ee)}.summary-basis summary{cursor:pointer;color:var(--steel,#79716a);font-size:12px}.summary-next{margin:14px 0 8px}.summary-correction{color:var(--accent,#a95028)}.summary-notes{display:flex;flex-wrap:wrap;gap:6px;align-items:center}.summary-notes button{font-size:11px}
footer{display:flex;flex-wrap:wrap;gap:7px;margin-top:14px}button{display:inline-flex;align-items:center;gap:4px;font:inherit;font-size:12px;border:1px solid var(--border,#e5dfd7);border-radius:8px;padding:5px 9px;background:transparent;color:inherit;cursor:pointer}button:hover{background:var(--bg,#f7f4ee)}button:disabled{opacity:.5;cursor:wait}.summary-primary{color:var(--accent,#a95028)}.compact{padding:13px 15px}.compact h3{font-size:13px;margin-bottom:10px}.compact .summary-parts{gap:6px}.compact footer{margin-top:10px}
@media(max-width:400px){.understanding-summary{padding:13px}.summary-eyebrow{font-size:10px}footer{gap:6px}footer button{padding:6px 8px}}
</style>
