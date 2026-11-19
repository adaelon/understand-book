<script setup lang="ts">
import { applicationOverlayTarget } from "../application-overlay";
import { captureUserThought, openPersonalNotes } from '../personal-notes';
import MusicExamplePiano from "./MusicExamplePiano.vue";
import { playPiano, stopPiano } from "../music-example-player";
import MessageThoughts from './MessageThoughts.vue';
import { t as uiText } from '../i18n';

import { inject } from 'vue';
import { deliveredAnswerText, shareHighlightKey } from '../reading-share';
const shareHighlight = inject(shareHighlightKey, null);
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch, type ComputedRef } from "vue";
const thoughtCaptureError=ref('');
async function rememberUserThought(turnId:string) {try {thoughtCaptureError.value='';await captureUserThought(props.activeChatSessionId,turnId);} catch(error) {thoughtCaptureError.value=(error as Error).message;}}
import { ArrowRight, ArrowUp, ChevronDown, ChevronLeft, ChevronUp, CircleHelp, ExternalLink, GitFork, Link2, Link2Off, Maximize2, Minimize2, Plus, Presentation, SlidersHorizontal, Square, Undo2, X } from "@lucide/vue";
import MobileLearningSheet from './MobileLearningSheet.vue';
import FollowUpPathPanel from './FollowUpPathPanel.vue';
import type { FollowUpAnchor, FollowUpNode, FollowUpOrigin, FollowUpStart } from '../follow-up-conversations';
import { answerFollowUpAnchor, answerFollowUpRange, markAnswerFollowUps, type AnswerFollowUpMark } from '../answer-follow-up-anchor';
import QuestionImageInput from './QuestionImageInput.vue';
import AgentPermission from './AgentPermission.vue';
import VoiceInput from './VoiceInput.vue';
import { questionImageUrl } from '../question-image';
import type { VideoQuestionImage } from '../../../core/src/generated/VideoQuestionImage';
import YeyeMascot from './YeyeMascot.vue';
import { api as sharedApi } from "../api";
import { bindSceneApi, network, readerKey } from '../reader-context';
const api = bindSceneApi(sharedApi, readerKey);
import { findNotes, noteKindLabel, notePreview, noteTimeLabel, type NoteFilter } from '../reading-notes';
import type {
  AgentAnswerPart,
  BoardImportSource,
  AgentEffect,
  AgentQuestionQuoteView,
  AskQuote,
  FormulaSemantics,
  HistoricalBackfillJobRequest,
  HistoricalBackfillStartRequest,
  HistoricalBackfillStateView,
  IntentArtifactOverlayV1,
  MemoryRecord,
  OuterOutcome,
  ProfileGovernanceActionRequest,
  ProfileMemoryState,
  ProfileMemoryUpdate,
  ProfileUsageTrace,
  ResidentGoal,
  SourcePopupView,
  TraceStep,
} from "../api";
import type { PdfAnnotationLocation } from "../pdf-annotation-projection";
import { answerNoteExcerpt } from "../answer-note-selection";
import AgentActivities from "./AgentActivities.vue";
import TutorActivities from "./TutorActivities.vue";
import TutorContinuation from './TutorContinuation.vue';
import InputSuggestionHint from './InputSuggestionHint.vue';
import { suggestionTab, useReadingGuidance } from '../reading-guidance';
import type { LearningPurpose } from '../generated/LearningPurpose';
import { learningPurposeSetup, type LearningPurposeStart, type LearningScopeKind, type LearningScopeOption } from '../learning-purpose';
import type { TutorContinuationView } from '../generated/TutorContinuationView';
import NoteDetail from "./NoteDetail.vue";
import AgentPresentation from "./AgentPresentation.vue";
import { sourceChipLabel } from "../source-chip.js";
import { answerDisplayParts, type AnswerDisplayPart } from "../answer-markdown";
import BoardSourcePreview from './BoardSourcePreview.vue';
import type { SourceWindowLoader } from '../api';
import PresentationSceneCard from './PresentationSceneCard.vue';
import { presentationKey, type PresentationWorkspace } from '../presentation-workspace';
import type { PresentationFollowUp } from '../generated/PresentationFollowUp';
import type { PresentationRef } from '../generated/PresentationRef';
import type { BoardFollowUp } from '../generated/BoardFollowUp';
import type { BoardDelivery } from '../generated/BoardDelivery';
import { boardDeliveryKey, mergeBoardDeliveries, type BoardLocation } from '../board-workspace';
import type { RunActivity } from "../agent-run-state";
import ProfileMemoryPanel from "./ProfileMemoryPanel.vue";
import SessionRecap from './SessionRecap.vue';
import { targetPublication, type RecapTarget } from '../session-recap';

import { videoTime } from "../video-question";
type ContextTab = "agent" | "artifacts" | "profile" | "formula" | "notes";

type AskDraft = AskQuote;
type DisplayQuestionQuote = AskDraft | AgentQuestionQuoteView;
interface ChatTurn {
  localAgentName?: string;
  localAgentId?: string;
  localAgentUnavailable?: boolean;
  noteReferences?: import("../generated/NoteRef").NoteRef[];
  musicQuestion?: import('../music-api').MusicQuestionContext | null;
  videoQuestion?: import("../../../core/src/generated/VideoQuestionContext").VideoQuestionContext | null;
  hasTeaching?: boolean;
  interaction?: import("../generated/ResolvedInteraction").ResolvedInteraction | null;
  spendStop?: import('../reader-host').RunNotice;
  teachingRef?: string | null;
  presentationFollowUp?: PresentationFollowUp | null;
  boardFollowUp?: BoardFollowUp | null;
  boardDelivery?: BoardDelivery | null;
  boardCommits?: BoardDelivery[];
  boardIncomplete?: boolean;
  boardDeliveryError?: string | null;
  turnId: string | null;
  user: string;
  outcome: OuterOutcome | null;
  pending: boolean;
  error?: string;
  questionAnchorLid: string | null;
  questionQuote: AgentQuestionQuoteView | null;
  questionSelection: AskDraft | null;
  liveEffects?: { effect_id: string; effect: AgentEffect }[];
  effectLabels: string[];
  activities?: RunActivity[];
  runTrace?: TraceStep[];
  runStatus?: string;
  draft?: import("./../agent-run-state").AnswerDraft | null;
  submissionUnknown?: boolean;
}
interface ChatSessionSummary {
  id: string;
  title: string;
  created_at: string;
  updated_at: string;
  turn_count: number;
  turns: ChatSessionTurnSummary[];
}
interface ChatSessionTurnSummary {
  user: string;
  question_source_label: string | null;
  question_quote: AgentQuestionQuoteView | null;
}

const props = withDefaults(defineProps<{
  followUpNodes?: FollowUpNode[];
  followUpBusy?: boolean;
  followUpError?: string;
  followUpAllowed?: boolean;
  videoImagesEnabled?: boolean;
  musicMode?: boolean;
  musicQuestion?: import('../music-api').MusicQuestionContext | null;
  videoQuestion?: import("../../../core/src/generated/VideoQuestionContext").VideoQuestionContext | null;
  videoError?: string;
  questionSendBlocked?: string;
  questionInputBlocked?: boolean;
  chat: ChatTurn[];
  chatSessions: ChatSessionSummary[];
  chatGoals?: ResidentGoal[];
  targetGoalId?: string | null;
  activeChatSessionId: string;
  fullscreen?: boolean;
  companionVisible?: boolean;
  waitingTurnId?: string | null;
  learningTitle?: string;
  courseActive?: boolean;
  learningAttention?: string;
  learningScopeOptions?: LearningScopeOption[];
  learningPurposeAvailable?: boolean;
  learningPurposeReason?: string;
  learningPurposeReady?: boolean;
  learningPurposeBusy?: boolean;
  closedBookReady?: boolean;
  agentInput: string;
  personalNoteReference?: import('../generated/NoteRef').NoteRef;
  sending: boolean;
  historyLoading?: boolean;
  historyError?: string | null;
  runConnection?: string;
  canStop?: boolean;
  stopping?: boolean;
  unquotedNotePlacementAvailable?: boolean;
  notePlacementSurface?: "markdown" | "pdf";
  noteSourceFingerprint?: string | null;
  selectedLid: string | null;
  selectedFormula: FormulaSemantics | null;
  formulaSelected?: boolean;
  formulaLoading?: boolean;
  formulaError?: string | null;
  contextNotes: MemoryRecord[];
  contextHighlights: MemoryRecord[];
  noteBookOrder?: string[];
  noteSourceLabels?: Record<string, string>;
  noteScopeKey?: string;
  annotationLocation?: Record<string, PdfAnnotationLocation>;
  renderMarkdown: (source: string) => string;
  effLabel: (effect: AgentEffect) => string;
  effState: (turnIndex: number, effectIndex: number) => string | undefined;
  isGoto: (effect: AgentEffect) => boolean;
  showEffectPrimary: (effect: AgentEffect) => boolean;
  showEffectSecondary: (effect: AgentEffect) => boolean;
  effectPrimaryLabel: (effect: AgentEffect) => string;
  effectSecondaryLabel: (effect: AgentEffect) => string;
  gotoBack: (effect: AgentEffect) => string;
  askDraft: AskDraft | null;
  profileMemory?: ProfileMemoryState | null;
  profileMemoryLoading?: boolean;
  profileMemoryBusy?: boolean;
  profileMemoryError?: string | null;
  profileMemoryNotice?: string | null;
  profileBackfill?: HistoricalBackfillStateView | null;
  profileBackfillLoading?: boolean;
  profileBackfillBusy?: boolean;
  profileBackfillError?: string | null;
  profileBackfillNotice?: string | null;
  profileUpdateStates?: Record<string, string>;
  intentArtifacts?: IntentArtifactOverlayV1 | null;
  preparationAvailable?: boolean;
  continuationEnabled?: boolean;
  continuationLabel?: string;
  intentArtifactsLoading?: boolean;
  intentArtifactsError?: string | null;
  requestedTab?: ContextTab | null;
  requestedTabRevision?: number;
  workspaceNotesAvailable?: boolean;
  workspaceNoteImporting?: boolean;
}>(), { companionVisible: true });
const emit = defineEmits<{
  (e: 'follow-up-start', start: FollowUpStart): void;
  (e: 'follow-up-enter', id: string): void;
  (e: 'follow-up-parent' | 'follow-up-load'): void;
  (e: 'save-workspace-note', source: import('../api').WorkspaceNoteImportSource): void;
  (e: 'open-workspace-notes'): void;
  (e: 'question-image', image: VideoQuestionImage | null): void;
  (e: 'question-image-loading', active: boolean): void;
  (e: 'media-source-enter', source: import('../api').MediaSourceSnapshot): void;
  (e: 'music-source-enter', source: import('../music-api').MusicSourceSnapshot): void;
  (e: "cancel-video-draft"): void;
  (e: 'request-formula'): void;
  (e: 'prepare-artifacts'): void;
  (e: 'recap-publication', target: RecapTarget): void;
  (e: "open-note-answer", note: MemoryRecord): void;
  (e: "record-note", receipt: PresentationFollowUp, title: string, summary: string): void;
  (e: "note-follow-up", message: string, receipt: PresentationFollowUp, memId: string): void;
  (e: "presentation-follow-up", message: string, receipt: import("../generated/PresentationFollowUp").PresentationFollowUp): void;
  (e: "update:agentInput", value: string): void;
  (e: "send-agent"): void;
  (e: 'edit-failed-question', turn: ChatTurn): void;
  (e: 'end-socratic-dialogue'): void;
  (e: 'clear-personal-note'): void;
  (e: 'continue-personal-note', turnId: string, reference: import('../generated/NoteRef').NoteRef): void;
  (e: 'continue-answer', turnId: string): void;
  (e: 'explain-answer', turnId: string): void;
  (e: 'practice-answer', turnId: string): void;
  (e: 'clarify-answer', turnId: string, excerpt?: string, receipt?: PresentationFollowUp): void;
  (e: 'choose-continuation', view: TutorContinuationView, optionId: string, receipt?: PresentationFollowUp): void;
  (e: 'return-reading'): void;
  (e: 'workspace-menu'): void;
  (e: 'learning-purpose', purpose: LearningPurpose, scope?: LearningScopeKind, request?: LearningPurposeStart): void;
  (e: 'resume-learning'): void;
  (e: "retry-history"): void;
  (e: "stop-agent"): void;
  (e: "toggle-fullscreen"): void;
  (e: "continue-goal", goalId: string): void;
  (e: "show-allowance"): void;
  (e: "cancel-goal", goalId: string): void;
  (e: "target-goal", goalId: string): void;
  (e: "clear-goal-target"): void;
  (e: "new-chat"): void;
  (e: "select-chat", sessionId: string): void;
  (e: "delete-chat", sessionId: string): void;
  (e: "clear-ask"): void;
  (e: "undo-effect", turnIndex: number, effectIndex: number, effect: AgentEffect): void;
  (e: "keep-effect", turnIndex: number, effectIndex: number, effect: AgentEffect): void;
  (e: "save-answer-selection", turn: ChatTurn, text: string): void;
  (e: "share-answer", turn: ChatTurn, text?: string): void;
  (e: "add-to-board", source: BoardImportSource): void;
  (e: "open-board-location", location: BoardLocation): void;
  (e: "place-note", note: MemoryRecord): void;
  (e: "edit-note" | "delete-note" | "modify-highlight" | "delete-highlight", record: MemoryRecord): void;
  (e: "goto", lid: string): void;
  (e: "focus-source", source: { lid: string; quote: string | null; memId?: string }): void;
  (e: "refresh-profile"): void;
  (e: "mutate-profile", action: ProfileGovernanceActionRequest): void;
  (e: "confirm-sensitive-profile"): void;
  (e: "start-profile-backfill", request: HistoricalBackfillStartRequest): void;
  (e: "mutate-profile-backfill", action: "cancel" | "retry" | "clear", request: HistoricalBackfillJobRequest): void;
  (e: "undo-profile-update", turnIndex: number, updateIndex: number, update: ProfileMemoryUpdate): void;
  (e: "agent-source-will-open", source: { turnId: string; sourceRefId: string }): void;
  (e: "agent-source-opened", evidence?: import('../api').SourceOpenView['evidence_range']): void;
  (e: "original-selection", ranges: import('../api').EvidenceRange[]): void;
  (e: "failed-agent-settings", id?: string): void;
  (e: "refresh-artifacts"): void;
  (e: "open-artifacts"): void;
  (e: "artifact-cited", artifactId: string): void;
}>();

const activeTab = ref<ContextTab>("agent");
const workspace = defineModel<PresentationWorkspace | null>('presentationWorkspace', { default: null });
const presentationRequested = defineModel<boolean>('presentationRequested', { default: false });
const presentationUnavailable = computed(() => network.value.identity?.capabilities?.presentation.authoring === false);
watch(presentationUnavailable, unavailable => { if (unavailable) presentationRequested.value = false; }, { immediate: true });
const workspaceVisible = computed(() => !!workspace.value && !workspace.value.suspended);
const conversationMedia = window.matchMedia('(min-width: 1280px)');
const mobileConversationMedia = window.matchMedia('(max-width: 1023px)');
const wideViewport = ref(conversationMedia.matches);
const mobileViewport = ref(mobileConversationMedia.matches);
const wideConversation = computed(() => !!props.fullscreen && wideViewport.value && !workspaceVisible.value);
const mobileConversation = computed(() => mobileViewport.value && activeTab.value === 'agent' && !workspaceVisible.value);
function updateConversationViewport() { wideViewport.value = conversationMedia.matches; mobileViewport.value = mobileConversationMedia.matches; }
const mobileToolsOpen = ref(false);
const mobileLearningButton = ref<HTMLButtonElement | null>(null);
const mobileResponseTurn = ref<string | null>(null);
const mobileGoalHost = ref<HTMLElement | null>(null);
const mobileTeachingHost = ref<HTMLElement | null>(null);
const mobileSessionHost = ref<HTMLElement | null>(null);
const mobileRecordsHost = ref<HTMLElement | null>(null);
const responseEditingHost = ref<HTMLElement | null>(null);
// Move the existing controls between layouts so goal editors and drafts stay mounted.
const goalSidebarHost = ref<HTMLElement | null>(null);
const teachingSidebarHost = ref<HTMLElement | null>(null);
const sessionSidebarHost = ref<HTMLElement | null>(null);
const responseSidebarHost = ref<HTMLElement | null>(null);
const recordsSidebarHost = ref<HTMLElement | null>(null);
const recordsSheetHost = ref<HTMLElement | null>(null);
const useLearningSheet = computed(() => !wideConversation.value);
const goalToolsTarget = computed(() => useLearningSheet.value ? mobileGoalHost.value : wideConversation.value ? goalSidebarHost.value : null);
const teachingToolsTarget = computed(() => useLearningSheet.value ? mobileTeachingHost.value : wideConversation.value ? teachingSidebarHost.value : null);
const sessionToolsTarget = computed(() => mobileConversation.value ? mobileSessionHost.value : wideConversation.value ? sessionSidebarHost.value : null);
const recordsToolsTarget = computed(() => mobileConversation.value ? mobileRecordsHost.value : wideConversation.value ? recordsSidebarHost.value : recordsSheetHost.value);
const followUpSide = ref<HTMLElement>();
const followUpSheet = ref<HTMLElement>();
const followUpToggle = ref<HTMLButtonElement>();
const followUpPanel = ref<InstanceType<typeof FollowUpPathPanel>>();
const followUpTarget = computed(() => wideConversation.value ? followUpSide.value : mobileViewport.value ? followUpSheet.value : undefined);
const followUpLayout = computed(() => wideConversation.value ? 'side' : mobileViewport.value ? 'sheet' : 'tray');
const followUpOpen = ref(false);
const previewId = ref('');
const followUpDrafts = defineModel<FollowUpStart[]>('followUpDrafts',{ default:() => [] });
const pathNodes = computed(() => props.followUpNodes || []);
const visibleDrafts = computed(() => {
  const family = new Set(pathNodes.value.map(node => node.session_id));
  family.add(props.activeChatSessionId);
  return followUpDrafts.value.filter(draft => family.has(draft.parent_session_id));
});
const activeBranch = computed(() => pathNodes.value.find(node => node.session_id === props.activeChatSessionId));
const canReturnToParent = computed(() => pathNodes.value.some(node => node.session_id === activeBranch.value?.origin?.parent_session_id));
const hasPaths = computed(() => props.followUpAllowed && !!props.activeChatSessionId &&
  (!!activeBranch.value?.origin || pathNodes.value.length > 1 || visibleDrafts.value.length > 0 || !!props.followUpError));
const previewingTray = computed(() => hasPaths.value && followUpOpen.value && followUpLayout.value === 'tray');
const previewDraft = computed(() => visibleDrafts.value.find(draft => 'draft:' + draft.client_request_id === previewId.value));
const selectedPath = computed(() => previewDraft.value || pathNodes.value.some(node => node.session_id === previewId.value)
  ? previewId.value : props.activeChatSessionId);
