import { reactive } from 'vue';
import { ApiError, type AgentChatTurn } from './api';
import { noteRequest, notesOwner, notesPanel, ownerRequest, startThought, type NoteContent, type NoteRef } from './personal-notes';

export interface PersonalConversation { session_id: string; title: string; turns: AgentChatTurn[] }
export type ThoughtIntent = 'discuss' | 'link' | 'synthesis';
export const personalChat = reactive({ open: false, opening: false, sessionId: '', turns: [] as AgentChatTurn[], title: '', text: '', error: '', busy: false, submitting: false,
  references: [] as NoteRef[], replyTo: null as string | null, intent: 'discuss' as ThoughtIntent, pending: null as Record<string, unknown> | null, turnId: '', proposal: '', proposalTurn: '', dismissed: false });
const storageKey = () => `understand-book:personal-chat:${notesOwner()}`;
let accepting = false, opening = false;
export function persistPersonalChat() {
  try { const saved = JSON.stringify({ sessionId: personalChat.sessionId, text: personalChat.text, references: personalChat.references,
    replyTo: personalChat.replyTo, intent: personalChat.intent, pending: personalChat.pending, turnId: personalChat.turnId, proposal: personalChat.proposal, proposalTurn: personalChat.proposalTurn, dismissed: personalChat.dismissed });
    localStorage.setItem(storageKey(), saved);
    if (personalChat.sessionId) localStorage.setItem(`${storageKey()}:${personalChat.sessionId}`, saved); }
  catch { personalChat.error = '设备暂时无法保存讨论草稿，请保持此窗口打开。'; }
}
export async function refreshPersonalChat() {
  if (!personalChat.sessionId) return;
  const id = personalChat.sessionId, owner = notesOwner();
  const value = await noteRequest<PersonalConversation>('chat_history', { session_id: id });
  if (owner !== notesOwner() || personalChat.sessionId !== id) return;
  personalChat.turns = value.turns; personalChat.title = value.title;
  const active = value.turns.find(turn => turn.status === 'pending_assistant');
  personalChat.busy = !!active; if (active) personalChat.turnId = active.turn_id;
  const answer = value.turns.find(turn => turn.turn_id === personalChat.turnId);
  if (answer && !active && answer.outcome?.answer && personalChat.intent === 'link' && !personalChat.dismissed && personalChat.proposalTurn !== answer.turn_id) {
    personalChat.proposal = answer.outcome.answer; personalChat.proposalTurn = answer.turn_id; persistPersonalChat();
  }
}
export async function openPersonalChat(references?: NoteRef[], intent: ThoughtIntent = 'discuss', prompt?: string, sessionId?: string) {
  personalChat.error = '';
  if (opening || accepting || ((references || sessionId) && personalChat.pending)) { personalChat.open = true; notesPanel.open = false; personalChat.error = '先确认这次发送的结果，再切换讨论。输入仍保留在这里。'; return; }
  opening = true; personalChat.opening = true;
  const owner = notesOwner();
  try {
    if (personalChat.sessionId) persistPersonalChat();
    if (references) {
      Object.assign(personalChat, { sessionId: '', title: '', turns: [], busy: false, references, replyTo: null, turnId: '', pending: null, intent,
        proposal: '', proposalTurn: '', dismissed: false, text: prompt ?? (intent === 'link' ? '请比较这两条想法，只提出一段具体的联系说明，并指出成立的条件。先作为建议供我修改和采用。' : intent === 'synthesis' ? '请依据所选想法，整理一份可编辑的综合解释草稿，保留条件、反例和还不确定的地方。' : '我想推敲这条想法。请从我的原话出发，帮助我发现需要补充的条件或例子。') });
      personalChat.open = true; notesPanel.open = false;
      const created = await noteRequest<{ session_id: string }>('chat_create', {});
      if (owner !== notesOwner()) return;
      personalChat.sessionId = created.session_id;
    } else if (sessionId) {
      Object.assign(personalChat, { sessionId, turns: [], busy: false, pending: null, turnId: '', references: [], replyTo: null, intent: 'discuss', text: '', proposal: '', proposalTurn: '', dismissed: false }, JSON.parse(localStorage.getItem(`${storageKey()}:${sessionId}`) ?? 'null') ?? {});
    } else {
      const saved = JSON.parse(localStorage.getItem(storageKey()) ?? 'null');
      if (saved) Object.assign(personalChat, saved);
    }
    personalChat.open = true; notesPanel.open = false;
    await refreshPersonalChat(); if (owner === notesOwner()) persistPersonalChat();
  } catch (failure) { if (owner === notesOwner()) personalChat.error = (failure as Error).message; }
  finally { opening = false; personalChat.opening = false; }
}
export async function sendPersonalChat() {
  if (personalChat.busy || accepting || opening || (!personalChat.pending && !personalChat.text.trim())) return;
  accepting = true; personalChat.submitting = true;
  personalChat.error = ''; personalChat.busy = true;
  const owner = notesOwner();
  try {
    if (!personalChat.sessionId) personalChat.sessionId = (await noteRequest<{ session_id: string }>('chat_create', {})).session_id;
    if (!personalChat.pending) personalChat.pending = { scope: 'personal', session_id: personalChat.sessionId, message: personalChat.text,
      client_request_id: crypto.randomUUID(), expected_chat_head: personalChat.turns.at(-1)?.turn_id ?? null, response_language: 'zh-CN',
      ...(personalChat.replyTo ? { note_reply_to: personalChat.replyTo } : { note_refs: personalChat.references }) };
    persistPersonalChat();
    const accepted = await ownerRequest<{ turn_id: string }>('/agent/runs', 'POST', personalChat.pending);
    if (notesOwner() !== owner) return;
    personalChat.turnId = accepted.turn_id;
    if (personalChat.references.length || personalChat.replyTo) personalChat.replyTo = accepted.turn_id;
    personalChat.text = ''; personalChat.pending = null; persistPersonalChat(); await refreshPersonalChat();
  } catch (failure) {
    if (notesOwner() !== owner) return;
    personalChat.error = (failure as Error).message;
    if (failure instanceof ApiError && failure.status < 500 && ![408,429].includes(failure.status) && failure.errorCode !== 'ADMISSION_PREPARING') personalChat.pending = null;
    personalChat.busy = false; persistPersonalChat();
  } finally { accepting = false; personalChat.submitting = false; }
}
export async function stopPersonalChat() {
  try { await ownerRequest(`/agent/runs/${encodeURIComponent(personalChat.turnId)}/cancel`, 'POST', {}); await refreshPersonalChat(); }
  catch (failure) { personalChat.error = (failure as Error).message; }
}
export async function capturePersonalAnswer(turnId: string, assistant: boolean) {
  try {
    const content = await noteRequest<NoteContent>(assistant ? 'from_assistant_turn' : 'from_turn', { session_id: personalChat.sessionId, turn_id: turnId });
    personalChat.open = false; startThought(content.context, content.content, content);
  } catch (failure) { personalChat.error = (failure as Error).message; }
}

export function continueWithReading() {
  const turn=personalChat.turns.at(-1);if(!turn)return;
  const event=new CustomEvent('personal-note-discuss',{cancelable:true,detail:{references:personalChat.references,
    personal_chat_ref:{session_id:personalChat.sessionId,turn_id:turn.turn_id},
    message:personalChat.text.trim() || '结合当前打开的资料，继续推敲刚才的讨论，检查想法成立的条件。'}});
  window.dispatchEvent(event);
  if(event.defaultPrevented){personalChat.open=false;persistPersonalChat();}
  else personalChat.error='请先在资料库打开要使用的资料，等待当前回答结束，再点击在当前资料中继续。原讨论会保留。';
}
