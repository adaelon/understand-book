import { createApp, defineComponent, h, ref } from "vue";
import NoteEditorPanel from "./components/NoteEditorPanel.vue";
import RightRail from "./components/RightRail.vue";
import { renderMarkdown } from "./md";
import "./style.css";
import { api } from "./api";
import TutorControl from './components/TutorControl.vue';
import TutorPanel from './components/TutorPanel.vue';
import { useTutorControl } from './useTutorControl';
import type { PresentationWorkspace } from './presentation-workspace';

const fixture = await fetch("/api/fixture").then(response => response.json());
createApp(defineComponent({ setup() {
  const opened = ref(false);
  const rn2 = new URLSearchParams(location.search).has('notes');
  const notes = ref<import('./api').MemoryRecord[]>([]);
  const selected = ref(fixture.session_id);
  const editing = ref<{ receipt: import('./generated/PresentationFollowUp').PresentationFollowUp; title: string; summary: string; content: string }>();
  const collapsed = ref(false), saving = ref(false), noteError = ref('');
  const refreshNotes = async () => { notes.value = await api.recall({ type: 'note' }); };
  if (rn2) void refreshNotes();
  const saveNote = async () => {
    const draft = editing.value;
    if (!draft || saving.value) return;
    saving.value = true; noteError.value = '';
    try { await api.save({ type: 'note', content: draft.content, note: { association: { kind: 'presentation', receipt: draft.receipt } } }); editing.value = undefined; await refreshNotes(); }
    catch (e) { noteError.value = String(e); }
    finally { saving.value = false; }
  };

  const followUp = ref("");
  const input = ref('');
  const workspace = ref<PresentationWorkspace | null>(null);
  const tutor = useTutorControl();
  const showTutor = new URLSearchParams(location.search).has('tutor');
  const panel = ref(false);
  const tutorAction = async (action: import('./generated/TutorAction').TutorAction) => {
    await tutor.act(action);
    window.dispatchEvent(new Event('tutor-state-changed'));
  };
  if (showTutor) void tutor.load();
  const control = () => showTutor ? h(TutorControl, { enabled: !!tutor.state.value?.control.enabled, label: tutor.label.value,
    busy: tutor.busy.value, unavailable: !tutor.state.value || !!tutor.pending.value, error: tutor.error.value,
    onToggle: () => tutorAction({ kind: 'set_enabled', enabled: !tutor.state.value?.control.enabled }), onManage: () => { panel.value = true; } }) : null;
  const chat = ref<any[]>([{ teachingRef: fixture.teaching_ref, turnId: fixture.turn_id, user: "解释证据召回率", outcome: fixture.outcome, pending: false,
    questionAnchorLid: null, questionQuote: null, questionSelection: null, effectLabels: [] }]);
  return () => h("main", { style: "max-width:760px;margin:24px auto;padding:8px" }, [
    rn2 ? h('button', { onClick: async () => {
      const history = await api.agentHistoryDelete(fixture.session_id);
      selected.value = history.active_session_id;
      chat.value = [];
    } }, '删除原聊天') : null,
    rn2 ? h('button', { onClick: async () => {
      const { history } = await api.agentNew(); selected.value = history.active_session_id; chat.value = [];
    } }, '选择新对话') : null,
    rn2 && editing.value ? (collapsed.value ? h('button', { style: 'position:fixed;z-index:130;bottom:8px;right:8px', onClick: () => { collapsed.value = false; } }, '继续编辑笔记') : h(NoteEditorPanel, {
      content: editing.value.content, title: editing.value.title, summary: editing.value.summary, revision: editing.value.receipt.reference.revision,
      saving: saving.value, error: noteError.value, 'onUpdate:content': (value: string) => { editing.value!.content = value; },
      onSave: saveNote, onCollapse: () => { collapsed.value = true; }, onDiscard: () => { editing.value = undefined; },
    })) : null,
    control(),
    panel.value ? h(TutorPanel, { state: tutor.state.value, busy: tutor.busy.value, error: tutor.error.value,
      pending: !!tutor.pending.value, sourceId: tutor.readiness.value?.source_id ?? 'fixture-book', label: tutor.label.value,
      readiness: tutor.readiness.value, readinessError: tutor.readinessError.value, onRefreshReadiness: tutor.loadReadiness,
      onAction: tutorAction, onRetry: tutor.retry, onClose: () => { panel.value = false; } }) : null,
    workspace.value?.suspended ? h('button', { onClick: () => { workspace.value!.suspended = false; } }, '返回演示') : null,
    h("p", { "data-testid": "reader-status" }, opened.value ? "已在正文中打开来源" : "保持当前阅读位置"),
    h("p", { "data-testid": "follow-up-status" }, followUp.value),
    h(RightRail, {
      chat: chat.value,
      presentationWorkspace: workspace.value,
      'onUpdate:presentationWorkspace': (value: PresentationWorkspace | null) => { workspace.value = value; },
      chatSessions: [], activeChatSessionId: selected.value, agentInput: input.value, sending: false,
      'onUpdate:agentInput': (value: string) => { input.value = value; },
      showTrace: {}, latestTrace: [], selectedLid: null, selectedFormula: null, contextNotes: notes.value, contextHighlights: [],
      renderMarkdown, effLabel: () => "", effState: () => undefined, isGoto: () => false,
      showEffectPrimary: () => false, showEffectSecondary: () => false, effectPrimaryLabel: () => "", effectSecondaryLabel: () => "", gotoBack: () => "", askDraft: null,
      onRecordNote: (receipt: import('./generated/PresentationFollowUp').PresentationFollowUp, title: string, summary: string) => {
        if (!rn2) return;
        if (!editing.value) editing.value = { receipt, title, summary, content: '' };
        collapsed.value = false;
      },
      onDeleteNote: async (note: import('./api').MemoryRecord) => { await api.delete(note.mem_id); await refreshNotes(); },
      onNoteFollowUp: async (message: string, receipt: import('./generated/PresentationFollowUp').PresentationFollowUp, id: string) => {
        await api.agentChat(message, { presentation_follow_up: receipt, note_mem_id: id }); followUp.value = '笔记追问已完成';
      },
      onAgentSourceOpened: () => { opened.value = true; if (showTutor && workspace.value) workspace.value.suspended = true; },
      onPresentationFollowUp: async (message: string, receipt: import("./generated/PresentationFollowUp").PresentationFollowUp) => {
        const outcome = await api.agentChat(message, { presentation_follow_up: receipt });
        chat.value.push({ turnId: `follow-${chat.value.length}`, user: message, outcome, pending: false,
          presentationFollowUp: receipt, questionAnchorLid: null, questionQuote: null, questionSelection: null, effectLabels: [] });
        followUp.value = "追问已完成";
      },
    }, { 'tutor-control': control }),
  ]);
} })).mount("#app");