let pathTrigger: HTMLElement | null = null;
let prePreviewTop: number | null = null;
watch(() => props.activeChatSessionId, () => {
  followUpOpen.value = false; previewId.value = ''; pathTrigger = null; prePreviewTop = null;
});
watch(pathNodes, nodes => {
  const opened = new Set(nodes.map(node => node.origin?.client_request_id));
  if (followUpDrafts.value.some(draft => opened.has(draft.client_request_id)))
    followUpDrafts.value = followUpDrafts.value.filter(draft => !opened.has(draft.client_request_id));
});
watch(() => props.followUpAllowed, allowed => {
  if (!allowed) { followUpOpen.value = false; previewId.value = ''; }
});
watch(activeTab, tab => { if (tab !== 'agent') followUpOpen.value = false; });
const followUpMarks = computed(() => {
  const byTurn = new Map<string, AnswerFollowUpMark[]>();
  function add(id:string, source:FollowUpStart) {
    if (source.parent_session_id !== props.activeChatSessionId) return;
    const mark = { session_id:id, anchor:source.anchor };
    const entries = byTurn.get(source.parent_turn_id);
    if (entries) entries.push(mark); else byTurn.set(source.parent_turn_id,[mark]);
  }
  if (props.followUpAllowed) {
    for (const node of pathNodes.value) if (node.origin) add(node.session_id,node.origin);
    for (const draft of visibleDrafts.value) add('draft:' + draft.client_request_id,draft);
  }
  return byTurn;
});
watch([mobileConversation, workspaceVisible], () => { mobileToolsOpen.value = false; });
const workspaceKey = computed(() => workspace.value ? presentationKey(workspace.value.turnId, workspace.value.reference) : '');
const chatHead = computed(() => props.chat.filter(t => t.turnId).at(-1)?.turnId ?? null);
const latestAnswerTurn = computed(() => {
  const turn = props.chat.at(-1);
  return turn?.turnId && !turn.pending && (!turn.error || turn.spendStop) && deliveredAnswerText(turn) ? turn : null;
});
const responseHosts = ref(new Map<string, string>());
const inSocraticDialogue = computed(() => latestAnswerTurn.value?.outcome?.answer_view?.parts.some(part => part.kind === 'socratic_question') ?? false);
const responseTurns = computed(() => new Set(responseHosts.value.values()));
const structuredResponseAvailable = computed(() => !!latestAnswerTurn.value?.turnId && (responseTurns.value.has(latestAnswerTurn.value.turnId) || hasSuggestion(latestAnswerTurn.value.turnId)));
const mobileComposerVisible = computed(() => !mobileResponseTurn.value);
const activityInstances = new Map<string, InstanceType<typeof TutorActivities>>();
const guidanceActivityReady = ref(new Set<string>());
function activityGuidanceAvailable(turnId: string, available: boolean) {
  if (available) guidanceActivityReady.value.add(turnId); else guidanceActivityReady.value.delete(turnId);
}
function responseAvailable(turnId: string, available: boolean, host = 'plain') {
  const key = `${turnId}:${host}`;
  if (available) responseHosts.value.set(key, turnId); else responseHosts.value.delete(key);
}
const learningResponseLabel = computed(() => {
  const turnId = latestAnswerTurn.value?.turnId;
  if (turnId && (responseTurns.value.has(turnId) || inSocraticDialogue.value)) return uiText('回答问题');
  if (turnId && hasSuggestion(turnId)) return uiText('查看下一步');
  return uiText('说说我的理解');
});
function focusLearningResponse() {
  const turnId = latestAnswerTurn.value?.turnId;
  if (!turnId) return;
  if (responseTurns.value.has(turnId)) {
    if (activityInstances.get(turnId)?.focusResponse()) return;
    for (const [key, instance] of presentationInstances) {
      if (key.startsWith(`${turnId}:`) && instance.focusResponse()) return;
    }
  }
  if (hasSuggestion(turnId)) {
    const turn = Array.from(transcriptRef.value?.querySelectorAll<HTMLElement>('[data-turn-id]') ?? [])
      .find(node => node.dataset.turnId === turnId);
    const continuation = turn?.querySelector<HTMLElement>('.tutor-continuation');
    if (continuation) {
      continuation.scrollIntoView({ block: 'start' });
      continuation.querySelector<HTMLButtonElement>('button:not(:disabled)')?.focus({ preventScroll: true });
      return;
    }
  }
  editQuestion();
}
function responseEditing(turnId: string, editing: boolean) {
  if (editing) {
    if (mobileResponseTurn.value && mobileResponseTurn.value !== turnId) activityInstances.get(mobileResponseTurn.value)?.endResponseEditing();
    mobileResponseTurn.value = turnId;
    void keepMobileResponseVisible();
  } else if (mobileResponseTurn.value === turnId) mobileResponseTurn.value = null;
}
function closeResponseEditor() {
  if (mobileResponseTurn.value) activityInstances.get(mobileResponseTurn.value)?.endResponseEditing();
  mobileResponseTurn.value = null;
}
async function keepMobileResponseVisible() {
  if (!mobileConversation.value || !mobileResponseTurn.value) return;
  await nextTick();
  requestAnimationFrame(() => {
    const input = document.activeElement as HTMLElement | null;
    if (input?.matches('[data-workspace-input="tutor"]') && transcriptRef.value?.contains(input)) input.scrollIntoView({ block: 'nearest' });
  });
}
const suggestionHosts = ref(new Set<string>());
function suggestionAvailable(key: string, available: boolean) { if (available) suggestionHosts.value.add(key); else suggestionHosts.value.delete(key); }
function hasSuggestion(turnId: string) { return [...suggestionHosts.value].some(key => key.startsWith(`${turnId}:`)); }
watch(() => props.activeChatSessionId, () => { suggestionHosts.value.clear(); responseTurns.value.clear(); activityInstances.clear(); guidanceActivityReady.value.clear(); mobileResponseTurn.value = null; mobileToolsOpen.value = false; });
function editQuestion() {
  closeResponseEditor();
  discussionOpen.value = true;
  void nextTick(() => document.querySelector<HTMLTextAreaElement>('.agent-input textarea')?.focus());
}
function locateAccepted(turnId: string) { workspace.value = null; discussionOpen.value = true; void nextTick(() => scrollToTurn(turnId)); }
function returnReading() { workspace.value = null; emit('return-reading'); }
const retainedNote = ref<MemoryRecord | null>(null);
const noteFollowUp = ref<MemoryRecord | null>(null);
const retainedPresentation = ref<InstanceType<typeof AgentPresentation>>();
const retainedReceipt = computed(() => retainedNote.value?.note?.association.kind === 'presentation' ? retainedNote.value.note.association.receipt : undefined);
watch(() => props.contextNotes, notes => {
  if (noteFollowUp.value && !notes.some(n => n.mem_id === noteFollowUp.value!.mem_id)) noteFollowUp.value = null;
  if (retainedNote.value && !notes.some(n => n.mem_id === retainedNote.value!.mem_id)) { retainedNote.value = null; workspace.value = null; }
});
async function openNotePresentation(note: MemoryRecord, restore = false) {
  if (note.note?.association.kind !== 'presentation') return;
  const receipt = note.note.association.receipt;
  const sameChat = receipt.session_id === props.activeChatSessionId && props.chat.some(t => t.turnId === receipt.turn_id && t.outcome && answerParts(t.outcome).some(p => p.kind === 'presentation' && presentationKey(t.turnId!, p) === presentationKey(receipt.turn_id, receipt.reference)));
  activeTab.value = 'agent';
  // Reuse an already mounted version so merely opening it cannot reset its state.
  if (sameChat) { retainedNote.value = null; await locateScene(receipt, restore); noteFollowUp.value = note; return; }
  noteFollowUp.value = note;
  retainedNote.value = note;
  workspace.value = { turnId: receipt.turn_id, reference: receipt.reference };
  sceneBound.value = true;
  await nextTick();
  if (restore) await retainedPresentation.value?.restore(receipt);
}
function retainedSent(message: string, receipt: PresentationFollowUp) {
  if (!retainedNote.value || !props.activeChatSessionId) return;
  discussionOpen.value = true;
  emit('note-follow-up', message, receipt, retainedNote.value.mem_id);
}
const sceneBound = ref(true);
const videoContextHost = ref<HTMLElement | null>(null);
const discussionOpen = ref(false);
const split = ref(63);
const workspaceTools = ref<HTMLElement>();
const workspaceTeachingTarget = ref<HTMLElement | null>(null);
const workspaceTop = ref(60);
let toolbarObserver: ResizeObserver | undefined;
watch(workspaceVisible, async visible => {
  toolbarObserver?.disconnect();
  if (!visible) return;
  await nextTick();
  if (!workspaceTools.value) return;
  toolbarObserver = new ResizeObserver(() => {
    workspaceTop.value = (workspaceTools.value?.getBoundingClientRect().bottom ?? 44) + 4;
  });
  toolbarObserver.observe(workspaceTools.value);
});
onBeforeUnmount(() => toolbarObserver?.disconnect());
const presentationInstances = new Map<string, InstanceType<typeof AgentPresentation>>();
const presentationTeachingHosts = ref(new Set<string>());
function teachingHostAvailable(key: string, available: boolean) {
  if (available) presentationTeachingHosts.value.add(key); else presentationTeachingHosts.value.delete(key);
}
const loadedPresentations = ref(new Set<string>());
let loadedSession = props.activeChatSessionId;
watch(() => [props.chat, props.activeChatSessionId, workspace.value] as const, () => {
  if (loadedSession !== props.activeChatSessionId) {
    retainedNote.value = null; noteFollowUp.value = null; workspace.value = null; loadedPresentations.value.clear(); presentationInstances.clear(); loadedSession = props.activeChatSessionId;
  }
  if (workspace.value) return;
  for (const turn of props.chat) for (const part of turn.outcome ? answerParts(turn.outcome) : []) {
    if (part.kind === 'presentation' && turn.turnId) loadedPresentations.value.add(presentationKey(turn.turnId, part));
  }
}, { immediate: true, deep: true });
function selectPresentation(turnId: string, reference: PresentationRef, open = true) {
  retainedNote.value = null; noteFollowUp.value = null;
  loadedPresentations.value.add(presentationKey(turnId, reference));
  workspace.value = open ? { turnId, reference } : null;
  sceneBound.value = true;
  if (open) void scrollToTurn(turnId);
}
function hostedTeachingPresentations(turn: ChatTurn): PresentationRef[] {
  if (!turn.turnId || !turn.outcome) return [];
  return answerParts(turn.outcome).filter((part): part is Extract<typeof part, { kind: 'presentation' }> =>
    part.kind === 'presentation' && presentationTeachingHosts.value.has(presentationKey(turn.turnId!, part))
    && (!workspaceVisible.value || workspaceKey.value === presentationKey(turn.turnId!, part)));
}
function submitMessage() {
  if (voiceActive.value || imageLoading.value || props.questionSendBlocked || props.sending || props.canStop || props.stopping || props.historyLoading || props.historyError || (!props.agentInput.trim() && !questionImage.value)) return;
  if (workspaceVisible.value && sceneBound.value) {
    discussionOpen.value = true;
    followTranscript.value = true;
    if (retainedNote.value) void retainedPresentation.value?.followUp();
    else void presentationInstances.get(workspaceKey.value)?.followUp();
  } else emit('send-agent');
}
const imageInput = ref<InstanceType<typeof QuestionImageInput>>(), imageLoading = ref(false);
const imagesEnabled = computed(() => props.videoImagesEnabled || props.musicMode);
const questionImage = computed(() => props.musicMode ? props.musicQuestion?.image : props.videoQuestion?.image);
function questionImageLoading(active: boolean) { imageLoading.value = active; emit('question-image-loading', active); }
function presentationSent(message: string, receipt: PresentationFollowUp) {
  followTranscript.value = true;
  discussionOpen.value = true;
  const note = noteFollowUp.value;
  if (note?.note?.association.kind === 'presentation') emit('note-follow-up', message, note.note.association.receipt, note.mem_id);
  else emit('presentation-follow-up', message, receipt);
}
async function locateScene(receipt: PresentationFollowUp, restore = false) {
  if (receipt.session_id !== props.activeChatSessionId) {
    const note = props.contextNotes.find(n => n.note?.association.kind === 'presentation' && n.note.association.receipt.saved_state_ref === receipt.saved_state_ref);
    if (note) { await openNotePresentation(note, restore); return; }
  }
  selectPresentation(receipt.turn_id, receipt.reference);
  await nextTick();
  if (restore) await presentationInstances.get(workspaceKey.value)?.restore(receipt);
}
const historySessions = computed(() => props.chatSessions.filter(session => session.turn_count > 0));
const historyOpen = ref(false);
const recapOpen = ref(false);
const recapButton = ref<HTMLButtonElement>();
const historyButton = ref<HTMLButtonElement>();
const chatMenuOpen = ref(false);
watch(wideConversation, () => { chatMenuOpen.value = false; });
const contextMenuOpen = ref(false);
const contextTools = ref<HTMLElement | null>(null);
const contextMenuButton = ref<HTMLButtonElement | null>(null);
const chatActions = ref<HTMLElement | null>(null);
const chatMenuButton = ref<HTMLButtonElement | null>(null);
function focusChatAction(button: HTMLButtonElement | null | undefined) {
  (button?.getClientRects().length ? button : mobileConversation.value ? mobileLearningButton.value : chatMenuButton.value)?.focus({ preventScroll: true });
}
function dismissChatMenu(event: PointerEvent) {
  if (!(event.target instanceof Node)) return;
  if (!chatActions.value?.contains(event.target)) chatMenuOpen.value = false;
  if (!contextTools.value?.contains(event.target)) contextMenuOpen.value = false;
}
function closeChatMenuAfterAction(event: MouseEvent) {
  if (!(event.target instanceof Element) || !event.target.closest('button')) return;
  chatMenuOpen.value = false;
  mobileToolsOpen.value = false;
  if (mobileConversation.value) mobileLearningButton.value?.focus({ preventScroll: true });
  else if (!wideConversation.value) chatMenuButton.value?.focus({ preventScroll: true });
}
function leaveMobileGoalTools(event: MouseEvent) {
  if ((event.target as Element).closest('.goal-manage')) {
    mobileToolsOpen.value = false;
    mobileLearningButton.value?.focus({ preventScroll: true });
  }
}
watch(historyOpen, (open, previous) => {
  if (previous && !open) void nextTick(() => focusChatAction(historyButton.value));
});
const recapPresentation = ref<{ turnId: string; reference: PresentationRef } | null>(null);
function closeRecap() {
  recapOpen.value = false;
  void nextTick(() => focusChatAction(recapButton.value));
}
watch(() => props.activeChatSessionId, () => { recapOpen.value = false; recapPresentation.value = null; });
async function openRecapTarget(target: RecapTarget) {
  if (target.session_id !== props.activeChatSessionId) throw new Error(uiText('对话已切换，请重新打开回顾'));
  if (target.kind === 'turn') {
    activeTab.value = 'agent'; followTranscript.value = false;
    if (!await scrollToTurn(target.turn_id)) throw new Error(uiText('该回合当前不可用，请刷新对话'));
    return;
  }
  // Recheck current availability while preserving the panel's factual cutoff.
  const scope = readerKey();
  const current = await api.sessionRecap(target.session_id, target.through_seq);
  if (scope !== readerKey() || target.session_id !== props.activeChatSessionId) throw new Error(uiText('阅读现场已切换'));
  const item = target.kind === 'source'
    ? current.sources.find(s => s.source_ref_id === target.source.source_ref_id && s.evidence[0].turn_id === target.turn_id)
    : current.effects.find(e => e.effect_id === target.effect.effect_id && e.evidence[0].turn_id === target.turn_id);
  if (!item || item.unavailable_reason) throw new Error(item?.unavailable_reason ?? uiText('原记录当前不可用'));
  const publication = targetPublication(target), active = network.value.workspace?.published_book_ref;
  if (network.value.enabled && publication && (publication.book_id !== active?.book_id || publication.publication_id !== active?.publication_id)) {
    emit('recap-publication', target); return;
  }
  if (target.kind === 'source') {
    const turn = props.chat.find(t => t.turnId === target.turn_id);
    if (!turn) throw new Error(uiText('关联回合当前不可用，请刷新对话'));
    await openAgentSources(turn, [target.source.source_ref_id], { currentTarget: recapButton.value ?? null }, true);
  } else if (target.effect.effect.kind === 'presentation') {
    const reference = target.effect.effect.reference;
    await api.presentationRead(target.session_id, target.turn_id, reference);
    if (!props.chat.some(t => t.turnId === target.turn_id && t.outcome && answerParts(t.outcome).some(p => p.kind === 'presentation' && presentationKey(target.turn_id, p) === presentationKey(target.turn_id, reference)))) {
      recapPresentation.value = { turnId: target.turn_id, reference };
    }
    selectPresentation(target.turn_id, reference);
  } else if (target.effect.object_id && ['Note', 'Highlight'].includes(target.effect.effect.effect.kind)) {
    const records = await api.recall();
    const record = records.find(r => r.mem_id === ('object_id' in item ? item.object_id : null));
    if (scope !== readerKey() || target.session_id !== props.activeChatSessionId) throw new Error(uiText('阅读现场已切换'));
    if (!record || !await showMemory(record.mem_id)) throw new Error(uiText('原成果当前不可用，请刷新笔记'));
  } else {
    activeTab.value = 'agent'; followTranscript.value = false;
    if (!await scrollToTurn(target.turn_id)) throw new Error(uiText('操作记录当前不可用'));
  }
}
const fullscreenButton = ref<HTMLButtonElement | null>(null);
const noteQuery = ref('');
const noteFilter = ref<NoteFilter>('all');
const noteOrder = ref<'recent' | 'original'>('recent');
const filteredNotes = computed(() => findNotes([...props.contextNotes, ...props.contextHighlights],
  noteQuery.value, noteFilter.value, noteOrder.value, props.noteBookOrder ?? [...props.contextNotes, ...props.contextHighlights].map(n => n.mem_id), noteAssociationTitle));
function noteAssociationTitle(note: MemoryRecord): string {
  const association = note.note?.association;
  if (association?.kind === 'presentation') return association.title;
  if (association?.kind === 'answer') {
    return props.chatSessions.find(s => s.id === association.session_id)?.title || uiText('助手回答');
  }
  return props.noteSourceLabels?.[note.mem_id] || note.note?.source_bindings?.[0]?.label_snapshot || uiText('来源未知');
}
watch(() => props.noteScopeKey, () => {
  noteQuery.value = ''; noteFilter.value = 'all'; noteOrder.value = 'recent';
  openNoteDetails.value.clear();
  retainedNote.value = null; noteFollowUp.value = null; workspace.value = null;
});
const openNoteDetails = ref(new Set<string>());
const transcriptRef = ref<HTMLElement | null>(null);
const transcriptLearningHost = ref<HTMLElement | null>(null);
const learningDockTarget = computed(() => wideConversation.value ? responseSidebarHost.value : transcriptLearningHost.value);
const followTranscript = ref(true);
function trackTranscriptScroll() {
  const el = transcriptRef.value;
  if (el) followTranscript.value = el.scrollHeight - el.scrollTop - el.clientHeight < 48;
}
async function scrollToTurn(turnId: string): Promise<boolean> {
  await nextTick();
  const turn = Array.from(transcriptRef.value?.querySelectorAll<HTMLElement>("[data-turn-id]") ?? [])
    .find((candidate) => candidate.dataset.turnId === turnId);
  if (!turn) return false;
  turn.scrollIntoView({ block: "start" });
  return true;
}
const openGoals = computed(() => (props.chatGoals ?? []).filter(goal => goal.status === "open"));
const targetGoal = computed(() => openGoals.value.find(goal => goal.id === props.targetGoalId) ?? null);
const taskDetailsOpen = ref(false);
const taskSummaryButton = ref<HTMLButtonElement | null>(null);
let taskDetailsTrigger: HTMLElement | null = null;
const taskCloseButton = ref<HTMLButtonElement | null>(null);
const runActive = computed(() => props.sending || props.canStop || props.stopping);
const stopAvailable = computed(() => props.canStop || props.stopping);
function stopAgent() {
  if (!props.canStop || props.stopping) return;
  emit('stop-agent');
}
const showConversationWelcome = computed(() => !props.courseActive && props.chat.length === 0
  && !props.historyLoading && !props.historyError && !runActive.value && !workspaceVisible.value);
const companionActive = computed(() => props.companionVisible !== false && activeTab.value === 'agent'
  && !workspaceVisible.value && !props.historyLoading && !props.historyError);
const mobileCompanion = ref<InstanceType<typeof YeyeMascot> | null>(null);
const fullscreenCompanion = ref<InstanceType<typeof YeyeMascot> | null>(null);
const tabCompanion = ref<InstanceType<typeof YeyeMascot>[]>([]);
function prepareCompletionFeedback() {
  if (!companionActive.value) return;
  const mascot = mobileConversation.value ? mobileCompanion.value
    : props.fullscreen ? fullscreenCompanion.value : tabCompanion.value[0];
  return mascot?.prepareCompletionFeedback();
}
const taskSummary = computed(() => {
  if (runActive.value) return uiText('正在执行');
  return openGoals.value.length === 1 ? goalStopLabel(openGoals.value[0].last_stop_reason) : uiText('{0} 项任务尚未完成', [openGoals.value.length]);
});
function openTaskDetails(event?: Event) {
  taskDetailsTrigger = event?.currentTarget as HTMLElement | null;
  taskDetailsOpen.value = true;
  void nextTick(() => taskCloseButton.value?.focus({ preventScroll: true }));
}
function closeTaskDetails() {
  taskDetailsOpen.value = false;
  void nextTick(() => focusChatAction((taskDetailsTrigger ?? taskSummaryButton.value) as HTMLButtonElement | null));
}
function supplementGoal(goalId: string) {
  emit('target-goal', goalId);
  taskDetailsOpen.value = false;
  editQuestion();
}
watch(() => [props.activeChatSessionId, openGoals.value.length] as const, ([session, count], previous) => {
  if (!count || session !== previous?.[0]) taskDetailsOpen.value = false;
});
function goalStopLabel(reason: string | null | undefined): string {
  if (!reason) return uiText('任务尚未完成');
  if (reason === "TURN_LIMIT_EXCEEDED") return uiText('本次达到运行上限');
  if (reason === "AGENT_RUN_CANCELLED") return uiText('本次生成已停止');
  if (reason === "AGENT_NO_PROGRESS") return uiText('本次未能继续推进');
  return uiText('本次运行未完成');
}
const agentInputRef = ref<HTMLTextAreaElement | null>(null);
const voiceActive = ref(false);
async function insertVoice(value: string) {
  const draft = props.agentInput + (props.agentInput && !props.agentInput.endsWith('\n') ? '\n' : '') + value;
  emit('update:agentInput', draft); composeExpanded.value = true;
  await nextTick(); agentInputRef.value?.focus(); agentInputRef.value?.setSelectionRange(draft.length, draft.length);
}
const composeExpanded = ref(false);
const inputComposing = ref(false);
const guidanceTurn = computed(() => {
  const turn = latestAnswerTurn.value;
  return turn && !turn.error && !turn.spendStop && !turn.outcome?.incomplete ? turn : null;
});
const learningActionBlocked = computed(() => !props.learningPurposeAvailable || !props.learningPurposeReady
  || !!props.learningPurposeBusy || !!props.questionInputBlocked || !!props.questionSendBlocked
  || !!props.historyLoading || !!props.historyError || runActive.value);
const currentLearningScope = computed(() => props.learningScopeOptions?.[0]);
function startCurrentLearning() {
  if (learningActionBlocked.value || !currentLearningScope.value || props.courseActive) return;
  const setup = learningPurposeSetup('understand', props.musicMode, props.videoImagesEnabled);
  emit('learning-purpose', 'understand', currentLearningScope.value.value, {
    message: setup!.fallbackMessage, display: uiText('带我学这一部分'),
  });
}
const showAnswerPractice = computed(() => {
  const turn = guidanceTurn.value;
  if (!turn?.turnId || !props.learningPurposeAvailable || props.courseActive || inSocraticDialogue.value
    || responseTurns.value.has(turn.turnId) || hasSuggestion(turn.turnId)) return false;
  // Teaching activities load separately. Wait for their actual response state before offering a new task.
  return !(turn.hasTeaching || turn.teachingRef || turn.outcome?.trace.some(step => step.tool === 'tutor.step'))
    || guidanceActivityReady.value.has(turn.turnId);
});
function practiceFromAnswer(kind: 'explain' | 'apply') {
  const turnId = guidanceTurn.value?.turnId;
  if (!showAnswerPractice.value || !turnId || learningActionBlocked.value || (kind === 'explain' && !props.closedBookReady)) return;
  if (kind === 'explain') emit('explain-answer', turnId);
  else emit('practice-answer', turnId);
}
const composerGuidancePurpose = computed(() => {
  if (props.askDraft) return props.askDraft.status !== 'partial' && props.askDraft.quote.trim() ? 'selection' as const : null;
  if (showConversationWelcome.value) return 'welcome' as const;
  return guidanceTurn.value ? 'followup' as const : null;
});
const composerGuidanceActive = computed(() => {
  if (inSocraticDialogue.value || !composerGuidancePurpose.value || activeTab.value !== 'agent'
    || !props.learningPurposeAvailable || props.questionInputBlocked || props.questionSendBlocked
    || props.historyLoading || props.historyError || runActive.value || workspaceVisible.value
    || mobileResponseTurn.value || targetGoal.value || questionImage.value || imageLoading.value) return false;
  const turn = props.chat.at(-1);
  if (turn?.pending) return false;
  if (turn?.turnId && (responseTurns.value.has(turn.turnId) || hasSuggestion(turn.turnId))) return false;
  if (turn?.turnId && (turn.hasTeaching || turn.teachingRef || turn.outcome?.trace.some(step => step.tool === 'tutor.step'))
    && !guidanceActivityReady.value.has(turn.turnId)) return false;
  return true;
});
const composerGuidance = useReadingGuidance({
  purpose: () => composerGuidancePurpose.value,
  scope: () => composerGuidancePurpose.value === 'selection' ? 'selection'
    : composerGuidancePurpose.value === 'followup' ? 'material' : props.learningScopeOptions?.[0]?.value,
  active: () => composerGuidanceActive.value,
  text: () => props.agentInput,
  recentTurns: () => props.chat.slice(-2).filter(turn => turn.turnId && !turn.pending && !turn.error && deliveredAnswerText(turn)).map(turn => ({
    turn_id: turn.turnId!, user: turn.user, assistant: deliveredAnswerText(turn),
    source_quote: turn.questionQuote?.status === 'resolved' ? turn.questionQuote.quote : '',
  })),
});
const composerSuggestionVisible = computed(() => !!composerGuidance.value && !props.agentInput.length
  && !inputComposing.value && composerGuidanceActive.value);
async function acceptComposerSuggestion() {
  if (!composerSuggestionVisible.value) return;
  const text = composerGuidance.value!.draft;
  emit('update:agentInput', text); composeExpanded.value = true;
  await nextTick(); agentInputRef.value?.focus(); agentInputRef.value?.setSelectionRange(text.length, text.length);
}
const composeTall = ref(false);
let inputResizeObserver: ResizeObserver | undefined;
let inputWidth = 0;
function resizeAgentInput() {
  const input = agentInputRef.value;
  if (!input) return;
  if (!input.clientWidth) return;
  const previousHeight = input.style.height;
  input.style.height = '44px';
  const height = composeExpanded.value || composerSuggestionVisible.value ? Math.min(120, Math.max(44, input.scrollHeight + 2)) : 44;
  input.style.height = `${height}px`;
  composeTall.value = height > 48;
  if (previousHeight !== input.style.height && followTranscript.value) void scrollTranscriptToLatest();
}
function collapseAgentInput() {
  composeExpanded.value = false;
  if (agentInputRef.value) agentInputRef.value.scrollTop = 0;
  agentInputRef.value?.blur();
}
onMounted(() => {
  conversationMedia.addEventListener('change', updateConversationViewport);
  mobileConversationMedia.addEventListener('change', updateConversationViewport);
  window.visualViewport?.addEventListener('resize', keepMobileResponseVisible);
  document.addEventListener('pointerdown', dismissChatMenu);
  document.addEventListener('selectionchange', onAnswerSelectionChange);
  document.addEventListener('pointerup', finishAnswerSelection);
  document.addEventListener('pointercancel', cancelAnswerDrag);
  window.addEventListener('blur', cancelAnswerDrag);
  window.addEventListener('scroll', positionAnswerSelection, true);
  window.addEventListener('resize', positionAnswerSelection);
  window.visualViewport?.addEventListener('resize', positionAnswerSelection);
  window.visualViewport?.addEventListener('scroll', positionAnswerSelection);
  answerSelectionObserver = new ResizeObserver(positionAnswerSelection);
  if (transcriptRef.value) answerSelectionObserver.observe(transcriptRef.value);
  inputResizeObserver = new ResizeObserver(entries => {
    const width = entries[0]?.contentRect.width ?? 0;
    if (width === inputWidth) return;
    inputWidth = width;
    resizeAgentInput();
  });
  if (agentInputRef.value) inputResizeObserver.observe(agentInputRef.value);
  resizeAgentInput();
});
watch(() => [props.agentInput, activeTab.value, props.fullscreen, composeExpanded.value, mobileResponseTurn.value, composerSuggestionVisible.value], () => resizeAgentInput(), { flush: 'post' });
watch(() => props.sending, sending => {
  if (sending && mobileConversation.value) { collapseAgentInput(); closeResponseEditor(); }
});
const tabs: { id: ContextTab; label: string }[] = [
  { id: "agent", label: uiText('问答') },
  { id: "profile", label: uiText('画像') },
  { id: "formula", label: uiText('公式') },
  { id: "notes", label: uiText('笔记') },
];
watch(
  () => [props.requestedTab, props.requestedTabRevision] as const,
  ([tab]) => {
    if (tab && tabs.some((candidate) => candidate.id === tab)) selectTab(tab);
  },
);
const noteCount = computed(() => props.contextNotes.length + props.contextHighlights.length);
watch(() => props.selectedLid, () => { if (activeTab.value === 'formula') emit('request-formula'); });
const profileAttentionCount = computed(() => (
  (props.profileMemory?.pending_candidates.length ?? 0)
  + (props.profileMemory?.status.pending_sensitive_confirmation ? 1 : 0)
));
watch(() => [props.askDraft, props.activeChatSessionId] as const, async ([draft,session],[oldDraft,oldSession]) => {
  if (!draft || draft === oldDraft) return;
  activeTab.value = "agent";
  closeResponseEditor();
  await nextTick();
  if (!mobileViewport.value || session === oldSession) agentInputRef.value?.focus();
});

async function scrollTranscriptToLatest() {
  await nextTick();
  const el = transcriptRef.value;
  if (!el || !followTranscript.value) return;
  el.scrollTop = el.scrollHeight;
}
watch(() => props.activeChatSessionId, () => {
  followTranscript.value = true;
  void scrollTranscriptToLatest();
});
watch(
  () => {
    const last = props.chat[props.chat.length - 1];
    return [
      props.chat.length,
      last?.pending ? 1 : 0,
      last?.activities?.map(a => `${a.step_id}:${a.status}:${a.label}`).join(",") ?? "",
      last?.draft?.message_id ?? -1,
      last?.draft?.revision ?? -1,
      last?.draft?.view?.parts.reduce((size, part) => size + (part.kind === 'markdown' || part.kind === 'socratic_question' ? part.text.length : 0), 0) ?? 0,
      last?.outcome?.answer?.length ?? 0,
      props.askDraft ? 1 : 0,
    ].join(":");
  },
  () => { void scrollTranscriptToLatest(); },
  { flush: "post" },
);

const answerSelection = ref<{ x: number; y: number; text: string | null; turn: ChatTurn; anchor: FollowUpAnchor | null } | null>(null);
const answerSelectionError = ref('');
const answerPopover = ref<HTMLElement | null>(null);
const answerSelectionPositioned = ref(false);
let answerSelectionRange: Range | null = null;
let answerSelectionObserver: ResizeObserver | undefined;
let answerSelectionDragging = false;

interface AgentSourcePopupState {
  fromRecap: boolean;
  noteMemId?: string;
  turnId: string;
  sourceRefIds: string[];
  activeSourceRefId: string;
  sources: SourcePopupView[];
  loading: boolean;
  opening: boolean;
  error: string | null;
  trigger: HTMLElement | null;
}

const agentSourcePopup = ref<AgentSourcePopupState | null>(null);
let sourceRequestSequence = 0;
const activeAgentSource = computed(() => {
  const popup = agentSourcePopup.value;
  return popup?.sources.find((source) => source.source_ref_id === popup.activeSourceRefId) ?? null;
});
function sourceTabLabel(source: SourcePopupView) {
  const turn = props.chat.find(turn => turn.turnId === agentSourcePopup.value?.turnId);
  const sources = turn?.outcome?.answer_view?.sources ?? turn?.draft?.view?.sources ?? agentSourcePopup.value?.sources ?? [];
  return sourceChipLabel(sources, source.source_ref_id);
}
const readAgentSourceWindow = computed<SourceWindowLoader | undefined>(() => {
  const popup = agentSourcePopup.value, source = activeAgentSource.value;
  if (!popup || !source) return;
  return options => api.agentSourceRead(popup.turnId, source.source_ref_id, options, popup.noteMemId);
});

