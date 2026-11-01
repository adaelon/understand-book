<script setup lang="ts">
import { ArrowUpRight, CircleHelp, Check, PencilLine } from '@lucide/vue';
import { thinkingTime, type UnderstandingSummary } from '../note-thinking';
const props = defineProps<{ item: UnderstandingSummary; compact?: boolean; busy?: boolean; noteId?: string }>();
defineEmits<{ evidence: []; correct: []; discuss: []; practice: []; note: [reference: import('../generated/NoteRef').NoteRef] }>();
const capability: Record<string, string> = { explanation: '解释', application: '应用', relation: '关系理解', transfer: '迁移', evaluation: '来源辨析', production: '构造', recall: '回忆', recognition: '识别', calculation: '计算' };
</script>

<template>
  <article class="understanding-summary" :class="{ compact }" :aria-label="item.label">
    <header><div><span class="summary-eyebrow">{{ capability[item.capability] || '理解观察' }} · {{ item.nature === 'performance' ? '本次活动表现' : '系统暂定判断' }}</span><h3>{{ item.label }}</h3></div><span v-if="item.correction" class="summary-corrected">已纠正</span></header>
    <div class="summary-parts">
      <section v-if="item.demonstrated.length" class="demonstrated"><h4><Check :size="14" />已能做到</h4><p v-for="text in item.demonstrated" :key="text">{{ text }}</p></section>
      <section v-if="item.difficulties.length" class="difficulties"><h4><PencilLine :size="14" />当前卡点</h4><p v-for="text in item.difficulties" :key="text">{{ text }}</p></section>
      <section v-if="item.unconfirmed.length" class="unconfirmed"><h4><CircleHelp :size="14" />还待确认</h4><p v-for="text in item.unconfirmed" :key="text">{{ text }}</p></section>
    </div>
    <p v-if="compact && item.note_refs.some(r=>r.note_id===noteId)" class="summary-conditions">依据这条想法第 {{item.note_refs.filter(r=>r.note_id===noteId).map(r=>r.revision).join('、')}} 版的相关表达</p>
    <p v-if="compact" class="summary-conditions">{{item.assistance}}</p>
    <p v-if="item.source_session_title" class="summary-conditions">原讨论 · {{item.source_session_title}}</p>
    <template v-if="!compact">
      <p v-if="item.correction" class="summary-correction">你的补充：{{ item.correction }}</p>
      <p class="summary-conditions">{{ item.assistance }} · {{ thinkingTime(item.occurred_at) }}</p>
      <details class="summary-basis"><summary>最近依据</summary><blockquote>{{ item.learner_quote || '此记录未保留原话' }}</blockquote><button :disabled="busy" @click="$emit('evidence')">查看原话、活动与帮助</button></details>
      <p v-if="item.next_step" class="summary-next">接下来：{{ item.next_step }}</p>
      <div v-if="item.note_refs.length" class="summary-notes"><span>关联想法</span><button v-for="(reference,index) in item.note_refs" :key="`${reference.note_id}:${reference.revision}`" @click="$emit('note',reference)">{{ reference.note_id === noteId ? '这条想法' : `想法 ${index+1}` }} · 第 {{ reference.revision }} 版</button></div>
    </template>
    <footer><button v-if="item.target && (noteId || item.note_refs.length)" class="summary-primary" :disabled="busy" @click="$emit('discuss')">继续推敲<ArrowUpRight :size="14" /></button><button v-if="item.target && (noteId || item.note_refs.length)" :disabled="busy" @click="$emit('practice')">练一练</button><button :disabled="busy" @click="$emit('evidence')">查看依据</button><button v-if="!compact" :disabled="busy" @click="$emit('correct')">纠正判断</button></footer>
  </article>
</template>

<style scoped>
.understanding-summary{border:1px solid var(--border,#e5dfd7);border-radius:14px;background:var(--surface,#fffdf9);padding:18px;color:var(--text,#302d29);font-size:13px;line-height:1.65;overflow-wrap:anywhere}
header{display:flex;align-items:flex-start;justify-content:space-between;gap:12px}h3{font-size:15px;line-height:1.5;margin:5px 0 14px;font-weight:650}.summary-eyebrow,.summary-conditions,.summary-notes{font-size:11px;color:var(--steel,#79716a)}.summary-corrected{font-size:11px;white-space:nowrap;color:var(--accent,#a95028)}
.summary-parts{display:grid;gap:10px}.summary-parts section{padding-left:12px;border-left:2px solid #d9d3c9}.summary-parts h4{display:flex;align-items:center;gap:6px;margin:0 0 3px;font-size:12px;font-weight:600}.summary-parts p{margin:0}.demonstrated h4{color:#4e7867}.difficulties h4{color:var(--accent,#a95028)}.summary-parts .demonstrated{border-color:#a7c5b4}.summary-parts .difficulties{border-color:#d7a184}.unconfirmed h4{color:var(--steel,#79716a)}
.summary-conditions{margin:12px 0 8px}blockquote{margin:8px 0;padding:10px 12px;border-left:2px solid var(--border,#e5dfd7);white-space:pre-wrap;background:var(--bg,#f7f4ee)}.summary-basis summary{cursor:pointer;color:var(--steel,#79716a);font-size:12px}.summary-next{margin:14px 0 8px}.summary-correction{color:var(--accent,#a95028)}.summary-notes{display:flex;flex-wrap:wrap;gap:6px;align-items:center}.summary-notes button{font-size:11px}
footer{display:flex;flex-wrap:wrap;gap:7px;margin-top:14px}button{display:inline-flex;align-items:center;gap:4px;font:inherit;font-size:12px;border:1px solid var(--border,#e5dfd7);border-radius:8px;padding:5px 9px;background:transparent;color:inherit;cursor:pointer}button:hover{background:var(--bg,#f7f4ee)}button:disabled{opacity:.5;cursor:wait}.summary-primary{color:var(--accent,#a95028)}.compact{padding:13px 15px}.compact h3{font-size:13px;margin-bottom:10px}.compact .summary-parts{gap:6px}.compact footer{margin-top:10px}
@media(max-width:400px){.understanding-summary{padding:13px}.summary-eyebrow{font-size:10px}footer{gap:6px}footer button{padding:6px 8px}}
</style>
