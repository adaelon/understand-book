import { createApp, defineComponent, h, ref } from "vue";
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
  const followUp = ref("");
  const input = ref('');
  const workspace = ref<PresentationWorkspace | null>(null);
  const tutor = useTutorControl();
  const showTutor = new URLSearchParams(location.search).has('tutor');
  const panel = ref(false);
  if (showTutor) void tutor.load();
  const control = () => showTutor ? h(TutorControl, { enabled: !!tutor.state.value?.control.enabled, label: tutor.label.value,
    busy: tutor.busy.value, unavailable: !tutor.state.value || !!tutor.pending.value, error: tutor.error.value,
    onToggle: () => tutor.act({ kind: 'set_enabled', enabled: !tutor.state.value?.control.enabled }), onManage: () => { panel.value = true; } }) : null;
  const chat = ref<any[]>([{ teachingRef: fixture.teaching_ref, turnId: fixture.turn_id, user: "解释证据召回率", outcome: fixture.outcome, pending: false,
    questionAnchorLid: null, questionQuote: null, questionSelection: null, effectLabels: [] }]);
  return () => h("main", { style: "max-width:760px;margin:24px auto;padding:8px" }, [
    control(),
    panel.value ? h(TutorPanel, { state: tutor.state.value, busy: tutor.busy.value, error: tutor.error.value,
      pending: !!tutor.pending.value, sourceId: 'fixture-book', label: tutor.label.value,
      readiness: tutor.readiness.value, readinessError: tutor.readinessError.value, onRefreshReadiness: tutor.loadReadiness,
      onAction: tutor.act, onRetry: tutor.retry, onClose: () => { panel.value = false; } }) : null,
    workspace.value?.suspended ? h('button', { onClick: () => { workspace.value!.suspended = false; } }, '返回演示') : null,
    h("p", { "data-testid": "reader-status" }, opened.value ? "已在正文中打开来源" : "保持当前阅读位置"),
    h("p", { "data-testid": "follow-up-status" }, followUp.value),
    h(RightRail, {
      chat: chat.value,
      presentationWorkspace: workspace.value,
      'onUpdate:presentationWorkspace': (value: PresentationWorkspace | null) => { workspace.value = value; },
      chatSessions: [], activeChatSessionId: fixture.session_id, agentInput: input.value, sending: false,
      'onUpdate:agentInput': (value: string) => { input.value = value; },
      showTrace: {}, latestTrace: [], selectedLid: null, selectedFormula: null, contextNotes: [], contextHighlights: [],
      renderMarkdown, effLabel: () => "", effState: () => undefined, isGoto: () => false,
      showEffectPrimary: () => false, showEffectSecondary: () => false, effectPrimaryLabel: () => "", effectSecondaryLabel: () => "", gotoBack: () => "", askDraft: null,
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