function answerParts(outcome: OuterOutcome): AgentAnswerPart[] {
  return outcome.answer_view?.parts?.length
    ? outcome.answer_view.parts
    : outcome.answer
      ? [{ kind: "markdown", text: outcome.answer }]
      : [];
}

function incompleteNotice(outcome: OuterOutcome): string | null {
  if (!outcome.incomplete || !outcome.warning) return null;
  switch (outcome.warning) {
    case "CONTEXT_BUDGET_EXCEEDED":
    case "ACTIVE_CONTEXT_EXHAUSTED":
      return uiText('这次内容较长，先提供当前可用的回答；其余部分可以继续补充。');
    case "COMPACTION_FAILED":
      return uiText('已提供当前可用的回答；剩余内容暂未完成，可以继续补充。');
    default:
      return uiText('已提供当前可用内容；未完成的讲解、演示或其他操作可以接着完善。');
  }
}


// Each turn tracks its own content dependencies; opening a menu must not parse the transcript.
const answerRoots = new Map<string, HTMLElement>();
function socraticTarget(turn: ChatTurn, prompt: string) {
  if (!turn.turnId || !turn.outcome?.answer_view?.parts.some(part => part.kind === 'socratic_question' && part.text === prompt)) return;
  return answerRoots.get(turn.turnId)?.querySelector<HTMLElement>('.socratic-question') ?? undefined;
}
const answerDisplayCache = new WeakMap<ChatTurn, { answer: ComputedRef<AnswerDisplayPart[]>; draft: ComputedRef<AnswerDisplayPart[]> }>();
function turnDisplayParts(turn: ChatTurn, draft = false): AnswerDisplayPart[] {
  let cached = answerDisplayCache.get(turn);
  if (!cached) {
    const rendered = computed(() => turn.outcome
      ? answerDisplayParts(answerParts(turn.outcome), turn.outcome.answer_view?.sources ?? [], false, !!turn.turnId, props.renderMarkdown) : []);
    cached = {
      answer: computed(() => {
        const parts = rendered.value;
        const marks = turn.turnId ? followUpMarks.value.get(turn.turnId) : undefined;
        if (!marks?.length) return parts;
        return parts.map(part => part.kind === 'html' ? {...part, html:markAnswerFollowUps(part,marks)} : part);
      }),
      draft: computed(() => turn.draft?.view
        ? answerDisplayParts(turn.draft.view.parts, turn.draft.view.sources, true, true, props.renderMarkdown) : []),
    };
    answerDisplayCache.set(turn, cached);
  }
  return (draft ? cached.draft : cached.answer).value;
}
const pianoSourceTurn = ref<string | null>(null);
watch(() => props.activeChatSessionId, () => stopPiano(true));
watch(() => props.sending, busy => { if (busy) stopPiano(true); });
watch(activeTab, () => stopPiano(true));
onBeforeUnmount(() => stopPiano(true));
function pianoSource(id: string) {
  const turn = props.chat.find(turn => turn.turnId === pianoSourceTurn.value);
  if (turn) void openAgentSources(turn, [id], { currentTarget: document.activeElement });
}
function onAnswerSourceClick(turn: ChatTurn, parts: AgentAnswerPart[], event: MouseEvent) {
  const target = event.target;
  if (!(target instanceof Element)) return;
  const mark = target.closest<HTMLElement>('mark[data-follow-up-id][role="button"]');
  if (mark && props.followUpAllowed && (window.getSelection()?.isCollapsed ?? true)) {
    openFollowUpPath(mark.dataset.followUpId!,mark); return;
  }
  const listen = target.closest<HTMLButtonElement>('button[data-music-example-index]');
  if (listen && (event.currentTarget as Element).contains(listen)) {
    const part = parts[Number(listen.dataset.musicExampleIndex)];
    if (part?.kind === 'music_example') {
      pianoSourceTurn.value = turn.turnId;
      void playPiano(part.example, listen.dataset.musicPitch === undefined ? undefined : Number(listen.dataset.musicPitch));
    }
    return;
  }
  const button = target.closest<HTMLButtonElement>("button[data-answer-source-index]");
  if (!button || !(event.currentTarget as Element).contains(button)) return;
  const part = parts[Number(button.dataset.answerSourceIndex)];
  if (part?.kind !== "sources") return;
  event.stopPropagation();
  void openAgentSources(turn, part.source_ref_ids, { currentTarget: button });
}

function closeAgentSourcePopup() {
  sourceRequestSequence += 1;
  agentSourcePopup.value = null;
}

function onFullscreenKeydown(event: KeyboardEvent) {
  if (event.defaultPrevented) return;
  if (event.key === 'Escape' && followUpOpen.value && !wideConversation.value) {
    event.preventDefault(); closeFollowUpPath(); return;
  }
  if (event.key === 'Escape' && mobileToolsOpen.value) { event.preventDefault(); mobileToolsOpen.value = false; return; }
  if (event.key === 'Escape' && mobileConversation.value && mobileResponseTurn.value) { event.preventDefault(); closeResponseEditor(); return; }
  // The native share dialog owns Escape, including after a browser download moves focus.
  if (event.key === 'Escape' && document.querySelector('.share-image-panel[open]')) return;
  if (event.key === 'Escape' && taskDetailsOpen.value) { event.preventDefault(); closeTaskDetails(); return; }
  if (event.key === 'Escape' && contextMenuOpen.value) {
    event.preventDefault(); contextMenuOpen.value = false; contextMenuButton.value?.focus(); return;
  }
  if (event.key === 'Escape' && chatMenuOpen.value) {
    event.preventDefault(); chatMenuOpen.value = false; chatMenuButton.value?.focus(); return;
  }
  if (event.key === 'Escape' && recapOpen.value) { event.preventDefault(); closeRecap(); return; }
  if (event.key === 'Escape' && agentSourcePopup.value) { event.preventDefault(); closeAgentSourcePopup(); return; }
  if (event.key !== "Escape" || !props.fullscreen) return;
  if (document.querySelector(".agent-presentation.expanded")) return;
  event.preventDefault();
  if (historyOpen.value) { historyOpen.value = false; return; }
  emit("toggle-fullscreen");
  void nextTick(() => focusChatAction(fullscreenButton.value));
}
window.addEventListener("keydown", onFullscreenKeydown);
onBeforeUnmount(() => {
  conversationMedia.removeEventListener('change', updateConversationViewport);
  mobileConversationMedia.removeEventListener('change', updateConversationViewport);
  window.visualViewport?.removeEventListener('resize', keepMobileResponseVisible);
  inputResizeObserver?.disconnect();
  answerSelectionObserver?.disconnect();
  document.removeEventListener('selectionchange', onAnswerSelectionChange);
  document.removeEventListener('pointerup', finishAnswerSelection);
  document.removeEventListener('pointercancel', cancelAnswerDrag);
  window.removeEventListener('blur', cancelAnswerDrag);
  window.removeEventListener('scroll', positionAnswerSelection, true);
  window.removeEventListener('resize', positionAnswerSelection);
  window.visualViewport?.removeEventListener('resize', positionAnswerSelection);
  window.visualViewport?.removeEventListener('scroll', positionAnswerSelection);
  document.removeEventListener('pointerdown', dismissChatMenu);
  window.removeEventListener("keydown", onFullscreenKeydown);
});

async function openAgentSources(turn: Pick<ChatTurn, 'turnId'>, sourceRefIds: string[], event: { currentTarget: EventTarget | null }, fromRecap = false, noteMemId?: string) {
  if (!turn.turnId || sourceRefIds.length === 0) return;
  const requestSequence = ++sourceRequestSequence;
  agentSourcePopup.value = {
    fromRecap, noteMemId,
    turnId: turn.turnId,
    sourceRefIds: [...sourceRefIds],
    activeSourceRefId: sourceRefIds[0],
    sources: [],
    loading: true,
    opening: false,
    error: null,
    trigger: event.currentTarget instanceof HTMLElement ? event.currentTarget : null,
  };
  try {
    const sources = await Promise.all(
      sourceRefIds.map((sourceRefId) => api.agentSourceResolve(turn.turnId!, sourceRefId, noteMemId)),
    );
    if (requestSequence !== sourceRequestSequence || !agentSourcePopup.value) return;
    agentSourcePopup.value.sources = sources;
    agentSourcePopup.value.loading = false;
  } catch (error) {
    if (requestSequence !== sourceRequestSequence || !agentSourcePopup.value) return;
    agentSourcePopup.value.loading = false;
    agentSourcePopup.value.error = error instanceof Error ? error.message : String(error);
  }
}

function selectAgentSource(sourceRefId: string) {
  if (agentSourcePopup.value?.sources.some((source) => source.source_ref_id === sourceRefId)) {
    agentSourcePopup.value.activeSourceRefId = sourceRefId;
  }
}

function retryAgentSource() {
  const popup = agentSourcePopup.value;
  if (!popup) return;
  if (activeAgentSource.value) void openActiveAgentSourceInReader();
  else void openAgentSources(popup, popup.sourceRefIds, { currentTarget: popup.trigger }, popup.fromRecap, popup.noteMemId);
}

async function openActiveAgentSourceInReader() {
  const popup = agentSourcePopup.value;
  const source = activeAgentSource.value;
  if (!popup || !source || source.stale || !source.can_open_in_reader || popup.opening) return;
  if (source.media) { emit('media-source-enter', source.media); closeAgentSourcePopup(); return; }
  if (source.music) { emit('music-source-enter', source.music); closeAgentSourcePopup(); return; }
  const requestSequence = sourceRequestSequence;
  popup.opening = true;
  popup.error = null;
  emit("agent-source-will-open", { turnId: popup.turnId, sourceRefId: source.source_ref_id });
  try {
    const opened = await api.agentSourceOpen(popup.turnId, source.source_ref_id, popup.noteMemId);
    if (requestSequence !== sourceRequestSequence) return;
    if (opened.source_url) {
      window.open(opened.source_url, "_blank", "noopener,noreferrer");
      popup.opening = false;
      return;
    }
    closeAgentSourcePopup();
    emit("agent-source-opened", opened.evidence_range);
  } catch (error) {
    if (requestSequence !== sourceRequestSequence || !agentSourcePopup.value) return;
    agentSourcePopup.value.opening = false;
    agentSourcePopup.value.error = error instanceof Error ? error.message : String(error);
  }
}

function onAgentInputKeydown(event: KeyboardEvent) {
  if (event.isComposing || event.keyCode === 229 || inputComposing.value) return;
  if (composerSuggestionVisible.value && suggestionTab(event, inputComposing.value)) {
    event.preventDefault(); event.stopPropagation(); void acceptComposerSuggestion(); return;
  }
  if (event.key === 'Escape' && composeExpanded.value) {
    event.preventDefault(); event.stopPropagation(); collapseAgentInput(); return;
  }
  if ((event.ctrlKey || event.metaKey) && event.key === "Enter") { event.preventDefault(); submitMessage(); }
}

const notesPanel = ref<HTMLElement | null>(null);
async function showMemory(memId: string): Promise<boolean> {
  activeTab.value = 'notes';
  noteQuery.value = ''; noteFilter.value = 'all'; openNoteDetails.value.add(memId);
  await nextTick();
  const card = Array.from(notesPanel.value?.querySelectorAll<HTMLElement>('[data-mem-id]') ?? [])
    .find(el => el.dataset.memId === memId);
  if (!card) return false;
  if (card instanceof HTMLDetailsElement) card.open = true;
  card.scrollIntoView({ block: 'nearest', behavior: 'instant' });
  card.focus({ preventScroll: true });
  return true;
}
function replaceMemory(oldId: string, record: MemoryRecord) {
  if (openNoteDetails.value.delete(oldId)) openNoteDetails.value.add(record.mem_id);
  if (retainedNote.value?.mem_id === oldId) retainedNote.value = record;
  if (noteFollowUp.value?.mem_id === oldId) noteFollowUp.value = record;
}
function focusActivityHelp(deliveryRefs: string[]) {
  const node = Array.from(document.querySelectorAll<HTMLElement>('[data-delivery-ref]'))
    .find(node => deliveryRefs.includes(node.dataset.deliveryRef ?? ''));
  node?.scrollIntoView({ block: 'center' });
  node?.querySelector<HTMLButtonElement>('[data-help-action]')?.focus();
}
function prepareProductTour() {
  if (mobileResponseTurn.value) return false;
  if (workspace.value) workspace.value.suspended = true;
  mobileToolsOpen.value = false; historyOpen.value = false;
  return true;
}
function showCourseFeedback() { if (transcriptRef.value) transcriptRef.value.scrollTop = 0; }
defineExpose({ showCourseFeedback, focusActivityHelp, scrollToTurn, scrollToFollowUpOrigin, showMemory, replaceMemory, openRecapTarget, openNotePresentation, locateScene, prepareCompletionFeedback, prepareProductTour });

watch(() => props.activeChatSessionId, () => { closeAgentSourcePopup(); clearAnswerSelection(); });
watch(activeTab, clearAnswerSelection);
watch(() => props.chat, () => {
  const popup = agentSourcePopup.value;
  if (!popup || popup.fromRecap) return;
  const turn = props.chat.find(turn => turn.turnId === popup.turnId);
  const view = turn?.pending ? turn.draft?.view : turn?.outcome?.answer_view;
  if (!view?.parts.some(part => part.kind === 'presentation') && !popup.sourceRefIds.every(id => view?.sources.some(source => source.source_ref_id === id))) closeAgentSourcePopup();
}, { deep: true });

function clearAnswerSelection() {
  answerSelection.value = null;
  answerSelectionRange = null;
  answerSelectionError.value = '';
  answerSelectionPositioned.value = false;
}
function dismissAnswerSelection() {
  clearAnswerSelection();
  window.getSelection()?.removeAllRanges();
}
function startAnswerSelection(event: PointerEvent) {
  if (event.button !== 0) return;
  if ((event.target as HTMLElement | null)?.closest('[data-note-attachment]')) return;
  clearAnswerSelection();
  answerSelectionDragging = true;
}
function finishAnswerSelection() {
  if (!answerSelectionDragging) return;
  answerSelectionDragging = false;
  onAnswerSelectionChange();
}
function cancelAnswerDrag() {
  if (!answerSelectionDragging) return;
  answerSelectionDragging = false;
  clearAnswerSelection();
}
function onAnswerSelectionChange() {
  if (answerSelectionDragging) return;
  answerSelectionError.value = '';
  const sel = window.getSelection();
  if (!sel || sel.rangeCount === 0 || sel.isCollapsed || activeTab.value !== 'agent') {
    clearAnswerSelection();
    return;
  }
  const range = sel.getRangeAt(0);
  const start = range.startContainer.nodeType === 3 ? range.startContainer.parentElement : (range.startContainer as HTMLElement);
  const end = range.endContainer.nodeType === 3 ? range.endContainer.parentElement : (range.endContainer as HTMLElement);
  if (start?.closest('[data-note-attachment]') || end?.closest('[data-note-attachment]')) { clearAnswerSelection(); return; }
  const startAnswer = start?.closest?.(".ans-text");
  if (!startAnswer || startAnswer !== end?.closest?.(".ans-text") || !transcriptRef.value?.contains(startAnswer)) {
    clearAnswerSelection();
    return;
  }
  const turnId = startAnswer.closest<HTMLElement>('.turn')?.dataset.turnId;
  const turn = props.chat.find(candidate => candidate.turnId === turnId);
  if (!turn || turn.pending || turn.error || !turn.turnId || !turn.outcome) {
    clearAnswerSelection();
    return;
  }
  const root = start?.closest<HTMLElement>('.answer-markdown');
  const part = root ? turnDisplayParts(turn)[Number(root.dataset.answerPart)] : undefined;
  const text = root && part?.kind === 'html' && root.contains(range.endContainer)
    ? answerNoteExcerpt(range, part.markdown, root) : null;
  if (!text) {
    answerSelectionError.value = uiText('这段选区无法对应单个连续的回答片段，请在同一段回答文字内重新选择。');
  }
  answerSelectionRange = range.cloneRange();
  answerSelectionPositioned.value = false;
  const anchor = props.followUpAllowed && !turn.outcome.incomplete && root && part?.kind === 'html'
    ? answerFollowUpAnchor(range,part,root) : null;
  answerSelection.value = { x: 0, y: 0, text, turn, anchor };
  void nextTick(positionAnswerSelection);
}
function positionAnswerSelection() {
  const selected = answerSelection.value;
  const range = answerSelectionRange;
  const popup = answerPopover.value;
  const transcript = transcriptRef.value;
  if (!selected || !range || !popup || !transcript) return;
  const bounds = transcript.getBoundingClientRect();
  const viewport = window.visualViewport;
  const top = Math.max(bounds.top, viewport?.offsetTop ?? 0);
  const bottom = Math.min(bounds.bottom, (viewport?.offsetTop ?? 0) + (viewport?.height ?? window.innerHeight));
  const rects = Array.from(range.getClientRects()).filter(rect => rect.height > 0 && rect.bottom > top && rect.top < bottom);
  if (!bounds.height || !rects.length || !transcript.contains(range.commonAncestorContainer)) {
    clearAnswerSelection();
    return;
  }
  const first = rects[0], last = rects[rects.length - 1];
  const popupBounds = popup.getBoundingClientRect();
  const above = first.top - popupBounds.height - 10 >= top + 4;
  const anchor = above ? first : last;
  const left = viewport?.offsetLeft ?? 0;
  const right = left + (viewport?.width ?? window.innerWidth);
  selected.x = Math.max(left + 8, Math.min(anchor.right - popupBounds.width, right - popupBounds.width - 8));
  selected.y = Math.max(top + 4, Math.min(above ? first.top - popupBounds.height - 10 : last.bottom + 10, bottom - popupBounds.height - 4));
  answerSelectionPositioned.value = true;
}
function saveAnswerSelection(turn: ChatTurn) {
  const selected = answerSelection.value;
  if (!selected?.text || selected.turn !== turn) return;
  emit("save-answer-selection", turn, selected.text);
  clearAnswerSelection();
  window.getSelection()?.removeAllRanges();
}
function shareAnswerSelection() {
  const selected = answerSelection.value;
  if (!selected?.text || !deliveredAnswerText(selected.turn)) return;
  emit('share-answer', selected.turn, selected.text);
  clearAnswerSelection();
  window.getSelection()?.removeAllRanges();
}
function clarifySelection() {
  const selected = answerSelection.value;
  if (!selected?.text || !selected.turn.turnId || !deliveredAnswerText(selected.turn) || props.sending) return;
  emit('clarify-answer', selected.turn.turnId, selected.text);
  dismissAnswerSelection();
}
function branchFromSelection() {
  const selection = answerSelection.value;
  if (!props.followUpAllowed || !selection?.anchor || !selection.turn.turnId) return;
  const excerpt = selection.anchor.excerpt;
  const prior = visibleDrafts.value.find(draft => draft.parent_session_id === props.activeChatSessionId &&
    draft.parent_turn_id === selection.turn.turnId && draft.anchor.start_part === selection.anchor!.start_part &&
    draft.anchor.start_offset === selection.anchor!.start_offset && draft.anchor.end_part === selection.anchor!.end_part &&
    draft.anchor.end_offset === selection.anchor!.end_offset && draft.anchor.excerpt === excerpt);
  const draft = prior ?? { parent_session_id:props.activeChatSessionId, parent_turn_id:selection.turn.turnId,
    anchor:selection.anchor, client_request_id:crypto.randomUUID(), question:uiText('这里说的「{0}」是什么意思？',[compactText(excerpt,80)]) };
  if (!prior) followUpDrafts.value = [...followUpDrafts.value,draft];
  dismissAnswerSelection();
  openFollowUpPath('draft:' + draft.client_request_id,null);
}
function openFollowUpPath(id: string, trigger: HTMLElement | null) {
  if (!props.followUpAllowed) return;
  if (!followUpOpen.value && !wideConversation.value) {
    prePreviewTop = transcriptRef.value?.scrollTop ?? null;
    pathTrigger = trigger;
  }
  followUpOpen.value = true;
  emit('follow-up-load');
  previewFollowUp(id);
  if (followUpLayout.value !== 'sheet') void nextTick(() => followUpPanel.value?.focusNode(id));
}
function previewFollowUp(id: string) {
  previewId.value = id;
  const origin = previewDraft.value ?? pathNodes.value.find(n => n.session_id === id)?.origin;
  if (origin?.parent_session_id === props.activeChatSessionId && followUpLayout.value !== 'sheet')
    void revealFollowUpQuote(origin);
}
function closeFollowUpPath() {
  followUpOpen.value = false;
  const top = prePreviewTop, trigger = pathTrigger, current = props.activeChatSessionId;
  prePreviewTop = null; pathTrigger = null;
  void nextTick(() => {
    if (current !== props.activeChatSessionId) return;
    if (top !== null && transcriptRef.value) transcriptRef.value.scrollTop = top;
    const target = trigger?.isConnected && trigger.getClientRects().length ? trigger : followUpToggle.value;
    target?.focus({preventScroll:true});
  });
}
function discardFollowUp() {
  followUpDrafts.value = followUpDrafts.value.filter(draft => draft !== previewDraft.value);
  previewId.value = props.activeChatSessionId;
  if (!wideConversation.value) closeFollowUpPath();
}
function editFollowUpQuestion(text: string) {
  if (previewDraft.value) followUpDrafts.value = followUpDrafts.value.map(draft =>
    draft === previewDraft.value ? {...draft,question:text} : draft);
}
function activateFollowUpMark(event:KeyboardEvent) {
  if (event.key !== 'Enter' && event.key !== ' ') return;
  const mark = (event.target as HTMLElement).closest<HTMLElement>('mark[data-follow-up-id][role="button"]');
  if (!mark || !props.followUpAllowed) return;
  event.preventDefault(); openFollowUpPath(mark.dataset.followUpId!,mark);
}
function alignTranscript(rect:DOMRect) {
  const transcript = transcriptRef.value;
  if (!transcript) return;
  const box = transcript.getBoundingClientRect();
  if (rect.top < box.top + 8 || rect.bottom > box.bottom - 12) {
    transcript.scrollTop += rect.top - box.top - Math.min(40, box.height / 4);
  }
}
async function revealFollowUpQuote(origin:Pick<FollowUpOrigin,'parent_session_id'|'parent_turn_id'|'anchor'>):Promise<boolean> {
  await nextTick();
  if (props.activeChatSessionId !== origin.parent_session_id) return false;
  const turn = props.chat.find(t => t.turnId === origin.parent_turn_id);
  const root = answerRoots.get(origin.parent_turn_id);
  if (!root || !turn) return false;
  const parts = turnDisplayParts(turn);
  for (const element of root.querySelectorAll<HTMLElement>('[data-answer-part]')) {
    const part = parts[Number(element.dataset.answerPart)];
    if (part?.kind !== 'html') continue;
    const range = answerFollowUpRange(part,element,origin.anchor);
    if (!range) continue;
    followTranscript.value = false;
    if (typeof range.getBoundingClientRect === 'function') alignTranscript(range.getBoundingClientRect());
    for (const mark of element.querySelectorAll<HTMLElement>('mark[data-follow-up-id]')) {
      if (range.intersectsNode(mark)) mark.animate?.([{backgroundColor:'var(--brand-accent-soft)'},{backgroundColor:'transparent'}],
        {duration:window.matchMedia('(prefers-reduced-motion: reduce)').matches ? 0 : 1500});
    }
    return true;
  }
  return false;
}
async function scrollToFollowUpOrigin(origin:FollowUpOrigin):Promise<boolean> {
  if (await revealFollowUpQuote(origin)) return true;
  if (props.activeChatSessionId === origin.parent_session_id) await scrollToTurn(origin.parent_turn_id);
  return false;
}
function compactText(value: string, max = 96): string {
  const text = value.replace(/\s+/g, " ").trim();
  return text.length > max ? `${text.slice(0, max)}...` : text;
}
function leadingQuote(content: string): string | null {
  const lines = content.split("\n");
  const quoteLines: string[] = [];
  for (const line of lines) {
    if (line.startsWith(">")) quoteLines.push(line.replace(/^>\s?/, ""));
    else if (quoteLines.length > 0 && line.trim() === "") break;
    else if (quoteLines.length > 0) break;
  }
  const quote = quoteLines.join(" ").replace(/\s+/g, " ").trim();
  return quote || null;
}
function selectionEvidence(turn: ChatTurn): import('../api').EvidenceRange[] {
  const stored = turn.questionQuote?.evidence_ranges;
  if (stored?.length) return stored;
  const ranges = turn.questionSelection?.ranges;
  return turn.questionSelection?.status === 'resolved' && ranges?.length
    ? [{start_lid:ranges[0].lid,end_lid:ranges[ranges.length-1].lid,ranges}]
    : [];
}
function askQuoteLabel(quote: DisplayQuestionQuote): string {
  if ("label" in quote) return quote.label;
  return quote.status === "partial" ? uiText('部分定位引用') : uiText('引用来源');
}
function annotationLocationLabel(record: MemoryRecord): string | null {
  const status = props.annotationLocation?.[record.mem_id];
  if (status === "exact") return uiText('PDF 已定位');
  if (status === "partial") return uiText('PDF 部分定位');
  if (status === "unmapped") return uiText('PDF 未定位');
  return null;
}
function noteSourceLid(note: MemoryRecord): string | null {
  if (note.type === 'highlight' || note.selection_context) return note.anchor.lid ?? null;
  if (note.note_placement?.kind === 'pdf_region' && note.note_placement.source_fingerprint === props.noteSourceFingerprint && props.annotationLocation?.[note.mem_id] === 'exact') return note.anchor.lid ?? null;
  if (note.note_placement?.kind !== "lid_block"
    || note.note_placement.source_fingerprint !== props.noteSourceFingerprint
    || note.anchor.lid !== note.note_placement.lid) {
    return null;
  }
  return note.note_placement.lid;
}
function canFocusNoteSource(note: MemoryRecord): boolean {
  return noteSourceLid(note) !== null;
}
function noteSourceLabel(note: MemoryRecord): string {
  const placement = note.note_placement;
  if (placement?.source_fingerprint && placement.source_fingerprint !== props.noteSourceFingerprint) return uiText('来源已变更');
  if (placement?.kind === 'pdf_region') return uiText('第 {0} 页', [placement.page_index + 1]);
  return noteAssociationTitle(note);
}
function notePlacementActionLabel(note: MemoryRecord): string | null {
  if (!props.unquotedNotePlacementAvailable || note.selection_context) return null;
  if (!note.note_placement) return uiText('放置到正文');
  const surface = props.notePlacementSurface ?? "markdown";
  if ((surface === "markdown" && note.note_placement.kind !== "lid_block")
    || (surface === "pdf" && note.note_placement.kind !== "pdf_region")) return null;
  if (note.note_placement.source_fingerprint !== props.noteSourceFingerprint) return uiText('重新放置');
  if (note.note_placement.kind === "pdf_region") {
    return props.annotationLocation?.[note.mem_id] === "exact" ? uiText('移动') : uiText('重新放置');
  }
  return uiText('移动');
}
function isLongNote(note: MemoryRecord): boolean {
  return note.content.length > 360 || note.content.split("\n").length > 8;
}
function excerpt(rec: MemoryRecord): string {
  const c = rec.content.replace(/\s+/g, " ").trim();
  return c.length > 120 ? `${c.slice(0, 120)}…` : c;
}
function turnSourceLabel(turn: ChatSessionTurnSummary): string | null {
  return turn.question_source_label ?? turn.question_quote?.label ?? null;
}
function openHistorySession(sessionId: string) {
  if (!sessionId || sessionId === props.activeChatSessionId) return;
  emit("select-chat", sessionId);
  historyOpen.value = false;
}
function openHistorySessionFromCard(sessionId: string, event: MouseEvent | KeyboardEvent) {
  const target = event.target instanceof HTMLElement ? event.target : null;
  if (target?.closest("button,a,input,textarea,select")) return;
  if (event instanceof KeyboardEvent) event.preventDefault();
  openHistorySession(sessionId);
}
function deleteHistorySession(sessionId: string) {
  if (!sessionId) return;
  emit("delete-chat", sessionId);
}

