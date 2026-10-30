<script setup lang="ts">
import { nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue';
import { ArrowLeft, Send, Square, X } from '@lucide/vue';
import { renderMarkdown } from '../md';
import MessageThoughts from './MessageThoughts.vue';
import { continueWithReading, capturePersonalAnswer, openPersonalChat, personalChat as chat, persistPersonalChat, refreshPersonalChat, sendPersonalChat, stopPersonalChat } from '../personal-chat';
import { noteRequest, notesOwner, openPersonalNotes, personalNotes } from '../personal-notes';
const sessions = ref<{session_id:string;title:string}[]>([]), adopting=ref(false), adopted=ref(false), composing=ref(false);
const panel = ref<HTMLElement | null>(null);
let priorFocus: HTMLElement | null = null;
function keydown(event: KeyboardEvent) {
  if (!chat.open) return;
  if ((event.target as HTMLElement | null)?.closest('[data-personal-notes]')) return;
  if (event.key === 'Escape' && !event.isComposing) close();
  if (event.key !== 'Tab' || !panel.value) return;
  const elements = [...panel.value.querySelectorAll<HTMLElement>('button:not(:disabled),input,textarea,select,[tabindex="0"]')].filter(element => element.offsetParent !== null);
  const first = elements[0], last = elements.at(-1);
  if (event.shiftKey && document.activeElement === first) { event.preventDefault(); last?.focus(); }
  if (!event.shiftKey && document.activeElement === last) { event.preventDefault(); first?.focus(); }
}
onMounted(() => window.addEventListener('keydown', keydown));
onBeforeUnmount(() => window.removeEventListener('keydown', keydown));
let timer:ReturnType<typeof setTimeout>|undefined;
async function poll(){
  clearTimeout(timer);if(!chat.open)return;
  try {await refreshPersonalChat();}catch(failure){chat.error=(failure as Error).message;}
  if(chat.open)timer=setTimeout(poll,chat.busy?1200:5000);
}
watch(()=>chat.open,async open=>{clearTimeout(timer);if(open){priorFocus=document.activeElement as HTMLElement;nextTick(()=>panel.value?.focus());sessions.value=(await noteRequest<{sessions:{session_id:string;title:string}[]}>('chats',{}).catch(()=>({sessions:[]}))).sessions;void poll();}});
watch(()=>chat.sessionId,()=>{adopted.value=false;});
watch(notesOwner,()=>{clearTimeout(timer);Object.assign(chat,{open:false,sessionId:'',turns:[],text:'',pending:null,busy:false,references:[],replyTo:null,proposal:''});});
onBeforeUnmount(()=>clearTimeout(timer));
async function adopt(){
  if(chat.references.length!==2||!chat.proposal.trim())return;
  adopting.value=true;chat.error='';
  try{await personalNotes.link(chat.references[0]!,chat.references[1]!,chat.proposal);adopted.value=true;chat.dismissed=true;persistPersonalChat();}
  catch(failure){chat.error=(failure as Error).message;}finally{adopting.value=false;}
}
function close(){chat.open=false;persistPersonalChat();priorFocus?.focus();}
function back(){close();openPersonalNotes();}
</script>
<template>
  <div v-if="chat.open" class="personal-chat-overlay" @click.self="close">
    <section ref="panel" tabindex="-1" class="personal-chat" role="dialog" aria-modal="true" aria-label="推敲我的想法">
      <header><button class="round" aria-label="回到我的想法" @click="back"><ArrowLeft :size="20"/></button><div><small>从自己的想法出发</small><h2>{{chat.title || '一起推敲'}}</h2></div><button class="round" aria-label="收起讨论" @click="close"><X :size="21"/></button></header>
      <label v-if="sessions.length" class="chat-picker">继续之前的讨论<select aria-label="继续之前的讨论" :value="chat.sessionId" :disabled="!!chat.pending || chat.submitting || chat.opening" @change="openPersonalChat(undefined,'discuss',undefined,($event.target as HTMLSelectElement).value)"><option value="">选择讨论</option><option v-for="session in sessions" :key="session.session_id" :value="session.session_id">{{session.title}}</option></select></label>
      <div v-if="chat.references.length || chat.replyTo" class="chat-references"><span>正在推敲的想法</span><button v-for="(reference,index) in chat.references" :key="reference.note_id" @click="close();openPersonalNotes(reference)">想法 {{index+1}} · 第 {{reference.revision}} 版</button><button v-if="!chat.busy && !chat.pending" @click="chat.references=[];chat.replyTo=null;persistPersonalChat()">结束关联</button></div>
      <button v-if="chat.turns.length" class="reading-continue" :disabled="chat.busy || !!chat.pending" @click="continueWithReading">在当前打开的资料中继续</button>
      <div class="personal-chat-messages" aria-live="polite">
        <p v-if="!chat.turns.length" class="chat-empty">可以比较想法、补充例子，或说说哪里还没想明白。</p>
        <article v-for="turn in chat.turns" :key="turn.turn_id" class="personal-chat-turn">
          <div class="chat-user"><small>我的表达</small><MessageThoughts :session-id="chat.sessionId" :turn-id="turn.turn_id" role="user" :text="turn.user"><p>{{turn.user}}</p></MessageThoughts><button @click="capturePersonalAnswer(turn.turn_id,false)">留为我的想法</button></div>
          <div v-if="turn.outcome?.answer" class="chat-answer"><MessageThoughts :session-id="chat.sessionId" :turn-id="turn.turn_id" role="assistant" :text="turn.outcome.answer"><div class="md" v-html="renderMarkdown(turn.outcome.answer)"></div></MessageThoughts><button @click="capturePersonalAnswer(turn.turn_id,true)">作为 AI 草稿继续编辑</button></div>
          <p v-if="turn.status==='pending_assistant'" class="chat-progress">正在推敲… 可以收起，回答会留在这条讨论里。</p>
          <p v-if="turn.error" class="chat-error">{{turn.status==='cancelled'?'本轮已停止。':turn.error.message}}</p>
        </article>
        <section v-if="chat.intent==='link' && chat.proposal && !chat.dismissed" class="link-proposal"><h3>联系建议 · 还未保存</h3><p>依据上面两条想法的确切版本。可以修改说明后采用。</p><textarea v-model="chat.proposal" aria-label="AI 联系建议" @input="persistPersonalChat"/><div><button :disabled="adopting || !chat.proposal.trim()" @click="adopt">采用为个人联系</button><button @click="chat.dismissed=true;persistPersonalChat()">忽略</button></div></section>
        <p v-if="adopted" role="status">已保存联系，可在两条想法中查看。</p>
      </div>
      <form class="personal-chat-composer" @submit.prevent="sendPersonalChat"><p v-if="chat.error" class="chat-error" role="alert">{{chat.error}}</p><textarea v-model="chat.text" aria-label="讨论我的想法" placeholder="写下补充、疑问或一个例子…" :readonly="!!chat.pending || chat.submitting" @input="persistPersonalChat" @compositionstart="composing=true" @compositionend="composing=false" @keydown.ctrl.enter.prevent="!composing && sendPersonalChat()"/><footer><span>{{chat.pending?'发送结果待确认，重试会使用同一次请求。':'Ctrl + Enter 发送 · 原笔记由你决定何时修改'}}</span><button v-if="chat.busy" type="button" @click="stopPersonalChat"><Square :size="14"/>停止</button><button v-else type="submit" :disabled="chat.opening || (!chat.text.trim() && !chat.pending)"><Send :size="15"/>{{chat.pending?'确认并重试':'发送'}}</button></footer></form>
    </section>
  </div>
</template>
<style scoped>
.personal-chat-overlay{position:fixed;inset:0;z-index:210;background:#18221966;backdrop-filter:blur(4px);padding:24px;display:grid;place-items:center}.personal-chat{width:min(820px,100%);height:min(880px,100%);background:var(--canvas,#fffdf7);color:var(--ink,#242720);border:1px solid var(--line,#ddd);border-radius:22px;display:flex;flex-direction:column;box-shadow:0 20px 80px #0002;overflow:hidden}.personal-chat header{display:flex;align-items:center;gap:16px;padding:20px 24px;border-bottom:1px solid var(--line,#ddd)}header div{flex:1;min-width:0}h2{margin:5px 0 0;font-size:19px;overflow:hidden;text-overflow:ellipsis;white-space:nowrap}small{color:var(--steel,#687066);font-size:11px}.personal-chat button{font:inherit;cursor:pointer;border:1px solid var(--line,#ddd);border-radius:9px;padding:8px 11px;background:transparent;color:inherit;display:inline-flex;align-items:center;gap:6px}.personal-chat button:disabled{opacity:.5;cursor:default}.personal-chat .round{border:0;padding:8px}.reading-continue{margin:4px 24px;align-self:flex-start;font-size:12px!important}.chat-picker{display:flex;align-items:center;gap:12px;font-size:12px;padding:12px 24px}.chat-picker select{flex:1;min-width:0;max-width:450px;background:transparent;border:1px solid var(--line,#ddd);border-radius:6px;padding:7px}.chat-references{display:flex;gap:8px;align-items:center;flex-wrap:wrap;padding:8px 24px;font-size:12px;color:var(--steel,#687066)}.chat-references button{padding:5px 8px;font-size:12px}.personal-chat-messages{flex:1;overflow:auto;overscroll-behavior:contain;padding:20px 32px}.chat-empty{text-align:center;padding:24px 10px;color:var(--steel,#687066);line-height:1.8}.personal-chat-turn{margin-bottom:28px}.chat-user{margin:0 0 20px 10%;padding:16px 20px;background:var(--surface,#f0f1e8);border-radius:14px;white-space:pre-wrap}.chat-user p{line-height:1.7;margin:8px 0}.chat-user button,.chat-answer button{font-size:12px;border:0;color:var(--steel,#687066)}.chat-answer{line-height:1.8;overflow-wrap:anywhere}.chat-answer :deep(pre){max-width:100%;overflow:auto}.chat-progress{font-size:13px;color:var(--steel,#687066)}.chat-error{color:#a23c32;font-size:13px;line-height:1.7}.personal-chat-composer{padding:16px 24px;border-top:1px solid var(--line,#ddd);background:var(--canvas,#fffdf7)}textarea{box-sizing:border-box;width:100%;min-height:90px;resize:vertical;border:1px solid var(--line,#ddd);background:transparent;border-radius:12px;padding:12px;font:inherit;color:inherit;line-height:1.7}.personal-chat-composer footer{display:flex;align-items:center;gap:12px;margin-top:8px}.personal-chat-composer footer span{flex:1;font-size:11px;color:var(--steel,#687066)}.personal-chat-composer footer button,.link-proposal button:first-child{background:var(--ink,#242720);color:var(--canvas,#fffdf7)}.link-proposal{background:var(--surface,#f0f1e8);padding:18px;border-radius:12px}.link-proposal h3{margin:0;font-size:15px}.link-proposal p{font-size:12px;line-height:1.7}.link-proposal textarea{min-height:150px}.link-proposal div{display:flex;gap:8px;margin-top:8px}@media(max-width:600px){.personal-chat-overlay{padding:0}.personal-chat{height:var(--app-content-height, 100dvh);border-radius:0;border:0}.personal-chat header{padding:16px}.personal-chat-messages{padding:16px}.personal-chat-composer{padding:12px 16px;padding-bottom:max(12px,env(safe-area-inset-bottom))}.chat-picker,.chat-references{padding:8px 16px}.chat-user{margin-left:5%}.personal-chat-composer footer span{max-width:70%;line-height:1.5}}
</style>