function selectTab(tab: ContextTab) {
  activeTab.value = tab;
  contextMenuOpen.value = false;
  chatMenuOpen.value = false;
  if (tab === "profile") emit("refresh-profile");
  if (tab === 'formula') emit('request-formula');
}

function memoryUpdateLabel(update: ProfileMemoryUpdate): string {
  return {
    remembered: uiText('画像已记住'),
    corrected: uiText('画像已纠正'),
    forgotten: uiText('画像已忘记'),
    needs_clarification: uiText('画像需要澄清'),
    needs_sensitive_confirmation: uiText('敏感画像等待确认'),
    sensitive_confirmation_cancelled: uiText('敏感画像保存已取消'),
    rejected: uiText('画像更新未保存'),
  }[update.kind];
}

function memoryUpdateKey(turnIndex: number, updateIndex: number): string {
  return `${turnIndex}:${updateIndex}`;
}

function canUndoMemoryUpdate(update: ProfileMemoryUpdate): boolean {
  return update.kind === "remembered" || update.kind === "corrected";
}

function hasProfileUsage(usage: ProfileUsageTrace): boolean {
  return usage.injected_fact_ids.length > 0
    || usage.claimed_used_fact_ids.length > 0
    || usage.influences.length > 0;
}

function influenceLabel(influence: ProfileUsageTrace["influences"][number]): string {
  return {
    retrieval_plan: uiText('检索计划'),
    explanation_depth: uiText('讲解深度'),
    terminology: uiText('术语选择'),
    example_choice: uiText('示例选择'),
    navigation: uiText('阅读导航'),
  }[influence];
}
</script>

<template>
  <aside class="right-rail" :class="{ fullscreen: props.fullscreen && !workspaceVisible, 'conversation-wide': wideConversation, 'conversation-mobile': mobileConversation, 'compact-conversation': useLearningSheet, 'presentation-workspace': workspaceVisible, 'discussion-open': discussionOpen }" :style="{ '--presentation-split': `${split}%`, '--presentation-top': `${workspaceTop}px` }">
    <div v-if="workspaceVisible" ref="workspaceTools" class="workspace-tools">
      <div v-if="$slots['learning-details']" class="workspace-learning-details"><slot name="learning-details" /></div>
      <label class="workspace-split">{{ uiText('演示宽度') }} <input v-model="split" type="range" min="40" max="72" :aria-label="uiText('演示宽度')" /></label>
      <button class="discussion-toggle" type="button" :aria-expanded="discussionOpen" @click="discussionOpen = !discussionOpen"><span class="discussion-indicator" aria-hidden="true"></span>{{ discussionOpen ? uiText('收起讨论') : uiText('展开讨论') }}<ChevronDown :size="14" aria-hidden="true" /></button>
      <button v-if="stopAvailable" type="button" class="stop-work-button" :disabled="stopping" @click="stopAgent"><Square :size="12" fill="currentColor" aria-hidden="true" />{{ stopping ? uiText('正在停止') : uiText('停止') }}</button>
    </div>
    <div v-if="!workspaceVisible" class="rail-toolbar agent-head" data-tour="reader-agent">
      <div v-show="mobileConversation" class="mobile-conversation-header">
        <button class="mobile-back" type="button" :aria-label="courseActive ? uiText('返回学习') : musicMode ? uiText('返回聆听') : uiText('返回阅读')" @click="closeResponseEditor(); collapseAgentInput(); returnReading()"><ChevronLeft :size="21" aria-hidden="true" /></button>
        <button ref="mobileLearningButton" class="mobile-learning-title" type="button" :aria-label="uiText('学习与会话')" :aria-expanded="mobileToolsOpen" @click="mobileToolsOpen = true"><span><YeyeMascot ref="mobileCompanion" variant="avatar" :size="24" :active="companionActive && mobileConversation && !mobileToolsOpen" />{{ props.musicMode ? uiText('音乐伴听') : props.videoImagesEnabled ? uiText('视频伴读') : uiText('问这本书') }}<ChevronDown :size="13" aria-hidden="true" /></span><small :class="{ attention: !!props.learningAttention }">{{ props.learningAttention || props.learningTitle || (props.musicMode ? uiText('从声音或谱面问起') : props.videoImagesEnabled ? uiText('一起弄懂正在看的内容') : uiText('围绕这本书聊一聊')) }}</small></button>
        <div ref="mobileRecordsHost" class="mobile-records" />
      </div>
      <h3 v-show="!mobileConversation" class="rail-title"><YeyeMascot ref="fullscreenCompanion" variant="avatar" :size="32" :active="companionActive && !mobileConversation && !!props.fullscreen && !historyOpen" />{{ props.musicMode ? uiText('音乐伴听') : props.videoImagesEnabled ? uiText('视频伴读') : uiText('问这本书') }}</h3>
      <div v-if="!props.fullscreen" class="context-tabs" role="tablist" :aria-label="musicMode ? uiText('辅助聆听功能') : uiText('辅助阅读功能')">
      <button
        v-for="tab in tabs.slice(0, 2)"
        :key="tab.id"
        class="tab"
        :class="{ active: activeTab === tab.id }"
        type="button"
        role="tab"
        :id="`reader-tab-${tab.id}`"
        :aria-selected="activeTab === tab.id"
        :aria-controls="`reader-panel-${tab.id}`"
        @click="selectTab(tab.id)"
      >
        <YeyeMascot v-if="tab.id === 'agent'" ref="tabCompanion" variant="avatar" :size="24" :active="companionActive && !mobileConversation && !props.fullscreen && !historyOpen" />
        {{ props.videoImagesEnabled && tab.id === 'agent' ? uiText('伴读') : tab.label }}
        <span v-if="tab.id === 'profile' && profileAttentionCount" class="tab-badge">
          {{ profileAttentionCount }}
        </span>
      </button>
      <div ref="contextTools" class="context-tools">
        <button ref="contextMenuButton" class="context-menu-toggle" :class="{ active: !['agent', 'profile'].includes(activeTab) }" type="button" :aria-label="musicMode ? uiText('伴听工具') : uiText('阅读工具')" :aria-expanded="contextMenuOpen" aria-controls="reader-secondary-tabs" @click="contextMenuOpen = !contextMenuOpen; chatMenuOpen = false">
          {{ tabs.find(tab => tab.id === activeTab && !['agent', 'profile'].includes(tab.id))?.label ?? uiText('工具') }}
          <ChevronDown :size="14" aria-hidden="true" />
        </button>
        <div v-show="contextMenuOpen" id="reader-secondary-tabs" class="secondary-tabs" role="tablist" :aria-label="musicMode ? uiText('更多伴听工具') : uiText('更多阅读工具')">
          <button v-for="tab in tabs.slice(2)" :key="tab.id" class="tab" :class="{ active: activeTab === tab.id }" type="button" role="tab" :id="`reader-tab-${tab.id}`" :aria-selected="activeTab === tab.id" :aria-controls="`reader-panel-${tab.id}`" @click="selectTab(tab.id)">
            {{ tab.label }}
          </button>
        </div>
      </div>
      </div>
      <button v-if="openGoals.length && activeTab === 'agent' && !mobileConversation" class="mobile-task-toggle" type="button" :aria-expanded="taskDetailsOpen" aria-controls="reader-task-details" @click="openTaskDetails">{{ openGoals.length }} {{ uiText('项待办') }}</button>
      <div v-show="!mobileConversation" ref="chatActions" class="chat-actions" :class="{ 'menu-open': chatMenuOpen }">
          <button v-show="!wideConversation" ref="chatMenuButton" class="chat-menu-toggle" type="button" :aria-label="uiText('问答操作')" :aria-expanded="chatMenuOpen" aria-controls="chat-secondary-actions" @click="chatMenuOpen = !chatMenuOpen; contextMenuOpen = false">{{ uiText('会话') }}<ChevronDown :size="14" aria-hidden="true" /></button>
          <Teleport :to="sessionToolsTarget || 'body'" :disabled="!sessionToolsTarget">
          <div v-show="chatMenuOpen || wideConversation || mobileConversation" id="chat-secondary-actions" class="chat-secondary-actions" @click.capture="closeChatMenuAfterAction">
          <button class="new-chat" :title="uiText('新对话')" :disabled="props.historyLoading || !!props.historyError" @click="emit('new-chat')">{{ uiText('新对话') }}</button>
          <button ref="recapButton" class="recap-button" :disabled="!props.activeChatSessionId || props.historyLoading || !!props.historyError" @click="recapOpen = true">{{ musicMode ? uiText('本次聆听回顾') : uiText('本次阅读回顾') }}</button>
          <button ref="historyButton" class="history-button" :title="uiText('打开对话历史')" :disabled="props.historyLoading || !!props.historyError" @click="historyOpen = true"> {{ uiText('本书历史') }} <span>{{ historySessions.length }}</span>
          </button>
          <slot name="learning-details" />
          </div>
          </Teleport>
          <button
            ref="fullscreenButton"
            type="button"
            class="fullscreen-button"
            :aria-label="props.fullscreen ? uiText('退出问答全屏') : uiText('问答全屏')"
            :aria-pressed="!!props.fullscreen"
            @click="emit('toggle-fullscreen')"
          ><Minimize2 v-if="props.fullscreen" :size="16" aria-hidden="true" /><Maximize2 v-else :size="16" aria-hidden="true" /></button>
        </div>
    </div>

    <MobileLearningSheet :open="useLearningSheet && mobileToolsOpen" :title="mobileConversation ? uiText('学习与会话') : uiText('学习设置')" @close="mobileToolsOpen = false">
      <div ref="mobileGoalHost" @click.capture="leaveMobileGoalTools" />
      <div v-if="!mobileConversation" class="mobile-tool-section"><p>{{uiText('学习记录')}}</p><div ref="recordsSheetHost" @click.capture="mobileToolsOpen=false"/></div>
      <div class="mobile-tool-section"><p>{{ uiText('学习方式') }}</p><div ref="mobileTeachingHost" /></div>
      <nav v-show="!workspaceVisible" class="mobile-tool-section" :aria-label="uiText('本书会话')"><p>{{ uiText('会话') }}</p><div ref="mobileSessionHost" /></nav>
      <nav v-if="!workspaceVisible" class="mobile-tool-section mobile-tool-links" :aria-label="musicMode ? uiText('伴听工具') : uiText('阅读工具')">
        <button v-if="openGoals.length" type="button" @click="mobileToolsOpen = false; openTaskDetails($event)">{{ openGoals.length }} {{ uiText('项待办') }}<ArrowRight :size="15" aria-hidden="true" /></button>
        <button v-for="tab in tabs.filter(tab => tab.id !== 'agent')" :key="tab.id" type="button" @click="mobileToolsOpen = false; selectTab(tab.id)">{{ tab.label }}<ArrowRight :size="15" aria-hidden="true" /></button>
        <button type="button" @click="mobileToolsOpen = false; emit('workspace-menu')">{{ uiText('阅读设置') }}<ArrowRight :size="15" aria-hidden="true" /></button>
      </nav>
    </MobileLearningSheet>

    <MobileLearningSheet v-if="followUpAllowed && mobileViewport" :open="hasPaths && followUpOpen && activeTab === 'agent'" :title="uiText('追问路径')" @close="closeFollowUpPath">
      <div ref="followUpSheet" data-follow-up-host="sheet" />
    </MobileLearningSheet>
    <section id="reader-panel-agent" v-show="activeTab === 'agent'" class="tab-panel agent-panel" role="tabpanel" :aria-labelledby="props.fullscreen ? undefined : 'reader-tab-agent'" :aria-label="props.fullscreen ? uiText('问这本书') : undefined">
      <aside v-show="wideConversation" class="conversation-sidebar conversation-context" :aria-label="uiText('学习与会话')">
        <div ref="goalSidebarHost" class="sidebar-goal" />
        <section class="sidebar-section"><p class="sidebar-label">{{ uiText('会话') }}</p><div ref="followUpSide" data-follow-up-host="side" /><nav ref="sessionSidebarHost" :aria-label="uiText('本书会话')" /></section>
        <div class="sidebar-section sidebar-teaching"><p class="sidebar-label">{{ uiText('学习方式') }}</p><div ref="teachingSidebarHost" /></div>
      </aside>
      <div class="conversation-main">
      <Teleport :to="goalToolsTarget || 'body'" :disabled="!goalToolsTarget">
      <slot name="learning-goal" />
      </Teleport>
      <div class="discussion-context" :class="{ 'compact-context': useLearningSheet && !mobileConversation }">
      <button v-if="useLearningSheet && !mobileConversation && ($slots['learning-goal'] || $slots['tutor-control'])" type="button" class="workspace-settings" :aria-label="uiText('学习设置')" aria-haspopup="dialog" :aria-expanded="mobileToolsOpen" @click="mobileToolsOpen = true">
        <SlidersHorizontal :size="15" aria-hidden="true" /><span>{{ uiText('学习设置') }}</span><small v-if="learningAttention || learningTitle" :title="learningAttention || learningTitle">{{ learningAttention || learningTitle }}</small>
      </button>
      <button v-if="openGoals.length" ref="taskSummaryButton" class="task-summary-toggle" type="button" :aria-expanded="taskDetailsOpen" aria-controls="reader-task-details" @click="openTaskDetails">
        <span class="task-summary-status">{{ taskSummary }}</span>
        <span v-if="!useLearningSheet" class="task-summary-title">{{ openGoals.length === 1 ? openGoals[0].interpretation : uiText('查看未完成任务') }}</span>
        <span class="task-summary-action">{{ uiText('查看任务') }}</span>
      </button>
      </div>
      <div v-if="taskDetailsOpen && openGoals.length" class="goal-overlay" @click.self="closeTaskDetails">
        <section id="reader-task-details" class="goal-details" role="dialog" :aria-label="uiText('当前任务详情')">
          <header class="goal-details-head"><strong>{{ uiText('当前任务') }}</strong><button ref="taskCloseButton" type="button" :aria-label="uiText('关闭任务详情')" @click="closeTaskDetails"><X :size="16" aria-hidden="true" /></button></header>
          <div class="goal-list" :aria-label="uiText('当前任务')">
            <div v-for="goal in openGoals" :key="goal.id" class="goal-card">
              <strong>{{ goal.interpretation }}</strong>
              <p>{{ runActive ? uiText('当前对话正在执行') : goalStopLabel(goal.last_stop_reason) }}<span v-if="goal.result_refs.length"> {{ uiText('· 已有部分结果') }}</span></p>
              <p v-if="goal.requirements.length" class="goal-requirements">{{ uiText('交付要求：') }}{{ goal.requirements.map(item => item.description === '制作并交付本次问题的演示页' ? uiText('制作并交付本次问题的演示页') : item.description).join('；') }}</p>
              <ul v-if="goal.working.items.length" class="goal-work-items"><li v-for="item in goal.working.items" :key="item.id"><span>{{ { pending: uiText('待做'), in_progress: uiText('进行中'), completed: uiText('已完成') }[item.status] }}</span>{{ item.description }}</li></ul>
              <div class="goal-actions">
                <button :disabled="runActive" @click="emit('continue-goal', goal.id); closeTaskDetails()">{{ uiText('继续任务') }}</button>
                <button :disabled="runActive" @click="supplementGoal(goal.id)">{{ uiText('补充要求') }}</button>
                <button :disabled="runActive" @click="emit('cancel-goal', goal.id); closeTaskDetails()">{{ uiText('结束任务') }}</button>
              </div>
            </div>
          </div>
        </section>
      </div>

      <AgentPresentation v-if="retainedNote && retainedReceipt" ref="retainedPresentation"
        :session-id="retainedReceipt.session_id" :turn-id="retainedReceipt.turn_id" :reference="retainedReceipt.reference"
        :note-mem-id="retainedNote.mem_id" :note-receipt="retainedReceipt" :workspace="workspaceVisible"
        :toolbar-target="workspaceVisible ? workspaceTools : undefined" :shared-question="props.agentInput"
        @update:shared-question="emit('update:agentInput', $event)" @follow-up="retainedSent"
        :busy="!props.activeChatSessionId || props.sending || props.historyLoading || !!props.historyError"
        @expand="workspace = $event ? { turnId: retainedReceipt!.turn_id, reference: retainedReceipt!.reference } : null"
        @source="(id, anchor) => openAgentSources({ turnId: retainedReceipt!.turn_id }, [id], { currentTarget: anchor }, true, retainedNote!.mem_id)" />
      <SessionRecap v-if="recapOpen" :session-id="props.activeChatSessionId" :session-title="props.chatSessions.find(session => session.id === props.activeChatSessionId)?.title" :navigate="openRecapTarget" @close="closeRecap" />
      <AgentPresentation v-if="recapPresentation" :session-id="props.activeChatSessionId" :turn-id="recapPresentation.turnId"
        :toolbar-target="workspaceVisible && workspaceKey === presentationKey(recapPresentation.turnId, recapPresentation.reference) ? workspaceTools : undefined"
        :ref="instance => { if (instance && recapPresentation) presentationInstances.set(presentationKey(recapPresentation.turnId, recapPresentation.reference), instance as InstanceType<typeof AgentPresentation>); }"
        :reference="recapPresentation.reference" :workspace="workspaceVisible && workspaceKey === presentationKey(recapPresentation.turnId, recapPresentation.reference)"
        :busy="props.sending || props.historyLoading || !!props.historyError" @follow-up="presentationSent" @record-note="(receipt, title, summary) => emit('record-note', receipt, title, summary)"
        @add-to-board="receipt => emit('add-to-board', { kind: 'presentation', session_id: receipt.session_id, turn_id: receipt.turn_id, reference: receipt.reference, receipt })"
        @source="(id, anchor) => openAgentSources({ turnId: recapPresentation!.turnId }, [id], { currentTarget: anchor }, true)"
        @expand="selectPresentation(recapPresentation.turnId, recapPresentation.reference, $event)" />
      <div v-if="followUpAllowed && activeBranch?.origin" class="follow-up-origin">
        <button v-if="canReturnToParent" type="button" :disabled="followUpBusy" @click="emit('follow-up-parent')"><ChevronLeft :size="15" aria-hidden="true" />{{ uiText('回到原回答') }}</button>
        <small v-else>{{ uiText('原回答已不可用') }}</small>
        <span :title="activeBranch.origin.anchor.excerpt">{{ uiText('来自「{0}」',[activeBranch.origin.anchor.excerpt]) }}</span>
      </div>
      <div ref="transcriptRef" class="transcript" @scroll="trackTranscriptScroll" @keydown="activateFollowUpMark">
      <slot name="course-feedback" />
        <p v-if="props.historyLoading" role="status">{{ musicMode ? uiText('正在恢复对话，可以先听音乐或查看乐谱。') : uiText('正在恢复对话，可以先阅读正文。') }}</p>
        <div v-else-if="props.historyError" role="alert">
          <p>{{ uiText('对话暂时无法加载：') }}{{ props.historyError }}</p>
          <button @click="emit('retry-history')">{{ uiText('重试') }}</button>
        </div>
        <div v-for="(turn, ti) in props.chat" :key="turn.turnId ?? ti" class="turn" :data-turn-id="turn.turnId || undefined">
          <details v-if="turn.interaction?.answer_ref" class="turn-quote">
            <summary>{{ uiText('接续的原回答') }}</summary>
            <blockquote>{{ turn.interaction.answer_text }}</blockquote>
            <button @click="scrollToTurn(turn.interaction.answer_ref.turn_id)">{{ uiText('回到原回答') }}</button>
          </details>
          <div v-if="turn.questionQuote" class="turn-quote">
            <div class="turn-quote-head">
              <span>{{ askQuoteLabel(turn.questionQuote) }}</span>
              <button v-if="selectionEvidence(turn).length" type="button" @click="emit('original-selection', selectionEvidence(turn))">{{ uiText('回到原选区') }}</button>
            </div>
            <blockquote>{{ turn.questionQuote.quote }}</blockquote>
          </div>
          <p v-if="turn.musicQuestion" class="turn-video-context">{{ uiText('音乐问题 ·') }} {{ videoTime(turn.musicQuestion.audio.start_ms) }}–{{ videoTime(turn.musicQuestion.audio.end_ms) }}<span v-if="turn.musicQuestion.occurrence_id"> {{ uiText('· 当前演奏片段') }}</span></p>
          <p v-if="turn.videoQuestion" class="turn-video-context">{{ uiText('视频问题 ·') }} {{ videoTime(turn.videoQuestion.frame.actual_pts_ms) }}{{ turn.videoQuestion.region ? uiText(' · 已选区域') : '' }}<span v-if="turn.videoQuestion.range"> {{ uiText('· 时间范围') }} {{ videoTime(turn.videoQuestion.range.start_ms) }}–{{ videoTime(turn.videoQuestion.range.end_ms) }}</span></p>
          <details v-if="turn.musicQuestion?.image || turn.videoQuestion?.image" class="turn-question-image"><summary><img :src="questionImageUrl((turn.musicQuestion?.image || turn.videoQuestion?.image)!)" :alt="uiText('本次提问图片')" />{{ uiText('查看提问图片') }}</summary><img :src="questionImageUrl((turn.musicQuestion?.image || turn.videoQuestion?.image)!)" :alt="uiText('本次提问图片放大预览')" /></details>
          <MessageThoughts v-if="turn.turnId && !turn.pending" :session-id="props.activeChatSessionId" :turn-id="turn.turnId" role="user" :text="turn.user"><p class="u-msg">{{ turn.user }}</p></MessageThoughts><p v-else class="u-msg">{{ turn.user }}</p>
          <button v-if="turn.turnId && !turn.pending && turn.user.trim()" class="quiet" @click="rememberUserThought(turn.turnId)">留为我的想法</button>
          <div v-if="turn.noteReferences?.length && !turn.pending" class="turn-note-links"><span>这次用到的想法</span><button v-for="(reference,index) in turn.noteReferences" :key="`${reference.note_id}:${reference.revision}`" @click="openPersonalNotes(reference)">想法 {{index+1}} · 第 {{reference.revision}} 版</button><button v-if="turn.turnId" @click="emit('continue-personal-note',turn.turnId,turn.noteReferences[0]!)">围绕这些想法补充</button></div>
          <p v-if="thoughtCaptureError" role="alert">{{ thoughtCaptureError }}</p>
          <button v-if="turn.boardFollowUp" type="button" class="board-location board-question-link"
            @click="emit('open-board-location', { board_id: turn.boardFollowUp.board_id, item_id: turn.boardFollowUp.item_id, block_id: turn.boardFollowUp.block_id })">
            <span>{{ uiText('回到画板提问对象') }}</span><ArrowRight :size="14" aria-hidden="true" />
          </button>
          <PresentationSceneCard v-if="turn.presentationFollowUp" :receipt="turn.presentationFollowUp" :mem-id="props.contextNotes.find(n => n.note?.association.kind === 'presentation' && n.note.association.receipt.saved_state_ref === turn.presentationFollowUp?.saved_state_ref)?.mem_id" @locate="locateScene(turn.presentationFollowUp)" @restore="locateScene(turn.presentationFollowUp, true)" />
          <p v-if="turn.localAgentName" class="turn-agent-name">{{ uiText('本轮 Agent') }} · {{ turn.localAgentName }}</p>
          <AgentActivities v-if="turn.activities?.length" :activities="turn.activities" :active="turn.pending" />
          <template v-if="!network.enabled && turn.pending && turn.turnId">
            <AgentPermission v-for="step in turn.activities?.filter(activity => activity.name === 'acp.permission' && activity.status === 'running')"
              :key="step.step_id" :turn-id="turn.turnId" />
          </template>
          <div v-if="turn.runStatus || (turn.pending && stopAvailable)" class="run-progress-footer">
            <p v-if="turn.runStatus && (turn.runStatus !== uiText('正在运行') || !turn.activities?.length)" class="run-status" role="status">{{ turn.runStatus }}</p>
            <button v-if="turn.pending && stopAvailable" type="button" class="stop-work-button" :disabled="stopping" @click="stopAgent"><Square :size="12" fill="currentColor" aria-hidden="true" />{{ stopping ? uiText('正在停止') : uiText('停止') }}</button>
          </div>
          <p v-if="turn.pending && !turn.runStatus && !turn.activities?.length" class="pending">{{ uiText('正在接收运行...') }}</p>
          <YeyeMascot v-if="wideConversation && turn.pending && props.waitingTurnId === turn.turnId && !turn.draft?.view && !turn.error"
            class="waiting-mascot" variant="reading" :size="144" motion="waiting" :active="companionActive" />
          <div v-if="!turn.outcome && turn.liveEffects?.length" class="live-effects">
            <p v-for="item in turn.liveEffects" :key="item.effect_id">{{ props.effLabel(item.effect) }}</p>
          </div>
          <div v-if="turn.pending && turn.draft?.view" class="answer-draft ans-text md" :aria-label="uiText('回答草稿')" @click="onAnswerSourceClick(turn, turn.draft.view.parts, $event)">
            <template v-for="(part, pi) in turnDisplayParts(turn, true)" :key="`${turn.draft.message_id}-${turn.draft.revision}-${pi}`">
              <template v-if="part.kind === 'html'">
                <div v-for="(block, bi) in part.blocks" :key="bi" v-memo="[block]" class="answer-markdown" v-html="block"></div>
              </template>
            </template>
          </div>
          <div v-if="!turn.pending && turn.spendStop" class="incomplete" role="status">
            <p>{{ turn.spendStop.message }}</p>
            <button v-if="network.enabled" type="button" @click="emit('show-allowance')">{{ uiText('查看使用额度') }}</button>
          </div>
          <div v-else-if="!turn.pending && turn.error" class="failed-question" role="status">
            <p class="incomplete">{{ turn.error }}</p>
            <button v-if="turn.localAgentUnavailable && $slots['agent-picker']" type="button" class="retry-question"
              :disabled="runActive" @click="emit('failed-agent-settings', turn.localAgentId)">{{ turn.localAgentId ? uiText('检查本轮 Agent 连接') : uiText('管理我的 Agent…') }}</button>
            <button v-if="!turn.submissionUnknown && !turn.boardFollowUp && !turn.presentationFollowUp && turn.user.trim()" type="button" class="retry-question"
              :disabled="sending || historyLoading || !!historyError || questionInputBlocked" @click="emit('edit-failed-question', turn)">{{ uiText('编辑后重试') }}</button>
          </div>
          <div v-if="turn.boardCommits?.length" class="board-committed-parts" :aria-label="uiText('已保存的讲解')">
            <button v-for="(part, index) in mergeBoardDeliveries(turn.boardCommits)" :key="boardDeliveryKey(part)" type="button" class="board-location board-part-link"
              @click="emit('open-board-location', { board_id: part.board_id, item_id: part.item_ids[0], block_id: part.block_ids[0] })"><span>{{ uiText('已保存第{0}部分', [index + 1]) }} · {{ uiText('在画板中查看') }}</span><ArrowRight :size="14" aria-hidden="true" /></button>
            <p class="board-parts-progress" role="status">{{ turn.pending ? uiText('后续仍在讲解，可以先查看已保存的部分。') : turn.boardIncomplete ? uiText('讲解尚未全部完成，已保存的部分可以继续查看。') : '' }}</p>
          </div>
          <p v-if="!turn.pending && turn.boardDeliveryError" class="board-delivery-error incomplete" role="status">{{ uiText('内容没有加入画板：{0}', [turn.boardDeliveryError]) }}</p>

          <div v-if="turn.outcome && (!turn.error || turn.spendStop)" class="a-msg">
            <MessageThoughts :session-id="props.activeChatSessionId" :turn-id="turn.turnId ?? ''" role="assistant" :text="deliveredAnswerText(turn)">
            <div v-if="answerParts(turn.outcome).length" :ref="el => { if (turn.turnId) { if (el) answerRoots.set(turn.turnId, el as HTMLElement); else answerRoots.delete(turn.turnId); } }" class="ans-text md" @pointerdown="startAnswerSelection" @click="onAnswerSourceClick(turn, answerParts(turn.outcome), $event)">
              <template v-for="(part, pi) in turnDisplayParts(turn)" :key="pi">
                <div v-if="part.kind === 'html' && part.socratic && !turn.outcome.incomplete" class="socratic-question">
                  <div class="socratic-heading" data-reader-selection-ignore>
                    <span class="socratic-label">{{ uiText('苏格拉底式问答') }}</span>
                    <button v-if="turn === latestAnswerTurn" type="button" class="socratic-end"
                      :disabled="runActive || historyLoading || !!historyError || questionInputBlocked || !!questionSendBlocked"
                      @click="emit('end-socratic-dialogue')">{{ uiText('先到这里') }}</button>
                  </div>
                  <div class="answer-markdown" :data-answer-part="pi" v-html="part.html"></div>
                </div>
                <div
                  v-else-if="part.kind === 'html'"
                  class="answer-markdown"
                  :data-answer-part="pi"
                  v-html="part.html"
                ></div>
                <template v-else-if="part.kind === 'presentation' && turn.turnId">
                <button v-if="workspaceVisible" class="presentation-reference" @click="selectPresentation(turn.turnId, part)">{{ uiText('打开演示 · 版本') }} {{ part.revision }}</button>
                <AgentPresentation v-if="loadedPresentations.has(presentationKey(turn.turnId, part))"
                  :socratic-target="prompt => socraticTarget(turn, prompt)"
                  :toolbar-target="workspaceVisible && workspaceKey === presentationKey(turn.turnId, part) ? workspaceTools : undefined"
                  :teaching-target="workspaceVisible && workspaceKey === presentationKey(turn.turnId, part) ? workspaceTeachingTarget ?? undefined : undefined"
                  v-show="!workspaceVisible || workspaceKey === presentationKey(turn.turnId, part)"
                  :ref="instance => { if (instance) presentationInstances.set(presentationKey(turn.turnId!, part), instance as InstanceType<typeof AgentPresentation>); else presentationInstances.delete(presentationKey(turn.turnId!, part)); }"
                  :session-id="props.activeChatSessionId" :turn-id="turn.turnId" :reference="part" :teaching="!!turn.hasTeaching || !!turn.teachingRef || turn.outcome.trace.some(t => t.tool === 'tutor.step')"
                  :workspace="workspaceVisible && workspaceKey === presentationKey(turn.turnId, part)"
                  :continuation-enabled="!!continuationEnabled" :chat-head="chatHead" :answer-available="!!deliveredAnswerText(turn)"
                  @choose-continuation="(view, option, receipt) => emit('choose-continuation', view, option, receipt)"
                  @clarify-answer="receipt => emit('clarify-answer', turn.turnId!, undefined, receipt)"
                  @locate-turn="locateAccepted" @edit-question="editQuestion" @return-reading="returnReading"
                  @continuation-available="suggestionAvailable(`${turn.turnId}:${presentationKey(turn.turnId!, part)}`, $event)"
                  @response-available="responseAvailable(turn.turnId!, $event, presentationKey(turn.turnId!, part))"
                  @teaching-host-available="teachingHostAvailable(presentationKey(turn.turnId!, part), $event)"
                  :shared-question="props.agentInput" @update:shared-question="emit('update:agentInput', $event)"
                  @expand="selectPresentation(turn.turnId!, part, $event)"
                  :busy="props.sending || props.historyLoading || !!props.historyError" @follow-up="presentationSent" @record-note="(receipt, title, summary) => emit('record-note', receipt, title, summary)"
                  @add-to-board="receipt => emit('add-to-board', { kind: 'presentation', session_id: receipt.session_id, turn_id: receipt.turn_id, reference: receipt.reference, receipt })"
                  @source="(id, anchor) => openAgentSources(turn, [id], { currentTarget: anchor })" />
                </template>
              </template>
            </div>
            <p v-else class="ans-text">{{ uiText('暂无回答。') }}</p>
            </MessageThoughts>
            <div v-if="workspaceVisible && workspace?.turnId === turn.turnId" :ref="element => { workspaceTeachingTarget = element as HTMLElement | null; }" class="workspace-teaching" />
            <TutorActivities v-if="turn.turnId && !turn.pending && (turn.hasTeaching || turn.teachingRef || turn.outcome.trace.some(t => t.tool === 'tutor.step'))"
              :socratic-target="prompt => socraticTarget(turn, prompt)"
              :ref="instance => { if (instance) activityInstances.set(turn.turnId!, instance as InstanceType<typeof TutorActivities>); else activityInstances.delete(turn.turnId!); }"
              :session-id="props.activeChatSessionId" :turn-id="turn.turnId" :answer-text="turn.outcome.answer ?? undefined" :answer-target="() => answerRoots.get(turn.turnId!)" :sources="turn.outcome.answer_view?.sources ?? []"
              :hosted-presentations="hostedTeachingPresentations(turn)" :busy="sending || historyLoading || !!historyError || activeTab !== 'agent' || props.companionVisible === false" @locate-presentation="selectPresentation(turn.turnId!, $event)"
              :response-actions-target="mobileConversation ? responseEditingHost : null" @editing-response="responseEditing(turn.turnId!, $event)"
              @response-available="responseAvailable(turn.turnId!, $event)" @guidance-available="activityGuidanceAvailable(turn.turnId!, $event)" @source="(id, anchor) => openAgentSources(turn, [id], { currentTarget: anchor })"
              @goto="emit('goto', $event)" />
            <button v-if="deliveredAnswerText(turn) && turn.turnId && !workspaceVisible && turn !== latestAnswerTurn" class="clarify-answer" :disabled="sending || historyLoading || !!historyError" @click="emit('clarify-answer', turn.turnId)"><CircleHelp :size="16" aria-hidden="true" />{{ uiText('这一步没跟上') }}</button>
            <TutorContinuation v-if="turn.turnId && deliveredAnswerText(turn) && (turn.hasTeaching || turn.teachingRef || turn.outcome.trace.some(t => t.tool === 'tutor.step')) && !workspaceVisible"
              :session-id="activeChatSessionId" :turn-id="turn.turnId" :head="chatHead" :enabled="!!continuationEnabled" :busy="sending || historyLoading || !!historyError"
              @available="suggestionAvailable(`${turn.turnId}:plain`, $event)"
              @choose="(view, option, receipt) => emit('choose-continuation', view, option, receipt)" @locate="locateAccepted" @edit="editQuestion" @read="returnReading" />
            <div v-if="deliveredAnswerText(turn) && turn.turnId && !turn.outcome.answer_view?.parts.some(part => part.kind === 'socratic_question') && !hasSuggestion(turn.turnId) && !workspaceVisible && (turn.interaction?.source_bindings.length || turn.questionQuote?.status === 'resolved' || turn.outcome.answer_view?.sources.length)" class="reading-continuation">
              <button v-if="continuationEnabled" :disabled="sending" @click="emit('continue-answer', turn.turnId)">{{ uiText('继续理解这段') }}</button>
              <small v-else>{{ continuationLabel }}</small>
            </div>
            <button v-if="!turn.pending && turn.boardDelivery && !turn.boardDeliveryError && !turn.boardCommits?.some(part => boardDeliveryKey(part) === boardDeliveryKey(turn.boardDelivery!))" type="button" class="board-location board-delivery-link"
              @click="emit('open-board-location', { board_id: turn.boardDelivery.board_id, item_id: turn.boardDelivery.item_ids[0], block_id: turn.boardDelivery.block_ids[0] })">
              <span>{{ uiText('在画板中查看') }}</span><ArrowRight :size="14" aria-hidden="true" />
            </button>
            <details v-if="deliveredAnswerText(turn)" class="answer-actions">
              <summary>{{ uiText('回答操作') }}</summary>
              <button type="button" class="answer-add-board" @click="emit('add-to-board', { kind: 'answer', session_id: activeChatSessionId, turn_id: turn.turnId! })">{{ uiText('完整回答加入画板') }}</button>
              <button v-if="workspaceNotesAvailable" :disabled="workspaceNoteImporting" @click="emit('save-workspace-note', { kind: 'answer', session_id: activeChatSessionId, turn_id: turn.turnId! })">{{ uiText('保存到空间笔记') }}</button>
              <button @click="emit('share-answer', turn)">{{ uiText('生成理解卡') }}</button>
            </details>
            <p v-if="incompleteNotice(turn.outcome)" class="incomplete">
              {{ incompleteNotice(turn.outcome) }}
            </p>

            <div v-if="turn.outcome.effects.length" class="proposals">
              <p class="prop-h">{{ uiText('建议变更') }}</p>
              <div v-for="(eff, ei) in turn.outcome.effects" :key="ei" class="proposal">
                <span class="prop-label">{{ turn.effectLabels?.[ei] ?? props.effLabel(eff) }}</span>
                <template v-if="props.effState(ti, ei)">
                  <span class="done">{{ props.effState(ti, ei) }}</span>
                  <button v-if="props.effState(ti, ei) === uiText('已保留') && (eff.kind === 'Note' || eff.kind === 'Highlight')"
                    class="undo" @click="emit('undo-effect', ti, ei, eff)">{{ uiText('撤销保留') }}</button>
                </template>
                <template v-else>
                  <button v-if="props.isGoto(eff)" @click="emit('undo-effect', ti, ei, eff)">{{ uiText('返回原位置') }}</button>
                  <template v-else>
                    <button v-if="props.showEffectPrimary(eff)" @click="emit('keep-effect', ti, ei, eff)">
                      {{ props.effectPrimaryLabel(eff) }}
                    </button>
                    <button v-if="props.showEffectSecondary(eff)" class="undo" @click="emit('undo-effect', ti, ei, eff)">
                      {{ props.effectSecondaryLabel(eff) }}
                    </button>
                  </template>
                </template>
              </div>
            </div>

            <div v-if="turn.outcome.memory_updates.length" class="memory-update-list">
              <div
                v-for="(update, ui) in turn.outcome.memory_updates"
                :key="`${update.operation_id ?? update.kind}:${ui}`"
                class="memory-update-row"
              >
                <span>{{ memoryUpdateLabel(update) }}</span>
                <small v-if="update.message">{{ update.message }}</small>
                <em v-if="props.profileUpdateStates?.[memoryUpdateKey(ti, ui)]">
                  {{ props.profileUpdateStates[memoryUpdateKey(ti, ui)] }}
                </em>
                <button
                  v-else-if="canUndoMemoryUpdate(update)"
                  type="button"
                  :title="uiText('撤销画像更新')"
                  :aria-label="uiText('撤销画像更新')"
                  :disabled="props.profileMemoryBusy"
                  @click="emit('undo-profile-update', ti, ui, update)"
                >
                  <Undo2 :size="15" aria-hidden="true" />
                </button>
              </div>
            </div>

            <details v-if="hasProfileUsage(turn.outcome.profile_usage)" class="profile-usage">
              <summary>{{ uiText('学习偏好与背景 ·') }} {{ turn.outcome.profile_usage.injected_fact_ids.length }} {{ uiText('条') }}</summary>
              <p v-if="turn.outcome.profile_usage.influences.length">
                {{ turn.outcome.profile_usage.influences.map(influenceLabel).join(" · ") }}
              </p>
            </details>




          </div>
        </div>
        <div v-if="showConversationWelcome" class="empty conversation-welcome">
          <YeyeMascot class="welcome-mascot" :size="mobileConversation ? 128 : 144" motion="idle" :active="companionActive" />
          <h3>{{ props.musicMode ? uiText('听不懂、看不懂，都可以问。') : props.videoImagesEnabled ? uiText('没听懂的，一起弄懂。') : uiText('想从哪里开始？') }}</h3>
          <p>{{ (composerGuidancePurpose === 'welcome' ? composerGuidance?.question : '') || (props.videoImagesEnabled ? uiText('从当前画面问起，也可以框选一处细节。开始输入时，视频会暂停在这一刻。') : props.learningTitle ? uiText('接着上次学习，也可以随时调整目标。') : props.musicMode ? uiText('不认识谱线、音符或符号？在乐谱上框选，或上传、粘贴截图，直接问「这是什么」。') : uiText('输入问题或学习目标…')) }}</p>
          <div v-if="learningPurposeAvailable" class="welcome-learning-actions">
            <button type="button" class="start-current-learning" :disabled="learningActionBlocked || !currentLearningScope" :title="learningPurposeReason" @click="startCurrentLearning">{{ uiText('带我学这一部分') }}<ArrowRight :size="16" aria-hidden="true" /></button>
            <small v-if="currentLearningScope">{{ currentLearningScope.label }}</small>
          </div>
          <button v-if="historySessions.length" type="button" class="history-button empty-history-button" @click="historyOpen = true">{{ uiText('查看') }}{{ musicMode ? uiText('这首作品') : uiText('本书') }}{{ uiText('的') }} {{ historySessions.length }} {{ uiText('段历史对话') }}</button>
        </div>
        <div ref="transcriptLearningHost" />
      </div>

      <MusicExamplePiano v-if="musicMode" @source="pianoSource" />
      <div v-show="previewingTray" class="follow-up-tray" data-follow-up-host="tray">
        <Teleport :to="followUpTarget || 'body'" :disabled="!followUpTarget">
          <FollowUpPathPanel v-if="hasPaths && (wideConversation || followUpOpen)" ref="followUpPanel" :layout="followUpLayout" :nodes="pathNodes"
            :selected="selectedPath" :current="activeChatSessionId" :drafts="visibleDrafts" :busy="followUpBusy || historyLoading"
            :error="followUpError" @close="closeFollowUpPath" @cancel="discardFollowUp" @select="previewFollowUp"
            @enter="emit('follow-up-enter',$event)" @start="emit('follow-up-start',$event)"
            @update-question="editFollowUpQuestion" @retry="emit('follow-up-load')" />
        </Teleport>
      </div>
      <div v-show="!previewingTray" class="agent-input" data-tour="reader-input" :class="{ composing: composeExpanded, 'mobile-composing': mobileConversation && mobileComposerVisible, 'mobile-answering': mobileConversation && !!mobileResponseTurn }">
        <button v-if="hasPaths && !wideConversation && !mobileResponseTurn" ref="followUpToggle" type="button" class="follow-up-toggle"
          :aria-label="uiText('追问路径')" :aria-expanded="followUpOpen" @click="openFollowUpPath(selectedPath,followUpToggle ?? null)">
          <GitFork :size="14" aria-hidden="true" />{{ uiText('追问路径') }}<small>{{ activeBranch?.origin?.anchor.excerpt || uiText('原问题') }}</small><ChevronUp :size="14" aria-hidden="true" />
        </button>
        <p v-if="props.videoError" role="alert">{{ props.videoError }}</p>
        <p v-if="props.questionSendBlocked" role="status">{{ props.questionSendBlocked }}</p>
        <slot v-if="useLearningSheet" name="practice-offer" />
        <div v-show="mobileConversation && !!mobileResponseTurn" class="mobile-response-toolbar"><div ref="responseEditingHost" /><button type="button" class="mobile-editor-close" :aria-label="uiText('收起作答')" @click="closeResponseEditor"><X :size="18" aria-hidden="true" /></button></div>
        <Teleport :to="learningDockTarget || 'body'" :disabled="!learningDockTarget">
        <div v-if="latestAnswerTurn && !workspaceVisible && !historyLoading && !historyError" v-show="!mobileConversation || !mobileResponseTurn" class="learning-dock" :class="{ 'has-practice-actions': showAnswerPractice }" role="group" :aria-label="uiText('当前回答的学习操作')">
          <div class="learning-dock-copy"><span>{{ musicMode ? uiText('听到这里') : uiText('读到这里') }}</span><p>{{ responseTurns.has(latestAnswerTurn.turnId!) ? uiText('试着把理解变成自己的话') : uiText('有疑问，随时停下来聊一聊') }}</p></div>
          <div v-if="showAnswerPractice" class="answer-practice-actions">
            <button type="button" class="learning-explain" :disabled="learningActionBlocked || !closedBookReady" aria-describedby="answer-practice-explanation" @click="practiceFromAnswer('explain')">{{ uiText('我来解释一下') }}</button>
            <button type="button" class="learning-apply" :disabled="learningActionBlocked" @click="practiceFromAnswer('apply')">{{ uiText('给我一道应用题') }}</button>
            <p id="answer-practice-explanation" class="learning-practice-note">{{ uiText('解释练习会收起资料，按你的回答追问或补讲，可随时暂停。') }}</p>
          </div>
          <button type="button" class="learning-help" :aria-label="uiText('这一步没跟上')" :disabled="sending" @click="emit('clarify-answer', latestAnswerTurn.turnId!)"><CircleHelp :size="17" aria-hidden="true" />{{ mobileConversation ? uiText('没跟上') : uiText('这一步没跟上') }}</button>
          <button v-if="!showAnswerPractice && !courseActive" v-show="!mobileConversation || structuredResponseAvailable || inSocraticDialogue" type="button" class="learning-respond" :disabled="sending" @click="focusLearningResponse">{{ learningResponseLabel }}<ArrowRight :size="16" aria-hidden="true" /></button>
        </div>
        </Teleport>
        <div v-if="props.runConnection === 'reconnecting' || targetGoal || props.askDraft || props.personalNoteReference || (useLearningSheet && (videoQuestion || $slots['note-question']))" v-show="!mobileConversation || mobileComposerVisible || props.runConnection === 'reconnecting'" class="agent-input-context">
        <slot v-if="useLearningSheet" name="note-question" />
        <div v-if="useLearningSheet && videoQuestion" ref="videoContextHost" />
        <p v-if="props.runConnection === 'reconnecting'" role="status">{{ uiText('连接中断，正在重新连接；运行仍可继续。') }}</p>
        <div v-if="targetGoal" class="goal-target">
          <span>{{ uiText('补充任务：') }}{{ targetGoal.interpretation }}</span>
          <button :aria-label="uiText('取消补充任务')" @click="emit('clear-goal-target')">×</button>
        </div>
        <div v-if="props.personalNoteReference" class="ask-draft"><div class="ask-draft-head"><span>继续推敲这条想法 · 第 {{props.personalNoteReference.revision}} 版</span><button aria-label="结束想法关联" @click="emit('clear-personal-note')"><X :size="14" /></button></div></div>
        <div v-if="props.askDraft" class="ask-draft">
          <div class="ask-draft-head">
            <span>{{ askQuoteLabel(props.askDraft) }}</span>
            <button :title="uiText('清除引用来源')" :aria-label="uiText('清除引用来源')" @click="emit('clear-ask')">
              <X :size="14" aria-hidden="true" />
            </button>
          </div>
          <blockquote>{{ props.askDraft.quote }}</blockquote>
        </div>
        </div>
        <div v-if="(composeTall || (runActive && $slots['agent-picker'])) && (!mobileConversation || mobileComposerVisible)" class="compose-caption">
          <span>{{ runActive && $slots['agent-picker'] ? uiText('可以先写补充，本轮结束后发送。') : uiText('草稿会保留') }}</span>
          <button v-if="composeTall" type="button" :aria-label="uiText('收起输入')" @click="collapseAgentInput">{{ uiText('收起') }}</button>
        </div>
        <div v-show="!mobileConversation || mobileComposerVisible" class="agent-compose-row" :class="{ 'video-compose': imagesEnabled }">
        <p v-if="composerSuggestionVisible && composerGuidancePurpose !== 'welcome'" id="reading-input-invitation" class="reading-input-invitation">{{ composerGuidance!.question }}</p>
        <QuestionImageInput v-if="imagesEnabled" ref="imageInput" :image="questionImage" :disabled="sending || questionInputBlocked" @change="emit('question-image', $event)" @loading="questionImageLoading" />
        <textarea
          ref="agentInputRef"
          data-workspace-input="agent"
          :disabled="props.questionInputBlocked || (network.enabled && (props.historyLoading || !!props.historyError))"
          :value="props.agentInput"
          rows="1"
          :aria-label="musicMode ? uiText('围绕当前音乐提问') : uiText('围绕当前阅读内容提问')"
          :placeholder="inputComposing ? '' : inSocraticDialogue ? uiText('说说你的想法…') : composerSuggestionVisible ? composerGuidance!.draft : props.askDraft ? uiText('围绕引用来源提问...') : workspaceVisible && sceneBound ? uiText('围绕这页演示提问…') : props.videoImagesEnabled ? uiText('这里为什么这样？问问伴读…') : props.musicMode ? uiText('看不懂的谱线、音符，截图问问…') : !props.chat.length ? uiText('输入问题或学习目标…') : uiText('说说你的想法，或继续提问…')"
          :aria-describedby="composerSuggestionVisible ? (composerGuidancePurpose === 'welcome' ? 'reading-input-suggestion' : 'reading-input-invitation reading-input-suggestion') : undefined"
          @input="emit('update:agentInput', ($event.target as HTMLTextAreaElement).value)"
          @focus="composeExpanded = true"
          @keydown="onAgentInputKeydown"
          @compositionstart="inputComposing = true" @compositionend="inputComposing = false"
          @paste="imageInput?.paste($event)"
        />
        <InputSuggestionHint v-if="composerSuggestionVisible" id="reading-input-suggestion" class="reading-input-suggestion" @accept="acceptComposerSuggestion" />
        <div class="composer-controls">
        <slot name="agent-picker" />
        <VoiceInput class="composer-voice" :scope="props.activeChatSessionId ?? ''" :disabled="activeTab !== 'agent' || props.companionVisible === false || (mobileConversation && !mobileComposerVisible) || props.questionInputBlocked || sending || canStop || stopping || historyLoading || !!historyError" @insert="insertVoice" @active="voiceActive = $event" />
        <div v-if="useLearningSheet && workspaceVisible" class="workspace-compose-actions">
          <button v-if="workspaceVisible" type="button" class="workspace-binding" :aria-pressed="sceneBound" :aria-label="noteFollowUp ? uiText('附上记录时的演示状态') : uiText('附上当前演示状态')" :title="sceneBound ? uiText('不附演示状态') : uiText('附上当前演示')" @click="sceneBound = !sceneBound">
            <component :is="sceneBound ? Link2 : Link2Off" :size="14" aria-hidden="true" /><span>{{ sceneBound ? uiText('{0} · {1}', [noteFollowUp ? uiText('记录时的演示') : uiText('当前演示'), 'v' + workspace?.reference.revision]) : uiText('普通聊天') }}</span>
          </button>
        </div>
          <button type="button" class="presentation-request" data-tour="presentation" role="switch" :aria-checked="presentationRequested" :aria-label="uiText('演示页')"
            :title="presentationUnavailable ? uiText('演示制作暂不可用') : uiText('将本次问题制作成可交互的演示页')"
            :disabled="presentationUnavailable || sending || canStop || stopping || historyLoading || !!historyError || questionInputBlocked"
            @click="presentationRequested = !presentationRequested">
            <Presentation :size="16" aria-hidden="true" /><span>{{ uiText('演示页') }}</span><span class="presentation-switch-track" aria-hidden="true"><span /></span>
          </button>
          <span v-if="presentationRequested" class="presentation-request-hint" role="status">{{ uiText('本次回答制作演示页') }}</span>
          <span v-else-if="presentationUnavailable" class="presentation-request-hint">{{ uiText('演示制作暂不可用') }}</span>
          <button v-if="imagesEnabled" type="button" class="attach-image" :aria-label="questionImage ? uiText('替换图片') : uiText('添加图片')" :title="uiText('添加图片，也可直接粘贴')" :disabled="imageLoading || sending || questionInputBlocked" @click="imageInput?.chooseFile()"><Plus :size="20" aria-hidden="true" /></button>
          <Teleport :to="videoContextHost || 'body'" :disabled="!videoContextHost">
          <div v-if="videoQuestion" class="video-binding" :aria-label="uiText('问题绑定位置')" :title="uiText('问题绑定 {0}{1}', [videoTime(videoQuestion.frame.actual_pts_ms), videoQuestion.region ? uiText(' · 已选区域') : ''])">
            <span>{{ videoQuestion.range ? `${videoTime(videoQuestion.range.start_ms)}–${videoTime(videoQuestion.range.end_ms)}` : videoTime(videoQuestion.frame.actual_pts_ms) }}{{ videoQuestion.region ? uiText(' · 选区') : '' }}</span>
            <button type="button" :aria-label="uiText('取消草稿')" :title="uiText('取消草稿')" :disabled="imageLoading || sending" @click="emit('cancel-video-draft')"><X :size="12" aria-hidden="true" /></button>
          </div>
          </Teleport>
          <div v-if="$slots['tutor-control'] || $slots['learning-actions']" class="agent-input-tools">
            <Teleport :to="teachingToolsTarget || 'body'" :disabled="!teachingToolsTarget"><slot name="tutor-control" /><slot v-if="!useLearningSheet" name="practice-offer" /></Teleport>
            <Teleport :to="recordsToolsTarget || 'body'" :disabled="!recordsToolsTarget"><div v-if="$slots['learning-actions']" class="conversation-learning-actions" role="group" :aria-label="uiText('学习记录')"><slot v-if="!useLearningSheet" name="note-question" /><slot name="learning-actions" /></div></Teleport>
          </div>
        </div>
        <button v-if="stopAvailable" type="button" class="stop-agent" :aria-label="stopping ? uiText('正在停止') : uiText('停止')" :title="stopping ? uiText('正在停止') : uiText('停止生成')" :disabled="stopping" @click="stopAgent"><span>{{ stopping ? uiText('正在停止') : uiText('停止') }}</span><Square :size="14" fill="currentColor" aria-hidden="true" /></button>
        <button v-else type="button" class="send-agent" :aria-label="props.sending ? uiText('发送中') : uiText('发送')" :title="uiText('发送（Ctrl+Enter）')" :disabled="voiceActive || imageLoading || !!props.questionSendBlocked || props.sending || props.historyLoading || !!props.historyError || (!props.agentInput.trim() && !questionImage)" @click="submitMessage">
          <span>{{ props.sending ? uiText('发送中') : uiText('发送') }}</span>
          <ArrowUp :size="20" aria-hidden="true" />
        </button>
        </div>
      </div>
      </div>
      <aside v-show="wideConversation" class="conversation-sidebar conversation-response" :aria-label="uiText('学习助手')">
        <div ref="responseSidebarHost" />
        <div v-if="$slots['learning-actions']" class="sidebar-section"><p class="sidebar-label">{{ uiText('学习记录') }}</p><div ref="recordsSidebarHost" /></div>
      </aside>
    </section>

    <section id="reader-panel-profile" v-show="activeTab === 'profile'" class="tab-panel profile-panel" role="tabpanel" aria-labelledby="reader-tab-profile">
      <ProfileMemoryPanel
        :state="props.profileMemory"
        :loading="props.profileMemoryLoading"
        :busy="props.profileMemoryBusy"
        :error="props.profileMemoryError"
        :notice="props.profileMemoryNotice"
        :backfill="props.profileBackfill"
        :backfill-loading="props.profileBackfillLoading"
        :backfill-busy="props.profileBackfillBusy"
        :backfill-error="props.profileBackfillError"
        :backfill-notice="props.profileBackfillNotice"
        @refresh="emit('refresh-profile')"
        @mutate="emit('mutate-profile', $event)"
        @confirm-sensitive="emit('confirm-sensitive-profile')"
        @backfill-start="emit('start-profile-backfill', $event)"
        @backfill-action="(action, request) => emit('mutate-profile-backfill', action, request)"
        @goto="emit('goto', $event)"
      />
    </section>

    <section id="reader-panel-formula" v-show="activeTab === 'formula'" class="tab-panel context-panel" role="tabpanel" aria-labelledby="reader-tab-formula">
      <div class="panel-head">
        <p class="rail-kicker">{{ uiText('公式剖面') }}</p>
        <h3>{{ props.selectedFormula?.formula_lid ?? props.selectedLid ?? uiText('未选择') }}</h3>
      </div>
      <div v-if="props.selectedFormula" class="formula-card">
        <p class="formula-meaning">{{ props.selectedFormula.composition.meaning }}</p>
        <div v-if="props.selectedFormula.parameters.length" class="context-block">
          <h4>{{ uiText('参数') }}</h4>
          <dl>
            <template v-for="p in props.selectedFormula.parameters" :key="p.symbol">
              <dt>{{ p.symbol }}<span v-if="p.label"> · {{ p.label }}</span></dt>
              <dd>
                {{ p.meaning }}
                <span v-if="p.unit"> {{ uiText('· 单位:') }} {{ p.unit }}</span>
                <span v-if="p.domain"> {{ uiText('· 取值域:') }} {{ p.domain }}</span>
              </dd>
            </template>
          </dl>
        </div>
        <div v-if="props.selectedFormula.context_links.length" class="context-block">
          <h4>{{ uiText('上下文关系') }}</h4>
          <ul>
            <li v-for="link in props.selectedFormula.context_links" :key="`${link.target_lid}:${link.relation}`">
              <strong>{{ link.relation }}</strong> {{ link.description }}
            </li>
          </ul>
        </div>
      </div>
      <p v-else-if="props.formulaLoading" role="status">{{ uiText('正在读取公式解释…') }}</p>
      <p v-else-if="props.formulaError && props.formulaSelected" role="alert">{{ props.formulaError }} <button @click="emit('request-formula')">{{ uiText('重试') }}</button></p>
      <p v-else class="empty panel-empty">{{ props.formulaSelected ? uiText('这条公式暂时没有解释资料。') : uiText('在阅读区选择公式后查看预构建剖面。') }}</p>
    </section>

    <section ref="notesPanel" id="reader-panel-notes" v-show="activeTab === 'notes'" class="tab-panel context-panel" role="tabpanel" aria-labelledby="reader-tab-notes">
      <div class="panel-head">
        <p class="rail-kicker">{{ uiText('全部笔记') }}</p>
        <h3>{{ noteCount }} {{ uiText('条') }}</h3>
        <button v-if="workspaceNotesAvailable" class="ghost-pill" @click="emit('open-workspace-notes')">{{ uiText('打开空间笔记') }}</button>
        <div v-if="noteCount" class="note-fold-controls">
          <button @click="filteredNotes.forEach(note => openNoteDetails.add(note.mem_id))">{{ uiText('展开') }}</button>
          <button @click="openNoteDetails.clear()">{{ uiText('收起') }}</button>
        </div>
      </div>
      <div v-if="noteCount" class="note-search-controls">
        <input v-model="noteQuery" type="search" :aria-label="uiText('搜索笔记')" :placeholder="uiText('搜索文字、摘录或关联标题')" />
        <label>{{ uiText('类型') }} <select v-model="noteFilter" :aria-label="uiText('笔记类型')">
          <option value="all">{{ uiText('全部') }}</option><option value="original">{{ uiText('原文') }}</option><option value="answer">{{ uiText('回答') }}</option>
          <option value="presentation">{{ uiText('演示') }}</option><option value="highlight">{{ uiText('高亮') }}</option><option value="unknown">{{ uiText('旧笔记（类型未知）') }}</option>
        </select></label>
        <label>{{ uiText('排序') }} <select v-model="noteOrder" :aria-label="uiText('笔记排序')"><option value="recent">{{ uiText('最近记录') }}</option><option value="original">{{ uiText('原文顺序') }}</option></select></label>
        <p role="status">{{ filteredNotes.length }} {{ uiText('条记录') }}</p>
      </div>
      <div v-if="noteCount" class="memory-list">
        <details v-for="note in filteredNotes" :key="note.mem_id"
          class="memory-card note-memory-card" :class="{ 'highlight-card': note.type === 'highlight' }"
          :data-mem-id="note.mem_id" tabindex="-1" :open="openNoteDetails.has(note.mem_id)"
          @toggle="($event.target as HTMLDetailsElement).open ? openNoteDetails.add(note.mem_id) : openNoteDetails.delete(note.mem_id)">
          <summary class="memory-meta note-memory-summary">
            <span class="memory-kind-with-location">{{ noteKindLabel(note) }}
              <small v-if="annotationLocationLabel(note)" :data-location="props.annotationLocation?.[note.mem_id]">{{ annotationLocationLabel(note) }}</small>
            </span>
            <button v-if="canFocusNoteSource(note)" class="note-source-button" @click.prevent.stop="emit('focus-source', { lid: noteSourceLid(note)!, quote: note.selection_context?.resolved_quote ?? (note.type === 'highlight' ? note.content : null), memId: note.mem_id })">{{ noteSourceLabel(note) }}</button>
            <span v-else class="note-association-title">{{ noteSourceLabel(note) }}</span>
            <em>{{ uiText('展开/收起') }}</em>
            <time class="note-time">{{ noteTimeLabel(note) }}</time>
            <div class="note-preview">{{ notePreview(note) }}</div>
          </summary>
          <NoteDetail v-if="note.type !== 'highlight' && openNoteDetails.has(note.mem_id)" :note="note"
            @locate="openNotePresentation(note)" @restore="openNotePresentation(note, true)"
            @answer="emit('open-note-answer', note)" @edit="emit('edit-note', note)" @delete="emit('delete-note', note)" />
          <template v-if="note.type === 'highlight' && openNoteDetails.has(note.mem_id)">
            <blockquote class="highlight-content">{{ note.content }}</blockquote>
            <button v-if="shareHighlight" @click="shareHighlight(note)">{{ uiText('生成分享图') }}</button>
            <div class="note-actions"><button @click="emit('modify-highlight', note)">{{ uiText('编辑') }}</button><button @click="emit('delete-highlight', note)">{{ uiText('删除') }}</button></div>
          </template>
          <button v-if="note.type !== 'highlight' && notePlacementActionLabel(note)" type="button" class="note-placement-action" data-note-placement-action :data-mem-id="note.mem_id" @click="emit('place-note', note)">{{ notePlacementActionLabel(note) }}</button>
          <button v-if="workspaceNotesAvailable && note.type !== 'highlight'" :disabled="workspaceNoteImporting" class="note-placement-action" @click="emit('save-workspace-note', { kind: 'reading_note', mem_id: note.mem_id })">{{ uiText('保存到空间笔记') }}</button>
        </details>
        <p v-if="!filteredNotes.length" class="empty">{{ uiText('没有匹配的记录。') }}</p>
      </div>
      <p v-else class="empty panel-empty">{{ uiText('暂无笔记或高亮。') }}</p>
    </section>
    <Teleport :to="applicationOverlayTarget()">
      <div
        v-if="answerSelection"
        ref="answerPopover"
        class="answer-popover"
        role="group"
        :aria-label="uiText('选中文字的操作')"
        :style="{ left: answerSelection.x + 'px', top: answerSelection.y + 'px', visibility: answerSelectionPositioned ? 'visible' : 'hidden' }"
        @mousedown.prevent
        @keydown.esc="clearAnswerSelection"
      >
        <p v-if="answerSelectionError" class="answer-selection-error" role="status">{{ answerSelectionError }}</p>
        <template v-else>
          <button v-if="followUpAllowed && answerSelection.anchor" type="button" :disabled="followUpBusy || historyLoading" @click="branchFromSelection">{{ uiText('另开追问') }}</button>
          <button type="button" @click="saveAnswerSelection(answerSelection.turn)">{{ uiText('记笔记') }}</button>
          <button v-if="deliveredAnswerText(answerSelection.turn)" type="button" :disabled="sending" @click="clarifySelection">{{ uiText('这一步没跟上') }}</button>
          <button v-if="deliveredAnswerText(answerSelection.turn)" type="button" @click="shareAnswerSelection">{{ uiText('生成理解卡') }}</button>
        </template>
        <button type="button" :aria-label="uiText('取消选区')" @click="dismissAnswerSelection">×</button>
      </div>
    </Teleport>

    <Teleport :to="applicationOverlayTarget()">
      <div v-if="historyOpen" class="history-backdrop" @click.self="historyOpen = false">
        <section class="history-dialog" role="dialog" aria-modal="true" aria-labelledby="history-title">
          <header class="history-dialog-head">
            <div>
              <p class="rail-kicker">{{ uiText('助手历史') }}</p>
              <h3 id="history-title">{{ uiText('对话历史') }}</h3>
            </div>
            <button class="history-close" :title="uiText('关闭历史')" :aria-label="uiText('关闭历史')" @click="historyOpen = false">×</button>
          </header>
          <div class="history-list">
            <article
              v-for="session in historySessions"
              :key="session.id"
              class="history-card"
              :class="{ active: session.id === props.activeChatSessionId }"
              :role="session.id === props.activeChatSessionId ? undefined : 'button'"
              :tabindex="session.id === props.activeChatSessionId ? -1 : 0"
              @click="openHistorySessionFromCard(session.id, $event)"
              @keydown.enter="openHistorySessionFromCard(session.id, $event)"
              @keydown.space="openHistorySessionFromCard(session.id, $event)"
            >
              <div class="history-card-head">
                <div>
                  <h4>{{ compactText(session.title, 72) }}</h4>
                  <p>{{ session.turn_count }} {{ uiText('个问题') }}</p>
                </div>
                <span v-if="session.id === props.activeChatSessionId" class="active-badge">{{ uiText('当前') }}</span>
              </div>

              <ol v-if="session.turns.length" class="history-turns">
                <li v-for="(turn, i) in session.turns" :key="`${session.id}:${i}`">
                  <p class="history-question">{{ compactText(turn.user, 180) }}</p>
                  <div class="history-anchor-row">
                    <span>{{ uiText('来源') }}</span>
                    <span class="history-source-label">{{ turnSourceLabel(turn) ?? uiText('无') }}</span>
                  </div>
                </li>
              </ol>
              <p v-else class="empty history-empty">{{ uiText('暂无问题。') }}</p>

              <div class="history-card-actions">
                <button
                  class="history-open"
                  :disabled="session.id === props.activeChatSessionId"
                  @click.stop="openHistorySession(session.id)"
                > {{ uiText('打开对话') }} </button>
                <button class="history-delete" @click.stop="deleteHistorySession(session.id)">{{ uiText('删除') }}</button>
              </div>
            </article>
            <p v-if="historySessions.length === 0" class="empty history-empty">{{ uiText('暂无保存的对话历史。') }}</p>
          </div>
        </section>
      </div>
    </Teleport>

    <BoardSourcePreview v-if="agentSourcePopup" context="answer" :source="activeAgentSource"
      :trigger="agentSourcePopup.trigger" :read-window="readAgentSourceWindow" :loading="agentSourcePopup.loading"
      :opening="agentSourcePopup.opening" :error="agentSourcePopup.error ?? undefined" @close="closeAgentSourcePopup" @retry="retryAgentSource">
      <template #sources>
        <div v-if="agentSourcePopup.sources.length > 1" class="source-tabs" role="tablist" :aria-label="uiText('来源列表')">
          <button v-for="source in agentSourcePopup.sources" :key="source.source_ref_id" type="button" role="tab"
            :aria-selected="source.source_ref_id === agentSourcePopup.activeSourceRefId"
            :class="{ active: source.source_ref_id === agentSourcePopup.activeSourceRefId }" @click="selectAgentSource(source.source_ref_id)">
            {{ sourceTabLabel(source) }}
          </button>
        </div>
      </template>
      <template #actions>
        <a v-if="activeAgentSource?.source_url && !activeAgentSource.stale" class="source-open-reader"
          :href="activeAgentSource.source_url" target="_blank" rel="noopener noreferrer">
          <ExternalLink :size="15" aria-hidden="true" />{{ uiText('查看参考书原文') }}
        </a>
        <button v-else type="button" class="source-open-reader"
          :disabled="!activeAgentSource || activeAgentSource.stale || !activeAgentSource.can_open_in_reader || agentSourcePopup.opening"
          @click="openActiveAgentSourceInReader">
          <ExternalLink :size="15" aria-hidden="true" />
          {{ agentSourcePopup.opening ? uiText('正在打开...') : activeAgentSource?.music ? uiText('回到这段音乐与乐谱') : activeAgentSource?.media ? uiText('进入原视频位置') : uiText('在正文中查看') }}
        </button>
      </template>
    </BoardSourcePreview>

  </aside>
</template>

<style scoped>
.socratic-question { margin: 1em 0 .4em; padding: .15em 0 .15em .95em; border-inline-start: 2px solid var(--brand-accent); }
.socratic-heading { display: flex; align-items: center; justify-content: space-between; gap: 8px; margin-bottom: .35em; }
.socratic-label { color: var(--brand-accent-deep); font: 600 12px/1.5 var(--sans); }
.socratic-end { padding: 3px 6px; border: 0; border-radius: 4px; background: transparent; color: var(--steel); font: 12px/1.5 var(--sans); cursor: pointer; }
.socratic-end:hover, .socratic-end:focus-visible { background: var(--surface-soft); color: var(--ink); }
.socratic-end:disabled { opacity: .5; cursor: default; }
@media (pointer: coarse) { .socratic-end { min-height: 44px; } }
.socratic-question :deep(p) { margin: 0; }
.turn-note-links{display:flex;flex-wrap:wrap;align-items:center;gap:7px;margin:12px 0;font-size:11px;color:var(--steel)}.turn-note-links button{font:inherit;color:inherit;background:var(--surface-soft);border:1px solid var(--hairline);border-radius:6px;padding:6px 9px;cursor:pointer}
.turn-question-image { margin: 8px 0 12px; font-size: 12px; color: var(--steel); }
.turn-question-image summary { display: flex; align-items: center; gap: 8px; cursor: zoom-in; }
.turn-question-image summary img { width: 120px; height: 80px; object-fit: contain; border: 1px solid var(--line); border-radius: 8px; }
.turn-question-image > img { display: block; max-width: 100%; max-height: 420px; object-fit: contain; margin-top: 8px; }
.reading-continuation { display: flex; flex-direction: column; align-items: flex-start; gap: 8px; margin-top: 16px; }
.reading-continuation small { color: var(--steel); line-height: 1.6; font-size: .75rem; }
.reading-continuation > button { min-height: 42px; padding: 9px 15px; border: 1px solid var(--line); border-radius: 9px; background: var(--surface-soft); color: var(--brand-green-deep); font-size: .84rem; }
.clarify-answer { display: inline-flex; align-items: center; gap: 7px; min-height: 40px; margin-top: 18px; padding: 8px 12px; border: 1px solid var(--line); border-radius: 9px; background: var(--canvas); color: var(--steel); font-size: .8rem; }
.clarify-answer:hover:not(:disabled) { color: var(--brand-green-deep); border-color: var(--brand-green); background: var(--surface-soft); }
.note-search-controls { display: flex; flex-wrap: wrap; gap: 12px; margin-bottom: 16px; }
.note-search-controls input { width: 100%; min-width: 0; }
.note-search-controls input, .note-search-controls select { min-height: 40px; padding: 8px; border: 1px solid var(--hairline); border-radius: 8px; background: var(--surface); color: var(--ink); }
.note-search-controls label { min-width: 0; }
.note-search-controls p { width: 100%; margin: 0; }
.note-time { grid-column: 1 / -1; color: var(--stone); font-size: 12px; }
.highlight-content { white-space: pre-wrap; overflow-wrap: anywhere; }

.right-rail {
  container-type: inline-size;
  container-name: reading-assistant;
  min-width: 0;
  border-left: 1px solid var(--hairline);
  background: var(--reader-card);
  backdrop-filter: saturate(160%) blur(18px);
  -webkit-backdrop-filter: saturate(160%) blur(18px);
  display: flex;
  flex-direction: column;
  overflow: hidden;
}
/* A backdrop filter makes fixed children relative to the rail. Keep the live
   iframe in place when expanding, but release that containing block. */
.right-rail:has(.agent-presentation.expanded) {
  backdrop-filter: none;
  -webkit-backdrop-filter: none;
}
.presentation-workspace::before { content: ''; position: fixed; inset: 0; z-index: 89; background: var(--canvas); }
.presentation-workspace .agent-panel { position: fixed; z-index: 90; top: var(--presentation-top); bottom: 12px; right: 12px; width: calc(100% - var(--presentation-split) - 24px); background: var(--canvas); border: 1px solid var(--hairline); border-radius: 12px; }
.presentation-workspace :deep(.agent-presentation.expanded) { inset: var(--presentation-top) auto 12px 12px; width: calc(var(--presentation-split) - 12px); max-height: none; }
.workspace-tools { position: fixed; z-index: 92; inset: max(4px, env(safe-area-inset-top)) 12px auto; min-height: 36px; display: flex; flex-wrap: wrap; justify-content: flex-end; align-items: center; gap: 4px 8px; background: var(--canvas); }
.workspace-tools :deep(.presentation-toolbar) { order: -1; padding: 2px 0; }
.workspace-learning-details :deep(button) { min-height: 44px; padding: 4px 8px; border: 0; background: transparent; color: var(--brand-green-deep); }
.workspace-split { display: flex; align-items: center; gap: 8px; }
.workspace-split { font-size: 12px; }
.workspace-split input { width: 80px; }
.discussion-toggle { display: none; }
.discussion-context { display: contents; }
.compact-context:not(:empty) { display: flex; flex: 0 0 auto; align-items: center; gap: 8px; min-width: 0; padding: 2px 10px; border-bottom: 1px solid var(--hairline-soft); }
.workspace-settings, .compact-context .task-summary-toggle { display: inline-flex; align-items: center; gap: 6px; min-width: 0; min-height: 36px; padding: 4px 6px; border: 0; border-radius: 6px; background: transparent; font: inherit; font-size: .76rem; text-align: left; }
.workspace-settings { flex: 1; color: var(--ink); }
.workspace-settings svg, .workspace-settings span { flex: none; }
.workspace-settings small { min-width: 0; overflow: hidden; white-space: nowrap; text-overflow: ellipsis; color: var(--steel); font-size: inherit; }
.compact-context .task-summary-toggle { width: auto; max-width: 50%; margin-left: auto; }
.compact-context .task-summary-status { max-width: none; }
.workspace-settings:hover, .compact-context .task-summary-toggle:hover { background: var(--surface-soft); }
.right-rail.compact-conversation .conversation-main .agent-input { container: question-field / inline-size; padding: 6px 10px max(8px, env(safe-area-inset-bottom)); }
@media (min-width: 1024px) {
  .right-rail.compact-conversation .rail-toolbar.agent-head { padding: 4px 12px; }
}
.compact-conversation .agent-compose-row { grid-template-columns: minmax(0, 1fr) auto; align-items: center; padding: 4px 6px; gap: 2px 6px; }
.compact-conversation .agent-compose-row textarea { grid-column: 1 / -1; }
.compact-conversation .agent-compose-row > .send-agent, .compact-conversation .agent-compose-row > .stop-agent { grid-column: 2; min-width: 0; padding-inline: 12px; }
.compact-conversation .composer-controls { grid-column: 1; }
.workspace-compose-actions { display: flex; align-items: center; gap: 2px; grid-column: 2; }
.workspace-binding { display: inline-flex; align-items: center; gap: 5px; min-height: 34px; padding: 4px 8px; border: 0; border-radius: 8px; background: transparent; color: var(--steel); font: inherit; font-size: .73rem; }
.workspace-binding[aria-pressed="true"] { color: var(--brand-green-deep); }
.workspace-binding:hover { background: var(--surface); }
.workspace-settings:focus-visible, .workspace-compose-actions button:focus-visible { outline: 2px solid var(--brand-accent); outline-offset: 1px; }
@container question-field (max-width: 400px) {
  .workspace-binding { width: 32px; justify-content: center; }
  .workspace-binding span { display: none; }
}
.scene-binding { display: flex; flex-wrap: wrap; gap: 8px; padding: 8px; font-size: .82rem; background: var(--surface); }
.presentation-workspace .agent-head { display: none; }
@media (max-width: 1023px) {
  .workspace-split { display: none; }
  .workspace-tools { inset-inline: 12px; gap: 0 8px; padding-bottom: 5px; border-bottom: 1px solid var(--hairline-soft); }
  .workspace-learning-details { margin-right: auto; }
  .workspace-learning-details :deep(button) { min-height: 36px; padding: 6px 2px; color: var(--steel); font-size: 12px; }
  .discussion-toggle { display: inline-flex; align-items: center; gap: 7px; min-height: 36px; padding: 6px 10px; border: 0; border-radius: 18px; color: var(--ink); background: var(--surface-soft); font-size: 12px; }
  .discussion-indicator { width: 5px; height: 5px; border-radius: 50%; background: var(--accent); }
  .discussion-toggle[aria-expanded="true"] svg { transform: rotate(180deg); }
  .presentation-workspace .agent-panel { right: 8px; left: 8px; width: auto; bottom: 8px; }
  .presentation-workspace :deep(.agent-presentation.expanded) { inset: var(--presentation-top) 8px max(8px, env(safe-area-inset-bottom)); width: auto; border-radius: 16px; box-shadow: 0 4px 20px #2523200a; }
  .presentation-workspace.discussion-open :deep(.agent-presentation.expanded) { visibility: hidden; }
  .presentation-workspace:not(.discussion-open) .agent-panel { visibility: hidden; }
  .presentation-workspace:not(.discussion-open) :deep(.agent-presentation.expanded) { visibility: visible; }
}
@media (max-width: 700px) {
  .workspace-tools :deep(.presentation-toolbar) { flex-basis: 100%; }
}
.right-rail.fullscreen {
  position: fixed;
  inset: var(--app-input-viewport-top, 0px) 0 auto;
  z-index: 70;
  width: 100%;
  height: var(--app-input-viewport-height, var(--app-content-height, 100dvh));
  max-height: none;
  border: 0;
  background: var(--canvas);
  backdrop-filter: none;
  -webkit-backdrop-filter: none;
}
.right-rail.fullscreen .agent-panel {
  width: 100%;
}
.right-rail.fullscreen .agent-head,
.right-rail.fullscreen .agent-input {
  width: 100%;
  max-width: 56rem;
  margin-inline: auto;
}
.right-rail.fullscreen .transcript > * {
  max-width: 52rem;
  margin-inline: auto;
}
.right-rail.fullscreen .transcript {
  min-height: 0;
}
.right-rail.fullscreen :deep(.conversation-learning-goal) { width: calc(100% - 48px); max-width: 52rem; margin-inline: auto; }
.right-rail.fullscreen .transcript { padding: 30px 24px 32px; }
.fullscreen-button {
  min-height: 40px;
  border: 1px solid var(--hairline);
  border-radius: 999px;
  background: var(--canvas);
  color: var(--ink);
  padding: 0.5rem 0.8rem;
  font-size: 0.82rem;
  font-weight: 600;
  white-space: nowrap;
}
.right-rail.fullscreen .agent-input {
  padding-bottom: max(0.75rem, env(safe-area-inset-bottom));
}
.context-tabs {
  flex: 0 0 auto;
  display: grid;
  grid-template-columns: repeat(6, minmax(0, 1fr));
  gap: 0.2rem;
  padding: 0.55rem;
  border-bottom: 1px solid var(--hairline-soft);
  background: var(--surface-soft);
}
.tab {
  position: relative;
  min-height: 40px;
  border: 0;
  border-bottom: 2px solid transparent;
  border-radius: 6px;
  color: var(--steel);
  background: transparent;
  padding: 0.55rem 0.25rem;
  font-size: 0.82rem;
  transition: color var(--motion-fast) var(--motion-ease), border-color var(--motion-tab) var(--motion-ease);
}
.tab-badge {
  position: absolute;
  top: 2px;
  right: 4px;
  min-width: 17px;
  height: 17px;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  border-radius: 999px;
  background: var(--reader-amber);
  color: var(--ink);
  padding: 0 4px;
  font-size: 0.62rem;
  font-variant-numeric: tabular-nums;
}
.tab.active {
  color: var(--brand-green-deep);
  border-bottom-color: var(--reader-coral);
  background: var(--reader-card);
}
.tab-panel {
  flex: 1;
  min-height: 0;
  overflow: hidden;
}
.agent-panel {
  position: relative;
  display: flex;
  flex-direction: column;
}
.conversation-main { display: flex; flex-direction: column; flex: 1; min-width: 0; min-height: 0; }
.agent-input-tools { display: flex; flex-wrap: wrap; align-items: flex-start; justify-content: space-between; gap: 4px 8px; min-width: 0; }
.agent-input-tools :deep(.tutor-control) { flex: 1 1 120px; }
.agent-input-tools :deep(.practice-margin-setting) { flex: 0 0 auto; order: 1; white-space: nowrap; }
.agent-input-tools :deep(.practice-offer) { flex: 1 0 100%; order: 2; box-sizing: border-box; margin-bottom: 0; }
.conversation-learning-actions { flex: 0 0 auto; display: flex; flex-wrap: wrap; justify-content: flex-end; gap: 6px; }
.conversation-learning-actions :deep(button) { min-height: 36px; padding: 4px 6px; border: 0; background: transparent; color: var(--brand-green-deep); font: inherit; font-size: .78rem; cursor: pointer; }
.empty h3 { margin: 0 0 8px; font-size: 1.18rem; font-weight: 500; color: var(--ink); }
.empty .empty-history-button { margin-top: 16px; }
.conversation-welcome { width: 100%; max-width: 360px; margin: 12px auto 0; text-align: center; }
.conversation-welcome .welcome-mascot { margin-bottom: 12px; }
.conversation-welcome h3 { font: 400 24px/1.5 var(--font-display); font-synthesis: none; }
.conversation-welcome p { margin: 0; line-height: 1.7; }
.right-rail.fullscreen .transcript > .conversation-welcome { max-width: 360px; }
.context-panel {
  overflow-y: auto;
  padding: 1rem;
}
.agent-head,
.panel-head {
  display: flex;
  justify-content: space-between;
  align-items: flex-start;
  gap: 0.75rem;
}
.agent-head {
  padding: 1rem 1rem 0.7rem;
}
.profile-panel {
  overflow: hidden;
}
.artifact-panel {
  min-width: 0;
  overflow: hidden;
}
.chat-actions {
  flex: 0 1 auto;
  display: flex;
  flex-wrap: wrap;
  justify-content: flex-end;
  gap: 0.4rem;
}
.chat-secondary-actions { display: contents; }
.chat-menu-toggle { display: none; }
.chat-actions > .new-chat { order: 1; }
.chat-actions button,
.history-card-actions button,
.history-goto {
  white-space: nowrap;
}
.chat-actions button:disabled {
  opacity: 0.45;
  cursor: default;
}
.history-button,
.recap-button,
.new-chat,
.history-open {
  min-height: 40px;
  border: 1px solid var(--ink);
  border-radius: 999px;
  background: var(--ink);
  color: #fff;
  padding: 0.5rem 0.95rem;
  font-size: 0.82rem;
  font-weight: 600;
}
.history-button {
  display: inline-flex;
  align-items: center;
  gap: 0.4rem;
  border-color: var(--hairline);
  background: var(--canvas);
  color: var(--ink);
}
.history-button span {
  min-width: 1.35rem;
  padding: 0.05rem 0.35rem;
  border-radius: 999px;
  color: var(--brand-on-accent);
  background: var(--brand-green);
  font-size: 0.72rem;
  text-align: center;
}
.panel-head {
  margin-bottom: 0.8rem;
}
.note-fold-controls {
  flex: 0 0 auto;
  display: flex;
  gap: 0.35rem;
}
.note-fold-controls button {
  min-height: 32px;
  border: 1px solid var(--hairline);
  border-radius: 8px;
  background: var(--canvas);
  color: var(--ink);
  font-size: 0.75rem;
}
.rail-kicker {
  margin: 0 0 0.15rem;
  color: var(--steel);
  font-size: 0.72rem;
  font-weight: 600;
  text-transform: uppercase;
  letter-spacing: 0;
}
.agent-head h3,
.panel-head h3 {
  margin: 0;
  font-size: 1rem;
}
.new-chat {
  flex: 0 0 auto;
}
.history-backdrop {
  position: fixed;
  inset: 0;
  z-index: 1000;
  display: grid;
  place-items: center;
  padding: 1.5rem;
  background: rgba(10, 10, 10, 0.28);
  backdrop-filter: blur(8px);
  -webkit-backdrop-filter: blur(8px);
  pointer-events: auto;
}
.history-dialog {
  width: min(760px, calc(var(--app-content-width, 100vw) - 2rem));
  max-height: min(78vh, 780px);
  display: flex;
  flex-direction: column;
  overflow: hidden;
  border: 1px solid var(--hairline);
  border-radius: 12px;
  background: rgba(255, 255, 255, 0.96);
  box-shadow: none;
  pointer-events: auto;
}
.history-dialog-head {
  flex: 0 0 auto;
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 1rem;
  padding: 1rem 1.1rem;
  border-bottom: 1px solid var(--hairline-soft);
}
.history-dialog-head h3 {
  margin: 0;
  font-size: 1.05rem;
}
.history-close {
  width: 44px;
  height: 44px;
  min-height: 44px;
  flex: 0 0 auto;
  border: 1px solid var(--hairline);
  border-radius: 999px;
  background: var(--canvas);
  color: var(--ink);
  padding: 0;
  font-size: 1.05rem;
  line-height: 1;
}
.history-list {
  min-height: 0;
  display: grid;
  gap: 0.75rem;
  overflow-y: auto;
  padding: 0.85rem;
  background: var(--canvas-parchment);
}
.history-card {
  border: 1px solid var(--hairline);
  border-radius: 12px;
  background: var(--canvas);
  padding: 0.9rem;
  cursor: pointer;
}
.history-card.active {
  border-color: var(--brand-green);
  box-shadow: inset 0 0 0 1px var(--reader-coral-soft);
  cursor: default;
}
.history-card:not(.active):hover {
  border-color: var(--ink);
}
.history-card:focus-visible {
  outline: 2px solid var(--brand-green);
  outline-offset: 2px;
}
.history-card-head {
  display: flex;
  justify-content: space-between;
  gap: 0.75rem;
  margin-bottom: 0.65rem;
}
.history-card-head h4 {
  margin: 0 0 0.15rem;
  color: var(--ink);
  font-size: 0.95rem;
  font-weight: 650;
}
.history-card-head p {
  margin: 0;
  color: var(--steel);
  font-size: 0.78rem;
}
.active-badge {
  align-self: flex-start;
  border-radius: 999px;
  background: var(--brand-green);
  color: var(--brand-on-accent);
  padding: 0.18rem 0.55rem;
  font-size: 0.72rem;
  font-weight: 650;
}
.history-turns {
  display: grid;
  gap: 0.55rem;
  margin: 0;
  padding: 0;
  list-style: none;
}
.history-turns li {
  border-top: 1px solid var(--hairline-soft);
  padding-top: 0.55rem;
}
.history-question {
  margin: 0 0 0.4rem;
  color: var(--charcoal);
  font-size: 0.88rem;
  line-height: 1.45;
  overflow-wrap: anywhere;
}
.history-anchor-row {
  display: flex;
  align-items: center;
  gap: 0.5rem;
  color: var(--steel);
  font-size: 0.76rem;
}
.history-source-label {
  color: var(--charcoal);
  font-weight: 600;
}
.history-card-actions {
  display: flex;
  justify-content: flex-end;
  gap: 0.45rem;
  margin-top: 0.8rem;
}
.history-card-actions button:disabled {
  border-color: var(--hairline);
  background: var(--hairline);
  color: var(--muted);
}
.history-delete {
  border: 1px solid var(--hairline);
  border-radius: 999px;
  background: var(--canvas);
  color: var(--brand-error);
  padding: 0.48rem 0.8rem;
  font-size: 0.82rem;
  font-weight: 600;
}
.history-empty {
  margin: 0;
}
.transcript {
  flex: 1;
  overflow-y: auto;
  overflow-wrap: anywhere;
  word-break: break-word;
  padding: 0 1rem 1rem;
}
.turn {
  margin-bottom: 1rem;
}
.turn-quote {
  margin: 0 0 0.45rem;
  border: 1px solid var(--hairline-soft);
  border-radius: 8px;
  background: var(--canvas);
  padding: 0.55rem 0.65rem;
}
.turn-quote-head {
  display: flex;
  gap: 0.5rem;
  margin-bottom: 0.35rem;
  color: var(--steel);
  font-size: 0.72rem;
  font-weight: 650;
  text-transform: uppercase;
}
.turn-quote-head code {
  margin-left: auto;
  font-family: var(--mono);
  text-transform: none;
}
.turn-quote-head button {
  margin-left:auto; border:0; padding:0; background:transparent; color:var(--steel); font:inherit;
  text-decoration:underline; text-underline-offset:2px; cursor:pointer;
}
.turn-quote blockquote {
  margin: 0;
  color: var(--slate);
  font-size: 0.84rem;
  max-height: 8rem;
  overflow-y: auto;
  overflow-wrap: anywhere;
}
.u-msg { width: fit-content; max-width: 90%; margin: 0 0 24px auto; padding: 10px 16px; border-radius: 16px 16px 4px 16px; background: var(--surface-soft); color: var(--slate); font-size: .88rem; line-height: 1.65; font-weight: 400; }
.a-msg { padding: 0; }
.turn + .turn { padding-top: 28px; }
.formula-card,
.memory-card {
  background: var(--canvas);
  border: 1px solid var(--hairline-soft);
  border-radius: 8px;
  padding: 0.7rem 0.8rem;
}
.ans-text {
  color: var(--charcoal);
  line-height: 1.85;
}
.a-msg :deep(.md h2) { margin: 1.5em 0 .75em; font-size: 1.4em; line-height: 1.5; letter-spacing: -.025em; }
.a-msg :deep(.md h3) { margin: 1.6em 0 .65em; font-size: 1.1em; line-height: 1.6; }
.a-msg :deep(.md p) { margin: .8em 0; }
.a-msg :deep(.md blockquote) { margin: 1em 0; padding: .5em 1em; border-left: 2px solid color-mix(in srgb, var(--brand-green) 65%, var(--line)); border-radius: 0 8px 8px 0; background: var(--surface-soft); color: var(--steel); font-size: .94em; }
.a-msg :deep(.md blockquote p) { margin: .25em 0; }
.turn-agent-name { margin:10px 0 4px; color:var(--steel); font-size:.78rem; font-weight:600; overflow-wrap:anywhere; }
.answer-markdown {
  display: contents;
}
.ans-text :deep(.agent-source-button) {
  min-height: 26px;
  max-width: 100%;
  display: inline-flex;
  align-items: center;
  gap: 0.28rem;
  margin: 0 0.2rem 0.15rem;
  border: 1px solid rgba(33, 112, 214, 0.32);
  border-radius: 6px;
  background: rgba(33, 112, 214, 0.08);
  color: #1764c0;
  padding: 0.18rem 0.42rem;
  vertical-align: baseline;
  font-size: 0.74rem;
  font-weight: 650;
  line-height: 1.25;
}
.ans-text :deep(.agent-source-button:hover:not(:disabled)) {
  border-color: #1764c0;
  background: rgba(33, 112, 214, 0.14);
}
.ans-text :deep(.agent-source-button span) {
  min-width: 0;
  overflow-wrap: anywhere;
}
.pending,
.empty {
  color: var(--steel);
}
.panel-empty {
  margin-top: 1rem;
}
.incomplete {
  color: var(--brand-error);
}
.proposals {
  margin-top: 0.7rem;
  border-top: 1px solid var(--hairline-soft);
  padding-top: 0.55rem;
}
.prop-h {
  margin: 0 0 0.35rem;
  color: var(--steel);
  font-size: 0.78rem;
}
.proposal {
  display: flex;
  align-items: center;
  gap: 0.5rem;
  margin: 0.3rem 0;
  font-size: 0.86rem;
}
.prop-label {
  flex: 1;
}
.proposal button.undo {
  color: var(--brand-error);
}
.done {
  color: var(--brand-green-deep);
  font-size: 0.82rem;
}
.memory-meta code {
  font-family: var(--mono);
}
.memory-card p {
  overflow-wrap: anywhere;
}
.answer-popover {
  position: fixed;
  z-index: 1100;
  display: flex;
  gap: 0.25rem;
  padding: 0.25rem;
  border: 1px solid var(--hairline);
  border-radius: 10px;
  background: var(--canvas);
  max-width: calc(var(--app-content-width, 100vw) - 16px);
  box-shadow: 0 2px 8px rgba(0, 0, 0, 0.24);
}
.answer-popover button {
  border: 0;
  border-radius: 999px;
  background: transparent;
  color: var(--ink);
  min-height: 44px;
  min-width: 44px;
  padding: 0.28rem 0.65rem;
  font-size: 0.82rem;
  white-space: nowrap;
}
.answer-popover button:first-child { color: var(--brand-green-deep); }
.answer-selection-error { max-width: 300px; margin: 0; padding: .4rem; font-size: .8rem; line-height: 1.5; }
.answer-actions { margin-top: 12px; font-size: .75rem; color: var(--steel); }
.answer-actions summary { width: fit-content; padding: 6px 0; cursor: pointer; }
.answer-actions button { margin-top:6px; min-height:36px; }
.board-location { display: inline-flex; align-items: center; gap: 6px; max-width: 100%; min-height: 40px; padding: 7px 10px; border: 1px solid var(--line); border-radius: 9px; background: var(--surface-soft); color: var(--brand-green-deep); font-size: .8rem; line-height: 1.5; text-align: left; }
.board-location svg { flex-shrink: 0; }
.board-location:hover { border-color: var(--brand-green); }
.board-question-link { display: flex; margin: -16px 0 20px auto; }
.board-delivery-link { margin-top: 12px; }
.board-committed-parts { display: flex; flex-wrap: wrap; gap: 7px; margin: 10px 0; }
.board-parts-progress { flex-basis: 100%; margin: 0; color: var(--steel); font-size: .8rem; line-height: 1.6; }
.board-parts-progress:empty { display: none; }
.board-delivery-error { overflow-wrap: anywhere; }
.ask-draft {
  border: 1px solid var(--hairline-soft);
  border-radius: 8px;
  background: var(--canvas);
  padding: 0.65rem 0.75rem;
}
.memory-update-list {
  display: grid;
  gap: 0.35rem;
  margin-top: 0.65rem;
  border-top: 1px solid var(--hairline-soft);
  padding-top: 0.55rem;
}
.memory-update-row {
  min-width: 0;
  display: grid;
  grid-template-columns: minmax(0, 1fr) auto;
  align-items: center;
  gap: 0.12rem 0.45rem;
  border-left: 2px solid var(--brand-tag);
  padding: 0.22rem 0 0.22rem 0.5rem;
  color: var(--charcoal);
  font-size: 0.78rem;
}
.memory-update-row small {
  grid-column: 1 / -1;
  color: var(--steel);
  overflow-wrap: anywhere;
}
.memory-update-row em {
  color: var(--brand-green-deep);
  font-size: 0.72rem;
  font-style: normal;
}
.memory-update-row button {
  width: 30px;
  height: 30px;
  min-height: 30px;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  border-radius: 50%;
  padding: 0;
}
.profile-usage {
  margin-top: 0.55rem;
  color: var(--steel);
  font-size: 0.72rem;
}
.profile-usage summary {
  color: var(--ink);
  cursor: pointer;
  font-weight: 600;
}
.profile-usage p {
  margin: 0.3rem 0 0;
}
.profile-usage code {
  display: block;
  margin-top: 0.3rem;
  overflow-wrap: anywhere;
  font-family: var(--mono);
  font-size: 0.68rem;
}
.ask-draft-head {
  display: flex;
  align-items: center;
  gap: 0.5rem;
  margin-bottom: 0.45rem;
  color: var(--steel);
  font-size: 0.74rem;
  font-weight: 650;
  text-transform: uppercase;
}
.ask-draft-head code {
  margin-left: auto;
  font-family: var(--mono);
  text-transform: none;
}
.ask-draft-head button {
  margin-left: auto;
  width: 24px;
  height: 24px;
  min-height: 24px;
  padding: 0;
}
.source-tabs {
  display: flex;
  flex-shrink: 0;
  gap: 0.35rem;
  overflow-x: auto;
  border-bottom: 1px solid var(--hairline-soft);
  padding: 0.55rem 0.85rem;
}
.source-tabs button {
  flex: 0 0 auto;
  min-height: 32px;
  border-radius: 6px;
  background: var(--surface);
  color: var(--steel);
  padding: 0.32rem 0.58rem;
  font-size: 0.74rem;
}
.source-tabs button.active {
  border-color: #1764c0;
  background: rgba(33, 112, 214, 0.08);
  color: #1764c0;
}
.source-open-reader {
  min-height: 38px;
  display: inline-flex;
  align-items: center;
  gap: 0.38rem;
  border-color: var(--accent);
  border-radius: 10px;
  background: var(--accent);
  color: #fff;
  padding: 0.48rem 0.72rem;
  font-size: 12px;
  text-decoration: none;
  font-weight: 650;
}
.source-open-reader:disabled {
  border-color: var(--hairline);
  background: var(--hairline);
  color: var(--muted);
}

.ask-draft blockquote {
  margin: 0;
  color: var(--slate);
  font-size: 0.84rem;
  max-height: 6rem;
  overflow-y: auto;
  overflow-wrap: anywhere;
}
.agent-input {
  border-top: 1px solid var(--hairline);
  flex: 0 0 auto;
  padding: 8px max(12px, env(safe-area-inset-right)) max(8px, env(safe-area-inset-bottom)) max(12px, env(safe-area-inset-left));
  display: flex;
  flex-direction: column;
  gap: 0.5rem;
  background: var(--canvas);
}
.learning-dock { display: flex; align-items: center; justify-content: flex-end; flex-wrap: wrap; gap: 10px; padding: 8px 0 12px; border-bottom: 1px solid var(--hairline-soft); }
.learning-dock-copy { display: none; margin-right: auto; }
.learning-dock-copy > span { font-size: .87rem; font-weight: 600; color: var(--ink); }
.learning-dock-copy p { margin: 3px 0 0; color: var(--steel); font-size: .75rem; }
.learning-dock button { display: inline-flex; align-items: center; justify-content: center; gap: 8px; min-height: 44px; padding: 10px 16px; border: 1px solid var(--line); border-radius: 11px; font-size: .84rem; font-weight: 500; white-space: nowrap; cursor: pointer; transition: background var(--motion-fast), border-color var(--motion-fast); }
.learning-dock .learning-help { color: var(--brand-green-deep); border-color: color-mix(in srgb, var(--brand-green) 32%, var(--line)); background: var(--surface-soft); }
.learning-dock .learning-help:hover:not(:disabled) { border-color: var(--brand-green); background: var(--reader-coral-soft); }
.learning-dock .learning-respond { background: var(--brand-green-deep); border-color: var(--brand-green-deep); color: #fff; box-shadow: 0 2px 4px color-mix(in srgb, var(--brand-accent-deep) 9%, transparent); }
.learning-dock .learning-respond:hover:not(:disabled) { background: color-mix(in srgb, var(--brand-green-deep) 88%, var(--ink)); }
.learning-dock button:disabled { opacity: .5; cursor: default; }
.transcript .learning-dock { justify-content: flex-start; gap: 4px; margin-top: 16px; padding: 8px 0 0; border-top: 1px solid var(--hairline-soft); border-bottom: 0; }
.transcript .learning-dock button { min-height: 36px; padding: 5px 8px; border: 0; background: transparent; color: var(--steel); box-shadow: none; font-size: .76rem; }
.agent-input .learning-dock { justify-content: flex-start; gap: 4px; padding: 0; border: 0; }
.agent-input .learning-dock-copy { display: none; }
.agent-input .learning-dock button { min-height: 30px; padding: 4px 8px; border: 0; background: transparent; box-shadow: none; color: var(--steel); font-size: .76rem; }
.agent-input .learning-dock button:hover:not(:disabled) { color: var(--ink); background: var(--surface-soft); }
@media (min-width: 1024px) { .right-rail.fullscreen .learning-dock-copy { display: block; } }
.right-rail.conversation-wide .agent-head { max-width: none; padding-inline: 24px; }
.right-rail.conversation-wide .agent-panel { display: grid; grid-template-columns: clamp(192px, 14vw, 248px) minmax(0, 1fr) clamp(208px, 16vw, 264px); }
.right-rail.conversation-wide .transcript { padding: 28px 32px 32px; }
.right-rail.conversation-wide .transcript > * { max-width: none; }
.right-rail.conversation-wide .agent-input { width: auto; max-width: none; margin: 0 24px; padding: 12px 0 max(12px, env(safe-area-inset-bottom)); }
.conversation-wide .agent-input-tools { display: none; }
.conversation-sidebar { min-width: 0; min-height: 0; overflow-y: auto; padding: 24px 16px; }
.conversation-context { border-right: 1px solid var(--hairline-soft); background: color-mix(in srgb, var(--surface-soft) 52%, var(--canvas)); }
.conversation-response { border-left: 1px solid var(--hairline-soft); }
.sidebar-section { margin-top: 24px; padding-top: 20px; border-top: 1px solid var(--hairline-soft); }
.sidebar-goal:empty + .sidebar-section { margin-top: 0; padding-top: 0; border-top: 0; }
.sidebar-label { margin: 0 0 12px; color: var(--steel); font-size: .72rem; letter-spacing: .04em; }
.right-rail.conversation-wide :deep(.conversation-learning-goal) { width: 100%; max-width: none; margin: 0; padding: 0; border: 0; }
.conversation-context :deep(.goal-heading) { grid-template-columns: 36px minmax(0, 1fr); gap: 12px 10px; }
.conversation-context :deep(.goal-icon) { grid-column: 1; grid-row: 1; }
.conversation-context :deep(.goal-label) { grid-column: 2; grid-row: 1; font-size: .76rem; }
.conversation-context :deep(.goal-text) { grid-column: 1 / -1; grid-row: 2; margin: 0; font-size: .95rem; line-height: 1.8; -webkit-line-clamp: 4; }
.conversation-context :deep(.goal-heading .goal-actions) { grid-column: 1 / -1; grid-row: 3; margin-left: -6px; }
.conversation-context .chat-secondary-actions { position: static; width: 100%; max-height: none; padding: 0; border: 0; background: transparent; box-shadow: none; gap: 6px; }
.conversation-context .chat-secondary-actions :deep(button) { display: flex; align-items: center; min-height: 40px; padding: 9px 10px; border: 0; border-radius: 6px; background: transparent; color: var(--ink); font-size: .82rem; text-align: left; }
.conversation-context .chat-secondary-actions :deep(button:hover:not(:disabled)) { background: var(--surface-soft); }
.conversation-context .chat-secondary-actions .new-chat { border: 1px solid var(--line); background: var(--canvas); }
.conversation-context .chat-secondary-actions .history-button { justify-content: space-between; }
.conversation-context :deep(.teaching-control-row) { gap: 4px; }
.conversation-context :deep(.teaching-hint) { flex-basis: 100%; line-height: 1.7; }
.conversation-response .learning-dock { flex-direction: column; align-items: stretch; gap: 10px; padding: 18px 14px 14px; border: 1px solid var(--line); border-radius: 14px; background: var(--surface-soft); }
.conversation-response .learning-dock-copy { margin: 0 0 10px; }
.conversation-response .learning-dock-copy > span { font-size: .95rem; }
.conversation-response .learning-dock-copy p { margin-top: 7px; line-height: 1.7; }
.conversation-response .learning-dock button { width: 100%; padding-inline: 10px; }
.conversation-response .learning-help { background: var(--canvas); }
.conversation-response .conversation-learning-actions { justify-content: stretch; }
.conversation-response .conversation-learning-actions :deep(button) { width: 100%; padding: 10px; text-align: left; border-radius: 8px; color: var(--ink); font-size: .84rem; }
.conversation-response .conversation-learning-actions :deep(button:hover) { background: var(--surface-soft); }
.goal-list { display: grid; gap: 0.5rem; }
.goal-card { border: 1px solid var(--hairline); border-radius: 0.6rem; padding: 0.6rem; background: var(--canvas-parchment); }
.goal-card strong { display: block; font-size: 0.83rem; overflow-wrap: anywhere; }
.goal-card p { margin: 0.35rem 0; color: var(--steel); font-size: 0.76rem; }
.goal-actions { display: flex; gap: 0.5rem; }
.goal-actions button { font-size: 0.75rem; }
.goal-target { display: flex; align-items: center; justify-content: space-between; gap: 0.5rem; font-size: 0.76rem; color: var(--steel); }
.goal-target span { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.agent-input textarea {
  box-sizing: border-box;
  flex: 1;
  width: 100%;
  min-width: 0;
  min-height: 44px;
  height: 44px;
  max-height: 120px;
  resize: none;
  font-size: 16px;
  line-height: 24px;
  padding: 9px 12px;
  grid-column: 1 / -1;
  border: 0;
  border-radius: 10px;
  background: transparent;
  box-shadow: none;
}
.agent-input textarea:focus { outline: none; }
.agent-input textarea::placeholder { color: var(--steel); opacity: .8; }
.reading-input-suggestion { grid-column: 1 / -1; padding: 0 12px 4px; }
.reading-input-invitation { grid-column: 1 / -1; margin: 0; padding: 4px 12px 0; color: var(--steel); font-size: 12px; line-height: 1.6; }
.agent-input:not(.composing) textarea { overflow-y: hidden; }
.agent-compose-row { display: grid; grid-template-columns: minmax(0, 1fr) auto; align-items: end; gap: 4px 8px; padding: 8px; border: 1px solid var(--line); border-radius: 18px; background: var(--surface-soft); }
.agent-compose-row:focus-within { border-color: color-mix(in srgb, var(--brand-accent) 55%, var(--line)); }
.agent-compose-row > .question-image-input { grid-column: 1 / -1; }
.composer-controls > .composer-voice { flex: 0 0 auto; }
.agent-compose-row > button { display: inline-flex; align-items: center; justify-content: center; gap: 8px; min-width: 96px; height: 44px; min-height: 44px; padding: 0 16px; border: 0; border-radius: 12px; font-size: 14px; font-weight: 600; line-height: 1; white-space: nowrap; }
.agent-compose-row > button > svg { flex: 0 0 auto; }
.agent-compose-row > button:focus-visible { outline: 2px solid var(--brand-accent); outline-offset: 3px; }
.agent-compose-row > button:disabled { background: var(--line); color: var(--steel); }
.agent-compose-row > .send-agent:not(:disabled) { border-color: var(--brand-accent); background: var(--brand-accent); color: var(--brand-on-accent); }
.agent-compose-row > .send-agent:hover:not(:disabled) { border-color: var(--brand-accent-deep); background: var(--brand-accent-deep); }
.agent-compose-row > .stop-agent { background: var(--ink); color: var(--canvas); }
.agent-compose-row > .stop-agent:disabled { background: var(--line); color: var(--steel); }
.run-progress-footer { display: flex; align-items: center; justify-content: space-between; gap: 10px; margin: 8px 0 16px; }
.run-progress-footer .run-status { margin: 0; }
.run-progress-footer > .stop-work-button { margin-left: auto; }
.stop-work-button { display: inline-flex; flex: 0 0 auto; align-items: center; justify-content: center; gap: 6px; min-height: 32px; padding: 5px 10px; border: 1px solid var(--line); border-radius: 16px; background: var(--canvas); color: var(--ink); font-size: 12px; cursor: pointer; }
.stop-work-button:hover:not(:disabled) { background: var(--surface-soft); border-color: var(--steel); }
.stop-work-button:disabled { color: var(--steel); cursor: default; }
.stop-work-button:focus-visible { outline: 2px solid var(--brand-accent); outline-offset: 3px; }
.composer-controls { display: flex; align-items: center; flex-wrap: wrap; gap: 4px 8px; min-width: 0; min-height: 34px; }
.presentation-request { display: inline-flex; align-items: center; gap: 6px; min-height: 34px; padding: 5px 9px; border: 1px solid var(--line); border-radius: 18px; background: var(--canvas); color: var(--steel); font-size: 12px; white-space: nowrap; cursor: pointer; }
.presentation-request:hover:not(:disabled) { border-color: var(--brand-accent); color: var(--ink); }
.presentation-request[aria-checked="true"] { border-color: var(--brand-accent); background: color-mix(in srgb, var(--brand-accent) 10%, var(--canvas)); color: var(--brand-accent-deep); }
.presentation-request:focus-visible { outline: 2px solid var(--brand-accent); outline-offset: 3px; }
.presentation-request:disabled { opacity: .55; cursor: default; }
.presentation-switch-track { display: inline-flex; align-items: center; width: 22px; height: 13px; padding: 2px; box-sizing: border-box; border-radius: 10px; background: var(--steel); }
.presentation-switch-track > span { width: 9px; height: 9px; border-radius: 50%; background: var(--canvas); }
.presentation-request[aria-checked="true"] .presentation-switch-track { background: var(--brand-accent); }
.presentation-request[aria-checked="true"] .presentation-switch-track > span { transform: translateX(9px); }
.presentation-request-hint { color: var(--steel); font-size: 11px; line-height: 1.5; }
.composer-controls .attach-image { display: grid; place-items: center; flex: 0 0 auto; width: 32px; height: 32px; min-height: 32px; padding: 0; border: 0; border-radius: 50%; background: transparent; color: var(--steel); }
.composer-controls .attach-image:hover:not(:disabled) { background: var(--line); color: var(--ink); }
.composer-controls .attach-image:disabled { opacity: .5; }
.video-binding { display: inline-flex; align-items: center; gap: 4px; min-width: 0; padding: 1px 3px 1px 8px; border-radius: 7px; background: var(--canvas); color: var(--steel); font-size: 11px; }
.video-binding > span { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.video-binding > button { display: grid; place-items: center; flex: 0 0 auto; width: 24px; height: 24px; min-height: 24px; padding: 0; border: 0; background: transparent; color: inherit; }
.composer-controls .agent-input-tools { flex: 1 1 auto; align-items: center; }
.composer-controls :deep(.teaching-hint.routine) { display: none; }
.composer-controls :deep(.teaching-switch), .composer-controls .conversation-learning-actions :deep(button) { min-height: 32px; font-size: .75rem; }
.composer-controls .conversation-learning-actions { margin-left: auto; }
.agent-input-context { display: grid; gap: 0.5rem; max-height: min(128px, calc(var(--app-input-viewport-height, var(--app-content-height, 100dvh)) * .22)); overflow-y: auto; }
.compose-caption { display: flex; justify-content: space-between; align-items: center; gap: 8px; color: var(--steel); font-size: .75rem; }
.compose-caption button { min-height: 28px; padding: 2px 8px; border: 0; background: transparent; }
.transcript { min-height: 0; padding: 12px 16px 16px; }
.rail-toolbar.agent-head { flex: 0 0 auto; min-height: 52px; padding: 4px 12px; align-items: center; flex-wrap: nowrap; gap: 8px; border-bottom: 1px solid var(--hairline-soft); z-index: 6; }
.rail-title { display: none; margin: 0; white-space: nowrap; }
.right-rail.fullscreen .rail-title { display: flex; align-items: center; gap: 10px; font-weight: 600; }
.rail-toolbar .context-tabs { display: flex; align-items: center; gap: 2px; padding: 0; border: 0; background: transparent; position: static; }
.rail-toolbar .tab { display: inline-flex; align-items: center; gap: 6px; padding: 6px 10px; min-height: 40px; white-space: nowrap; }
.context-tools, .chat-actions { position: relative; }
.chat-actions { flex: 0 0 auto; flex-wrap: nowrap; align-items: center; margin-left: auto; gap: 4px; }
.chat-menu-toggle, .context-menu-toggle { display: inline-flex; min-height: 40px; align-items: center; gap: 4px; padding: 5px 8px; border: 0; border-radius: 6px; background: transparent; color: var(--steel); font-size: .82rem; white-space: nowrap; }
.context-menu-toggle.active { background: var(--surface-soft); color: var(--ink); }
.tool-attention { color: var(--brand-green-deep); font-size: .75rem; }
.chat-actions .fullscreen-button { width: 40px; min-height: 40px; padding: 0; border: 0; border-radius: 6px; display: inline-flex; align-items: center; justify-content: center; }
.chat-secondary-actions, .secondary-tabs { display: flex; flex-direction: column; align-items: stretch; gap: 4px; position: absolute; z-index: 20; top: calc(100% + 4px); width: 230px; max-height: calc(var(--app-input-viewport-height, var(--app-content-height, 100dvh)) - 80px); overflow-y: auto; padding: 8px; border: 1px solid var(--hairline); border-radius: 10px; background: var(--canvas); box-shadow: 0 8px 24px #0002; }
.chat-secondary-actions { right: 0; }
.secondary-tabs { left: 0; width: 160px; }
.chat-secondary-actions button, .secondary-tabs .tab { justify-content: flex-start; text-align: left; min-height: 40px; padding: 7px 10px; border: 0; border-radius: 6px; background: transparent; color: var(--ink); font-weight: 400; }
.chat-secondary-actions button:hover, .secondary-tabs .tab:hover { background: var(--surface-soft); }
.task-summary-toggle { flex: 0 0 auto; display: flex; align-items: center; gap: 8px; width: 100%; min-height: 36px; padding: 6px 16px; border: 0; border-radius: 0; background: var(--canvas-parchment); font-size: .76rem; text-align: left; color: var(--steel); }
.mobile-task-toggle { display: none; }
.task-summary-status { flex: 0 1 auto; max-width: 50%; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
.task-summary-title { flex: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.task-summary-action { margin-left: auto; white-space: nowrap; color: var(--ink); }
.goal-overlay { position: absolute; z-index: 5; inset: 0; padding: 8px; background: #0002; }
.goal-details { max-height: 100%; overflow-y: auto; padding: 10px; border: 1px solid var(--hairline); border-radius: 10px; background: var(--canvas); box-shadow: 0 8px 24px #0002; }
.goal-details-head { display: flex; justify-content: space-between; align-items: center; gap: 8px; margin-bottom: 8px; }
.goal-details-head > button { display: inline-flex; align-items: center; justify-content: center; min-width: 40px; min-height: 40px; padding: 0; border: 0; background: transparent; }
.goal-work-items { display: grid; gap: 5px; list-style: none; margin: 8px 0; padding: 0; font-size: .78rem; }
.goal-work-items li { display: flex; align-items: baseline; gap: 8px; }
.goal-work-items span { flex: 0 0 auto; color: var(--steel); }
.goal-actions { flex-wrap: wrap; }
@container reading-assistant (max-width: 380px) {
  .rail-toolbar.agent-head { padding-inline: 8px; gap: 4px; }
  .rail-toolbar .tab { padding-inline: 8px; }
  .task-summary-title { display: none; }
  .secondary-tabs { left: auto; right: 0; }
}
@media (max-width: 1023px) {
  .rail-title { display: flex; align-items: center; gap: 8px; }
  .rail-toolbar .context-tabs { display: none; }
  .rail-toolbar.agent-head { min-height: 48px; padding: 0 8px 0 12px; gap: 4px; }
  .rail-title { font-size: 1rem; margin-right: auto; }
  .chat-actions { margin-left: 0; gap: 0; }
  .mobile-task-toggle { display: block; flex: 0 0 auto; border: 0; background: transparent; color: var(--brand-green-deep); min-height: 44px; padding: 0 6px; font-size: .76rem; white-space: nowrap; }
  .task-summary-toggle { display: none; }
  .presentation-workspace .task-summary-toggle { display: flex; }
  .agent-input { padding-top: 0; gap: 0; }
  .learning-dock { gap: 8px; padding: 10px 0; flex-wrap: nowrap; justify-content: space-between; }
  .learning-dock button { padding: 9px 11px; font-size: .8rem; gap: 6px; }
  .learning-dock .learning-respond { flex: 1; }
  .right-rail.fullscreen :deep(.conversation-learning-goal) { width: 100%; }
  .right-rail.fullscreen .transcript { padding: 20px 16px 24px; }
  .agent-input-context, .compose-caption { margin-bottom: 8px; }
  .conversation-learning-actions :deep(button) { min-height: 44px; padding: 4px 0 4px 6px; white-space: nowrap; }
  .goal-overlay { z-index: 10; display: flex; align-items: flex-end; padding: 0; }
  .goal-details { width: 100%; border-radius: 14px 14px 0 0; }
}
.formula-meaning {
  margin: 0 0 0.75rem;
  font-weight: 600;
}
.context-block {
  margin-top: 0.75rem;
}
.context-block h4 {
  margin: 0 0 0.35rem;
  font-size: 0.86rem;
}
.context-block dl {
  margin: 0;
}
.context-block dt {
  font-weight: 600;
}
.context-block dd {
  margin: 0 0 0.45rem 1rem;
  color: var(--slate);
}
.context-block ul {
  margin: 0;
  padding-left: 1.2rem;
}
.memory-list {
  display: grid;
  gap: 0.65rem;
}
.memory-meta {
  display: flex;
  justify-content: space-between;
  gap: 0.5rem;
  margin-bottom: 0.4rem;
  color: var(--steel);
  font-size: 0.75rem;
  font-weight: 600;
  text-transform: uppercase;
}
.note-memory-card[open] .note-preview {
  display: none;
}
.note-preview {
  grid-column: 1 / -1;
  margin: 0.45rem 0 0;
  color: var(--slate);
  font-size: 0.84rem;
  line-height: 1.45;
  display: -webkit-box;
  -webkit-line-clamp: 3;
  -webkit-box-orient: vertical;
  overflow: hidden;
  overflow-wrap: anywhere;
  text-transform: none;
}
.note-preview :deep(h1),
.note-preview :deep(h2),
.note-preview :deep(h3),
.note-preview :deep(h4),
.note-preview :deep(h5),
.note-preview :deep(h6) {
  margin: 0.35em 0 0.25em;
  font-size: 0.9rem;
  line-height: 1.25;
}
.note-preview :deep(table) {
  min-width: 100%;
  font-size: 0.78rem;
}
.note-preview :deep(th),
.note-preview :deep(td) {
  padding: 0.25em 0.35em;
}
.note-preview :deep(ul),
.note-preview :deep(ol) {
  padding-left: 1.25em;
}
.note-preview :deep(.katex-display) {
  margin: 0.35em 0;
}
.note-source-button {
  min-width: 0;
  flex: 1;
  border: 0;
  border-radius: 6px;
  padding: 0;
  color: var(--stone);
  background: transparent;
  font-family: var(--mono);
  font-size: 0.75rem;
  text-align: left;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  text-transform: none;
}
.note-source-button:hover {
  color: var(--ink);
}
.note-placement-action {
  width: 100%;
  min-height: 44px;
  margin-top: 0.65rem;
  border: 1px solid var(--hairline);
  border-radius: 7px;
  background: var(--surface);
  color: var(--ink);
}
.note-memory-summary {
  cursor: pointer;
  list-style: none;
  display: grid;
  grid-template-columns: auto minmax(0, 1fr) auto;
  align-items: center;
  gap: 0.35rem 0.5rem;
}
.note-memory-summary::-webkit-details-marker {
  display: none;
}
.note-association-title {
  min-width: 0;
  flex: 1;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  text-transform: none;
}
.note-memory-summary em {
  color: var(--stone);
  font-style: normal;
  text-transform: uppercase;
}
.memory-kind-with-location {
  display: inline-flex;
  align-items: center;
  gap: 0.35rem;
  min-width: 0;
}
.memory-kind-with-location small {
  padding: 0.08rem 0.28rem;
  border: 1px solid var(--hairline-soft);
  border-radius: 5px;
  color: var(--stone);
  font-size: 0.64rem;
  font-weight: 500;
  line-height: 1.2;
  text-transform: none;
  white-space: nowrap;
}
.memory-kind-with-location small[data-location="partial"],
.memory-kind-with-location small[data-location="unmapped"] {
  border-color: rgba(194, 132, 38, 0.28);
  color: #8a5a14;
  background: rgba(252, 239, 198, 0.42);
}
.highlight-card {
  background: #fffdf0;
}
.mobile-conversation-header { display: flex; align-items: center; width: 100%; min-width: 0; gap: 4px; }
.mobile-back, .mobile-editor-close { display: grid; place-items: center; flex: 0 0 44px; width: 44px; height: 44px; padding: 0; border: 0; border-radius: 12px; background: transparent; color: var(--steel); }
.mobile-learning-title { display: flex; flex: 1; flex-direction: column; justify-content: center; gap: 3px; min-width: 0; min-height: 52px; padding: 4px 0; border: 0; background: transparent; color: var(--ink); text-align: left; }
.mobile-learning-title > span { display: flex; align-items: center; gap: 7px; font-size: .92rem; font-weight: 600; }
.mobile-learning-title small { max-width: 100%; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; color: var(--steel); font-size: .68rem; font-weight: 400; }
.mobile-learning-title small.attention { color: var(--brand-green-deep); }
.mobile-records { flex: 0 0 auto; }
.mobile-records :deep(button) { min-height: 44px; padding: 6px 10px; border: 0; background: transparent; color: var(--brand-green-deep); font-size: .74rem; white-space: nowrap; }
.mobile-tool-section { padding: 16px 0; border-top: 1px solid var(--line); }
.mobile-tool-section > p { margin: 0 0 10px; color: var(--steel); font-size: .73rem; }
.mobile-tool-section .chat-secondary-actions { position: static; width: 100%; max-height: none; padding: 0; border: 0; border-radius: 0; background: transparent; box-shadow: none; }
.mobile-tool-section .chat-secondary-actions :deep(button) { display: flex; align-items: center; justify-content: space-between; min-height: 44px; padding: 8px 0; border: 0; background: transparent; text-align: left; color: var(--ink); font-size: .9rem; font-weight: 400; }
.mobile-tool-links { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 4px 20px; }
.mobile-tool-links > button { display: flex; align-items: center; justify-content: space-between; gap: 8px; min-height: 44px; padding: 8px 0; border: 0; background: transparent; text-align: left; color: var(--steel); font-size: .83rem; }
.mobile-response-toolbar { display: flex; align-items: flex-start; gap: 6px; }
.mobile-response-toolbar > div { flex: 1; min-width: 0; }
.mobile-response-toolbar :deep(.activity-actions) { gap: 6px; }
.mobile-response-toolbar :deep(button) { min-height: 44px; padding-inline: 12px; }
.mobile-response-toolbar :deep(.primary) { flex: 1; }
@media (max-width: 1023px) {
  .right-rail.conversation-mobile .agent-head { padding: 0 4px; min-height: 56px; }
  .right-rail.conversation-mobile .context-tabs { display: none; }
  .right-rail.conversation-mobile .transcript { padding: 18px 16px 24px; }
  .right-rail.conversation-mobile .agent-input { padding: 8px max(12px, env(safe-area-inset-right)) max(8px, env(safe-area-inset-bottom)) max(12px, env(safe-area-inset-left)); gap: 0; }
  .conversation-mobile .agent-input-tools { display: none; }
  .conversation-mobile .learning-dock { justify-content: flex-start; gap: 6px; padding: 0 0 4px; border: 0; flex-wrap: nowrap; }
  .conversation-mobile .learning-dock button { flex: 0 0 auto; min-height: 36px; min-width: 0; padding: 6px 9px; font-size: .75rem; }
  .conversation-mobile .learning-dock-copy { display: none; }
  .conversation-mobile .learning-dock :is(.learning-help, .learning-respond) { border-color: transparent; background: transparent; color: var(--steel); box-shadow: none; }
  .conversation-mobile .learning-dock :is(.learning-help, .learning-respond):hover:not(:disabled) { background: var(--surface-soft); color: var(--ink); }
  .conversation-mobile .composer-controls .attach-image { width: 40px; height: 40px; }
  .conversation-mobile .stop-work-button, .workspace-tools .stop-work-button { min-height: 40px; padding-inline: 12px; }
  .conversation-mobile .compose-caption { margin-bottom: 5px; padding: 0 4px; }
  .conversation-mobile .compose-caption button { min-height: 36px; }
}
.welcome-learning-actions { display: flex; flex-direction: column; align-items: center; gap: 8px; margin: 18px auto 8px; max-width: 300px; }
.welcome-learning-actions .start-current-learning { display: inline-flex; align-items: center; justify-content: center; gap: 8px; width: 100%; min-height: 44px; padding: 11px 16px; border: 1px solid var(--brand-green-deep); border-radius: 10px; background: var(--brand-green-deep); color: #fff; font: 500 .9rem/1.5 var(--sans); cursor: pointer; }
.welcome-learning-actions .start-current-learning:disabled { opacity: .5; cursor: default; }
.welcome-learning-actions small { color: var(--steel); font-size: .76rem; line-height: 1.6; overflow-wrap: anywhere; }
.learning-dock.has-practice-actions { flex-wrap: wrap; justify-content: flex-start; gap: 8px; }
.answer-practice-actions { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 8px; width: 100%; }
.learning-dock .answer-practice-actions button { min-width: 0; min-height: 42px; padding: 9px 10px; white-space: normal; border: 1px solid var(--line); border-radius: 9px; background: var(--surface-soft); color: var(--ink); font-size: .8rem; line-height: 1.5; }
.learning-dock .answer-practice-actions .learning-explain { border-color: var(--brand-green-deep); background: var(--brand-green-deep); color: #fff; }
.learning-dock .answer-practice-actions button:hover:not(:disabled) { filter: brightness(.97); }
.learning-practice-note { grid-column: 1 / -1; margin: 0; color: var(--steel); font-size: .72rem; line-height: 1.65; overflow-wrap: anywhere; }
.conversation-response .answer-practice-actions { grid-template-columns: minmax(0, 1fr); }
.follow-up-tray {
  flex:0 1 auto; min-height:0; max-height:68%; overflow:auto; overscroll-behavior:contain;
  border-top:1px solid var(--line); border-radius:12px 12px 0 0; padding:8px 16px 14px; background:var(--canvas);
  box-shadow:0 -3px 14px color-mix(in srgb,var(--ink) 5%,transparent);
}
[data-follow-up-host="side"]:not(:empty) { margin-bottom:14px; }
.follow-up-origin { flex:none; display:flex; align-items:center; gap:10px; padding:3px 14px; min-width:0; border-bottom:1px solid var(--hairline-soft); color:var(--steel); font-size:.78rem; }
.follow-up-origin button,.follow-up-toggle { display:flex; align-items:center; gap:4px; border:0; background:transparent; font:inherit; color:var(--brand-accent-deep); cursor:pointer; }
.follow-up-origin button { flex:none; min-height:32px; padding:0; }
.follow-up-origin span,.follow-up-toggle small { overflow:hidden; text-overflow:ellipsis; white-space:nowrap; }
.follow-up-toggle { width:100%; min-width:0; min-height:30px; gap:6px; padding:2px; font-size:.78rem; }
.follow-up-toggle small { margin-left:auto; font-size:inherit; color:var(--steel); }
.follow-up-toggle svg { flex:none; }
.follow-up-origin button:focus-visible,.follow-up-toggle:focus-visible { outline:2px solid var(--brand-accent); outline-offset:2px; }
.ans-text :deep(mark[data-follow-up-id]) {
  background:transparent; color:var(--brand-accent-deep); border-radius:2px; cursor:pointer;
  text-decoration:underline dotted; text-decoration-thickness:1px; text-underline-offset:.21em;
}
.ans-text :deep(mark[data-follow-up-id]:is(:hover,:focus-visible)) { background:var(--brand-accent-soft); }
.answer-popover { flex-wrap:wrap; }
@media(max-width:1023px) {
  .follow-up-toggle,.follow-up-origin button { min-height:44px; }
}
</style>
