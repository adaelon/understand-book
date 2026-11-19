<script setup lang="ts">
import { notesOwner, pendingNoteSource, noteSourceError, editThought, keepNoteDraft, notesPanel, openPersonalNotes, personalNotes, registerThoughtContext, startThought } from './personal-notes';
import { pendingUnderstandingAction, type UnderstandingAction, type UnderstandingReference } from './note-thinking';
import { pendingKnowledgeReference } from './knowledge-tree';
import UnderstandingMap from './components/UnderstandingMap.vue';
import { claimWarmupOffer, examDailyCount, MarginTracker, marginEnabled, marginQuietToday, recordMarginIgnored, setMarginEnabled, today, WARMUP_LIMIT } from './practice-pacing';
import { t as uiText, getLocale } from './i18n';

import { computed, markRaw, nextTick, onBeforeUnmount, onMounted, provide, reactive, ref, shallowRef, toRaw, watch } from "vue";
import { createYeyeFeedback } from './yeye-feedback';
import ShareImagePanel from './components/ShareImagePanel.vue';
import { answerShareSource, deliveredAnswerText, excerptShareSource, noteShareSource, shareHighlightKey, shareNoteKey, sharePresentationKey, type ShareSource } from './reading-share';
import { FolderOpen, Highlighter, Languages, MessageSquareText, RotateCcw, Save, Sparkles, X } from "@lucide/vue";
import { api as sharedApi, ApiError } from "./api";
import { activeOutline, readerOutline, outlineTitle, type OutlineItem } from './reader-outline';
import { pendingReadingActions, type PendingReadingAction } from './local-reading-action';
import { closedBookDraftKey as stableClosedBookDraftKey, closedBookMutationKey, executeClosedBookMutation, pendingClosedBookMutation } from './closed-book-recovery';
import { network, bindSceneApi, readerKey, readerPreferenceOwner, submittedRunDrafts, type ChatDraft, type LearningWorkspace } from './reader-context';
import { learningDraftKey, readLearningDraft, saveLearningDraft } from './learning-workspace-state';
import type { FollowUpNode, FollowUpOrigin, FollowUpStart } from './follow-up-conversations';
import InquiryCoursePlayer from './components/InquiryCoursePlayer.vue';
import { inquiryCourse } from './inquiry-course';
import type { CourseLaunchTarget, LearningTaskSelection, LearningTaskView } from './learning-tasks';
import type { LearningCollectionTarget } from './learning-collections';
import type { SharedInstance, SharedRelease } from './shared-workspaces';
import type { CourseLessonView } from './generated/CourseLessonView';
import type { CourseResourceState } from './generated/CourseResourceState';
import { useReaderTypography } from './useReaderTypography';
import ReaderTypographyPanel from './components/ReaderTypographyPanel.vue';
import BookCover from './components/BookCover.vue';
import { bookTitle } from './book-title';
import LocalLibraryHome, { type LibraryPage, type LocalBookAction } from './components/LocalLibraryHome.vue';
import WorkspaceDestinations from './components/WorkspaceDestinations.vue';
import VideoReaderPane from './components/VideoReaderPane.vue';
import MusicReaderPane from './components/MusicReaderPane.vue';
import type { MusicContext, MusicQuestionContext } from './music-api';
import { mediaSourceStart, type MediaSourceSnapshot } from './api';
import { createVideoQuestion, videoWindowId } from './video-question';
import { createVideoAdoption } from './video-adoption';
import type { PublishedVideo, VideoPlayback } from './video-api';
import type { VideoQuestionContext } from '../../core/src/generated/VideoQuestionContext';
import { publishedUrl, localRequest, courseCollection as sharedWorkspaces } from './reader-host';
import LoadingAnimation from './components/LoadingAnimation.vue';
import type { ScrollAnchor } from './reader-text-anchor';
import type { PaginationSource } from './reader-pagination';
import { ReaderWindowTextCache } from './reader-window-text';
import { FocusSource } from './reader-focus-source';
import { recoverSubmissions, recoverWorkspaceBinding, requestWorkspaceRecovery, pendingSubmissions, retrySubmission } from './reader-host';
import type { RecapTarget } from './session-recap';
import { resolveMcpSource, displayedSourceRange, type McpSource } from './mcp-source';
const recapProps = withDefaults(defineProps<{ recapTarget?: RecapTarget | null; mcpSource?: McpSource | null; chatDraft?: ChatDraft | null; readerVisible?: boolean; hostOpening?: boolean; materialTitle?: string; learningWorkspace?: LearningWorkspace | null; courseTrial?: { bookId: string; revision: string } | null }>(), { readerVisible: true });
const recapEmit = defineEmits<{ (e: 'mcp-source-consumed'): void; (e: 'mcp-source-error', message: string): void; (e: 'recap-publication', target: RecapTarget): void; (e: 'recap-consumed'): void; (e: 'update:chatDraft', draft: ChatDraft): void; (e: 'show-allowance'): void; (e: 'show-library'): void; (e: 'board-visibility', visible: boolean): void; (e: 'understanding-visibility', visible: boolean): void; (e:'learning-collection', target:LearningCollectionTarget):void; (e:'course-launch', target:CourseLaunchTarget, learningWorkspaceId:string):void }>();
import { runNotice as spendNotice, type RunNotice as SpendNotice } from './reader-host';
import { localCapabilities } from './local-capabilities';
const api = bindSceneApi(sharedApi, readerKey);
const mountedReaderKey = readerKey();
const publishedVideo = ref<PublishedVideo | null>(null);
const videoPlayback = ref<VideoPlayback | null>(null);
const videoMode = computed(() => !!videoPlayback.value || !!publishedVideo.value);
const musicContext = ref<MusicContext | null>(null);
const musicMode = computed(() => !!musicContext.value);
const mediaMode = computed(() => videoMode.value || musicMode.value);
const musicPane = ref<InstanceType<typeof MusicReaderPane>>();
const musicQuestionContext = ref<MusicQuestionContext | null>(null);
function askMusic(context: MusicQuestionContext) { musicQuestionContext.value = context; if (!context.image) agentInput.value ||= uiText('刚才这一段，应该注意听什么？'); workspaceRef.value?.showAssistant('agent'); }
function clearMusicQuestion() { musicQuestionContext.value = null; }
const videoDetailsLoading = ref(false), videoDetailsError = ref('');
const videoFrameReady = ref(false);
const videoPane = ref<InstanceType<typeof VideoReaderPane>>();
const videoQuestion = createVideoQuestion(videoWindowId(), input => api.videoDraft(input), id => {
  if (!videoPane.value) throw new Error(uiText('请等待视频加载'));
  return videoPane.value.captureQuestion(id);
});
const videoInitial = computed(() => {
  const video = videoPlayback.value ?? publishedVideo.value;
  return video ? {
    video: video.source.video, title: recapProps.materialTitle || videoPlayback.value?.title || video.source.original_path.split(/[\\/]/).pop()?.replace(/\.[^.]+$/, '').replace(/_/g, ' ') || uiText('视频学习'), stage: 'awaiting_transcript' as const,
    copied_bytes: 1, total_bytes: 1, duration_ms: video.source.duration_ms,
    playable: true, qa_ready: false as const, english_ready: true, failure: null, publication: video.publication,
  } : null;
});
const videoMediaUrl = ref('');
const videoAdoption = createVideoAdoption(
  () => !network.value.workspace?.learning_workspace_id && !!publishedVideo.value && !agentHistoryLoading.value && !agentHistoryError.value
    && !videoQuestion.restoring.value && !videoQuestion.draft.value && !videoQuestion.error.value
    && !agentInput.value && !sending.value && !chat.value.some(t => t.pending || t.submissionUnknown)
    && !videoPane.value?.editing,
  () => api.adoptVideoPublication(publishedVideo.value!.publication.publication_id),
  video => { if (publishedVideo.value) publishedVideo.value = { ...video, time_base: publishedVideo.value.time_base, timeline_origin_ms: publishedVideo.value.timeline_origin_ms }; },
);
let videoAdoptionTimer: ReturnType<typeof setTimeout> | undefined;
async function refreshVideoPublication() {
  // Shared workspace materials advance only through the user's selected release update.
  if (network.value.workspace?.learning_workspace_id) return;
  await videoAdoption.refresh();
  if (!disposed) videoAdoptionTimer = setTimeout(refreshVideoPublication, 3000);
}
watch(() => network.value.workspace?.published_book_ref.publication_id, async () => {
  // Recovery can discover a commit whose HTTP response was lost.
  await nextTick();
  if (!publishedVideo.value || disposed || videoAdoption.adopting.value
    || publishedVideo.value.publication.publication_id === network.value.workspace?.published_book_ref.publication_id) return;
  try { const video = await api.videoContext(); if (!disposed && video) publishedVideo.value = video; }
  catch (error) { videoAdoption.error.value = (error as Error).message; }
});
function onAgentInput(value: string) {
  agentInput.value = value;
  if (publishedVideo.value) videoQuestion.input(value);
  if (musicContext.value && value.trim() && !musicQuestionContext.value) { try { musicQuestionContext.value = musicPane.value?.captureQuestion() ?? null; } catch (failure) { banner.value = (failure as Error).message; } }
}
function enterMediaSource(source: MediaSourceSnapshot) {
  if (publishedVideo.value?.publication.book_id !== source.publication.book_id) return;
  const ms = mediaSourceStart(source);
  if (Number.isFinite(ms)) videoPane.value?.seek(ms);
}
function enterMusicSource(source: import('./music-api').MusicSourceSnapshot) {
  const current = musicContext.value?.publication, target = source.target;
  if (!current || current.book_id !== target.publication.book_id || current.publication_id !== target.publication.publication_id) return;
  try { musicPane.value?.openSource(target); workspaceRef.value?.showReader(); } catch (failure) { banner.value = (failure as Error).message; }
}
async function enterKnowledgeSource(event:Event){
  const {source,reader_key}= (event as CustomEvent<{source:import('./generated/KnowledgeSource').KnowledgeSource;reader_key:string}>).detail;
  if(reader_key!==readerKey())return;
  try{
    if(source.kind==='text') { openSourcePreview({ lid: source.lid, quote: null }); return; }
    if(source.kind==='music')musicPane.value?.openSource(source.target);
    else if(source.start_ms!=null)videoPane.value?.seek(source.start_ms);
    else throw new Error('这条来源尚无可定位的播放位置，请打开相关原文。');
    tutorPanelOpen.value=false;workspaceRef.value?.showReader();
  }catch(failure){banner.value=(failure as Error).message;}
}
async function startKnowledgeDiscussion(event:Event){
  const {node,reader_key}=(event as CustomEvent<{node:import('./knowledge-tree').KnowledgeNodeView;reader_key:string}>).detail;
  if(reader_key!==readerKey())return;
  if(sending.value){banner.value='请等当前回答完成后再从这里聊起。';return;}
  tutorPanelOpen.value=false;workspaceRef.value?.showAssistant('agent');
  await submitAgentMessage(`请带我理解「${node.label}」，讲解后问我一个问题，听听我的理解。`,`从「${node.label}」聊起`,null,undefined,{
    knowledge_focus:{reference:node.reference,node_id:node.id},source_id:node.reference.source_id,source_revision:node.reference.source_revision,
    session_id:activeChatSessionId.value,client_request_id:crypto.randomUUID(),expected_chat_head:actionHead(),
  });
}
function showKnowledge(event:Event){const detail=(event as CustomEvent).detail;if(detail.reader_key!==readerKey())return;pendingKnowledgeReference.value=detail.reference;openTutorPanel('knowledge');}
onMounted(()=>{window.addEventListener('knowledge-open-source',enterKnowledgeSource);window.addEventListener('knowledge-discuss',startKnowledgeDiscussion);window.addEventListener('knowledge-show',showKnowledge);});
onBeforeUnmount(()=>{window.removeEventListener('knowledge-open-source',enterKnowledgeSource);window.removeEventListener('knowledge-discuss',startKnowledgeDiscussion);window.removeEventListener('knowledge-show',showKnowledge);});
function cancelVideoDraft() { videoQuestion.clear(); agentInput.value = ''; }
let imageContext: import('../../core/src/generated/VideoQuestionContext').VideoQuestionContext | undefined;
let musicImageContext: MusicQuestionContext | undefined;
const musicImageError = ref('');
const questionImagePreparing = ref(false);
function prepareQuestionImage(active: boolean) {
  questionImagePreparing.value = active;
  if (!active) { imageContext = undefined; musicImageContext = undefined; return; }
  if (musicContext.value) {
    musicImageError.value = '';
    try { musicImageContext = musicQuestionContext.value ?? musicPane.value?.captureQuestion(); }
    catch (error) { musicImageError.value = (error as Error).message; }
    return;
  }
  try { imageContext = videoQuestion.draft.value?.context ?? videoPane.value?.captureQuestion(crypto.randomUUID()); }
  catch (error) { videoQuestion.error.value = (error as Error).message; }
}
function setQuestionImage(image: import('../../core/src/generated/VideoQuestionImage').VideoQuestionImage | null) {
  if (musicContext.value) {
    try {
      if (!image && !musicQuestionContext.value) return;
      const context = musicQuestionContext.value ?? musicImageContext ?? musicPane.value?.captureQuestion();
      if (!context) throw new Error(uiText('请等待乐谱加载'));
      musicQuestionContext.value = { ...context, image };
      musicImageError.value = '';
    } catch (error) { musicImageError.value = (error as Error).message; }
    return;
  }
  try { videoQuestion.setImage(image, imageContext); }
  catch (error) { videoQuestion.error.value = (error as Error).message; }
}

let disposed = false;
import { generateBookIdFromTitle } from "./book-id";
import type {
  AgentChatSessionSummary,
  AgentChatMeta,
  AgentChatTurn as StoredAgentChatTurn,
  AgentEffect,
  AgentHistoryResponse,
  ResidentGoal,
  AgentQuestionQuoteView,
  AskQuote,
  BuildIntentSelection,
  BuildIntentMode,
  BuildPlanningSource,
  BuildWorkbenchSnapshot,
  BuildStageId,
  BookLibraryEntry,
  ExecutorId,
  FormulaSemantics,
  ImageAssetManifestEntry,
  HistoricalBackfillJobRequest,
  HistoricalBackfillStartRequest,
  HistoricalBackfillStateView,
  IntentArtifactOverlayV1,
  IntentBuildReaderUsageKind,
  MemoryRecord,
  NoteBodyPlacement,
  OuterOutcome,
  PaperMinimapApplyOutcome,
  PaperMinimapCommand,
  PaperMinimapEffect,
  PaperMinimapLocalization,
  PaperMinimapStateResponse,
  PaperViewportPosition,
  PdfRegion,
  PdfSourceMap,
  PdfSourceMapEntry,
  ProfileManifest,
  ProfileGovernanceActionRequest,
  ProfileMemoryState,
  ProfileMemoryUpdate,
  ProfileSummary,
  ReaderLayoutAction,
  ReaderLayoutProposal,
  ReaderLayoutState,
  SelectionContext,
  SourceManifestV2,
  SourceReviewDecisionKind,
  SourceReviewLlmSuggestion,
  TraceStep,
  Viewport,
  WorkbenchAdapterMode,
} from "./api";
import {
  type PdfSelectionCapture,
  type PdfSelectionDraft,
  usePdfSelectionDraft,
} from "./pdf-selection-draft";
import { pdfSelectionCapabilities as getPdfSelectionCapabilities } from "./pdf-selection-capabilities";
import {
  type PdfSelectionTranslationInvalidation,
  usePdfSelectionTranslation,
} from "./pdf-selection-translation";
import PdfSelectionTranslationSurface from "./components/PdfSelectionTranslationSurface.vue";
import AlignmentQualityBar from "./components/AlignmentQualityBar.vue";
import {
  buildPdfProjectionBatch,
  EMPTY_PDF_ANNOTATION_PROJECTION,
  projectPdfAnnotations,
  type PdfUserAnnotationProjection,
} from "./pdf-annotation-projection";
import { isDisplayFormulaSource, renderFormulaSource, renderInlineMarkdown, renderMarkdown, renderTableSource } from "./md";
import {
  createMarkdownDomSourceMap,
  markMarkdownDomSourceRanges,
  type MarkedSourceRange,
} from "./markdown-source-map";
import { desktopLibraryNeedsSelection } from "./desktop-library";
import ModelProviderSettings from "./components/ModelProviderSettings.vue";
import LocalAgentSettings from "./components/LocalAgentSettings.vue";
import PersonalConnectionSettings, { type PersonalAccess } from "./components/PersonalConnectionSettings.vue";
import LearningExtensions from "./components/LearningExtensions.vue";
import PersonalNavigation, { type ExternalPassage } from "./components/PersonalNavigation.vue";
import { useLocalAgents } from "./local-agents";
import type { ModelSettingsClient } from "./desktop-provider";
import { rangeToMarkdown } from "./selection";
import {
  hasMappedPdfNavigationTarget,
  normalizePaperViewportForMinimap,
} from "./paper-minimap-navigation";
import {
  resolveReaderNavigationTarget,
  resolveReaderStateNavigationTarget,
} from "./reader-navigation";
import { groupHighlightForDisplay, recallBookAnnotations } from "./reader-annotations";
import {
  createPlacedNote,
  isMarkdownInlineNote,
  notePlacementCapability,
  reanchorPlacedNote,
  reconcilePlacedNoteCreate,
  reconcilePlacedNoteReanchor,
  type NotePlacementState,
  type NotePlacementSurface,
  useNotePlacementController,
} from "./note-placement";
import {
  getSourceReviewAutoRerunRequest,
  runSourceReviewLlmBatch,
  runSourceReviewPageGroupDecision,
  sourceReviewBatchTargets,
  type SourceReviewLlmBatchState,
} from "./source-review-batch";
import { buildUndoProfileAction } from "./profile-memory";
import {
  chooseAppSurface,
  workbenchAvailable,
  type AppSurface,
} from "./surface-selection";
import TopBar from "./components/TopBar.vue";
import CourseTutorControl from './components/CourseTutorControl.vue';
import PracticeOffer from './components/PracticeOffer.vue';
import ConversationLearningGoal from './components/ConversationLearningGoal.vue';
import { currentLearningSection, learningPurposeLabel, type LearningPurposeStart, type LearningScopeKind, type LearningScopeOption } from './learning-purpose';
import { provideReadingGuidance } from './reading-guidance';
import type { LearningPurpose } from './generated/LearningPurpose';
import TutorPanel from './components/TutorPanel.vue';
import { useTutorControl } from './useTutorControl';
import BuildWorkbenchPane from "./components/BuildWorkbenchPane.vue";
import BuildIntentPane from "./components/BuildIntentPane.vue";
import FileDropField from "./components/FileDropField.vue";
import LeftRail from "./components/LeftRail.vue";
import PdfReaderPane from "./components/PdfReaderPane.vue";
import ReaderPane from "./components/ReaderPane.vue";
import ReaderWorkspace, { type WorkspaceAuxTab } from "./components/ReaderWorkspace.vue";
import NoteEditorPanel from "./components/NoteEditorPanel.vue";
import RightRail from "./components/RightRail.vue";
import ClosedBookPractice from "./components/ClosedBookPractice.vue";
import type { ClosedBookPractice as ClosedBookState } from "./generated/ClosedBookPractice";
import ProductTour from './components/ProductTour.vue';
import type { ProductTourStep } from './product-tour';
import BoardWorkspace from './components/BoardWorkspace.vue';
import NotesWorkspace from './components/NotesWorkspace.vue';
import type { WorkspaceNoteImportSource, WorkspaceNoteSourceRequest } from './api';
import { mergeBoardDeliveries, type BoardQuestionRequest, type BoardRunView, type BoardLocation } from './board-workspace';
import type { BoardFollowUp } from './generated/BoardFollowUp';
import type { BoardDelivery } from './generated/BoardDelivery';
import BoardSourcePreview from './components/BoardSourcePreview.vue';
import type { BoardImportSource, BoardSourceRequest, SourcePopupView, SourceWindowLoader } from './api';
import { boardRouteUrl, readBoardRoute } from './board-route';
import { useAgentRun } from "./useAgentRun";
import { submissionWasAccepted, type UnknownSubmissionAudit } from "./agent-submission-recovery";
import {
  readReaderSurfacePreference,
  resolveReaderSurface,
  writeReaderSurfacePreference,
  type ReaderSurface,
} from "./reader-surface";
import type { ReaderSelectionSnapshot } from "./useReaderSelection";
import { createReadingContinuity, type PdfReadingAnchor, type ReadingReturnPoint } from "./useReadingContinuity";
import { effectId, dispositionLabel, type EffectDisposition } from "./effect-disposition";
import { agentFailureMessage, completedTeachingStep, runStatusText, type RunActivity, type RunSnapshot } from "./agent-run-state";
import type { WorkspaceLogicalState, WorkspaceProjection } from "./workspace-layout";

type NodeKind = import("./api").Manifest["tree"][number]["kind"];
type ManifestNode = import("./api").Manifest["tree"][number];
const typography = useReaderTypography(computed(readerPreferenceOwner));
function closeTypography() {
  typography.open.value = false;
  nextTick(() => {
    const trigger = Array.from(document.querySelectorAll<HTMLElement>('.tools-toggle, .focus-tools-toggle, [data-workspace-menu]'))
      .find(element => element.getClientRects().length);
    trigger?.focus({ preventScroll: true });
  });
}

export type { OutlineItem } from './reader-outline';
// ── 阅读区会话态 ──
const leafOrder = ref<string[]>([]); // 全书叶 LID 序(读位感分母 + 进度)
const kindByLid = ref<Map<string, NodeKind>>(new Map());
const imageAssetByLid = ref<Map<string, ImageAssetManifestEntry>>(new Map());
const appSurface = ref<AppSurface>("loading");
const diagnosticWorkbenchBookId = ref<string | null>(null);
const buildWorkbenchSnapshot = ref<BuildWorkbenchSnapshot | null>(null);
const buildWorkbenchLoading = ref(false);
const buildWorkbenchError = ref<string | null>(null);
const buildWorkbenchConfirming = ref(false);
const buildWorkbenchImporting = ref(false);
const buildWorkbenchActioning = ref(false);
const buildIntentOpen = ref(false);
const buildIntentSelection = ref<BuildIntentSelection | null>(null);
const buildIntentPlanningSource = ref<BuildPlanningSource | null>(null);
const buildIntentBusy = ref(false);
const buildIntentError = ref<string | null>(null);
let buildIntentRequestSeq = 0;
const intentArtifacts = ref<IntentArtifactOverlayV1 | null>(null);
const intentArtifactsLoading = ref(false);
const intentArtifactsError = ref<string | null>(null);
let intentArtifactRequestSeq = 0;
const sourceReviewLlmSuggestions = ref<Record<string, SourceReviewLlmSuggestion>>({});
const sourceReviewLlmAnalyzingBlockId = ref<string | null>(null);
const sourceReviewLlmErrors = ref<Record<string, string>>({});
const sourceReviewLlmBatch = ref<SourceReviewLlmBatchState | null>(null);
let buildWorkbenchActionOwner = 0;
let sourceReviewLlmBatchRunToken = 0;
let sourceReviewLlmRequestToken = 0;

function beginWorkbenchAction(): number {
  const owner = ++buildWorkbenchActionOwner;
  buildWorkbenchActioning.value = true;
  return owner;
}

function endWorkbenchAction(owner: number) {
  if (owner === buildWorkbenchActionOwner) buildWorkbenchActioning.value = false;
}

const sourceReviewEvidenceKey = computed(() => JSON.stringify(
  (buildWorkbenchSnapshot.value?.source_review?.unresolved ?? []).map((block) => [
    block.id,
    block.status,
    block.md_excerpt ?? "",
    block.pdf_excerpt ?? "",
    block.candidate_text ?? "",
  ]),
));
watch(sourceReviewEvidenceKey, (next, previous) => {
  if (next === previous) return;
  sourceReviewLlmBatchRunToken += 1;
  sourceReviewLlmRequestToken += 1;
  sourceReviewLlmSuggestions.value = {};
  sourceReviewLlmAnalyzingBlockId.value = null;
  sourceReviewLlmErrors.value = {};
  sourceReviewLlmBatch.value = null;
});
const sourceManifest = ref<SourceManifestV2 | null>(null);
const pdfSourceMap = ref<PdfSourceMap | null>(null);
const pdfRuntimeError = ref<string | null>(null);
const outlineItems = ref<OutlineItem[]>([]);
const titleByLid = ref<Map<string, string>>(new Map());
const viewport = ref<Viewport | null>(null);
let readerInteractionRevision = 0;
let manualReaderBoundary: Promise<void> = Promise.resolve();
let manualReaderState: { book_id: string; revision: number } | null = null;
const edgeLoading = ref(false);
const readerPaneRef = ref<{
  prepareFocus: (cursor?: { index: number; offset: number }) => Promise<void>;
  leaveFocus: (anchor: ScrollAnchor | null) => Promise<void>;
  suspendFocus: () => void;
  cancelTypographyRestore: () => void;
  captureScrollAnchor: (candidateLids: string[]) => ScrollAnchor | null;
  restoreScrollAnchor: (anchor: ScrollAnchor | null, current?: () => boolean) => Promise<void>;
  scrollLidIntoView: (lid: string) => Promise<boolean>;
  scrollSourceIntoView: (lid: string, range?: { start: number; end: number }) => Promise<boolean>;
} | null>(null);
const pdfReaderPaneRef = ref<{
  captureReadingAnchor: () => PdfReadingAnchor | null;
  restoreReadingAnchor: (anchor: PdfReadingAnchor) => Promise<boolean>;
} | null>(null);
const workspaceRef = ref<{
  showReader: () => void;
  showAssistant: (tab?: WorkspaceAuxTab) => void;
  toggleOutline: () => void;
  outlineOpen: boolean;
  toggleFocus: () => void;
} | null>(null);
const focusReading = ref(false);
const focusPaginationActive = ref(false);
const readerRegionVisible = ref(true);
const assistantVisible = ref(false);
const previousPdfAnchor = ref<PdfReadingAnchor | null>(null);
const mobileGlobalActionsOpen = ref(false);
const adaptiveNavigation = ref(false);
const rightRailRef = ref<{
  scrollToFollowUpOrigin: (origin: FollowUpOrigin) => Promise<boolean>;
  showCourseFeedback: () => void;
  prepareProductTour: () => boolean;
  prepareCompletionFeedback: () => (() => void) | undefined;
  focusActivityHelp: (deliveryRefs: string[]) => void; scrollToTurn: (turnId: string) => Promise<boolean>;
  showMemory: (memId: string) => Promise<boolean>;
  replaceMemory?: (oldId: string, record: MemoryRecord) => void;
  openNotePresentation: (record: MemoryRecord, restore?: boolean) => Promise<void>;
  locateScene: (receipt: import('./generated/PresentationFollowUp').PresentationFollowUp, restore?: boolean) => Promise<void>;
  openRecapTarget: (target: RecapTarget) => Promise<void>;
} | null>(null);
interface Segment {
  lid: string;
  text: string;
  kind: NodeKind;
  formula: FormulaSemantics | null;
  imageAsset: ImageAssetManifestEntry | null;
}
const segments = ref<Segment[]>([]); // 视口内连续正文(LID 隐形)
const focusVisible = shallowRef<{ position: ScrollAnchor; ranges: { lid: string; start: number; end: number }[]; sources: PaginationSource[] } | null>(null);
const annotations = ref<MemoryRecord[]>([]); // 当前书全部标注(客户端按 lid 过滤)
const pdfAnnotationProjection = ref<PdfUserAnnotationProjection>(EMPTY_PDF_ANNOTATION_PROJECTION);
const pdfAnnotationProjectionError = ref<string | null>(null);
let pdfAnnotationProjectionSeq = 0;
const HIGHLIGHT_GROUP_PREFIX = "highlight-group:";
const selectedLid = ref<string | null>(null);
const currentReadingLid = ref<string | null>(null);
const outlineNavigationLid = ref<string | null>(null);
const pendingNavigationLid = ref<string | null>(null);
const formulaDialog = ref<Segment | null>(null);
const formulaLoading = ref(false);
const formulaError = ref<string | null>(null);
let formulaRequest = 0;
const chapterTitle = ref<string>("");
interface SourceFocus {
  lid: string;
  quote: string | null;
  ranges?: Map<string, [number, number]>;
}
interface SourcePreview {
  focus: SourceFocus;
  source: SourcePopupView;
  readWindow: SourceWindowLoader;
}
const sourceFocus = ref<SourceFocus | null>(null);
const sourceManifestTree = ref<import("./api").Manifest | null>(null);
const sourcePreview = ref<SourcePreview | null>(null);
const profileSummary = ref<ProfileSummary | null>(null);
const profileManifest = ref<ProfileManifest | null>(null);
const profileMemory = ref<ProfileMemoryState | null>(null);
const profileMemoryLoading = ref(false);
const profileMemoryBusy = ref(false);
const profileMemoryError = ref<string | null>(null);
const profileMemoryNotice = ref<string | null>(null);
const profileUpdateStates = ref<Record<string, string>>({});
let profileMemoryRequestSeq = 0;
const profileBackfill = ref<HistoricalBackfillStateView | null>(null);
const profileBackfillLoading = ref(false);
const profileBackfillBusy = ref(false);
const profileBackfillError = ref<string | null>(null);
const profileBackfillNotice = ref<string | null>(null);
let profileBackfillRequestSeq = 0;
let profileBackfillPollTimer: number | null = null;
const readerLayout = ref<ReaderLayoutState | null>(null);
const pendingLayoutProposal = ref<ReaderLayoutProposal | null>(null);
const paperMinimapSnapshot = ref<PaperMinimapStateResponse | null>(null);
const paperMinimapLocalization = ref<PaperMinimapLocalization | null>(null);
const lastPaperMinimapEffect = ref<PaperMinimapEffect | null>(null);
const paperMinimapActionBusy = ref(false);
const paperProjectionLoading = ref(false);
const paperProjectionError = ref<string | null>(null);
const paperProjectionKey = ref("");
let paperProjectionSeq = 0;
let paperLocalizationRequest: { key: string; promise: Promise<PaperMinimapLocalization> } | null = null;
let paperPositionSyncTimer: number | null = null;
let paperPositionSyncRunning = false;
let paperPositionSyncCompletion: Promise<void> | null = null;
let completePaperPositionSync: (() => void) | null = null;
let pendingPaperViewport: PaperViewportPosition | null = null;
let pendingPaperSelection: string | null | undefined;

// goto 输入 + 错误条
const gotoInput = ref("");
const outlineSearch = ref("");
const banner = ref<string>("");
const entryView = new URLSearchParams(location.search).get('view');
const initialShelf = entryView === 'library' ? 'courses' : entryView === 'materials' || entryView === 'boards' || entryView === 'understanding' ? entryView : null;
const localPage = ref<LibraryPage | null>(network.value.enabled || readBoardRoute(location.search) ? null : initialShelf);
let startOnShelf = !!localPage.value;
const activeBookDir = ref<string | null>(null);
const shelfBusy = ref(false);
const readerVisible = computed(() => !localPage.value && recapProps.readerVisible);
const bookPickerOpen = ref(false);
const bookPickerLoading = ref(false);
const bookPickerError = ref<string | null>(null);
const bookPickerRoot = ref("");
const bookPickerBooks = ref<BookLibraryEntry[]>([]);
const bookPickerDir = ref("");
const bookPickerMode = ref<"open" | "create">("open");
const newBookTitle = ref("");
const generatedNewBookId = computed(() => generateBookIdFromTitle(newBookTitle.value));
const newBookPdf = ref<File | null>(null);
const openingBook = ref(false);
const desktopNeedsBook = ref(false);
const desktopHost = ref(false);
const localAgentHost = ref(false);
const localAgents = useLocalAgents();
const localAgentSettingsRef = ref<InstanceType<typeof LocalAgentSettings> | null>(null);
const personalAccess = ref<PersonalAccess | null>(null);
const personalConnectionSaving = ref(false);
watch(localAgentHost, async enabled => {
  personalAccess.value = null;
  if (!enabled) return;
  void localAgents.request();
  try {
    const access = await localRequest<PersonalAccess>('GET', '/personal/connection');
    if (localAgentHost.value && !personalAccess.value) personalAccess.value = access;
  } catch { /* Settings surfaces connection errors and permits retry. */ }
});
async function chooseLocalAgent(event: Event) {
  const select = event.currentTarget as HTMLSelectElement;
  const id = select.value;
  select.value = localAgents.selected.value;
  if (id === ':settings') { await openDesktopSettings('general'); return; }
  const settings = await localAgents.request({action: 'select', id: id || null});
  if (!settings) banner.value = localAgents.error.value;
}
function readyToUseAgent() {
  if (!localAgentHost.value || (localAgents.loaded.value && !localAgents.busy.value)) return true;
  banner.value = localAgents.busy.value ? uiText('请等待 Agent 设置完成。') : uiText('请在“管理我的 Agent”中重新检测连接。');
  return false;
}
async function openFailedAgentSettings(id?: string) {
  await openDesktopSettings('general');
  await nextTick();
  if (id) await localAgentSettingsRef.value?.testAgent(id);
}
const nativeSettings = '__TAURI_INTERNALS__' in window;
const readerOnly = ref(false);
const desktopSettingsOpen = ref(false);
const desktopSettingsSection = ref<'models' | 'general'>('models');
const desktopLibraryRoot = ref("");
const desktopLibraryAvailable = ref(true);
const desktopLibraryChanging = ref(false);
const desktopLibraryError = ref("");
const desktopProviderSaving = ref(false);
const codexPluginLoading = ref(false);
const codexPluginError = ref("");
type CodexPluginState =
  | "installed_by_setup"
  | "installed_externally"
  | "pending_configuration"
  | "codex_not_found"
  | "not_installed"
  | "error";
type CodexPluginStatus = {
  state: CodexPluginState;
  codex_path: string | null;
  marketplace_name: string;
  plugin_name: string;
  message: string;
};
const codexPluginStatus = ref<CodexPluginStatus | null>(null);
const localDebug = import.meta.env.VITE_LOCAL_DEBUG === '1';
const managedBuildEngine = localCapabilities()?.managedBuildEngine === true;
const localBuildStatus = ref<{ available: boolean; path: string } | null>(null);
const debugOpen = ref(false);
const leftRailOpen = ref(true);
const leftRailWidth = ref(240);
const rightRailWidth = ref(384);
const coursePlayer = ref<InstanceType<typeof InquiryCoursePlayer>>();
const localCourseWorkspace = ref<LearningWorkspace | null>(null);
const courseWorkspace = computed(() => recapProps.learningWorkspace ?? localCourseWorkspace.value);
const activeTrial = computed(() => recapProps.courseTrial?.bookId === buildWorkbenchSnapshot.value?.book_id ? recapProps.courseTrial : null);
const trialOpenError = ref('');
const coursePublication = computed(() => network.value.workspace?.published_book_ref ?? (localCourseWorkspace.value && buildWorkbenchSnapshot.value ? { book_id: buildWorkbenchSnapshot.value.book_id, publication_id: 'local' } : undefined));
const courseAvailable = ref(false), courseMode = ref(false);
const courseBlocked = ref(false);
const courseError = ref('');
const courseSurface = ref<'course' | 'source' | 'experiment'>('course');
const courseVisible = computed(() => courseMode.value && courseSurface.value !== 'source');
const courseLesson = ref<CourseLessonView | null>(null);
const courseCollectionInstance = ref<SharedInstance | null>(null), courseCollectionRelease = ref<SharedRelease | null>(null);
const courseCollectionLoading = ref(false), courseCollectionError = ref('');
let courseCollectionRequest = 0, courseCollectionLoaded = false;
const courseCollectionScope = computed(() => network.value.enabled && courseWorkspace.value
  ? `${network.value.epoch}:${network.value.identity?.user_id}:${courseWorkspace.value.learning_workspace_id}:${courseWorkspace.value.shared_release?.version ?? ''}` : null);
const courseCollectionKind = computed(() => new Set(courseCollectionRelease.value?.manifest.inquiry_course?.lessons.map(item => item.source.publication.book_id)).size > 1 ? 'course' as const : 'book' as const);
const courseCollection = computed(() => ({ kind:courseCollectionKind.value,
  title:courseCollectionKind.value === 'course' ? courseCollectionRelease.value!.manifest.name : readerBookTitle.value || uiText('本书'),
  external:network.value.enabled, overview:courseCollectionRelease.value?.manifest.inquiry_course, releaseVersion:courseCollectionRelease.value?.version,
}));
const courseUnitTitle = computed(() => courseCollectionRelease.value?.manifest.inquiry_course?.units.find(unit => unit.id === courseLesson.value?.summary.unit_id)?.title);
async function loadCourseCollection() {
  const scope = courseCollectionScope.value, workspace = courseWorkspace.value;
  if (!scope || !workspace || courseCollectionLoading.value) return;
  const request = ++courseCollectionRequest;
  courseCollectionLoading.value = true; courseCollectionError.value = '';
  try {
    if (!courseCollectionLoaded) {
      const value = await sharedWorkspaces.instance(workspace.learning_workspace_id);
      if (scope !== courseCollectionScope.value || request !== courseCollectionRequest) return;
      courseCollectionInstance.value = value.instance; courseCollectionLoaded = true;
    }
    const instance = courseCollectionInstance.value;
    const version = courseLesson.value?.progress.release_version ?? instance?.version;
    const release = instance && version !== instance.version ? await sharedWorkspaces.read(instance.release_id, version!) : instance?.release ?? null;
    if (scope === courseCollectionScope.value && request === courseCollectionRequest && version === (courseLesson.value?.progress.release_version ?? instance?.version)) courseCollectionRelease.value = release;
  } catch (error) {
    if (scope === courseCollectionScope.value && request === courseCollectionRequest) courseCollectionError.value = uiText('课程归属暂时无法读取：{0}', [errorMessage(error)]);
  } finally { if (request === courseCollectionRequest) {
    courseCollectionLoading.value = false;
    const instance = courseCollectionInstance.value;
    if (!courseCollectionError.value && instance && (courseLesson.value?.progress.release_version ?? instance.version) !== courseCollectionRelease.value?.version) void loadCourseCollection();
  } }
}
watch([courseCollectionScope, courseMode, () => courseLesson.value?.progress.release_version], ([scope, active, version], previous) => {
  if (scope !== previous?.[0]) {
    courseCollectionRequest++; courseCollectionLoaded = false; courseCollectionLoading.value = false;
    courseCollectionInstance.value = null; courseCollectionRelease.value = null; courseCollectionError.value = '';
  }
  const requestedVersion = version ?? courseCollectionInstance.value?.version;
  if (scope && active && (!courseCollectionLoaded || requestedVersion && requestedVersion !== courseCollectionRelease.value?.version)) void loadCourseCollection();
});
function returnBookCollection() {
  const publication = coursePublication.value;
  if (!network.value.enabled || !publication) return;
  const learningWorkspaceId = network.value.workspace?.learning_workspace_id ?? undefined;
  recapEmit('learning-collection', {kind:'book', publication, ...(learningWorkspaceId ? {learningWorkspaceId} : {})});
}
function returnCourseCollection() {
  const release = courseCollectionRelease.value;
  if (courseCollectionKind.value === 'course' && release) recapEmit('learning-collection', {kind:'course',releaseId:release.release_id,version:release.version,learningWorkspaceId:courseWorkspace.value!.learning_workspace_id});
  else returnBookCollection();
}
function launchCourseLesson(target:CourseLaunchTarget) {
  if (network.value.enabled && courseWorkspace.value) recapEmit('course-launch', target, courseWorkspace.value.learning_workspace_id);
}
const courseGoalChanged = computed(() => !!courseLesson.value && courseLesson.value.progress.goal_revision !== courseWorkspace.value?.goal.revision);
const courseExploration = ref<CourseResourceState | null>(null);
const pendingCourseLaunch = ref<{ lessonId?: string; runId?: string } | null>(null);
let courseLaunchPromise: Promise<CourseLessonView | null> | null = null;
let openingCourse = false;
let readingRequested = false;
const courseViewKey = computed(() => {
  const owner = network.value.enabled ? network.value.identity?.user_id : 'local', scope = courseWorkspace.value?.learning_workspace_id;
  const publication = coursePublication.value?.publication_id;
  return owner && scope && publication ? `understand-book:learning:${owner}:course-view:${scope}:${publication}` : null;
});
watch(() => buildWorkbenchSnapshot.value?.book_id, async bookId => {
  if (network.value.enabled) return;
  localCourseWorkspace.value = null; courseAvailable.value = false;
  courseMode.value = false; courseLesson.value = null;
  if (!bookId) return;
  const trial = activeTrial.value;
  const scope = trial ? `local-course-trial:${bookId}:${trial.revision}` : `local-course:${bookId}`;
  trialOpenError.value = '';
  try {
    const catalog = await inquiryCourse.catalog(scope);
    if (buildWorkbenchSnapshot.value?.book_id !== bookId || disposed) return;
    if (trial && catalog.course?.lessons.length !== 1) throw new Error(uiText('此试课版本需要包含一节代表课。'));
    if (!catalog.course) return;
    const sample = trial ? catalog.course.lessons[0] : null;
    const goal = { text: sample?.objective ?? '理解理论并独立解题', constraints: sample ? [sample.prerequisites] : ['已学线性代数和微积分'], revision: 1, created_at: 0 };
    localCourseWorkspace.value = { learning_workspace_id: scope, goal, goal_history: [], members: [], archived: false, created_at: 0, updated_at: 0 };
    if (sample) await openCourse({ lessonId: sample.id, record: false });
  } catch (e) {
    if (buildWorkbenchSnapshot.value?.book_id !== bookId || disposed) return;
    courseError.value = e instanceof Error ? e.message : uiText('本地课程暂时无法读取');
    if (trial) trialOpenError.value = courseError.value;
  }
}, { immediate: true });
let coursePreviousOutline = true;
let courseSourceVisited: string | null = null;
function courseChanged(value: CourseLessonView | null) {
  if (!value || value.progress.stage !== courseLesson.value?.progress.stage) courseSurface.value = 'course';
  if (value?.progress.run_id !== courseLesson.value?.progress.run_id || value?.round_index !== courseLesson.value?.round_index) {
    courseExploration.value = value?.exploration ?? null;
    courseSourceVisited = null;
  }
  courseLesson.value = value;
}
async function applyCourseLaunch() {
  if (courseLaunchPromise) return courseLaunchPromise;
  const player = coursePlayer.value, target = pendingCourseLaunch.value;
  if (!player || !target) return null;
  courseLaunchPromise = (async () => {
    const lesson = target.lessonId || target.runId ? await player.openLesson(target.lessonId, target.runId) : await player.resumeSaved();
    if (pendingCourseLaunch.value === target) pendingCourseLaunch.value = null;
    return lesson;
  })();
  try { return await courseLaunchPromise; }
  finally { courseLaunchPromise = null; }
}
async function openCourse(target: { lessonId?: string; runId?: string; chatSessionId?: string; record?: boolean } = {}, signal?: AbortSignal) {
  readingRequested = false;
  pendingCourseLaunch.value = target;
  await initialReaderLoad;
  signal?.throwIfAborted();
  if (readingRequested) throw new DOMException(uiText('已切换为阅读原文。'), 'AbortError');
  if (workspaceInitializationFailed) throw new Error(workspaceInitializationError ?? uiText('学习现场尚未准备好，请重试。'));
  await initialHistoryLoad;
  signal?.throwIfAborted();
  if (readingRequested) throw new DOMException(uiText('已切换为阅读原文。'), 'AbortError');
  if (disposed) throw new Error(uiText('学习现场已切换，请重新继续。'));
  if (agentHistoryError.value) throw new Error(agentHistoryError.value);
  if (target.chatSessionId) await restoreLearningChat(target.chatSessionId);
  // Establish the conversation used for continuation and closed-book work
  // before delivering the first lesson.
  if (target.record !== false) { await ensureLearningChat(); await nextTick(); }
  signal?.throwIfAborted();
  if (readingRequested) throw new DOMException(uiText('已切换为阅读原文。'), 'AbortError');
  if (!courseMode.value) coursePreviousOutline = leftRailOpen.value;
  courseMode.value = true; courseSurface.value = 'course'; leftRailOpen.value = false;
  if (courseViewKey.value) sessionStorage.setItem(courseViewKey.value, 'course');
  agentFullscreen.value = false;
  openingCourse = true;
  try {
    await nextTick();
    if (!coursePlayer.value) throw new Error(uiText('本材料的课程暂时无法打开。'));
    const lesson = await applyCourseLaunch();
    signal?.throwIfAborted();
    if (readingRequested) throw new DOMException(uiText('已切换为阅读原文。'), 'AbortError');
    if (lesson && ((target.runId && lesson.progress.run_id !== target.runId) || (target.lessonId && lesson.summary.id !== target.lessonId))) throw new Error(uiText('课程尚未打开，请重试。'));
    if (lesson) courseChanged(lesson);
    coursePlayer.value.showTask();
    workspaceRef.value?.showReader(); requestWorkspaceTab('agent');
    if (target.record !== false && courseLesson.value) await recordLearningTask({ course_run_id: courseLesson.value.progress.run_id });
    return courseLesson.value;
  } finally { openingCourse = false; }
}
watch([coursePlayer, courseViewKey], ([player, key]) => {
  if (!player) return;
  if (readingRequested || openingCourse || pendingCourseLaunch.value) return;
  if (!courseMode.value && key && sessionStorage.getItem(key) === 'course') void openCourse({record:false}).catch(error => { courseError.value = errorMessage(error); });
});
async function ensureLearningChat() {
  if (disposed) throw new Error(uiText('学习现场已切换，请重新继续。'));
  // The first course can start before the learner has sent any chat message.
  // Create its private conversation at this explicit engagement, so resumption
  // and later closed-book work have the same durable conversation identity.
  if (!activeChatSessionId.value && !agentHistoryError.value) {
    const result = await api.agentNew();
    if (disposed) throw new Error(uiText('学习现场已切换，请重新继续。'));
    applyAgentHistory(result.history);
  }
  if (!activeChatSessionId.value) throw new Error(uiText('学习对话尚未准备好，请重试。'));
}
async function recordLearningTask(selection: LearningTaskSelection) {
  if (disposed || activeTrial.value) return;
  await ensureLearningChat();
  await api.selectLearningTask({ chat_session_id: activeChatSessionId.value, ...selection });
  if (!network.value.enabled) await loadLocalLearningTasks();
}
const localLearningTasks = ref<LearningTaskView[]>([]);
const localLearningTaskBusy = ref(false);
const localLearningTask = computed(() => activeTrial.value ? undefined : localLearningTasks.value.find(task => task.status !== 'unavailable'));
async function loadLocalLearningTasks() {
  if (network.value.enabled) return;
  const book = buildWorkbenchSnapshot.value?.book_id;
  if (!book) return;
  const result = await api.learningTasks();
  if (!disposed && !network.value.enabled && buildWorkbenchSnapshot.value?.book_id === book) localLearningTasks.value = result.tasks;
}
watch(() => buildWorkbenchSnapshot.value?.book_id, () => {
  localLearningTasks.value = [];
  if (!network.value.enabled) void loadLocalLearningTasks().catch(error => { banner.value = uiText('接续位置暂时无法读取：{0}', [errorMessage(error)]); });
});
async function resumeLocalLearningTask() {
  const task = localLearningTask.value;
  if (!task || localLearningTaskBusy.value) return;
  localLearningTaskBusy.value = true;
  try { await resumeLearningTask(task); }
  catch (error) { banner.value = errorMessage(error); }
  finally { localLearningTaskBusy.value = false; }
}
async function courseEngaged(lesson: CourseLessonView) {
  if (openingCourse) return;
  await rememberLearningTask({ course_run_id: lesson.progress.run_id });
}
async function rememberLearningTask(selection: LearningTaskSelection = courseMode.value && courseLesson.value ? { course_run_id: courseLesson.value.progress.run_id } : { continue_conversation: true }) {
  try { await recordLearningTask(selection); }
  catch (error) { banner.value = uiText('接续位置尚未保存：{0}', [errorMessage(error)]); }
}
async function restoreLearningChat(sessionId: string) {
  await initialReaderLoad;
  await initialHistoryLoad;
  if (disposed) throw new Error(uiText('学习现场已切换，请重新继续。'));
  if (activeChatSessionId.value === sessionId && !agentHistoryError.value) return;
  ++startupHistoryRequestSeq;
  agentHistoryLoading.value = true; agentHistoryError.value = null;
  try {
    const history = await api.agentHistorySelect(sessionId);
    if (history.current.id !== sessionId) throw new Error(uiText('原学习对话暂时无法恢复。'));
    applyAgentHistory(history);
    await refreshClosedBook();
  } catch (error) { agentHistoryError.value = errorMessage(error); throw error; }
  finally { agentHistoryLoading.value = false; }
}
async function resumeLearningTask(task: LearningTaskView, signal?: AbortSignal) {
  if (task.course_run_id) pendingCourseLaunch.value={runId:task.course_run_id,lessonId:task.lesson_id ?? undefined};
  else {
    pendingCourseLaunch.value=null;
    if(courseViewKey.value)sessionStorage.removeItem(courseViewKey.value);
    courseMode.value=false;courseSurface.value='source';leftRailOpen.value=coursePreviousOutline;
  }
  await initialReaderLoad;
  signal?.throwIfAborted();
  if (workspaceInitializationFailed) throw new Error(workspaceInitializationError ?? uiText('学习现场尚未准备好，请重试。'));
  if (task.chat_session_id) await restoreLearningChat(task.chat_session_id);
  signal?.throwIfAborted();
  if (task.course_run_id) {
    await openCourse({ runId: task.course_run_id, lessonId: task.lesson_id ?? undefined, record: false },signal);
    if (courseLesson.value?.progress.status === 'paused') {
      await coursePlayer.value?.act({ kind: 'resume' });
      if (courseBlocked.value || courseError.value) throw new Error(courseError.value || uiText('本课尚未恢复，请重试。'));
    }
  }
  await refreshClosedBook();
  signal?.throwIfAborted();
  if (!closedReady.value) throw new Error(closedError.value || uiText('原练习暂时无法恢复。'));
  if (task.kind === 'external_question') {
    if (closedCover.value) throw new Error(uiText('请先结束闭卷练习，再回到原问题。'));
    const ref = task.source_ref;
    if (ref && (ref.book_id !== buildWorkbenchSnapshot.value?.book_id || ref.source_revision !== noteSourceFingerprint.value)) {
      throw new Error(uiText('来源材料版本不匹配。'));
    }
    const lid = ref?.lid ?? task.source_lid;
    const ranges = ref?.range ? new Map<string,[number,number]>([[ref.lid,[ref.range.start,ref.range.end]]]) : undefined;
    if (lid && !await doGoto(lid, null, undefined, ranges)) throw new Error(uiText('来源定位未完成，请重试。'));
    signal?.throwIfAborted();
    if (task.question && !agentInput.value.trim() && !askDraft.value) {
      onAgentInput(task.question);
      if (task.note_ref) personalDiscussion.value = {reference:task.note_ref};
    } else if (task.question && agentInput.value !== task.question) {
      banner.value = uiText('草稿已保留。待续问题为「{0}」', [task.question]);
    }
    workspaceRef.value?.showAssistant('agent');
    return;
  }
  if (task.kind === 'closed_book') {
    if (closedBook.value?.id !== task.closed_book_id) throw new Error(uiText('原练习已变化，请返回首页更新任务。'));
    if (closedBook.value.status === 'paused' || closedBook.value.status === 'ended') {
      await mutateClosedBook('resume');
      signal?.throwIfAborted();
      await askFirstClosedBookQuestion();
    }
  }
  signal?.throwIfAborted();
  await recordLearningTask(task.course_run_id ? { course_run_id: task.course_run_id } : { continue_conversation: true });
  if (task.kind === 'course') returnToCourse();
  else workspaceRef.value?.showAssistant('agent');
}
function returnToCourse() {
  courseSurface.value = 'course'; coursePlayer.value?.showTask(); workspaceRef.value?.showReader();
}
function revisitCourseExperiment() {
  courseSurface.value = 'experiment'; coursePlayer.value?.showExperiment(); workspaceRef.value?.showReader();
}
async function discussCourse(question?:string) {
  if(question)agentInput.value=agentInput.value.trim() ? agentInput.value+'\n\n'+question : question;
  requestWorkspaceTab('agent'); workspaceRef.value?.showAssistant('agent');
  await nextTick(); rightRailRef.value?.showCourseFeedback();
}
async function readCourseOriginal(source = courseLesson.value?.summary.source, locate = false, anchorLid?: string) {
  if (!source) return;
  courseSurface.value = 'source'; workspaceRef.value?.showReader();
  await nextTick();
  const lesson = courseLesson.value;
  if (!lesson) return;
  if (courseSourceVisited === lesson.progress.run_id && !locate || await doGoto(anchorLid ?? source.start_lid)) {
    courseSourceVisited = lesson.progress.run_id;
    if (!lesson.source_excerpt && lesson.progress.status === 'active' && !courseWorkspace.value?.archived
      && lesson.progress.goal_revision === courseWorkspace.value?.goal.revision) await coursePlayer.value?.act({ kind: 'source' });
  }
}
async function leaveCourse(signal?:AbortSignal) {
  signal?.throwIfAborted();
  if (courseBlocked.value) return false;
  if (courseLesson.value?.progress.status === 'active') {
    await coursePlayer.value?.act({ kind: 'pause' });
    signal?.throwIfAborted();
    if (courseBlocked.value || courseLesson.value?.progress.status === 'active') return false;
  }
  if (courseViewKey.value) sessionStorage.removeItem(courseViewKey.value);
  courseMode.value = false; courseSurface.value = 'source'; leftRailOpen.value = coursePreviousOutline;
  workspaceRef.value?.showReader();
  return true;
}
async function exitCourse() {
  if (await leaveCourse() && activeTrial.value) {
    const url = new URL(location.href);
    url.searchParams.delete('course_book');
    url.searchParams.delete('trial');
    location.assign(url.href);
  }
}
async function openReading(signal?:AbortSignal):Promise<void> {
  readingRequested = true; pendingCourseLaunch.value = null;
  let savedCourseView = !!courseViewKey.value && sessionStorage.getItem(courseViewKey.value) === 'course';
  if (courseViewKey.value) sessionStorage.removeItem(courseViewKey.value);
  await initialReaderLoad; signal?.throwIfAborted();
  if (disposed) throw new Error(uiText('学习现场已切换，请重新继续。'));
  if (workspaceInitializationFailed) throw new Error(workspaceInitializationError ?? uiText('学习现场尚未准备好，请重试。'));
  await initialHistoryLoad; signal?.throwIfAborted();
  if (agentHistoryError.value) throw new Error(agentHistoryError.value);
  if (courseViewKey.value) {
    savedCourseView ||= sessionStorage.getItem(courseViewKey.value) === 'course';
    sessionStorage.removeItem(courseViewKey.value);
  }
  await nextTick();
  if (savedCourseView && !courseLesson.value && coursePlayer.value) {
    const saved = await coursePlayer.value.resumeSaved(); signal?.throwIfAborted();
    if (saved) courseChanged(saved);
  }
  await refreshClosedBook(); signal?.throwIfAborted();
  if (!closedReady.value) throw new Error(closedError.value || uiText('原练习暂时无法恢复。'));
  if (closedBook.value && ['active','paused','complete'].includes(closedBook.value.status)) {
    await leaveClosedBook('exit',signal); signal?.throwIfAborted();
    if (closedError.value || ['active','paused','complete'].includes(closedBook.value?.status ?? '')) throw new Error(closedError.value || uiText('当前练习尚未保存，请重试。'));
  }
  signal?.throwIfAborted();
  if (!await leaveCourse(signal)) throw new Error(courseError.value || uiText('当前课程尚未保存，请重试。'));
  readingRequested = true;
}
const workspaceStyle = computed(() => ({
  "--left-rail-width": leftRailOpen.value ? `${leftRailWidth.value}px` : "0px",
  "--left-resizer-width": leftRailOpen.value ? "6px" : "0px",
  "--right-rail-width": courseMode.value ? `${Math.max(rightRailWidth.value, 420)}px` : `${rightRailWidth.value}px`,
}));

function clampLayoutWidth(value: number, min: number, max: number): number {
  return Math.max(min, Math.min(max, value));
}
function startResize(which: "left" | "right", event: MouseEvent) {
  if (window.innerWidth < 1024) return;
  event.preventDefault();
  const startX = event.clientX;
  const startWidth = which === "left" ? leftRailWidth.value : rightRailWidth.value;
  const onMove = (move: MouseEvent) => {
    const delta = move.clientX - startX;
    if (which === "left") leftRailWidth.value = clampLayoutWidth(startWidth + delta, 180, 420);
    else rightRailWidth.value = clampLayoutWidth(startWidth - delta, 280, 560);
  };
  const onUp = () => {
    document.body.classList.remove("is-resizing-layout");
    window.removeEventListener("mousemove", onMove);
    window.removeEventListener("mouseup", onUp);
  };
  document.body.classList.add("is-resizing-layout");
  window.addEventListener("mousemove", onMove);
  window.addEventListener("mouseup", onUp, { once: true });
}

function fail(e: unknown) {
  if (disposed || (e instanceof ApiError && e.errorCode === 'CLIENT_CONTEXT_STALE')) return;
  if (e instanceof ApiError && e.errorCode === "CHAT_BUSY") { banner.value = uiText('此对话仍有排队、执行中或未保存的回合，请先处理原运行再删除。'); return; }
  if (e instanceof ApiError && e.errorCode === "REQUEST_KEY_CLOSED") { banner.value = uiText('原对话已删除或原请求已关闭，不会转投其他对话。'); return; }
  banner.value = errorMessage(e);
}

// 读位感:anchor 在叶序中的位置 → 进度%;章节 = anchor 顶层段(LID 首段)。
const readingAnchorLid = computed(() => currentReadingLid.value ?? viewport.value?.top_lid ?? null);
const outlineAnchorLid = computed(() => outlineNavigationLid.value ?? readingAnchorLid.value);
const outlineAnchorOffset = computed(() => sourceManifestTree.value?.tree.find(node => node.lid === outlineAnchorLid.value)?.span.start);
const activeOutlineItem = computed(() => activeOutline(outlineItems.value, outlineAnchorLid.value, outlineAnchorOffset.value));
const activeChapterTitle = computed(() => activeOutlineItem.value?.title ?? chapterTitle.value);
const readerBookTitle = computed(() => {
  const title = recapProps.materialTitle?.trim() || buildWorkbenchSnapshot.value?.input?.manifest?.display_title?.trim();
  if (title) return title;
  const book = bookPickerBooks.value.find(entry => entry.book_id === buildWorkbenchSnapshot.value?.book_id);
  return book ? bookTitle(book) : '';
});
const titlePageLid = computed(() => {
  const first = outlineItems.value[0];
  return first && readerBookTitle.value && first.title.trim() === readerBookTitle.value ? first.lid : null;
});
const readerPositionTitle = computed(() => activeOutlineItem.value?.lid === titlePageLid.value
  ? uiText('书名页') : activeChapterTitle.value);
const guideChapter = computed(() => currentLearningSection(sourceManifestTree.value, outlineItems.value, outlineAnchorLid.value));
const guidingChapter = ref(false);
const progressPct = computed(() => {
  const a = readingAnchorLid.value;
  if (!a || leafOrder.value.length === 0) return 0;
  const idx = leafOrder.value.indexOf(a);
  if (idx < 0) return 0;
  return Math.round(((idx + 1) / leafOrder.value.length) * 100);
});
function pdfCapabilityUsable(status: string | undefined): boolean {
  return status === "available" || status === "degraded";
}
const pdfReaderAvailable = computed(() => {
  const manifest = sourceManifest.value;
  return manifest?.capabilities?.view_pdf?.status === "available"
    && !!manifest.original_pdf
    && !!pdfSourceMap.value
    && pdfCapabilityUsable(manifest.capabilities.project_lid_to_pdf?.status);
});
const noteSourceFingerprint = ref<string | null>(null);
const readerSurfacePreference = ref<ReaderSurface | null>(null);
const readerSurfaceIdentity = computed(() => {
  const bookId = buildWorkbenchSnapshot.value?.book_id;
  const sourceFingerprint = noteSourceFingerprint.value;
  return bookId && sourceFingerprint ? { bookId, sourceFingerprint, userId: network.value.identity?.user_id } : null;
});
const activeReaderSurface = computed<ReaderSurface>(() => resolveReaderSurface(
  readerSurfacePreference.value,
  pdfReaderAvailable.value,
));
watch(readerSurfaceIdentity, (identity) => {
  if (!identity) {
    readerSurfacePreference.value = null;
    return;
  }
  try {
    readerSurfacePreference.value = readReaderSurfacePreference(window.localStorage, identity);
  } catch {
    readerSurfacePreference.value = null;
  }
}, { immediate: true });
const NOTE_PLACEMENT_CAPABILITIES = {
  markdown: true,
  pdf: true,
} as const;
const notePlacementController = useNotePlacementController();
const notePlacementState = notePlacementController.state;
type SavingNotePlacement = Extract<NotePlacementState, { phase: "saving" }>;
const notePlacementSurface = computed<NotePlacementSurface>(() => activeReaderSurface.value);
const unquotedNotePlacementAvailable = computed(() =>
  notePlacementCapability(notePlacementSurface.value, NOTE_PLACEMENT_CAPABILITIES),
);
watch(
  notePlacementSurface,
  (surface) => {
    const current = notePlacementState.value;
    if (current.phase !== "placing") return;
    notePlacementController.synchronizeContext({
      book_id: current.book_id,
      surface_kind: surface,
      source_fingerprint: current.source_fingerprint,
    });
  },
);
watch(appSurface, (surface) => {
  if (surface !== "reader") notePlacementController.cancel();
});
const pdfEntryByLid = computed(() => new Map((pdfSourceMap.value?.entries ?? []).map((entry) => [entry.lid, entry])));
const pdfMappedLids = computed(() => new Set(
  (pdfSourceMap.value?.entries ?? []).filter(pdfEntryHasRegion).map((entry) => entry.lid),
));
const pdfActiveLid = computed(() => selectedLid.value ?? readingAnchorLid.value);
const pdfViewportAnchorLid = ref<string | null>(null);
function pdfEntryHasRegion(entry: PdfSourceMapEntry | null | undefined): boolean {
  return !!entry?.primary_region || !!entry?.regions.length;
}
const selectedSegment = computed<Segment | null>(() => {
  if (!focusPaginationActive.value) return segments.value.find(seg => seg.lid === selectedLid.value) ?? null;
  const source = focusVisible.value?.sources.find(s => s.lid === selectedLid.value);
  return source ? { ...source, kind: source.kind as NodeKind, formula: formulaDialog.value?.lid === source.lid ? formulaDialog.value.formula : null, imageAsset: imageAssetFor(source.lid) } : null;
});
const selectedFormula = computed(() => selectedSegment.value?.formula ?? null);
const segmentByLid = computed(() => new Map(segments.value.map((seg) => [seg.lid, seg])));
function lidOrderIndex(lid: string | null | undefined): number {
  if (!lid) return Number.MAX_SAFE_INTEGER;
  const idx = leafOrder.value.indexOf(lid);
  if (idx >= 0) return idx;
  const childIdx = leafOrder.value.findIndex((leaf) => leaf.startsWith(`${lid}.`));
  return childIdx >= 0 ? childIdx : Number.MAX_SAFE_INTEGER;
}
function sortMemoryByBookOrder(a: MemoryRecord, b: MemoryRecord): number {
  return lidOrderIndex(a.anchor.lid) - lidOrderIndex(b.anchor.lid)
    || (a.anchor.lid ?? "").localeCompare(b.anchor.lid ?? "")
    || (a.range?.start ?? a.selection_context?.ranges[0]?.range.start ?? 0) - (b.range?.start ?? b.selection_context?.ranges[0]?.range.start ?? 0)
    || a.mem_id.localeCompare(b.mem_id);
}
const allNotes = computed(() => annotations.value.filter((r) => r.type === "note").sort(sortMemoryByBookOrder));
const allHighlights = computed(() =>
  annotations.value
    .filter((r) => r.type === "highlight" && (highlightGroupMembers(r)[0]?.mem_id ?? r.mem_id) === r.mem_id)
    .map(record => groupHighlightForDisplay(record, highlightGroupMembers(record)))
    .sort(sortMemoryByBookOrder),
);
const noteBookOrder = computed(() => [...allNotes.value, ...allHighlights.value].sort(sortMemoryByBookOrder).map(r => r.mem_id));
const noteSourceLabels = computed(() => Object.fromEntries(annotations.value.map(record => {
  let lid = record.anchor.lid ?? '';
  while (lid) {
    const title = titleByLid.value.get(lid);
    if (title) return [record.mem_id, stripMarkdownHeadingLine(title)];
    lid = lid.includes('.') ? lid.slice(0, lid.lastIndexOf('.')) : '';
  }
  return [record.mem_id, uiText('来源未知')];
})));
const visibleNotes = computed(() => {
  const visible = segments.value.map((seg) => seg.lid);
  const order = new Map(visible.map((lid, idx) => [lid, idx]));
  return annotations.value
    .filter((r) =>
      !!r.anchor.lid
      && (focusPaginationActive.value || order.has(r.anchor.lid))
      && isMarkdownInlineNote(r, noteSourceFingerprint.value)
    )
    .sort((a, b) => (order.get(a.anchor.lid ?? "") ?? 0) - (order.get(b.anchor.lid ?? "") ?? 0));
});
function layoutRevNumber(value: number | bigint): number {
  return Number(value);
}
const isPaperProfile = computed(() => profileSummary.value?.profile_id === "paper");
function resetPaperProjectionData() {
  paperProjectionSeq += 1;
  paperLocalizationRequest = null;
  if (paperPositionSyncTimer !== null) window.clearTimeout(paperPositionSyncTimer);
  paperPositionSyncTimer = null;
  pendingPaperViewport = null;
  pendingPaperSelection = undefined;
  paperMinimapSnapshot.value = null;
  paperMinimapLocalization.value = null;
  lastPaperMinimapEffect.value = null;
  paperMinimapActionBusy.value = false;
  paperProjectionLoading.value = false;
  paperProjectionError.value = null;
  paperProjectionKey.value = "";
}

function minimapRegionForPosition(position: PaperViewportPosition): string | null {
  const regions = paperMinimapSnapshot.value?.base.regions ?? [];
  const centerPage = Math.floor(position.center_page);
  const pageMatches = regions.filter((region) =>
    centerPage >= region.page_span.start_page && centerPage <= region.page_span.end_page);
  if (pageMatches.length <= 1 || !position.anchor_lid) return pageMatches[0]?.region_id ?? null;
  const anchorOrder = lidOrderIndex(position.anchor_lid);
  return pageMatches.find((region) => {
    const start = lidOrderIndex(region.lid_span.start_lid);
    const end = lidOrderIndex(region.lid_span.end_lid);
    return anchorOrder >= start && anchorOrder <= end;
  })?.region_id ?? pageMatches[0]?.region_id ?? null;
}

function paperMinimapStateFromOutcome(outcome: PaperMinimapApplyOutcome) {
  if (outcome.kind === "effect") return outcome.effect.after;
  if (outcome.kind === "noop") return outcome.state;
  return null;
}

function schedulePaperPositionSync() {
  if (paperPositionSyncTimer !== null) window.clearTimeout(paperPositionSyncTimer);
  paperPositionSyncTimer = window.setTimeout(() => {
    paperPositionSyncTimer = null;
    void flushPaperPositionSync();
  }, 180);
}

async function flushPaperPositionSync() {
  if (paperPositionSyncRunning) {
    await paperPositionSyncCompletion;
    return;
  }
  const snapshot = paperMinimapSnapshot.value;
  const viewportPosition = pendingPaperViewport;
  const selected = pendingPaperSelection;
  if (!snapshot || (!viewportPosition && selected === undefined)) return;
  pendingPaperViewport = null;
  pendingPaperSelection = undefined;
  const commands: PaperMinimapCommand[] = [];
  if (viewportPosition) {
    commands.push({ scope: "session", action: { kind: "update_viewport", position: viewportPosition } });
  }
  if (selected !== undefined) {
    commands.push({ scope: "session", action: { kind: "set_selected_lid", selected_lid: selected } });
  }
  paperPositionSyncRunning = true;
  paperPositionSyncCompletion = new Promise((resolve) => {
    completePaperPositionSync = resolve;
  });
  try {
    const outcome = await api.paperMinimapApply({
      base_state_rev: Number(snapshot.state.rev),
      actor: "user",
      reason: "sync deterministic PDF viewport and selection",
      commands,
    });
    const state = paperMinimapStateFromOutcome(outcome);
    if (state && paperMinimapSnapshot.value?.base.fingerprint === state.base_map_rev) {
      paperMinimapSnapshot.value = { ...paperMinimapSnapshot.value, state };
      if (lastPaperMinimapEffect.value
        && Number(lastPaperMinimapEffect.value.after_state_rev) !== Number(state.rev)) {
        lastPaperMinimapEffect.value = null;
      }
      if (state.viewport_position.anchor_lid && !(focusReading.value && activeReaderSurface.value === 'pdf')) currentReadingLid.value = state.viewport_position.anchor_lid;
    }
  } catch (error) {
    if (error instanceof ApiError && error.status === 409) {
      paperMinimapSnapshot.value = await api.paperMinimapState();
      pendingPaperViewport ??= viewportPosition;
      if (pendingPaperSelection === undefined) pendingPaperSelection = selected;
      schedulePaperPositionSync();
    } else {
      fail(error);
    }
  } finally {
    paperPositionSyncRunning = false;
    completePaperPositionSync?.();
    completePaperPositionSync = null;
    paperPositionSyncCompletion = null;
    if (pendingPaperViewport || pendingPaperSelection !== undefined) schedulePaperPositionSync();
  }
}

function onPdfViewportChange(position: PaperViewportPosition) {
  if (focusReading.value && activeReaderSurface.value === 'pdf') {
    pdfViewportAnchorLid.value = position.anchor_lid;
    currentReadingLid.value = position.anchor_lid;
    selectedLid.value = null;
  }
  if (!paperMinimapSnapshot.value) return;
  const bounded = normalizePaperViewportForMinimap(
    position,
    paperMinimapSnapshot.value.base.regions.map((region) => region.page_span),
  );
  if (!bounded) return;
  const normalized = { ...bounded, region_id: minimapRegionForPosition(bounded) };
  pendingPaperViewport = normalized;
  paperMinimapSnapshot.value = {
    ...paperMinimapSnapshot.value,
    state: { ...paperMinimapSnapshot.value.state, viewport_position: normalized },
  };
  schedulePaperPositionSync();
}

function queuePaperSelection(selected: string | null) {
  if (!isPaperProfile.value || !pdfReaderAvailable.value || !paperMinimapSnapshot.value) return;
  pendingPaperSelection = selected;
  paperMinimapSnapshot.value = {
    ...paperMinimapSnapshot.value,
    state: { ...paperMinimapSnapshot.value.state, selected_lid: selected },
  };
  schedulePaperPositionSync();
}

async function applyPaperMinimapCommands(commands: PaperMinimapCommand[], reason: string) {
  if (paperMinimapActionBusy.value) return;
  if (paperPositionSyncTimer !== null) {
    window.clearTimeout(paperPositionSyncTimer);
    paperPositionSyncTimer = null;
    await flushPaperPositionSync();
  } else if (paperPositionSyncRunning) {
    await flushPaperPositionSync();
  }
  const snapshot = paperMinimapSnapshot.value;
  if (!snapshot) return;
  paperMinimapActionBusy.value = true;
  try {
    const outcome = await api.paperMinimapApply({
      base_state_rev: Number(snapshot.state.rev),
      actor: "user",
      reason,
      commands,
    });
    const state = paperMinimapStateFromOutcome(outcome);
    if (outcome.kind === "effect") lastPaperMinimapEffect.value = outcome.effect;
    if (state) paperMinimapSnapshot.value = { ...snapshot, state };
    paperMinimapSnapshot.value = await api.paperMinimapState();
  } catch (error) {
    if (error instanceof ApiError && error.status === 409) {
      paperMinimapSnapshot.value = await api.paperMinimapState();
      lastPaperMinimapEffect.value = null;
    } else {
      fail(error);
    }
  } finally {
    paperMinimapActionBusy.value = false;
  }
}

async function setPaperMinimapMode(mode: import("./api").ReaderPaperMinimapState["mode"]) {
  if (mode === paperMinimapSnapshot.value?.state.mode) return;
  const modeLabel = { skim: uiText('速览'), abstract: uiText('摘要'), deep: uiText('深读') }[mode];
  await applyPaperMinimapCommands(
    [{ scope: "session", action: { kind: "set_mode_lens", mode } }],
    uiText('切换为{0}模式', [modeLabel]),
  );
}

async function setPaperMinimapLayer(layer: string, visible: boolean) {
  const layerLabel = {
    regions: uiText('章节区域'),
    landmarks: uiText('重点位置'),
    arguments: uiText('论证关系'),
    user: uiText('我的标记'),
  }[layer] ?? layer;
  await applyPaperMinimapCommands(
    [{ scope: "session", action: { kind: "set_layer_visibility", layer, visible } }],
    uiText('{0}{1}图层', [visible ? uiText('显示') : uiText('隐藏'), layerLabel]),
  );
}

async function togglePaperMinimapPin(landmarkId: string, pinned: boolean) {
  await applyPaperMinimapCommands(
    [{
      scope: "session",
      action: pinned
        ? { kind: "unpin_landmark", landmark_id: landmarkId }
        : { kind: "pin_landmark", landmark_id: landmarkId },
    }],
    pinned ? uiText('取消固定地标') : uiText('固定地标'),
  );
}

async function undoLastPaperMinimapEffect() {
  const effect = lastPaperMinimapEffect.value;
  const snapshot = paperMinimapSnapshot.value;
  if (!effect || !snapshot || paperMinimapActionBusy.value) return;
  paperMinimapActionBusy.value = true;
  try {
    const outcome = await api.paperMinimapApply({
      base_state_rev: Number(snapshot.state.rev),
      undo_effect_id: effect.effect_id,
    });
    const state = paperMinimapStateFromOutcome(outcome);
    if (state) paperMinimapSnapshot.value = { ...snapshot, state };
    lastPaperMinimapEffect.value = null;
    paperMinimapSnapshot.value = await api.paperMinimapState();
  } catch (error) {
    fail(error);
  } finally {
    paperMinimapActionBusy.value = false;
  }
}
async function loadPaperProjectionData(force = false) {
  if (!isPaperProfile.value) {
    resetPaperProjectionData();
    return;
  }
  const key = `${profileSummary.value?.profile_id}:${profileSummary.value?.profile_version}:${readingAnchorLid.value ?? ""}`;
  if (!force && paperProjectionKey.value === key) return;
  const seq = ++paperProjectionSeq;
  paperProjectionLoading.value = true;
  paperProjectionError.value = null;
  try {
    const minimap = await api.paperMinimapState();
    if (seq !== paperProjectionSeq) return;
    paperMinimapSnapshot.value = minimap;
    // English mode keeps the original paper labels instead of requesting Chinese display labels.
    if (getLocale() !== 'en' && paperMinimapLocalization.value?.base_map_rev !== minimap.base.fingerprint) {
      const localizationKey = `${minimap.base.book_id}:${minimap.base.fingerprint}`;
      if (paperLocalizationRequest?.key !== localizationKey) {
        const request = { key: localizationKey, promise: api.paperMinimapLocalize() };
        paperLocalizationRequest = request;
        void request.promise.then((localization) => {
          if (paperLocalizationRequest !== request) return;
          if (localization.base_map_rev === minimap.base.fingerprint) {
            paperMinimapLocalization.value = localization;
          }
        }).catch((error) => {
          if (paperLocalizationRequest === request) paperProjectionError.value = errorMessage(error);
        });
      }
    }
    paperProjectionKey.value = key;
  } catch (e) {
    if (seq !== paperProjectionSeq) return;
    paperProjectionError.value = errorMessage(e);
  } finally {
    if (seq === paperProjectionSeq) paperProjectionLoading.value = false;
  }
}

async function togglePaperMinimap() {
  const snapshot = paperMinimapSnapshot.value;
  if (!snapshot) return;
  const presentation = snapshot.state.presentation === "expanded" ? "collapsed" : "expanded";
  try {
    await api.paperMinimapApply({
      base_state_rev: Number(snapshot.state.rev),
      actor: "user",
      reason: presentation === "expanded" ? "user expanded paper minimap" : "user collapsed paper minimap",
      commands: [{
        scope: "session",
        action: { kind: "set_presentation", presentation },
      }],
    });
    paperMinimapSnapshot.value = await api.paperMinimapState();
  } catch (error) {
    fail(error);
  }
}
watch(
  () => [
    profileSummary.value?.profile_id,
    profileSummary.value?.profile_version,
    readingAnchorLid.value,
  ] as const,
  () => { void loadPaperProjectionData(); },
);

// ── 标注:高亮(整段 / 段内 range)+ 笔记 ──
// 整段高亮(range 缺省)→ <p> 背景;段内 range 高亮 → <mark>(见 renderSeg)`[ADR-0031]`。
function isHighlighted(lid: string): boolean {
  return annotations.value.some((r) => r.anchor.lid === lid && r.type === "highlight" && !r.range);
}
function highlightsOf(lid: string): MemoryRecord[] {
  return annotations.value.filter((r) => r.anchor.lid === lid && r.type === "highlight");
}

function newHighlightGroupId(): string {
  const id = globalThis.crypto?.randomUUID?.() ?? `${Date.now()}-${Math.random().toString(36).slice(2)}`;
  return `${HIGHLIGHT_GROUP_PREFIX}${id}`;
}

function highlightGroupId(rec: MemoryRecord): string | null {
  const id = rec.source_session_id?.trim();
  return id?.startsWith(HIGHLIGHT_GROUP_PREFIX) ? id : null;
}

function highlightGroupMembers(rec: MemoryRecord): MemoryRecord[] {
  const groupId = highlightGroupId(rec);
  if (!groupId) return [rec];
  return annotations.value
    .filter((r) => r.type === "highlight" && highlightGroupId(r) === groupId)
    .sort(sortMemoryByBookOrder);
}

function highlightGroupRepresentative(rec: MemoryRecord): MemoryRecord {
  const members = highlightGroupMembers(rec);
  const visibleOrder = new Map(segments.value.map((seg, idx) => [seg.lid, idx]));
  const visibleMembers = members
    .filter((m) => visibleOrder.has(m.anchor.lid ?? ""))
    .sort((a, b) => (visibleOrder.get(a.anchor.lid ?? "") ?? Number.MAX_SAFE_INTEGER) - (visibleOrder.get(b.anchor.lid ?? "") ?? Number.MAX_SAFE_INTEGER));
  return visibleMembers[0] ?? members[0] ?? rec;
}

function isHighlightCardRepresentative(rec: MemoryRecord): boolean {
  return highlightGroupRepresentative(rec).mem_id === rec.mem_id;
}

function highlightCardsOf(lid: string): MemoryRecord[] {
  return highlightsOf(lid).filter(isHighlightCardRepresentative)
    .map(record => groupHighlightForDisplay(record, highlightGroupMembers(record)));
}

function renderInlineText(s: string): string {
  return renderInlineMarkdown(s);
}
function inlineMathSource(symbol: string): string {
  const trimmed = symbol.trim();
  if (!trimmed) return "";
  const dollar = /^\$(.*)\$$/.exec(trimmed);
  if (dollar) return `$${dollar[1]}$`;
  const paren = /^\\\((.*)\\\)$/.exec(trimmed);
  if (paren) return `$${paren[1]}$`;
  return `$${trimmed.replace(/\$/g, "\\$")}$`;
}
function renderFormulaSymbol(symbol: string): string {
  return renderInlineMarkdown(inlineMathSource(symbol));
}

function escapeHtml(s: string): string {
  return s
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;");
}

function isRawAssetSegment(seg: { kind?: NodeKind }): boolean {
  return seg.kind === "code" || seg.kind === "table" || seg.kind === "image";
}

function renderSegmentText(seg: { kind?: NodeKind }, text: string): string {
  if (seg.kind === "table") return renderTableSource(text);
  if (isRawAssetSegment(seg)) return escapeHtml(text);
  return seg.kind === "formula" ? renderFormulaSource(text) : renderInlineText(text);
}

interface MarkdownHeadingDisplay {
  level: number;
  text: string;
  offset: number;
}

function markdownHeadingDisplay(text: string): MarkdownHeadingDisplay | null {
  const match = /^(\s{0,3})(#{1,6})([ \t]+)([^\r\n]*?)([ \t]+#+)?[ \t]*(?:\r?\n)?$/.exec(text);
  if (!match || !match[4].trim()) return null;
  return {
    level: match[2].length,
    text: match[4].trimEnd(),
    offset: match[1].length + match[2].length + match[3].length,
  };
}

function markdownHeadingLevel(seg: { text: string; kind?: NodeKind }): number | null {
  if (seg.kind !== "chapter" && seg.kind !== "section" && seg.kind !== "paragraph") return null;
  return markdownHeadingDisplay(seg.text)?.level ?? null;
}

function stripMarkdownHeadingLine(line: string): string {
  return markdownHeadingDisplay(line)?.text ?? line;
}

function displayText(seg: { text: string; kind?: NodeKind }): { text: string; offset: number } {
  const heading = markdownHeadingLevel(seg) ? markdownHeadingDisplay(seg.text) : null;
  if (heading) return { text: heading.text, offset: heading.offset };
  return { text: seg.text, offset: 0 };
}

function clampRange(n: number, max: number): number {
  return Math.max(0, Math.min(max, n));
}

function leadingQuote(content: string): string | null {
  const quoteLines: string[] = [];
  for (const line of content.split("\\n")) {
    if (line.startsWith(">")) quoteLines.push(line.replace(/^>\\s?/, ""));
    else if (quoteLines.length > 0 && line.trim() === "") break;
    else if (quoteLines.length > 0) break;
  }
  const quote = quoteLines.join(" ").replace(/\\s+/g, " ").trim();
  return quote || null;
}

interface FocusCharRef {
  lid: string;
  offset: number;
}
interface FocusStream {
  text: string;
  refs: Array<FocusCharRef | null>;
}

const markdownEscapedPunctuation = /[\\`*_[\]{}()#+\-.!|>%]/;

function escapedMarkdownLiteral(text: string, index: number): { ch: string; offset: number } | null {
  if (text[index] !== "\\" || index + 1 >= text.length) return null;
  const next = text[index + 1];
  return markdownEscapedPunctuation.test(next) ? { ch: next, offset: index + 1 } : null;
}

function normalizeSourceMatchText(text: string): string {
  let out = "";
  let inSpace = false;
  for (let i = 0; i < text.length; i++) {
    const escaped = escapedMarkdownLiteral(text, i);
    const ch = escaped?.ch ?? text[i];
    if (escaped) i = escaped.offset;
    if (/\s/.test(ch)) {
      if (!inSpace) {
        out += " ";
        inSpace = true;
      }
      continue;
    }
    out += ch;
    inSpace = false;
  }
  return out.trim();
}

function appendNormalizedText(stream: FocusStream, lid: string, text: string) {
  let inSpace = stream.text.endsWith(" ");
  for (let i = 0; i < text.length; i++) {
    const escaped = escapedMarkdownLiteral(text, i);
    const ch = escaped?.ch ?? text[i];
    const offset = escaped?.offset ?? i;
    if (escaped) i = escaped.offset;
    if (/\s/.test(ch)) {
      if (!inSpace) {
        stream.text += " ";
        stream.refs.push({ lid, offset });
        inSpace = true;
      }
      continue;
    }
    stream.text += ch;
    stream.refs.push({ lid, offset });
    inSpace = false;
  }
}

function joinsInlineFlow(prev: Segment, next: Segment): boolean {
  return prev.kind === "formula" || next.kind === "formula";
}

function sourceFocusStreamIn(sourceSegments: Segment[], focus: SourceFocus): FocusStream | null {
  const start = sourceSegments.findIndex((seg) => seg.lid === focus.lid);
  if (start < 0) return null;
  const stream: FocusStream = { text: "", refs: [] };
  let prev: Segment | null = null;
  for (const seg of sourceSegments.slice(start)) {
    if (prev && !joinsInlineFlow(prev, seg) && stream.text && !stream.text.endsWith(" ")) {
      stream.text += " ";
      stream.refs.push(null);
    }
    appendNormalizedText(stream, seg.lid, displayText(seg).text);
    prev = seg;
  }
  return stream;
}

function commonPrefixLen(a: string, b: string): number {
  const n = Math.min(a.length, b.length);
  let i = 0;
  while (i < n && a[i] === b[i]) i++;
  return i;
}

function sourceFocusRangesIn(sourceSegments: Segment[], focus: SourceFocus | null): Map<string, [number, number]> {
  const out = new Map<string, [number, number]>();
  if (!focus) return out;
  if (!focus.quote) {
    const seg = sourceSegments.find((s) => s.lid === focus.lid);
    if (seg) out.set(focus.lid, [0, displayText(seg).text.length]);
    return out;
  }
  const quote = normalizeSourceMatchText(focus.quote);
  if (!quote) return out;
  const stream = sourceFocusStreamIn(sourceSegments, focus);
  if (!stream) return out;

  let start = stream.text.indexOf(quote);
  let len = quote.length;
  if (start < 0) {
    let bestStart = -1;
    let bestLen = 0;
    for (let i = 0; i < stream.text.length; i++) {
      const n = commonPrefixLen(stream.text.slice(i), quote);
      if (n > bestLen) {
        bestLen = n;
        bestStart = i;
      }
    }
    const minUseful = Math.min(8, quote.length, stream.text.length);
    if (bestLen < minUseful) {
      const seg = sourceSegments.find((s) => s.lid === focus.lid);
      if (seg) out.set(focus.lid, [0, displayText(seg).text.length]);
      return out;
    }
    start = bestStart;
    len = bestLen;
  }

  for (let i = start; i < start + len && i < stream.refs.length; i++) {
    const ref = stream.refs[i];
    if (!ref) continue;
    const current = out.get(ref.lid);
    if (current) {
      current[0] = Math.min(current[0], ref.offset);
      current[1] = Math.max(current[1], ref.offset + 1);
    } else {
      out.set(ref.lid, [ref.offset, ref.offset + 1]);
    }
  }
  return out;
}

function sourceFocusRange(text: string, focus: SourceFocus | null, lid: string): [number, number] | null {
  if (!focus || focus.lid !== lid) return null;
  if (!focus.quote) return [0, text.length];
  const exact = text.indexOf(focus.quote);
  if (exact >= 0) return [exact, exact + focus.quote.length];
  const normalizedQuote = focus.quote.replace(/\s+/g, " ").trim();
  const compact = text.replace(/\s+/g, " ");
  const approx = compact.indexOf(normalizedQuote);
  if (approx < 0) return [0, text.length];
  return [0, text.length];
}

// 段正文渲染:先完整渲染 Markdown,再把规范 source range 投影到 DOM 包 <mark>,避免切断行内语法 `[ADR-0031]`。
// chapter/section 的 Markdown 标题符号只在显示层剥掉,不改 book.text 原文与 LID 锚点。
function renderSeg(seg: Segment): string {
  const focus = sourceFocus.value;
  const display = isRawAssetSegment(seg) ? { text: seg.text, offset: 0 } : displayText(seg);
  const hls = highlightsOf(seg.lid).filter((h) => h.range);
  const focusRange = focus?.ranges
    ? displayedSourceRange(focus.ranges.get(seg.lid), display.offset, display.text.length)
    : sourceFocusRangesIn(segments.value, focus).get(seg.lid) ?? sourceFocusRange(display.text, focus, seg.lid);
  if (hls.length === 0 && !focusRange) return renderSegmentText(seg, display.text);
  const ranges: MarkedSourceRange[] = hls
    .map((h) => {
      const start = clampRange(h.range!.start - display.offset, display.text.length);
      const end = clampRange(h.range!.end - display.offset, display.text.length);
      return { start, end, className: "hl-mark" };
    })
    .filter(({ start, end }) => end > start);
  if (focusRange) ranges.push({ start: focusRange[0], end: focusRange[1], className: "source-focus-mark" });
  const root = document.createElement("span");
  root.innerHTML = renderSegmentText(seg, display.text);
  markMarkdownDomSourceRanges(display.text, root, ranges);
  return root.innerHTML;
}
function hlExcerpt(rec: MemoryRecord): string {
  const members = highlightGroupMembers(rec);
  const c = members.map((m) => m.content).join(" ").replace(/\s+/g, " ").trim();
  if (!rec.range && members.length === 1) return uiText('(整段)');
  return c.length > 40 ? c.slice(0, 40) + "…" : c;
}

// 高亮删除 / 修改(=移除后重新框选;高亮无可编辑正文,改 = 改范围 `[ADR-0031]`)。
async function deleteHighlight(rec: MemoryRecord) {
  try {
    banner.value = "";
    await Promise.all(highlightGroupMembers(rec).map((h) => api.delete(h.mem_id)));
    await refreshAnnotations();
  } catch (e) {
    fail(e);
  }
}
async function modifyHighlight(rec: MemoryRecord) {
  await deleteHighlight(rec);
  if (!banner.value) banner.value = uiText('已移除该高亮——重新框选文字再点「🖍 高亮选区」即可改范围。');
}

// One page-local draft, fixed to its material and selected association.
const noteCollapsed = ref(false);
const noteError = ref('');
type NoteDraft = {
  materialKey: string;
  initialContent: string;
  legacy?: boolean;
  excerpt?: import('./generated/NoteExcerpt').NoteExcerpt | null;
  request?: import('./api').NoteCreateRequest;
  receipt?: import('./generated/PresentationFollowUp').PresentationFollowUp;
  title?: string;
  summary?: string;
  lid: string;
  memId: string | null;
  layer: string;
  content: string;
  selectionContext: SelectionContext | null;
};
const noteEditor = ref<NoteDraft | null>(null);
const noteSaving = ref(false);
const noteDirty = computed(() => !!noteEditor.value && (noteEditor.value.memId
  ? noteEditor.value.content !== noteEditor.value.initialContent || !!noteEditor.value.selectionContext
  : !!noteEditor.value.content.trim() || !!noteEditor.value.excerpt));
const noteTransition = ref<{ label: string; resolve: (proceed: boolean) => void } | null>(null);
async function beforeNoteLeave(label = uiText('离开当前材料')): Promise<boolean> {
  if (notesWorkspaceRef.value && !await notesWorkspaceRef.value.beforeLeave()) return false;
  if (boardWorkspaceRef.value && !await boardWorkspaceRef.value.flush()) return false;
  if (noteTransition.value || noteSaving.value) return false;
  if (!noteDirty.value) { cancelNote(); return true; }
  if (document.activeElement instanceof HTMLTextAreaElement) document.activeElement.blur();
  noteCollapsed.value = false;
  return new Promise(resolve => { noteTransition.value = { label, resolve }; });
}
async function resolveNoteTransition(action: 'save' | 'discard' | 'continue') {
  const pending = noteTransition.value;
  if (!pending || noteSaving.value) return;
  if (action === 'save' && !await saveNote()) return;
  if (noteTransition.value !== pending) return;
  if (action === 'discard') cancelNote();
  noteTransition.value = null;
  pending.resolve(action !== 'continue');
}
defineExpose({ openUnderstanding, closeUnderstanding, openCourse, openReading, resumeLearningTask, openTutorPanel, beforeNoteLeave, prepareExamQuestion, openWorkspaceNotes, openBoard, openBoardLocation, closeBoard, prepareWorkspaceNoteQuestion, openWorkspaceNoteSource, navigateWorkspaceNoteSource, workspaceNotesChanged, prepareProductTour,
  productTourReady: computed(() => appSurface.value === 'reader' && readerVisible.value && !notesVisible.value && !sending.value && !agentHistoryLoading.value && !agentHistoryError.value && (!boardVisible.value || !!boardWorkspaceRef.value?.productTourReady)),
  startExamPractice: (days: number) => startExamPractice(days) });
function prepareExamQuestion(message: string) {
  askDraft.value = null;
  targetGoalId.value = null;
  agentInput.value = message;
  workspaceRef.value?.showAssistant('agent');
}
async function beginNote(draft: Omit<NoteDraft, 'materialKey' | 'initialContent'>) {
  try {
    if (draft.memId) {
      if (draft.selectionContext) {
        const note=await personalNotes.read({note_id:draft.memId});
        const source=await api.prepareNote({content:draft.content,note:{association:{kind:'selection'},retained_excerpt:draft.selectionContext.raw_quote},selection_context:draft.selectionContext});
        const editing=editThought(note); editing.value.source=source.source;editing.value.capture=source.capture;editing.value.context=source.context;keepNoteDraft(editing);
        openPersonalNotes({note_id:draft.memId});notesPanel.draftId=editing.draft_id;
      } else openPersonalNotes({note_id:draft.memId});
      return;
    }
    const value=await api.prepareNote({content:draft.content,note:draft.request,selection_context:draft.selectionContext,lid:draft.lid || undefined,label:draft.title});
    startThought(value.context,value.content,value);
  } catch (error) { fail(error); }
}
function refreshPersonalNotes() { void refreshAnnotations().catch(fail); }
onMounted(() => window.addEventListener('personal-notes-changed',refreshPersonalNotes));
onBeforeUnmount(() => window.removeEventListener('personal-notes-changed',refreshPersonalNotes));
function openNewNote(lid = selectedLid.value, content = "", selectionContext: SelectionContext | null = null) {
  void beginNote({ lid:lid ?? '', memId: null, layer: 'long_term', content, selectionContext,
    title: uiText('原文'), excerpt: selectionContext ? { kind: 'original', text: selectionContext.raw_quote } : null,
    request: selectionContext ? { association: { kind: 'selection' }, retained_excerpt: selectionContext.raw_quote } : undefined });
}
function recordPresentationNote(receipt: import('./generated/PresentationFollowUp').PresentationFollowUp, title: string, summary = '') {
  const fixed = { ...receipt, reference: { ...receipt.reference } };
  void beginNote({ lid: '', memId: null, layer: 'long_term', content: '', selectionContext: null,
    receipt: fixed, request: { association: { kind: 'presentation', receipt: fixed } }, title, summary });
}
function openEditNote(rec: MemoryRecord) {
  const association = rec.note?.association;
  void beginNote({
    lid: rec.anchor.lid ?? "",
    memId: rec.mem_id,
    layer: rec.layer,
    content: rec.content,
    selectionContext: null,
    legacy: !rec.note, excerpt: rec.note?.retained_excerpt,
    title: association?.kind === 'presentation' ? association.title : association?.kind === 'answer' ? uiText('助手回答') : uiText('阅读笔记'),
    receipt: association?.kind === 'presentation' ? association.receipt : undefined,
  });
}
async function showAnnotationInNotes(record: MemoryRecord) {
  workspaceRef.value?.showAssistant('notes');
  requestWorkspaceTab('notes');
  await nextTick();
  await rightRailRef.value?.showMemory(record.mem_id);
}
function cancelNote() {
  if (noteSaving.value) return;
  noteEditor.value = null;
}
// 保存:新建走 save;编辑走原子 replace,默认继承旧锚与 selection context。
async function saveNote() {
  const ed = noteEditor.value;
  if (!ed || noteSaving.value || ed.materialKey !== noteMaterialKey.value) return false;
  const current = () => !disposed && ed.materialKey === noteMaterialKey.value && noteEditor.value === ed;
  const content = ed.content;
  if (!content.trim() && !ed.excerpt) return false;
  noteSaving.value = true;
  noteError.value = '';
  try {
    banner.value = "";
    if (ed.memId) {
      const saved = await api.replace({
        mem_id: ed.memId,
        content,
        selection_context: ed.selectionContext ?? undefined,
      });
      if (!current()) return false;
      if (saved) {
        rightRailRef.value?.replaceMemory?.(ed.memId, saved);
        annotations.value = annotations.value.map(record => record.mem_id === ed.memId ? saved : record);
      }
    } else if (ed.request) {
      await api.save({
        type: "note",
        content,
        layer: ed.layer,
        selection_context: ed.selectionContext ?? undefined,
        note: ed.request,
      });
    } else {
      await api.note(ed.lid, content);
    }
    if (!current()) return false;
    noteEditor.value = null;
    banner.value = uiText('笔记已保存');
    try { await refreshAnnotations(); }
    catch (e) { if (!disposed && ed.materialKey === noteMaterialKey.value) fail(e); }
    return true;
  } catch (e) {
    if (current()) { noteError.value = errorMessage(e); fail(e); }
    return false;
  } finally { noteSaving.value = false; }
}
async function deleteNote(rec: MemoryRecord) {
  if (!window.confirm(uiText('删除这条笔记?'))) return;
  const scope = noteMaterialKey.value;
  try {
    banner.value = '';
    await api.delete(rec.mem_id);
    if (disposed || scope !== noteMaterialKey.value) return;
    if (noteEditor.value?.memId === rec.mem_id) noteEditor.value = null;
    annotations.value = annotations.value.filter(record => record.mem_id !== rec.mem_id);
    banner.value = uiText('笔记已删除');
    await refreshAnnotations();
  } catch (e) { if (!disposed && scope === noteMaterialKey.value) fail(e); }
}
async function openNoteAnswer(record: MemoryRecord) {
  const association = record.note?.association;
  if (association?.kind !== 'answer' || agentHistoryLoading.value) return;
  const scope = noteMaterialKey.value;
  if (association.session_id !== activeChatSessionId.value) await selectChat(association.session_id);
  if (disposed || scope !== noteMaterialKey.value) return;
  if (association.session_id !== activeChatSessionId.value || !chat.value.some(turn => turn.turnId === association.turn_id)) {
    banner.value = uiText('原回答当前不可用，已保存的笔记与摘录仍可阅读。'); return;
  }
  presentationWorkspace.value = null;
  workspaceRef.value?.showAssistant('agent'); requestWorkspaceTab('agent');
  await nextTick();
  if (disposed || scope !== noteMaterialKey.value) return;
  if (!await rightRailRef.value?.scrollToTurn(association.turn_id)) banner.value = uiText('原回答当前不可用，已保存的笔记与摘录仍可阅读。');
}
async function openNoteAssociation(record: MemoryRecord, restore = false) {
  if (record.note?.association.kind === 'answer') { await openNoteAnswer(record); return; }
  workspaceRef.value?.showAssistant('agent'); requestWorkspaceTab('agent');
  await nextTick();
  await rightRailRef.value?.openNotePresentation(record, restore);
}

function kindOf(lid: string): NodeKind {
  return kindByLid.value.get(lid) ?? "paragraph";
}
function loadOutlineTitles(manifest: import('./api').Manifest) {
  titleByLid.value = new Map(manifest.tree
    .filter(node => node.children.length > 0 || node.kind === "chapter" || node.kind === "section")
    .map(node => [node.lid, outlineTitle(node.title ?? "", uiText('正文'))]));
  outlineItems.value = readerOutline(manifest, uiText('正文'));
}
function isAsset(seg: Segment): boolean {
  return seg.kind === "code" || seg.kind === "table" || seg.kind === "image" || seg.kind === "formula";
}
function imageMeta(text: string): { alt: string; src: string } | null {
  const m = text.trim().match(/^!\[([^\]]*)\]\(([^)]+)\)$/);
  return m ? { alt: m[1], src: m[2] } : null;
}
function imageAssetFor(lid: string): ImageAssetManifestEntry | null {
  return imageAssetByLid.value.get(lid) ?? null;
}
function imageRenderSrc(asset: ImageAssetManifestEntry | null | undefined): string | null {
  if (!asset) return null;
  return asset.url_path ?? (asset.status === "external" ? asset.original_src : null);
}
async function formulaFor(lid: string, kind: NodeKind): Promise<FormulaSemantics | null> {
  if (kind !== "formula") return null;
  try {
    return await api.formulaSemantics(lid);
  } catch (e) {
    if (e instanceof ApiError && e.status === 404) return null;
    throw e;
  }
}

function resetPdfAnnotationProjection() {
  pdfAnnotationProjectionSeq += 1;
  pdfAnnotationProjection.value = EMPTY_PDF_ANNOTATION_PROJECTION;
  pdfAnnotationProjectionError.value = null;
}

async function refreshPdfAnnotationProjection(records: MemoryRecord[]) {
  const seq = ++pdfAnnotationProjectionSeq;
  pdfAnnotationProjectionError.value = null;
  if (
    !pdfReaderAvailable.value
    || !pdfCapabilityUsable(sourceManifest.value?.capabilities?.project_ranges_to_pdf?.status)
  ) {
    pdfAnnotationProjection.value = EMPTY_PDF_ANNOTATION_PROJECTION;
    return;
  }
  const batch = buildPdfProjectionBatch(records, {
    source_fingerprint: noteSourceFingerprint.value,
    source_map: pdfSourceMap.value,
  });
  if (!batch.requests.length) {
    pdfAnnotationProjection.value = projectPdfAnnotations(batch, { projections: [] });
    return;
  }
  try {
    const response = await api.pdfRangesProject(batch.requests);
    if (seq !== pdfAnnotationProjectionSeq) return;
    pdfAnnotationProjection.value = projectPdfAnnotations(batch, response);
  } catch (error) {
    if (seq !== pdfAnnotationProjectionSeq) return;
    pdfAnnotationProjection.value = projectPdfAnnotations(batch, { projections: [] });
    pdfAnnotationProjectionError.value = errorMessage(error);
  }
}

let annotationReadSequence = 0;
async function refreshAnnotations(): Promise<MemoryRecord[]> {
  const sequence = ++annotationReadSequence;
  const materialKey = noteMaterialKey.value;
  const bookId = buildWorkbenchSnapshot.value?.book_id;
  if (!bookId) {
    annotations.value = [];
    resetPdfAnnotationProjection();
    return [];
  }
  const publication = network.value.enabled ? network.value.workspace?.published_book_ref.publication_id ?? null : null;
  const records = (await recallBookAnnotations(bookId, api.recall)).filter(record => !record.note || record.note.material.publication_id === publication);
  if (disposed || materialKey !== noteMaterialKey.value || sequence !== annotationReadSequence) return [];
  annotations.value = records;
  await refreshPdfAnnotationProjection(records);
  return records;
}

type SegmentLoadMode = "replace" | "append" | "prepend";

const windowTextCache = computed(() => new ReaderWindowTextCache(sourceManifestTree.value?.tree ?? [], api.text));
async function hydrateSegments(lids: string[], supplied?: import('./api').BookText[]): Promise<Segment[]> {
  const cache = windowTextCache.value;
  if (supplied) cache.remember(supplied);
  const texts = supplied ?? await cache.read(lids);
  return texts.map((t) => {
      const kind = kindOf(t.lid);
      return {
        lid: t.lid,
        text: t.text,
        kind,
        formula: null,
        imageAsset: kind === "image" ? imageAssetFor(t.lid) : null,
      };
    });
}

function mergeSegments(current: Segment[], incoming: Segment[], mode: SegmentLoadMode): Segment[] {
  if (mode === "replace") return incoming;
  const seen = new Set(current.map((seg) => seg.lid));
  const unique = incoming.filter((seg) => !seen.has(seg.lid));
  return mode === "append" ? [...current, ...unique] : [...unique, ...current];
}

// 视口加载:连续 visible_lid 合并读取后按原文坐标拆回各段。
async function loadWindow(
  vp: Viewport,
  mode: SegmentLoadMode = "replace",
  navigationTargetLid = vp.top_lid,
  stillCurrent: () => boolean = () => true,
  withAnnotations = true,
  texts?: import('./api').BookText[],
) {
  const next = focusPaginationActive.value ? [] : await hydrateSegments(vp.visible_lids, texts);
  if (!stillCurrent()) return;
  viewport.value = vp;
  if (mode === "replace") {
    selectedLid.value = navigationTargetLid;
    currentReadingLid.value = navigationTargetLid;
  }
  segments.value = mergeSegments(segments.value, next, mode);
  if (mode === "replace") {
    await readerPaneRef.value?.scrollLidIntoView(navigationTargetLid);
  }
  if (!stillCurrent()) return;
  if (withAnnotations) await refreshAnnotations();
  await loadChapter(vp.top_lid);
}

// 阅读区与服务端 reader 同步(agent 可能改了视口 → 重新拉 state 渲染)。
async function ensureProfileManifest(summary: ProfileSummary, stillCurrent: () => boolean = () => true) {
  const current = profileManifest.value;
  if (current?.profile_id === summary.profile_id && current.profile_version === summary.profile_version) return;
  const manifest = await api.profileManifest(summary.profile_id);
  if (stillCurrent()) profileManifest.value = manifest;
}
async function applyReaderState(st: Awaited<ReturnType<typeof api.state>>) {
  profileSummary.value = st.profile;
  readerLayout.value = st.layout;
  await ensureProfileManifest(st.profile);
}

function profileOperationId(kind: string): string {
  const random = globalThis.crypto?.randomUUID?.()
    ?? `${Date.now()}-${Math.random().toString(36).slice(2)}`;
  return `profile-ui-${kind}-${random}`;
}

async function refreshProfileMemory(preserveFeedback = false) {
  const requestSeq = ++profileMemoryRequestSeq;
  profileMemoryLoading.value = true;
  if (!preserveFeedback) {
    profileMemoryError.value = null;
    profileMemoryNotice.value = null;
  }
  try {
    const state = await api.profileMemory();
    if (requestSeq !== profileMemoryRequestSeq) return;
    profileMemory.value = state;
  } catch (error) {
    if (requestSeq !== profileMemoryRequestSeq) return;
    profileMemoryError.value = errorMessage(error);
  } finally {
    if (requestSeq === profileMemoryRequestSeq) profileMemoryLoading.value = false;
  }
}

async function refreshProfileBackfill(preserveFeedback = false) {
  const requestSeq = ++profileBackfillRequestSeq;
  profileBackfillLoading.value = true;
  if (!preserveFeedback) {
    profileBackfillError.value = null;
    profileBackfillNotice.value = null;
  }
  try {
    const state = await api.profileBackfill();
    if (requestSeq !== profileBackfillRequestSeq) return;
    profileBackfill.value = state;
  } catch (error) {
    if (requestSeq !== profileBackfillRequestSeq) return;
    profileBackfillError.value = errorMessage(error);
  } finally {
    if (requestSeq === profileBackfillRequestSeq) profileBackfillLoading.value = false;
  }
}

async function refreshProfileSurface(preserveFeedback = false) {
  if (network.value.enabled) { await refreshProfileMemory(preserveFeedback); return; }
  await Promise.all([
    refreshProfileMemory(preserveFeedback),
    refreshProfileBackfill(preserveFeedback),
  ]);
}

function scheduleProfileBackfillPoll() {
  if (profileBackfillPollTimer !== null) window.clearTimeout(profileBackfillPollTimer);
  profileBackfillPollTimer = null;
  const active = profileBackfill.value?.jobs.some((job) =>
    job.status === "queued" || job.status === "running") ?? false;
  if (!active) return;
  profileBackfillPollTimer = window.setTimeout(async () => {
    profileBackfillPollTimer = null;
    await Promise.all([
      refreshProfileBackfill(true),
      refreshProfileMemory(true),
    ]);
    scheduleProfileBackfillPoll();
  }, 750);
}

watch(
  () => profileBackfill.value?.jobs
    .map((job) => `${job.job_id}:${job.status}:${job.processed_through}`)
    .join("|") ?? "",
  scheduleProfileBackfillPoll,
);

async function startProfileBackfill(request: HistoricalBackfillStartRequest) {
  if (profileBackfillBusy.value) return;
  profileBackfillBusy.value = true;
  profileBackfillError.value = null;
  profileBackfillNotice.value = null;
  try {
    profileBackfill.value = await api.profileBackfillStart(request);
    profileBackfillNotice.value = uiText('历史回填已开始');
    await refreshProfileMemory(true);
  } catch (error) {
    profileBackfillError.value = errorMessage(error);
  } finally {
    profileBackfillBusy.value = false;
  }
}

async function mutateProfileBackfill(
  action: "cancel" | "retry" | "clear",
  request: HistoricalBackfillJobRequest,
) {
  if (profileBackfillBusy.value) return;
  profileBackfillBusy.value = true;
  profileBackfillError.value = null;
  profileBackfillNotice.value = null;
  try {
    profileBackfill.value = action === "cancel"
      ? await api.profileBackfillCancel(request)
      : action === "retry"
        ? await api.profileBackfillRetry(request)
        : await api.profileBackfillClear(request);
    profileBackfillNotice.value = {
      cancel: uiText('历史回填已中止'),
      retry: uiText('历史回填已继续'),
      clear: uiText('历史回填记录已清除'),
    }[action];
    await refreshProfileMemory(true);
  } catch (error) {
    profileBackfillError.value = errorMessage(error);
  } finally {
    profileBackfillBusy.value = false;
  }
}

function profileMutationLabel(action: ProfileGovernanceActionRequest): string {
  return {
    remember: uiText('画像已记住'),
    correct: uiText('画像已纠正'),
    forget: uiText('画像已忘记'),
    confirm: uiText('画像已确认'),
    reject: uiText('候选已忽略'),
    change_scope: uiText('画像范围已更新'),
    add_collection_rule: uiText('自动收集规则已添加'),
    remove_collection_rule: uiText('自动收集规则已移除'),
  }[action.kind];
}

async function mutateProfile(action: ProfileGovernanceActionRequest) {
  if (profileMemoryBusy.value) return null;
  if (!profileMemory.value) await refreshProfileMemory();
  const state = profileMemory.value;
  if (!state) return null;
  profileMemoryBusy.value = true;
  profileMemoryError.value = null;
  profileMemoryNotice.value = null;
  try {
    const response = await api.profileMemoryApply({
      expected_document_revision: state.status.document_revision,
      action,
    });
    profileMemoryNotice.value = response.kind === "applied"
      ? profileMutationLabel(action)
      : uiText('敏感画像等待确认');
    await refreshProfileMemory(true);
    return response;
  } catch (error) {
    if (error instanceof ApiError && error.status === 409) {
      await refreshProfileMemory(true);
      profileMemoryError.value = error.errorCode === "MEMORY_DOCUMENT_REVISION_CONFLICT"
        ? uiText('画像已在后台更新，已载入最新版本，请重试刚才的操作。')
        : error.message;
    } else {
      profileMemoryError.value = errorMessage(error);
    }
    return null;
  } finally {
    profileMemoryBusy.value = false;
  }
}

async function undoProfileUpdate(
  turnIndex: number,
  updateIndex: number,
  update: ProfileMemoryUpdate,
) {
  const key = `${turnIndex}:${updateIndex}`;
  if (!profileMemory.value) await refreshProfileMemory();
  const action = buildUndoProfileAction(
    profileMemory.value,
    update,
    profileOperationId("undo"),
  );
  if (!action) {
    profileUpdateStates.value = { ...profileUpdateStates.value, [key]: uiText('无法撤销') };
    return;
  }
  const response = await mutateProfile(action);
  if (!response) return;
  const status = response.kind === "applied" ? uiText('已撤销') : uiText('等待确认');
  profileUpdateStates.value = { ...profileUpdateStates.value, [key]: status };
  profileMemoryNotice.value = response.kind === "applied" ? uiText('画像更新已撤销') : uiText('敏感画像等待确认');
}

async function syncViewport(forcePaperProjection = false, preferReaderSelection = false) {
  const st = await api.state();
  await applyReaderState(st);
  const navigationTargetLid = preferReaderSelection
    ? resolveReaderStateNavigationTarget(st.viewport.top_lid, st.selection, leafOrder.value)
    : st.viewport.top_lid;
  await loadWindow(st.viewport, "replace", navigationTargetLid);
  if (st.visible_position) await restoreVisiblePosition(st.visible_position);
  await loadPaperProjectionData(forcePaperProjection);
}

// 章节标题:取 anchor 顶层段(LID 首段)原文首行作标签(读位感「第N章…」)。
async function loadChapter(anchorLid: string) {
  const top = anchorLid.split(".")[0];
  const title = titleByLid.value.get(top);
  if (title !== undefined) {
    chapterTitle.value = title.slice(0, 40);
    return;
  }
  try {
    const t = await api.text(top);
    const fallback = kindOf(top) === 'image' && leafOrder.value[0] === top ? uiText('封面') : uiText('正文');
    chapterTitle.value = outlineTitle(t.text, fallback).slice(0, 40);
  } catch {
    chapterTitle.value = top;
  }
}

async function loadPdfRuntimeArtifacts(stillCurrent: () => boolean = () => true) {
  sourceManifest.value = null;
  pdfSourceMap.value = null;
  pdfRuntimeError.value = null;
  try {
    const manifest = await api.sourceManifest();
    if (!stillCurrent()) return;
    sourceManifest.value = manifest;
    if (!manifest) return;
    if (
      manifest.capabilities.view_pdf.status === "available"
      && pdfCapabilityUsable(manifest.capabilities.project_lid_to_pdf.status)
    ) {
      const map = await api.pdfSourceMap();
      if (!stillCurrent()) return;
      pdfSourceMap.value = map;
      await refreshPdfAnnotationProjection(annotations.value);
    }
  } catch (e) {
    if (!stillCurrent()) return;
    if (e instanceof ApiError && e.status === 404) return;
    pdfRuntimeError.value = errorMessage(e);
  }
}

async function loadBuildWorkbenchSnapshot(): Promise<BuildWorkbenchSnapshot | null> {
  buildWorkbenchLoading.value = true;
  buildWorkbenchError.value = null;
  try {
    let snapshot = await api.buildWorkbench();
    buildWorkbenchSnapshot.value = snapshot;
    snapshot = await maybeAutoRerunSourceReview(snapshot);
    buildWorkbenchSnapshot.value = snapshot;
    return snapshot;
  } catch (e) {
    buildWorkbenchError.value = errorMessage(e);
    buildWorkbenchSnapshot.value = null;
    return null;
  } finally {
    buildWorkbenchLoading.value = false;
  }
}

async function maybeAutoRerunSourceReview(
  snapshot: BuildWorkbenchSnapshot,
  actionOwner?: number,
): Promise<BuildWorkbenchSnapshot> {
  if (readerOnly.value) return snapshot;
  const request = getSourceReviewAutoRerunRequest(snapshot);
  if (!request) return snapshot;

  const ownsAction = actionOwner === undefined;
  const owner = actionOwner ?? beginWorkbenchAction();
  buildWorkbenchSnapshot.value = snapshot;
  try {
    return await api.workbenchJobStart(request);
  } catch (e) {
    buildWorkbenchError.value = uiText('复核决定已保存，但自动重新运行来源对齐失败：{0}', [errorMessage(e)]);
    return snapshot;
  } finally {
    if (ownsAction) endWorkbenchAction(owner);
  }
}

async function refreshBuildWorkbench() {
  const snapshot = await loadBuildWorkbenchSnapshot();
  if (!snapshot) return;
  await showSurfaceForWorkbenchSnapshot(snapshot);
}

async function showSurfaceForWorkbenchSnapshot(snapshot: BuildWorkbenchSnapshot) {
  const surface = chooseAppSurface(snapshot);
  if (surface === "reader" && diagnosticWorkbenchBookId.value !== snapshot.book_id) {
    await init();
    return;
  }
  appSurface.value = "workbench";
}

async function confirmSidecarPlan(fields: Record<string, unknown>) {
  buildWorkbenchConfirming.value = true;
  buildWorkbenchError.value = null;
  try {
    buildWorkbenchSnapshot.value = await api.sidecarPlanConfirm(fields);
  } catch (e) {
    buildWorkbenchError.value = errorMessage(e);
  } finally {
    buildWorkbenchConfirming.value = false;
  }
}

async function importWorkbenchInput(payload: {
  target_dir?: string;
  book_id?: string;
  display_title?: string;
  paper_md_path?: string;
  paper_pdf_path?: string;
  paper_md_text?: string;
  paper_pdf_base64?: string;
}) {
  buildWorkbenchImporting.value = true;
  buildWorkbenchError.value = null;
  try {
    buildWorkbenchSnapshot.value = await api.workbenchInputImport(payload);
    diagnosticWorkbenchBookId.value = null;
    appSurface.value = "workbench";
  } catch (e) {
    buildWorkbenchError.value = errorMessage(e);
  } finally {
    buildWorkbenchImporting.value = false;
  }
}

async function applyWorkbenchAction(action: () => Promise<BuildWorkbenchSnapshot>) {
  const actionOwner = beginWorkbenchAction();
  buildWorkbenchError.value = null;
  try {
    let snapshot = await action();
    buildWorkbenchSnapshot.value = snapshot;
    snapshot = await maybeAutoRerunSourceReview(snapshot, actionOwner);
    buildWorkbenchSnapshot.value = snapshot;
    await showSurfaceForWorkbenchSnapshot(snapshot);
  } catch (e) {
    buildWorkbenchError.value = errorMessage(e);
  } finally {
    endWorkbenchAction(actionOwner);
  }
}

async function createBuildJob() {
  await applyWorkbenchAction(() => api.workbenchJobCreate());
}

async function startBuildJob(payload: {
  job_id?: string;
  stage: BuildStageId;
  executor: ExecutorId;
  run_id?: string;
  adapter_mode?: WorkbenchAdapterMode;
}) {
  await applyWorkbenchAction(() => api.workbenchJobStart(payload));
}

async function resumeBuildJob(jobId: string) {
  await applyWorkbenchAction(() => api.workbenchJobResume(jobId));
}

async function resolveBuildDecision(payload: { job_id: string; decision_id: string; answer: string }) {
  await applyWorkbenchAction(() => api.workbenchDecisionResolve(payload));
}

async function resolveExecutorPermission(payload: { job_id: string; request_id: string; granted: boolean }) {
  await applyWorkbenchAction(() => api.workbenchPermissionResolve(payload));
}

async function resolveSourceReview(payload: {
  job_id?: string;
  block_id: string;
  decision: SourceReviewDecisionKind;
  replacement_text?: string;
  note?: string;
}) {
  await applyWorkbenchAction(() => api.workbenchSourceReviewResolve(payload));
}

async function resolveSourceReviewGroup(payload: {
  job_id?: string;
  group_id: string;
  note?: string;
}) {
  const snapshot = buildWorkbenchSnapshot.value;
  if (!snapshot || buildWorkbenchActioning.value) return;
  const actionOwner = beginWorkbenchAction();
  const bookId = snapshot.book_id;
  const stillCurrent = () => buildWorkbenchSnapshot.value?.book_id === bookId;
  buildWorkbenchError.value = null;

  try {
    let nextSnapshot = await runSourceReviewPageGroupDecision({
      snapshot,
      groupId: payload.group_id,
      jobId: payload.job_id,
      note: payload.note,
      resolve: (decision) => api.workbenchSourceReviewResolve(decision),
      onSnapshot: (persistedSnapshot) => {
        if (stillCurrent()) buildWorkbenchSnapshot.value = persistedSnapshot;
      },
    });
    if (!stillCurrent()) return;
    buildWorkbenchSnapshot.value = nextSnapshot;
    nextSnapshot = await maybeAutoRerunSourceReview(nextSnapshot, actionOwner);
    if (stillCurrent()) {
      buildWorkbenchSnapshot.value = nextSnapshot;
      await showSurfaceForWorkbenchSnapshot(nextSnapshot);
    }
  } catch (e) {
    if (stillCurrent()) buildWorkbenchError.value = errorMessage(e);
  } finally {
    endWorkbenchAction(actionOwner);
  }
}

async function analyzeSourceReview(payload: { block_id: string }) {
  if (sourceReviewLlmBatch.value?.status === "running" || buildWorkbenchActioning.value) return;
  const blockId = payload.block_id;
  const requestToken = ++sourceReviewLlmRequestToken;
  const bookId = buildWorkbenchSnapshot.value?.book_id;
  const evidenceKey = sourceReviewEvidenceKey.value;
  const stillCurrent = () => (
    requestToken === sourceReviewLlmRequestToken
    && buildWorkbenchSnapshot.value?.book_id === bookId
    && sourceReviewEvidenceKey.value === evidenceKey
  );
  sourceReviewLlmAnalyzingBlockId.value = blockId;
  const nextErrors = { ...sourceReviewLlmErrors.value };
  const nextSuggestions = { ...sourceReviewLlmSuggestions.value };
  delete nextErrors[blockId];
  delete nextSuggestions[blockId];
  sourceReviewLlmErrors.value = nextErrors;
  sourceReviewLlmSuggestions.value = nextSuggestions;
  try {
    const suggestion = await api.workbenchSourceReviewAnalyze(blockId);
    if (stillCurrent()) {
      sourceReviewLlmSuggestions.value = {
        ...sourceReviewLlmSuggestions.value,
        [blockId]: suggestion,
      };
    }
  } catch (e) {
    if (stillCurrent()) {
      sourceReviewLlmErrors.value = {
        ...sourceReviewLlmErrors.value,
        [blockId]: errorMessage(e),
      };
    }
  } finally {
    if (stillCurrent() && sourceReviewLlmAnalyzingBlockId.value === blockId) {
      sourceReviewLlmAnalyzingBlockId.value = null;
    }
  }
}

async function applyAllSourceReviewWithLlm() {
  const snapshot = buildWorkbenchSnapshot.value;
  if (
    !snapshot
    || buildWorkbenchActioning.value
    || sourceReviewLlmAnalyzingBlockId.value
    || sourceReviewLlmBatch.value?.status === "running"
  ) return;
  const targets = sourceReviewBatchTargets(snapshot);
  if (!targets.length) return;

  const runToken = ++sourceReviewLlmBatchRunToken;
  sourceReviewLlmRequestToken += 1;
  sourceReviewLlmAnalyzingBlockId.value = null;
  const actionOwner = beginWorkbenchAction();
  const bookId = snapshot.book_id;
  const targetIds = new Set(targets.map((block) => block.id));
  const nextErrors = { ...sourceReviewLlmErrors.value };
  const nextSuggestions = { ...sourceReviewLlmSuggestions.value };
  for (const blockId of targetIds) {
    delete nextErrors[blockId];
    delete nextSuggestions[blockId];
  }
  sourceReviewLlmErrors.value = nextErrors;
  sourceReviewLlmSuggestions.value = nextSuggestions;
  buildWorkbenchError.value = null;

  const latestJobId = [...snapshot.jobs]
    .sort((left, right) => left.updated_at.localeCompare(right.updated_at))
    .at(-1)?.job_id;
  const stillCurrent = () => (
    runToken === sourceReviewLlmBatchRunToken
    && buildWorkbenchSnapshot.value?.book_id === bookId
  );

  try {
    const result = await runSourceReviewLlmBatch({
      snapshot,
      jobId: latestJobId,
      analyze: (blockId) => api.workbenchSourceReviewAnalyze(blockId),
      resolve: (payload) => api.workbenchSourceReviewResolve(payload),
      formatError: errorMessage,
      isCancelled: () => !stillCurrent(),
      onState: (state) => {
        if (!stillCurrent()) return;
        sourceReviewLlmBatch.value = state;
        sourceReviewLlmAnalyzingBlockId.value = state.status === "running"
          ? state.current_block_id
          : null;
      },
      onSuggestion: (suggestion) => {
        if (!stillCurrent()) return;
        sourceReviewLlmSuggestions.value = {
          ...sourceReviewLlmSuggestions.value,
          [suggestion.block_id]: suggestion,
        };
      },
      onFailure: (failure) => {
        if (!stillCurrent()) return;
        sourceReviewLlmErrors.value = {
          ...sourceReviewLlmErrors.value,
          [failure.block_id]: failure.message,
        };
      },
      onSnapshot: (nextSnapshot) => {
        if (stillCurrent()) buildWorkbenchSnapshot.value = nextSnapshot;
      },
    });
    if (stillCurrent()) {
      let nextSnapshot = result.snapshot;
      buildWorkbenchSnapshot.value = nextSnapshot;
      sourceReviewLlmBatch.value = result.state;
      nextSnapshot = await maybeAutoRerunSourceReview(nextSnapshot, actionOwner);
      if (stillCurrent()) buildWorkbenchSnapshot.value = nextSnapshot;
    }
  } catch (e) {
    if (stillCurrent()) buildWorkbenchError.value = errorMessage(e);
  } finally {
    if (stillCurrent()) sourceReviewLlmAnalyzingBlockId.value = null;
    endWorkbenchAction(actionOwner);
  }
}

async function draftSidecarPlan(payload: { request: string }) {
  await applyWorkbenchAction(() => api.sidecarPlanDraft(payload));
}

async function enterReader() {
  const snapshot = buildWorkbenchSnapshot.value;
  if (!snapshot || snapshot.readiness.route !== "reader") return;
  diagnosticWorkbenchBookId.value = null;
  await init();
}

async function openBuildWorkbench() {
  const snapshot = await loadBuildWorkbenchSnapshot();
  if (!snapshot || !workbenchAvailable(snapshot)) return;
  diagnosticWorkbenchBookId.value = snapshot.book_id;
  appSurface.value = "workbench";
}

async function openBuildIntent() {
  buildIntentOpen.value = true;
  buildIntentError.value = null;
  const request = ++buildIntentRequestSeq;
  try {
    await api.buildIntentStatus();
  } catch (error) {
    if (request === buildIntentRequestSeq) buildIntentError.value = errorMessage(error);
  }
}

function closeBuildIntent() {
  buildIntentOpen.value = false;
}

let intentUsageEventSequence = 0;

function nextIntentUsageEventId(kind: IntentBuildReaderUsageKind): string {
  const nonce = typeof crypto !== "undefined" && typeof crypto.randomUUID === "function"
    ? crypto.randomUUID()
    : `${Date.now()}-${++intentUsageEventSequence}`;
  return `${kind}-${nonce}`;
}

function recordIntentUsage(kind: IntentBuildReaderUsageKind, artifactId?: string) {
  if (network.value.enabled) return;
  void api.intentUsageEvent({
    event_id: nextIntentUsageEventId(kind),
    occurred_at: new Date().toISOString(),
    kind,
    ...(artifactId ? { artifact_id: artifactId } : {}),
  }).catch(() => undefined);
}

async function refreshIntentArtifacts(): Promise<boolean> {
  const request = ++intentArtifactRequestSeq;
  intentArtifactsLoading.value = true;
  intentArtifactsError.value = null;
  try {
    const response = await api.intentArtifacts();
    if (request === intentArtifactRequestSeq) intentArtifacts.value = response.overlay;
    return request === intentArtifactRequestSeq;
  } catch (error) {
    if (request !== intentArtifactRequestSeq) return false;
    if (error instanceof ApiError && error.status === 404) {
      intentArtifacts.value = null;
    } else {
      intentArtifactsError.value = uiText('暂时无法读取目标成果');
    }
    return false;
  } finally {
    if (request === intentArtifactRequestSeq) intentArtifactsLoading.value = false;
  }
}

async function openIntentArtifacts() {
  if (!await refreshIntentArtifacts()) return;
  for (const artifact of intentArtifacts.value?.artifacts ?? []) {
    if (artifact.state === "accepted") recordIntentUsage("artifact_opened", artifact.artifact_id);
  }
}

async function draftBuildIntent(payload: {
  mode: BuildIntentMode;
  user_goal?: string;
  edit_plan_id?: string;
}) {
  const request = ++buildIntentRequestSeq;
  buildIntentBusy.value = true;
  buildIntentError.value = null;
  try {
    const response = payload.edit_plan_id && payload.mode === "goal_directed"
      ? await api.buildIntentEdit({
          plan_id: payload.edit_plan_id,
          user_goal: payload.user_goal,
        })
      : await api.buildIntentDraft({
          mode: payload.mode,
          user_goal: payload.user_goal,
        });
    if (request === buildIntentRequestSeq) {
      buildIntentSelection.value = response.selection;
      buildIntentPlanningSource.value = response.planning_source;
    }
  } catch (error) {
    if (request === buildIntentRequestSeq) buildIntentError.value = errorMessage(error);
  } finally {
    if (request === buildIntentRequestSeq) buildIntentBusy.value = false;
  }
}

async function confirmBuildIntent(payload: { plan_id: string; plan_digest: string }) {
  const request = ++buildIntentRequestSeq;
  buildIntentBusy.value = true;
  buildIntentError.value = null;
  try {
    const response = await api.buildIntentConfirm(payload.plan_id, payload.plan_digest);
    if (request === buildIntentRequestSeq) {
      buildIntentSelection.value = response.selection;
      buildIntentPlanningSource.value = response.planning_source;
      if (!network.value.enabled) void refreshIntentArtifacts();
    }
  } catch (error) {
    if (request === buildIntentRequestSeq) buildIntentError.value = errorMessage(error);
  } finally {
    if (request === buildIntentRequestSeq) buildIntentBusy.value = false;
  }
}

async function rejectBuildIntent(payload: { plan_id: string }) {
  const request = ++buildIntentRequestSeq;
  buildIntentBusy.value = true;
  buildIntentError.value = null;
  try {
    const response = await api.buildIntentReject(payload.plan_id);
    if (request === buildIntentRequestSeq) {
      buildIntentSelection.value = response.selection;
      buildIntentPlanningSource.value = response.planning_source;
    }
  } catch (error) {
    if (request === buildIntentRequestSeq) buildIntentError.value = errorMessage(error);
  } finally {
    if (request === buildIntentRequestSeq) buildIntentBusy.value = false;
  }
}

let workspaceLoadSeq = 0;
let initialReaderLoad: Promise<void> | null = null;
let initialHistoryLoad: Promise<void> | null = null;
let workspaceInitializationFailed = false;
let workspaceInitializationError: string | null = null;
async function loadVideoDetails(current: () => boolean) {
  videoDetailsLoading.value = true;
  videoDetailsError.value = '';
  try {
    const video = await api.videoContext();
    if (!current()) return;
    if (!video) throw new Error(uiText('视频字幕暂不可用，请重新打开资料。'));
    publishedVideo.value = video;
    await videoQuestion.restore();
    if (!current()) return;
    if (videoQuestion.draft.value) agentInput.value = videoQuestion.draft.value.message;
  } catch (error) {
    if (current()) videoDetailsError.value = (error as Error).message;
  } finally { if (current()) videoDetailsLoading.value = false; }
}
async function init() {
  const request = ++workspaceLoadSeq;
  const current = () => !disposed && request === workspaceLoadSeq;
  workspaceInitializationFailed = false;
  try {
    appSurface.value = "loading";
    focusTransition++;
    focusReading.value = false;
    focusPaginationActive.value = false;
    startupHistoryRequestSeq += 1;
    agentHistoryLoading.value = true;
    agentHistoryError.value = null;
    intentArtifactRequestSeq += 1;
    intentArtifacts.value = null;
    intentArtifactsLoading.value = false;
    intentArtifactsError.value = null;
    const desktop = network.value.enabled ? { desktop_host: false, reader_only: true, active_book: true, book_dir: null, library_root: "", library_root_available: true } : await api.desktopStatus().catch((error) => {
      if (error instanceof ApiError && error.status === 404) return null;
      throw error;
    });
    desktopHost.value = Boolean(desktop?.desktop_host);
    readerOnly.value = Boolean(desktop?.reader_only);
    localAgentHost.value = desktopHost.value && !readerOnly.value && !network.value.enabled;
    desktopLibraryRoot.value = desktop?.library_root ?? "";
    desktopLibraryAvailable.value = desktop?.library_root_available ?? true;
    desktopNeedsBook.value = Boolean(desktop && !desktop.active_book);
    activeBookDir.value = desktop?.book_dir ?? null;
    if (desktopLibraryNeedsSelection(desktop)) {
      desktopSettingsSection.value = 'general';
      desktopSettingsOpen.value = true;
      bookPickerOpen.value = false;
      return;
    }
    if (startOnShelf || (desktop && !desktop.active_book)) {
      startOnShelf = false;
      setLocalPage(localPage.value ?? 'courses');
      await loadBookLibrary();
      return;
    }
    desktopNeedsBook.value = false;
    const [video, initialMusic] = await Promise.all([api.videoPlayback(), api.musicContext()]);
    let music = initialMusic;
    if (!current()) return;
    // Standalone listening resumes the current calibration of the same recording.
    // Shared courses and explicit historical sources retain their chosen versions.
    if (music && network.value.enabled && !network.value.workspace?.learning_workspace_id
      && !recapProps.recapTarget && !recapProps.mcpSource && !recapProps.chatDraft?.message) {
      try {
        const update = await api.adoptMusicPublication(music.publication.publication_id);
        if (!current()) return;
        if (update.music) music = update.music;
      } catch (failure) {
        if (!current()) return;
        banner.value = failure instanceof Error ? failure.message : uiText('新版跟谱暂时无法读取，请稍后刷新。');
      }
    }
    musicContext.value = music; musicQuestionContext.value = null;
    videoPlayback.value = video;
    publishedVideo.value = null;
    const videoDetails = video ? loadVideoDetails(current) : Promise.resolve();
    if (video || music) {
      readerOnly.value = true;
      videoMediaUrl.value = network.value.enabled ? `/api${publishedUrl('video/media')}` : '/api/book/video/media';
      appSurface.value = 'reader';
    }
    const workbench = network.value.enabled || video || music ? null : await loadBuildWorkbenchSnapshot();
    if (network.value.enabled || video || music) {
      const reference = music?.publication ?? video?.publication ?? network.value.workspace!.published_book_ref;
      buildWorkbenchSnapshot.value = { book_id: reference.book_id, readiness: { route: "reader", reasons: [], stages: {} } } as unknown as BuildWorkbenchSnapshot;
    }
    if (!workbench && buildWorkbenchError.value) {
      appSurface.value = readerOnly.value ? "waiting-materials" : "workbench";
      return;
    }
    if (workbench) {
      const surface = chooseAppSurface(workbench, readerOnly.value);
      if (surface !== "reader") {
        appSurface.value = surface;
        return;
      }
    }
    const pdfReady = loadPdfRuntimeArtifacts(current);
    const [m, assets, noteSource, st] = await Promise.all([
      api.manifest(), api.assetManifest(), api.sourceFingerprint(),
      network.value.enabled ? Promise.resolve(network.value.workspace!.reader) : api.state(),
    ]);
    if (!current()) return;
    if (buildWorkbenchSnapshot.value?.book_id !== noteSource.book_id) {
      throw new Error("Note source identity does not match the active book");
    }
    noteSourceFingerprint.value = noteSource.source_fingerprint;
    await nextTick(); // Apply this book's saved reading surface before deciding what blocks display.
    sourceManifestTree.value = m;
    kindByLid.value = new Map(m.tree.map((n) => [n.lid, n.kind]));
    imageAssetByLid.value = new Map(assets.images.map((img) => [img.lid, img]));
    leafOrder.value = m.tree.filter((n) => n.children.length === 0).map((n) => n.lid);
    loadOutlineTitles(m);
    profileSummary.value = st.profile;
    readerLayout.value = st.layout;
    void ensureProfileManifest(st.profile, current).catch(error => { if (current()) fail(error); });
    await Promise.all([
      loadWindow(st.viewport, "replace", st.viewport.top_lid, current, false),
      readerSurfacePreference.value === "markdown" ? Promise.resolve() : pdfReady,
      videoDetails,
    ]);
    if (!current()) return;
    appSurface.value = "reader";
    if (banner.value === workspaceInitializationError) banner.value = '';
    workspaceInitializationError = null;
    await nextTick();
    if (current() && st.visible_position) await restoreVisiblePosition(st.visible_position);
    void refreshAnnotations().catch(error => { if (current()) fail(error); });
    void loadPaperProjectionData();
    initialHistoryLoad = loadStartupHistory();
    void refreshProfileSurface();
    recordIntentUsage("reader_ready");
    // The listening surface opens the score directly. Build artifacts can still be fetched on demand.
    if (!network.value.enabled && !music) void refreshIntentArtifacts();
  } catch (e) {
    if (!current()) return;
    workspaceInitializationFailed = true;
    agentHistoryLoading.value = false;
    agentHistoryError.value = errorMessage(e);
    workspaceInitializationError = errorMessage(e);
    appSurface.value = buildWorkbenchSnapshot.value?.readiness.route === "workbench" ? "workbench" : "reader";
    fail(e);
    requestWorkspaceRecovery(e);
  }
}

let workspaceRecoveryTask: Promise<void> | null = null;
let workspaceRecoveryTimer: number | null = null;
async function recoverWorkspaceState() {
  if (workspaceRecoveryTask || appSurface.value !== "reader" || agentHistoryLoading.value) return workspaceRecoveryTask;
  const expectedBookId = buildWorkbenchSnapshot.value?.book_id;
  if (!expectedBookId && !workspaceInitializationFailed) return;
  workspaceRecoveryTask = (async () => {
    try {
      // A failed initial read may not have reached book identity or history yet.
      if (workspaceInitializationFailed) { await init(); return; }
      const source = await api.sourceFingerprint();
      if (buildWorkbenchSnapshot.value?.book_id !== expectedBookId) return;
      if (source.book_id !== expectedBookId) {
        residentRun.forget();
        resetBookSessionUi();
        await init();
        return;
      }
      if (source.source_fingerprint !== noteSourceFingerprint.value) {
        await init();
        return;
      }
      if (network.value.enabled) await recoverSubmissions();
      if (agentHistoryError.value) await loadStartupHistory();
      else applyAgentHistory(await api.agentHistory());
    } catch (error) {
      requestWorkspaceRecovery(error);
      if (error instanceof ApiError && (error.status === 401 || error.status === 403)) {
        banner.value = uiText('认证已失效，请通过现有受保护入口恢复登录。');
      } else if (navigator.onLine === false) {
        banner.value = uiText('当前离线；恢复网络后将核对原运行，不会自动重提问题。');
      }
    } finally {
      workspaceRecoveryTask = null;
    }
  })();
  return workspaceRecoveryTask;
}

function scheduleWorkspaceRecovery(event?: Event) {
  // NetworkApp owns wake-up authentication and attachment; wait for its completion.
  if (network.value.enabled && event && ['visibilitychange', 'pageshow', 'online'].includes(event.type)) return;
  if (document.visibilityState === "hidden" || workspaceRecoveryTask || workspaceRecoveryTimer !== null) return;
  workspaceRecoveryTimer = window.setTimeout(() => {
    workspaceRecoveryTimer = null;
    void recoverWorkspaceState();
  }, 0);
}
async function refreshLinkedReader() {
  try {
    const state = await api.state();
    await applyReaderState(state);
    await loadWindow(state.viewport, "replace", resolveReaderStateNavigationTarget(state.viewport.top_lid, state.selection, leafOrder.value));
    if (state.visible_position) await restoreVisiblePosition(state.visible_position);
  } catch (error) { if (!disposed) fail(error); }
}

async function invokeDesktop<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  if (!("__TAURI_INTERNALS__" in window)) {
    throw new Error(uiText('桌面命令仅在 Understand Book 应用中可用'));
  }
  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<T>(command, args);
}

async function refreshCodexPluginStatus() {
  codexPluginLoading.value = true;
  codexPluginError.value = "";
  try {
    if (managedBuildEngine) {
      localBuildStatus.value = await invokeDesktop('local_build_status');
      return;
    }
    codexPluginStatus.value = await invokeDesktop<CodexPluginStatus>("codex_plugin_status");
  } catch (error) {
    codexPluginError.value = error instanceof Error ? error.message : String(error);
  } finally {
    codexPluginLoading.value = false;
  }
}

async function openDesktopSettings(section: 'models' | 'general' = 'models') {
  desktopSettingsSection.value = section === 'general' ? 'general' : 'models';
  desktopSettingsOpen.value = true;
  if (nativeSettings) await refreshCodexPluginStatus();
}

function closeDesktopSettings() {
  if (desktopLibraryChanging.value || desktopProviderSaving.value || personalConnectionSaving.value || (nativeSettings && !desktopLibraryAvailable.value)) return;
  desktopSettingsOpen.value = false;
}

const desktopModelSettingsClient: ModelSettingsClient = {
  listModels: (service, api_protocol) => localRequest('POST', '/model-settings/catalog', { service, api_protocol }),
  load: () => nativeSettings ? invokeDesktop("desktop_model_settings") : localRequest('GET', '/model-settings'),
  save: input => nativeSettings ? invokeDesktop("save_desktop_model_settings", { input }) : localRequest('POST', '/model-settings', input),
  test: (modelConfigId, input) => nativeSettings ? invokeDesktop("test_desktop_model", { modelConfigId, input }) : localRequest('POST', '/model-settings/test', { model_config_id: modelConfigId, input }),
};

async function chooseDesktopLibraryDirectory() {
  desktopLibraryChanging.value = true;
  desktopLibraryError.value = "";
  try {
    const { open } = await import("@tauri-apps/plugin-dialog");
    const selected = await open({
      directory: true,
      multiple: false,
      title: uiText('选择 Understand Book 书库位置'),
    });
    if (!selected || Array.isArray(selected)) return;
    const result = await invokeDesktop<{ library_root: string }>("set_desktop_library_directory", {
      selectedDir: selected,
    });
    desktopLibraryRoot.value = result.library_root;
    desktopLibraryAvailable.value = true;
    await loadBookLibrary();
    desktopSettingsOpen.value = false;
    if (desktopNeedsBook.value || localPage.value) {
      startOnShelf = false;
      setLocalPage('materials');
    } else {
      await init();
    }
  } catch (error) {
    desktopLibraryError.value = error instanceof Error ? error.message : String(error);
  } finally {
    desktopLibraryChanging.value = false;
  }
}

async function installCodexPlugin() {
  codexPluginLoading.value = true;
  codexPluginError.value = "";
  try {
    codexPluginStatus.value = await invokeDesktop<CodexPluginStatus>("install_codex_plugin");
  } catch (error) {
    codexPluginError.value = error instanceof Error ? error.message : String(error);
  } finally {
    codexPluginLoading.value = false;
  }
}

function codexPluginStateLabel(state: CodexPluginState): string {
  return {
    installed_by_setup: uiText('已由安装程序安装'),
    installed_externally: uiText('已在 Codex 中安装'),
    pending_configuration: uiText('需要安装插件'),
    codex_not_found: uiText('请先安装 Codex'),
    not_installed: uiText('尚未安装'),
    error: uiText('需要处理'),
  }[state];
}
onMounted(() => {
  window.addEventListener('beforeunload', noteBeforeUnload);
  document.addEventListener('pointerdown', collapseNoteOutside);
  window.addEventListener("agent-admission-changed", scheduleWorkspaceRecovery);
  window.addEventListener("workspace-recovered", scheduleWorkspaceRecovery);
  window.addEventListener("linked-reader-changed", refreshLinkedReader);
  window.addEventListener("tutor-response", onTutorResponse);
  if (!startOnShelf) void tutor.load();
  initialReaderLoad = init();
  videoAdoptionTimer = setTimeout(refreshVideoPublication, 3000);
  document.addEventListener("visibilitychange", scheduleWorkspaceRecovery);
  window.addEventListener("pageshow", scheduleWorkspaceRecovery);
  window.addEventListener("online", scheduleWorkspaceRecovery);
});
onBeforeUnmount(() => {
  disposed = true;
  videoAdoption.dispose(); clearTimeout(videoAdoptionTimer);
  clearNoteScope();
  window.removeEventListener('beforeunload', noteBeforeUnload);
  document.removeEventListener('pointerdown', collapseNoteOutside);
  window.removeEventListener("agent-admission-changed", scheduleWorkspaceRecovery);
  window.removeEventListener("workspace-recovered", scheduleWorkspaceRecovery);
  window.removeEventListener("linked-reader-changed", refreshLinkedReader);
  window.removeEventListener("tutor-response", onTutorResponse);
  startupHistoryRequestSeq += 1;
  document.removeEventListener("visibilitychange", scheduleWorkspaceRecovery);
  window.removeEventListener("pageshow", scheduleWorkspaceRecovery);
  window.removeEventListener("online", scheduleWorkspaceRecovery);
  if (workspaceRecoveryTimer !== null) window.clearTimeout(workspaceRecoveryTimer);
  workspaceRecoveryTimer = null;
  if (paperPositionSyncTimer !== null) window.clearTimeout(paperPositionSyncTimer);
  if (profileBackfillPollTimer !== null) window.clearTimeout(profileBackfillPollTimer);
  notePlacementController.cancel();
  pdfSelectionTranslation.invalidate("unmount");
});

// ── 四动作 ──
async function onScrollEdge(direction: "up" | "down") {
  if (focusPaginationActive.value || edgeLoading.value || !viewport.value) return;
  const loaded = segments.value;
  if (!loaded.length || !leafOrder.value.length) return;
  const firstIdx = leafOrder.value.indexOf(loaded[0].lid);
  const lastIdx = leafOrder.value.indexOf(loaded[loaded.length - 1].lid);
  if (firstIdx < 0 || lastIdx < 0) return;
  const count = Math.max(1, viewport.value.width);
  const nextLids = direction === "down"
    ? leafOrder.value.slice(lastIdx + 1, Math.min(lastIdx + 1 + count, leafOrder.value.length))
    : leafOrder.value.slice(Math.max(0, firstIdx - count), firstIdx);
  if (!nextLids.length) return;
  const navigation = readerNavigationRequest, context = readerContextKey.value;
  const current = () => !disposed && navigation === readerNavigationRequest && context === readerContextKey.value && !focusPaginationActive.value;
  const anchor = direction === "up"
    ? readerPaneRef.value?.captureScrollAnchor(loaded.map((seg) => seg.lid)) ?? null
    : null;
  edgeLoading.value = true;
  try {
    banner.value = "";
    if (!sourceFocus.value?.ranges) sourceFocus.value = null;
    const next = await hydrateSegments(nextLids);
    if (!current()) return;
    segments.value = mergeSegments(segments.value, next, direction === "down" ? "append" : "prepend");
    if (direction === "up") await readerPaneRef.value?.restoreScrollAnchor(anchor);
  } catch (e) {
    if (current()) fail(e);
  } finally {
    edgeLoading.value = false;
  }
}
let readerNavigationRequest = 0;
let noteSourceNavigating = false;
watch([pendingNoteSource, appSurface, () => buildWorkbenchSnapshot.value?.book_id, () => videoPane.value, () => network.value.workspace?.learning_workspace_id], async ([target]) => {
  if (!target || noteSourceNavigating) return;
  const boardInOpenWorkspace = !!target.board_id && !!target.workspace_id && target.workspace_id === network.value.workspace?.learning_workspace_id;
  if (!network.value.enabled && target.book_id !== buildWorkbenchSnapshot.value?.book_id) {
    noteSourceNavigating = true;
    try {
      const library = await api.bookLibrary();
      const original = library.books.find(book => book.book_id === target.book_id && book.route === 'reader');
      if (!original) throw new Error('原资料当前不可用，原话和保留摘录仍可阅读。');
      await submitOpenBook(original.dir);
      if (buildWorkbenchSnapshot.value?.book_id !== target.book_id) throw new Error(bookPickerError.value || '原资料暂时无法打开');
    } catch (failure) { noteSourceError.value = (failure as Error).message; pendingNoteSource.value = null; }
    finally { noteSourceNavigating = false; }
  }
  if (pendingNoteSource.value !== target || appSurface.value !== 'reader' || (!boardInOpenWorkspace && target.book_id !== buildWorkbenchSnapshot.value?.book_id)) return;
  if (network.value.enabled && !boardInOpenWorkspace && target.publication_id !== network.value.workspace?.published_book_ref.publication_id) return;
  if (network.value.enabled && target.workspace_id !== (network.value.workspace?.learning_workspace_id ?? null)) return;
  if (localPage.value) {
    setLocalPage(null);
    closeUnderstanding();
    if (!target.board_id) closeBoard();
  }
  if (target.video_time_ms != null && !videoPane.value) return;
  noteSourceNavigating = true;
  try {
    await nextTick();
    if (target.board_id) await openBoardLocation({board_id:target.board_id,item_id:target.board_item_id ?? undefined});
    else if (target.session_id && target.turn_id) {
      if (target.session_id !== activeChatSessionId.value) await selectChat(target.session_id);
      if (target.session_id !== activeChatSessionId.value || !chat.value.some(turn=>turn.turnId===target.turn_id)) throw new Error('原对话当前不可用，想法仍保留在这里。');
      workspaceRef.value?.showAssistant('agent'); requestWorkspaceTab('agent'); await nextTick();
      if (target.origin?.kind === 'presentation') await rightRailRef.value?.locateScene(target.origin.receipt, true);
      else if (!await rightRailRef.value?.scrollToTurn(target.turn_id)) throw new Error('原消息暂时无法定位');
    }
    else if (target.video_time_ms != null) videoPane.value?.seek(target.video_time_ms);
    else if (target.lid && !await doGoto(target.lid)) throw new Error('原位置暂时无法定位，想法仍保留在这里。');
    const understanding=pendingUnderstandingAction.value;
    if(understanding && understanding.context.session_id===target.session_id && understanding.context.turn_id===target.turn_id) {
      pendingUnderstandingAction.value=null;
      await startUnderstandingAction(understanding);
    }
    notesPanel.open = false; noteSourceError.value = '';
  } catch (failure) { noteSourceError.value = (failure as Error).message; }
  finally { pendingNoteSource.value = null; noteSourceNavigating = false; }
});
async function doGoto(lid: string, focusQuote?: string | null, restoreCurrent?: () => boolean, sourceRanges?: Map<string, [number, number]>): Promise<boolean> {
  if (!lid) return false;
  if (!restoreCurrent) readingContinuity.cancelRestore();
  const request = ++readerNavigationRequest;
  const contextKey = readerContextKey.value;
  const current = () => !disposed && request === readerNavigationRequest && contextKey === readerContextKey.value && (restoreCurrent?.() ?? true);
  readerPaneRef.value?.cancelTypographyRestore();
  pendingNavigationLid.value = lid;
  let previewAnchor: ScrollAnchor | null = null;
  const previousReadingLid = currentReadingLid.value;
  try {
    banner.value = "";
    pendingFocusPosition = null;
    clearTimeout(focusPositionTimer);
    if (!current()) return false;
    sourceFocus.value = sourceRanges ? { lid, quote: focusQuote ?? null, ranges: sourceRanges } : focusQuote === undefined ? null : { lid, quote: focusQuote };
    if (focusPaginationActive.value) {
      const target = resolveReaderNavigationTarget(lid, leafOrder.value) ?? lid;
      let offset = sourceRanges?.get(target)?.[0] ?? 0;
      if (focusQuote && !sourceRanges) {
        const source = await api.text(target);
        if (!current()) return false;
        offset = Math.max(0, source.text.indexOf(focusQuote));
        sourceFocus.value = { lid: target, quote: focusQuote, ranges: new Map([[target, [offset, offset + focusQuote.length]]]) };
      }
      outlineNavigationLid.value = lid;
      return await readerPaneRef.value?.scrollSourceIntoView(target, { start: offset, end: offset + 1 }) ?? false;
    }
    const target = resolveReaderNavigationTarget(lid, leafOrder.value);
    const width = viewport.value?.width ?? 20;
    const index = target ? leafOrder.value.indexOf(target) : -1;
    const start = Math.min(index, Math.max(0, leafOrder.value.length - width));
    const cached = index < 0 ? null : windowTextCache.value.peek(leafOrder.value.slice(start, start + width));
    const command = api.goto(lid, !cached);
    const localTarget = activeReaderSurface.value === 'markdown' && target && segments.value.some(segment => segment.lid === target);
    if (localTarget) previewAnchor = readerPaneRef.value?.captureScrollAnchor(segments.value.map(segment => segment.lid)) ?? null;
    const [gotoEffect] = await Promise.all([command, localTarget ? readerPaneRef.value?.scrollLidIntoView(target) : undefined]);
    if (!current()) return false;
    outlineNavigationLid.value = lid;
    const navigationTargetLid = resolveReaderNavigationTarget(lid, leafOrder.value)
      ?? gotoEffect.viewport.top_lid;
    await loadWindow(gotoEffect.viewport, "replace", navigationTargetLid, current, true,
      gotoEffect.texts ?? windowTextCache.value.peek(gotoEffect.viewport.visible_lids) ?? undefined);
    if (!current()) return false;
    queuePaperSelection(lid);
    if (activeReaderSurface.value === "pdf" && !hasMappedPdfNavigationTarget(
      lid,
      navigationTargetLid,
      pdfMappedLids.value,
    )) {
      banner.value = uiText('PDF 暂无 {0} 的可靠定位，已保留当前 PDF 页面。', [navigationTargetLid]);
    }
    await loadPaperProjectionData();
    gotoInput.value = "";
    return true;
  } catch (e) {
    if (!current()) return false;
    if (previewAnchor) {
      await readerPaneRef.value?.restoreScrollAnchor(previewAnchor);
      if (!current()) return false;
      currentReadingLid.value = previousReadingLid;
    }
    if (outlineNavigationLid.value === lid) outlineNavigationLid.value = null;
    fail(e);
    return false;
  } finally {
    if (current()) pendingNavigationLid.value = null;
  }
}
let openingMcpSource = false;
async function openPersonalPassage(source: ExternalPassage) {
  if (closedCover.value || !localAgentHost.value) return false;
  const location = source.source_ref;
  if (buildWorkbenchSnapshot.value?.book_id !== location.book_id) {
    await submitOpenBook(source.book_dir);
    if (buildWorkbenchSnapshot.value?.book_id === location.book_id && !closedReady.value) await refreshClosedBook();
  }
  if (closedCover.value || disposed || buildWorkbenchSnapshot.value?.book_id !== location.book_id
    || noteSourceFingerprint.value !== location.source_revision) return false;
  readerSurfacePreference.value = 'markdown';
  workspaceRef.value?.showReader();
  await nextTick();
  const ranges = location.range ? new Map<string,[number,number]>([[location.lid, [location.range.start, location.range.end]]]) : undefined;
  if (!await doGoto(location.lid, null, undefined, ranges)) return false;
  await nextTick();
  const displayed = await readerPaneRef.value?.scrollSourceIntoView(location.lid, location.range);
  return displayed === true && !closedCover.value;
}
function refreshExternalWrites() {
  refreshPersonalNotes();
  void loadLocalLearningTasks().catch(fail);
}
watch(() => [recapProps.mcpSource, appSurface.value] as const, async ([source, surface]) => {
  if (!source || surface !== 'reader' || openingMcpSource || !sourceManifestTree.value) return;
  openingMcpSource = true;
  const current = () => !disposed && recapProps.mcpSource === source && readerKey() === mountedReaderKey;
  try {
    const publication = network.value.workspace?.published_book_ref;
    if (!publication || publication.book_id !== source.published_book_ref.book_id
      || publication.publication_id !== source.published_book_ref.publication_id) throw new Error(uiText('来源材料版本不匹配。'));
    const target = resolveMcpSource(source, sourceManifestTree.value);
    // Canonical UTF-16 citations open on the canonical text surface, including PDF materials.
    readerSurfacePreference.value = 'markdown';
    await nextTick();
    if (!current()) return;
    workspaceRef.value?.showReader();
    if (!await doGoto(target.lid, undefined, current)) throw new Error(uiText('来源定位未完成，请重试。'));
    if (!current()) return;
    sourceFocus.value = { lid: target.lid, quote: null, ranges: target.ranges };
    await nextTick();
    await readerPaneRef.value?.scrollSourceIntoView(target.lid);
    if (current()) recapEmit('mcp-source-consumed');
  } catch (failure) { if (current()) recapEmit('mcp-source-error', errorMessage(failure)); }
  finally { openingMcpSource = false; }
}, { flush: 'post' });

async function selectReaderSurface(surface: ReaderSurface) {
  if (surface === activeReaderSurface.value) return;
  if (surface === "pdf" && !pdfReaderAvailable.value) {
    banner.value = uiText('当前来源没有可用的 PDF 阅读表面。');
    return;
  }
  const previous = activeReaderSurface.value;
  const markdownAnchor = previous === "markdown"
    ? readerPaneRef.value?.captureScrollAnchor(segments.value.map((segment) => segment.lid)) ?? null
    : null;
  const pdfAnchor = previous === "pdf" ? pdfReaderPaneRef.value?.captureReadingAnchor() ?? null : null;
  if (previous === 'pdf') previousPdfAnchor.value = pdfAnchor;
  hlPopover.value = null;
  window.getSelection()?.removeAllRanges();
  cancelPdfSelectionDraftFor("reader-surface-switch");
  readerSurfacePreference.value = surface;
  const identity = readerSurfaceIdentity.value;
  if (identity) {
    try { writeReaderSurfacePreference(window.localStorage, identity, surface); } catch { /* Device preference remains in memory. */ }
  }
  await nextTick();
  if (surface === "pdf") {
    const lid = markdownAnchor?.lid ?? readingAnchorLid.value;
    if (lid && !hasMappedPdfNavigationTarget(lid, lid, pdfMappedLids.value)) {
      banner.value = uiText('PDF 暂无 {0} 的可靠定位，已保留当前 PDF 页面。', [lid]);
    }
    return;
  }
  const lid = pdfAnchor?.anchorLid;
  if (focusReading.value) {
    const index = focusPaginationSource.value.leaves.findIndex(node => node.lid === (lid ?? readingAnchorLid.value));
    await readerPaneRef.value?.prepareFocus(index < 0 ? undefined : { index, offset: 0 });
    return;
  }
  if (!lid) {
    banner.value = uiText('当前 PDF 位置没有可靠的 Markdown 映射，已保留此前正文读位。');
    return;
  }
  if (segments.value.some((segment) => segment.lid === lid)) {
    await readerPaneRef.value?.scrollLidIntoView(lid);
  } else if (await doGoto(lid)) {
    await readerPaneRef.value?.scrollLidIntoView(lid);
  }
}
function focusSource(source: { lid: string; quote: string | null; memId?: string }) {
  const record = annotations.value.find(r => r.mem_id === source.memId);
  const ranges = new Map<string, [number, number]>();
  for (const r of record?.selection_context?.ranges ?? (record?.range ? [{ lid: source.lid, range: record.range }] : [])) {
    const previous = ranges.get(r.lid);
    ranges.set(r.lid, previous ? [Math.min(previous[0], r.range.start), Math.max(previous[1], r.range.end)] : [r.range.start, r.range.end]);
  }
  openSourcePreview({ ...source, ...(ranges.size ? { ranges } : {}) }, 'notes', source.memId);
}
function focusLocalSource(source: SourceFocus) {
  openSourcePreview(source, 'reader');
}
function errorMessage(e: unknown): string {
  if (e instanceof ApiError && e.errorCode === 'CLIENT_CONTEXT_STALE') return '';
  return e instanceof Error ? e.message : String(e);
}
let sourcePreviewReturn: ReadingReturnPoint | null = null;
function openSourcePreview(target: SourceFocus, origin: 'reader' | 'notes' = 'reader', memId?: string) {
  sourcePreviewReturn = { ...captureReadingReturnPoint(''), origin, memId };
  const order = leafOrder.value;
  const lid = resolveReaderNavigationTarget(target.lid, order) ?? target.lid;
  const focus = { ...target, lid };
  const source: SourcePopupView = {
    source_ref_id: lid, label: outlineItems.value.find(item => item.lid === target.lid)?.title || uiText('引用来源'),
    material_title: readerBookTitle.value, highlighted_quote: target.quote ?? '',
    context_before: '', context_after: '', stale: false, can_open_in_reader: true,
  };
  let matchedRanges: Map<string, [number, number]> | undefined;
  const readWindow: SourceWindowLoader = async ({ anchor_lid, direction = 'around', include_outline }) => {
    const anchor = resolveReaderNavigationTarget(anchor_lid ?? lid, order) ?? lid;
    const index = order.indexOf(anchor);
    const start = direction === 'after' ? index + 1 : Math.max(0, index - (direction === 'before' ? 20 : 6));
    const end = direction === 'before' ? index : start + 20;
    const page = await hydrateSegments(index < 0 ? [anchor] : order.slice(start, end));
    if (!focus.ranges && !matchedRanges && page.some(segment => segment.lid === lid)) matchedRanges = sourceFocusRangesIn(page, focus);
    const ranges = focus.ranges ?? matchedRanges;
    return {
      source_ref_id: lid, source_lid: lid, focus_lid: anchor,
      previous_lid: start > 0 ? page[0]?.lid ?? null : null,
      next_lid: end < order.length ? page.at(-1)?.lid ?? null : null,
      outline: include_outline ? outlineItems.value : undefined,
      segments: page.map(segment => {
        const range = ranges?.get(segment.lid);
        const offset = focus.ranges ? 0 : displayText(segment).offset;
        return { lid: segment.lid, text: segment.text, kind: segment.kind,
          highlights: range ? [{ start: range[0] + offset, end: range[1] + offset }] : [] };
      }),
    };
  };
  banner.value = '';
  sourcePreview.value = { focus, source, readWindow };
}
function closeSourcePreview() {
  sourcePreviewReturn = null;
  sourcePreview.value = null;
}
async function openSourceInReader() {
  const focus = sourcePreview.value?.focus;
  if (!focus) return;
  const point = sourcePreviewReturn;
  closeSourcePreview();
  tutorPanelOpen.value = false;
  workspaceRef.value?.showReader();
  if (await doGoto(focus.lid, focus.quote, undefined, focus.ranges) && point?.contextKey === workspaceContextKey.value) {
    readingContinuity.push(point);
    updateReadingReturn();
  }
}
const pdfSelectionSession = usePdfSelectionDraft((capture) =>
  api.pdfSelectionResolve({ rects: capture.rects, raw_quote: capture.raw_quote }),
);
const pdfSelectionTranslation = usePdfSelectionTranslation((request) =>
  api.pdfSelectionTranslate(request),
);
const pdfSelectionState = pdfSelectionSession.state;
const pdfSelectionTranslationState = pdfSelectionTranslation.state;
const pdfSelectionCapabilities = computed(() => getPdfSelectionCapabilities(
  pdfSelectionState.value.draft?.status ?? "unresolved",
  pdfSelectionState.value.draft?.diagnostic,
));
const showPdfTranslationSettings = computed(() =>
  desktopHost.value
  && pdfSelectionTranslationState.value.error?.error_code === "TRANSLATION_PROVIDER_UNCONFIGURED",
);
const pdfReselectTarget = ref<{
  kind: "note" | "highlight";
  record: MemoryRecord;
} | null>(null);
const pdfSelectionToolbarStyle = computed(() => {
  const rect = pdfSelectionState.value.capture?.screen_rect;
  if (!rect) return {};
  const center = (rect.left + rect.right) / 2;
  return {
    left: `clamp(178px, ${center}px, calc(100vw - 178px))`,
    top: `${Math.max(8, rect.top - 48)}px`,
  };
});

function onPdfSelectionCapture(capture: PdfSelectionCapture) {
  pdfSelectionTranslation.invalidate("selection");
  void pdfSelectionSession.capture(capture);
}

function cancelPdfSelectionDraftFor(reason: PdfSelectionTranslationInvalidation) {
  pdfSelectionTranslation.invalidate(reason);
  pdfSelectionSession.cancel();
  pdfReselectTarget.value = null;
}

function cancelPdfSelectionDraft() {
  cancelPdfSelectionDraftFor("close");
}

function onReaderViewportInteraction() {
  readerNavigationRequest++;
  pendingNavigationLid.value = null;
  readerPaneRef.value?.cancelTypographyRestore();
  readingContinuity.cancelRestore();
  clearOutlineNavigation();
  const interaction = readerInteractionRevision;
  if (residentRun.active.value) manualReaderBoundary = api.state().then(state => { if (readerInteractionRevision === interaction) manualReaderState = state; }).catch(fail);
}
function onPdfViewportInteraction() {
  pdfSelectionTranslation.invalidate("viewport");
  onReaderViewportInteraction();
}

function translatePdfSelection() {
  const draft = pdfSelectionState.value.draft;
  if (
    !draft
    || !getPdfSelectionCapabilities(draft.status).canTranslate
    || pdfSelectionTranslationState.value.phase === "loading"
  ) return;
  void pdfSelectionTranslation.start(draft);
}

function closePdfSelectionTranslation() {
  pdfSelectionTranslation.invalidate("close");
}

function retryPdfSelectionTranslation() {
  void pdfSelectionTranslation.retry();
}

async function copyPdfSelectionTranslation(markdown: string) {
  try {
    await navigator.clipboard.writeText(markdown);
    banner.value = uiText('译文已复制');
  } catch (error) {
    fail(error);
  }
}

function selectionContextOf(draft: PdfSelectionDraft): SelectionContext {
  return {
    status: draft.status,
    ...(draft.resolution_basis ? { resolution_basis: draft.resolution_basis } : {}),
    raw_quote: draft.raw_quote,
    resolved_quote: draft.resolved_quote,
    ranges: draft.ranges,
  };
}

function completePdfSelectionAction() {
  pdfSelectionTranslation.invalidate("existing-action");
  pdfSelectionSession.complete();
  pdfReselectTarget.value = null;
  window.getSelection()?.removeAllRanges();
}

function reselectPdfNote(note: MemoryRecord) {
  if (!note.selection_context && note.note_placement?.kind === "pdf_region") {
    void startNotePlacement(note);
    return;
  }
  pdfSelectionTranslation.invalidate("existing-action");
  pdfSelectionSession.cancel();
  pdfReselectTarget.value = { kind: "note", record: note };
  banner.value = uiText('重新框选 PDF 文字后选择“笔记”；保存成功前原位置保持不变。');
}

function reselectPdfHighlight(highlight: MemoryRecord) {
  pdfSelectionTranslation.invalidate("existing-action");
  pdfSelectionSession.cancel();
  pdfReselectTarget.value = { kind: "highlight", record: highlight };
  banner.value = uiText('重新框选 PDF 文字后选择“高亮”；新范围保存成功前原高亮保持不变。');
}

// ── 自由选区:可跨多个 LID,高亮按 LID 拆 range;Note/Ask AI 锚到起点 LID `[ADR-0031]` ──
interface MarkdownSelectedRange {
  lid: string;
  start: number;
  end: number;
}
interface MarkdownSelectionPopover {
  contextKey: string;
  x: number;
  y: number;
  anchorLid: string;
  ranges: MarkdownSelectedRange[];
  text: string;
  resolvedText: string;
}
const hlPopover = ref<MarkdownSelectionPopover | null>(null);
const markdownSelectionToolbar = ref<HTMLElement | null>(null);
watch(hlPopover, (popover) => {
  const toolbar = markdownSelectionToolbar.value;
  if (!popover || !toolbar) return;
  const { width, height } = toolbar.getBoundingClientRect();
  popover.x = Math.max(8 + width / 2, Math.min(window.innerWidth - 8 - width / 2, popover.x));
  popover.y = Math.max(8, Math.min(window.innerHeight - height - 8, popover.y - height - 8)) + 40;
}, { flush: 'post' });

function lidElementOf(node: Node | null): HTMLElement | null {
  const el = node && node.nodeType === 3 ? node.parentElement : (node as HTMLElement | null);
  return el ? el.closest("[data-lid]") : null;
}

function selectedRangesForElement(el: HTMLElement, range: Range, startEl: HTMLElement, endEl: HTMLElement): MarkdownSelectedRange[] {
  const lid = el.getAttribute("data-lid") ?? "";
  const seg = lid ? segmentByLid.value.get(lid) : null;
  if (!lid || !seg) return [];

  if (seg.kind === "formula") {
    return seg.text.length > 0 ? [{ lid, start: 0, end: seg.text.length }] : [];
  }

  const display = displayText(seg);
  const localRange = document.createRange();
  localRange.selectNodeContents(el);
  if (el === startEl) localRange.setStart(range.startContainer, range.startOffset);
  if (el === endEl) localRange.setEnd(range.endContainer, range.endOffset);
  return createMarkdownDomSourceMap(display.text, el)
    .sourceRangesForRange(localRange)
    .map(({ start, end }) => ({
      lid,
      start: clampRange(display.offset + start, seg.text.length),
      end: clampRange(display.offset + end, seg.text.length),
    }))
    .filter(({ start, end }) => end > start);
}

function selectionRanges(range: Range): MarkdownSelectedRange[] {
  const startEl = lidElementOf(range.startContainer);
  const endEl = lidElementOf(range.endContainer);
  if (!startEl || !endEl) return [];
  const root = startEl.closest(".prose");
  if (!root || !root.contains(endEl)) return [];

  return Array.from(root.querySelectorAll<HTMLElement>("[data-lid]"))
    .filter((el) => range.intersectsNode(el))
    .flatMap((el) => selectedRangesForElement(el, range, startEl, endEl));
}

function resolvedSelectionQuote(ranges: MarkdownSelectedRange[]): string {
  return ranges
    .map(({ lid, start, end }) => segmentByLid.value.get(lid)?.text.slice(start, end) ?? "")
    .join("");
}

function onSelectSeg(lid: string) {
  selectedLid.value = lid;
  currentReadingLid.value = lid;
  sourceFocus.value = null;
  queuePaperSelection(lid);
}

function onMarkdownNotePlacementInvalid() {
  if (notePlacementState.value.phase === "placing") {
    banner.value = uiText('这里不是可放置的正文目标，请点选真实正文块。');
  }
}

function notePlacementBookIsCurrent(saving: SavingNotePlacement): boolean {
  return buildWorkbenchSnapshot.value?.book_id === saving.original_book_id;
}

async function handleExplicitNotePlacementFailure(
  saving: SavingNotePlacement,
  error: ApiError,
) {
  if (!notePlacementBookIsCurrent(saving)) {
    notePlacementController.saveFailed(error);
    notePlacementController.cancel();
    return;
  }
  let records: MemoryRecord[] | null = null;
  try {
    records = await refreshAnnotations();
  } catch {
    // The mutation response is authoritative; a failed refresh must not change its classification.
  }
  if (!notePlacementBookIsCurrent(saving)) {
    notePlacementController.saveFailed(error);
    notePlacementController.cancel();
    return;
  }
  notePlacementController.saveFailed(error);
  if (saving.record && records && !records.some((record) => record.mem_id === saving.record?.mem_id)) {
    notePlacementController.cancel();
    banner.value = uiText('这条笔记已更新，请重新选择放置位置。');
    return;
  }
  if (error.errorCode === "STALE_NOTE_SOURCE" || error.errorCode === "STALE_PDF_NOTE_PLACEMENT") {
    try {
      const source = await api.sourceFingerprint();
      const nextPdfMap = saving.surface_kind === "pdf" ? await api.pdfSourceMap() : null;
      if (!notePlacementBookIsCurrent(saving)
        || source.book_id !== saving.original_book_id
        || notePlacementSurface.value !== saving.surface_kind
        || (saving.surface_kind === "pdf" && nextPdfMap?.book_id !== source.book_id)) {
        notePlacementController.cancel();
        return;
      }
      if (nextPdfMap) pdfSourceMap.value = nextPdfMap;
      if (saving.draft) {
        noteSourceFingerprint.value = source.source_fingerprint;
        notePlacementController.createDraft({
          ...saving.draft,
          source_fingerprint: source.source_fingerprint,
        });
      } else if (saving.record && records) {
        const authoritative = records.find((record) => record.mem_id === saving.record?.mem_id);
        if (authoritative) {
          noteSourceFingerprint.value = source.source_fingerprint;
          notePlacementController.startRecord({
            record: authoritative,
            book_id: source.book_id,
            surface_kind: saving.surface_kind,
            source_fingerprint: source.source_fingerprint,
          });
        }
      }
    } catch {
      // Keep the prior placing session; reopening the book will refresh the source identity.
    }
  }
  fail(error);
}

async function reconcileUncertainNotePlacement(
  saving: SavingNotePlacement,
  error: unknown,
) {
  notePlacementController.reconcileRequired(errorMessage(error));
  if (!notePlacementBookIsCurrent(saving)) {
    notePlacementController.reconciled("missing");
    return;
  }
  let records: MemoryRecord[];
  try {
    records = await refreshAnnotations();
  } catch (refreshError) {
    if (!notePlacementBookIsCurrent(saving)) {
      notePlacementController.reconciled("missing");
      return;
    }
    notePlacementController.reconciled("retry");
    fail(refreshError);
    return;
  }
  if (!notePlacementBookIsCurrent(saving)) {
    notePlacementController.reconciled("missing");
    return;
  }
  const reconciliation = saving.draft
    ? reconcilePlacedNoteCreate(saving.draft, saving.target, records)
    : saving.record
      ? reconcilePlacedNoteReanchor(saving.record, saving.target, records)
      : { outcome: "missing" as const };
  notePlacementController.reconciled(reconciliation.outcome);
  if (reconciliation.outcome === "committed") {
    selectedLid.value = saving.target.lid;
    banner.value = saving.subject.kind === "record"
      ? uiText('已确认笔记的新位置。')
      : uiText('已确认笔记保存成功。');
  } else if (reconciliation.outcome === "retry") {
    banner.value = uiText('未确认写入成功，已保留放置内容；请重新选择正文目标。');
  } else {
    banner.value = uiText('这条笔记已更新，请重新选择放置位置。');
  }
}

function sameNotePlacement(left: NoteBodyPlacement | null | undefined, right: NoteBodyPlacement): boolean {
  if (!left || left.kind !== right.kind) return false;
  if (left.kind === "lid_block" && right.kind === "lid_block") {
    return left.source_fingerprint === right.source_fingerprint && left.lid === right.lid;
  }
  if (left.kind !== "pdf_region" || right.kind !== "pdf_region") return false;
  return left.source_fingerprint === right.source_fingerprint
    && left.lid === right.lid
    && left.source_map_version === right.source_map_version
    && left.source_map_config_hash === right.source_map_config_hash
    && left.page_index === right.page_index
    && left.region_id === right.region_id;
}

async function submitNotePlacementTarget(placement: NoteBodyPlacement) {
  const current = notePlacementState.value;
  if (current.phase !== "placing"
    || placement.source_fingerprint !== current.source_fingerprint
    || (current.surface_kind === "markdown" && placement.kind !== "lid_block")
    || (current.surface_kind === "pdf" && placement.kind !== "pdf_region")) return;
  if (sameNotePlacement(current.record?.note_placement, placement)) {
    banner.value = uiText('这条笔记已在此处，请选择其他位置。');
    return;
  }
  const saving = notePlacementController.beginSaving(placement);
  if (!saving) return;
  try {
    const result = saving.draft
      ? await createPlacedNote(saving.draft, placement, {
          save: (input) => api.save(input),
          promote: (memId) => api.promote(memId),
        })
      : saving.record
        ? await reanchorPlacedNote(saving.record, placement, {
            reanchor: (memId, nextPlacement) => api.reanchor(memId, nextPlacement),
          })
        : null;
    notePlacementController.saveSucceeded();
    if (!result || !notePlacementBookIsCurrent(saving)) return;
    if (saving.record && 'mem_id' in result) {
      rightRailRef.value?.replaceMemory?.(saving.record.mem_id, result);
      annotations.value = annotations.value.map(record => record.mem_id === saving.record!.mem_id ? result : record);
    }
    try {
      await refreshAnnotations();
    } catch (refreshError) {
      if (!notePlacementBookIsCurrent(saving)) return;
      fail(refreshError);
      return;
    }
    if (!notePlacementBookIsCurrent(saving)) return;
    selectedLid.value = placement.lid;
    banner.value = saving.subject.kind === "record"
      ? saving.record?.note_placement
        ? uiText('笔记已移动到新的正文位置。')
        : uiText('笔记已放置到正文。')
      : "promoted" in result && result.promoted
        ? uiText('助手建议已保存为笔记。')
        : "status" in result && result.status === "EXISTING"
          ? uiText('这条笔记已存在。')
          : uiText('笔记已放置到正文。');
  } catch (error) {
    if (error instanceof ApiError) {
      await handleExplicitNotePlacementFailure(saving, error);
    } else {
      await reconcileUncertainNotePlacement(saving, error);
    }
  }
}

async function onMarkdownNotePlacementTarget(target: { lid: string }) {
  const current = notePlacementState.value;
  if (current.phase !== "placing" || current.surface_kind !== "markdown") return;
  await submitNotePlacementTarget({
    kind: "lid_block",
    source_fingerprint: current.source_fingerprint,
    lid: target.lid,
  });
}

async function onPdfNotePlacementTarget(target: { entry: PdfSourceMapEntry; region: PdfRegion }) {
  const current = notePlacementState.value;
  const map = pdfSourceMap.value;
  if (current.phase !== "placing" || current.surface_kind !== "pdf" || !map) return;
  const targetIsCurrent = map.entries.some((entry) =>
    entry.lid === target.entry.lid
    && entry.regions.some((region) =>
      region.pageIndex === target.region.pageIndex
      && region.region_id === target.region.region_id
      && region.bbox.every((coordinate, index) => coordinate === target.region.bbox[index])));
  if (!targetIsCurrent || map.book_id !== current.book_id) {
    banner.value = uiText('PDF 映射已变化，请重新选择正文区域。');
    return;
  }
  await submitNotePlacementTarget({
    kind: "pdf_region",
    source_fingerprint: current.source_fingerprint,
    lid: target.entry.lid,
    source_map_version: map.version,
    source_map_config_hash: map.config_hash,
    page_index: target.region.pageIndex,
    region_id: target.region.region_id,
  });
}

function onCurrentLid(lid: string) {
  currentReadingLid.value = lid;
}

function clearOutlineNavigation() {
  readerInteractionRevision++;
  outlineNavigationLid.value = null;
}

async function openFormulaDialog(seg: Segment) {
  selectedLid.value = seg.lid;
  currentReadingLid.value = seg.lid;
  sourceFocus.value = null;
  formulaDialog.value = { ...seg };
  await loadFormulaProfile(seg);
}

async function loadFormulaProfile(seg: Segment) {
  const request = ++formulaRequest;
  const context = workspaceContextKey.value;
  formulaError.value = null;
  formulaLoading.value = !seg.formula;
  if (seg.formula) return;
  try {
    const formula = await formulaFor(seg.lid, seg.kind);
    if (request !== formulaRequest || context !== workspaceContextKey.value || selectedLid.value !== seg.lid) return;
    const loaded = segments.value.find(value => value.lid === seg.lid && value.text === seg.text);
    if (loaded) loaded.formula = formula;
    if (formulaDialog.value?.lid === seg.lid) formulaDialog.value = { ...seg, formula };
  } catch (error) {
    if (request === formulaRequest && context === workspaceContextKey.value) formulaError.value = errorMessage(error);
  } finally {
    if (request === formulaRequest) formulaLoading.value = false;
  }
}

function requestSelectedFormula() {
  const seg = selectedSegment.value;
  if (seg?.kind === 'formula') void loadFormulaProfile(seg);
  else { formulaRequest++; formulaLoading.value = false; formulaError.value = null; }
}

function closeFormulaDialog() {
  formulaRequest++;
  formulaLoading.value = false;
  formulaDialog.value = null;
}


function onProseSelection(snapshot: ReaderSelectionSnapshot | null) {
  if (!snapshot) {
    sourceFocus.value = null;
    hlPopover.value = null;
    return;
  }
  const range = snapshot.range;
  const ranges = snapshot.canonical?.ranges ?? selectionRanges(range);
  if (ranges.length === 0) {
    hlPopover.value = null;
    return;
  }
  const quote = rangeToMarkdown(range);
  const resolvedQuote = snapshot.canonical?.resolvedText ?? resolvedSelectionQuote(ranges);
  if (!quote.trim() || !resolvedQuote.trim()) {
    hlPopover.value = null;
    return;
  }
  const rect = snapshot.rect;
  hlPopover.value = {
    contextKey: workspaceContextKey.value,
    x: rect.left + rect.width / 2,
    y: rect.top,
    anchorLid: ranges[0].lid,
    ranges,
    text: quote,
    resolvedText: resolvedQuote,
  };
}
async function confirmHighlight() {
  const p = hlPopover.value;
  if (!p || p.contextKey !== workspaceContextKey.value) {
    hlPopover.value = null;
    return;
  }
  try {
    banner.value = "";
    selectedLid.value = p.anchorLid;
    hlPopover.value = null;
    window.getSelection()?.removeAllRanges();
    const groupId = p.ranges.length > 1 ? newHighlightGroupId() : undefined;
    await Promise.all(p.ranges.map((r) => api.highlight(r.lid, { start: r.start, end: r.end }, groupId)));
    await refreshAnnotations();
  } catch (e) {
    fail(e);
  }
}
function markdownSelectionContext(popover: MarkdownSelectionPopover): SelectionContext {
  return {
    status: "resolved",
    resolution_basis: "exact",
    raw_quote: popover.text,
    resolved_quote: popover.resolvedText,
    ranges: popover.ranges.map((range) => ({
      lid: range.lid,
      range: { start: range.start, end: range.end },
    })),
  };
}
function noteSelection() {
  const p = hlPopover.value;
  if (!p || p.contextKey !== workspaceContextKey.value) {
    hlPopover.value = null;
    return;
  }
  hlPopover.value = null;
  openNewNote(p.anchorLid, "", markdownSelectionContext(p));
}
function askSelection() {
  const p = hlPopover.value;
  if (!p || p.contextKey !== workspaceContextKey.value) {
    hlPopover.value = null;
    return;
  }
  const quote = p.text.replace(/\s+/g, " ").trim();
  if (!quote) return;
  askDraft.value = { lid: p.anchorLid, quote, ...markdownSelectionContext(p) };
  selectedLid.value = p.anchorLid;
  agentInput.value = "";
  hlPopover.value = null;
  window.getSelection()?.removeAllRanges();
  workspaceRef.value?.showAssistant("agent");
}
function clearAskDraft() {
  askDraft.value = null;
}

// ── agent 对话区(外层 E agent 主入口)`[ADR-0030]` ──
interface ChatTurn {
  localAgentName?: string;
  localAgentId?: string;
  localAgentUnavailable?: boolean;
  noteReferences?: import("./generated/NoteRef").NoteRef[];
  boardFollowUp?: BoardFollowUp | null;
  boardDelivery?: BoardDelivery | null;
  boardCommits?: BoardDelivery[];
  boardIncomplete?: boolean;
  boardDeliveryError?: string | null;
  boardDeliveryErrorCode?: string;
  boardRequestId?: string;
  musicQuestion?: MusicQuestionContext | null;
  videoQuestion?: import("../../core/src/generated/VideoQuestionContext").VideoQuestionContext | null;
  hasTeaching?: boolean;
  interaction?: import("./generated/ResolvedInteraction").ResolvedInteraction | null;
  spendStop?: SpendNotice;
  teachingRef?: string | null;
  presentationFollowUp?: import('./generated/PresentationFollowUp').PresentationFollowUp | null;
  turnId: string | null;
  user: string;
  outcome: OuterOutcome | null;
  pending: boolean;
  error?: string;
  questionAnchorLid: string | null;
  questionQuote: AgentQuestionQuoteView | null;
  questionSelection: AskDraft | null;
  dispositions?: Record<string, EffectDisposition>;
  liveEffects?: { effect_id: string; effect: AgentEffect }[];
  effectLabels: string[];
  activities?: RunActivity[];
  runTrace?: TraceStep[];
  runStatus?: string;
  draft?: import("./agent-run-state").AnswerDraft | null;
  submissionUnknown?: boolean;
  requestId?: string;
  recoveryDraft?: ChatDraft;
}
interface UnknownSubmissionRecord extends UnknownSubmissionAudit {
  turn: ChatTurn;
}
type AskDraft = AskQuote;
const chat = ref<ChatTurn[]>([]);
const chatSessions = ref<AgentChatSessionSummary[]>([]);
const chatGoals = ref<ResidentGoal[]>([]);
const targetGoalId = ref<string | null>(recapProps.chatDraft?.goalId ?? null);
const activeChatSessionId = ref("");
const followUpNodes = ref<FollowUpNode[]>([]);
const preparedFollowUps = ref<FollowUpStart[]>([]);
const followUpBusy = ref(false);
const followUpError = ref('');
let switchingFollowUp = false;
const agentHistoryLoading = ref(true);
const agentHistoryError = ref<string | null>(null);
let startupHistoryRequestSeq = 0;
const agentInput = ref(recapProps.chatDraft?.message ?? "");
const presentationRequested = ref(recapProps.chatDraft?.presentationRequested ?? false);
const personalDiscussion=ref<{reference:import('./generated/NoteRef').NoteRef; replyTo?:string}|null>(null);
async function discussPersonalNote(event:Event){
  const detail=(event as CustomEvent).detail;
  if(appSurface.value!=='reader' || sending.value || agentHistoryLoading.value || agentHistoryError.value)return;
  event.preventDefault();
  const references=detail.references ?? [detail];const reference=references[0];
  personalDiscussion.value=reference?{reference}:null;notesPanel.open=false;workspaceRef.value?.showAssistant('agent');
  const message=detail.message ?? uiText('请和我一起推敲这条想法，先回应我表达的关系和疑问。');
  const turn=await submitAgentMessage(message,message,null,undefined,{note_refs:references,personal_chat_ref:detail.personal_chat_ref});
  if(turn?.turnId && reference)personalDiscussion.value={reference,replyTo:turn.turnId};
}
onMounted(()=>window.addEventListener('personal-note-discuss',discussPersonalNote));
onBeforeUnmount(()=>window.removeEventListener('personal-note-discuss',discussPersonalNote));
watch(activeChatSessionId,()=>{personalDiscussion.value=null;});
watch(agentInput, value => {
  if (publishedVideo.value && !value && videoQuestion.draft.value) videoQuestion.input('');
});
const closedBook = ref<ClosedBookState | null>(null);
const closedReady = ref(false), closedBusy = ref(false), closedError = ref(''), closedDraft = ref('');
const closedFailedRequest = ref('');
const closedCover = computed(() => appSurface.value === 'reader' && (!closedReady.value || ['active','paused','complete'].includes(closedBook.value?.status ?? '')));
const closedMessages = computed(() => {
  const state = closedBook.value;
  if (!state || state.needs_retry) return [];
  return state.visible_turn_ids.flatMap(id => {
    const turn = chat.value.find(t => t.turnId === id);
    if (!turn?.outcome || turn.pending) return [];
    const parts = turn.outcome.answer_view?.parts;
    const answer = parts?.filter(p => p.kind === 'markdown' || p.kind === 'socratic_question')
      .map(p => 'text' in p ? p.text : '').join('\n\n') || turn.outcome.answer || '';
    return [{ id, user: turn.user, answer }];
  });
});
const closedDraftKey = () => stableClosedBookDraftKey(network.value.identity?.user_id ?? 'local',activeChatSessionId.value,closedBook.value);
const closedMutationKey = () => closedBookMutationKey(network.value.identity?.user_id ?? 'local',activeChatSessionId.value);
function changeClosedDraft(value: string) { closedDraft.value=value; if (closedBook.value) localStorage.setItem(closedDraftKey(),value); }
let closedReadSequence = 0;
async function refreshClosedBook() {
  const chatId=activeChatSessionId.value, sequence=++closedReadSequence;
  if (!chatId) { closedReady.value=true; return; }
  try {
    const value=await api.closedBook({action:'state',chat_session_id:chatId});
    if (sequence!==closedReadSequence || activeChatSessionId.value!==chatId) return;
    const previousDraftKey=closedDraftKey();
    const submitted=localStorage.getItem(`${previousDraftKey}:pending-turn`);
    if (submitted && value?.visible_turn_ids.includes(submitted)) {
      localStorage.removeItem(previousDraftKey); localStorage.removeItem(`${previousDraftKey}:pending-turn`); closedDraft.value='';
    }
    const changed=value?.id!==closedBook.value?.id || value?.attempt!==closedBook.value?.attempt;
    closedBook.value=value;
    const pending=pendingClosedBookMutation(localStorage,closedMutationKey());
    closedError.value=pending && !closedBusy.value ? uiText('上次操作尚未确认，请重试。') : '';
    const restoredSubmission=localStorage.getItem(`${closedDraftKey()}:pending-turn`);
    if (restoredSubmission && value?.visible_turn_ids.includes(restoredSubmission)) {
      localStorage.removeItem(closedDraftKey()); localStorage.removeItem(`${closedDraftKey()}:pending-turn`); closedDraft.value='';
    }
    if (changed || !closedDraft.value) closedDraft.value=localStorage.getItem(closedDraftKey()) ?? value?.draft ?? '';
    closedReady.value=!!value || !pending;
  } catch (error) { if (sequence===closedReadSequence) {
    closedError.value=errorMessage(error);
    closedReady.value=false;
  } }
}
async function mutateClosedBook(action: string, extra: { target?: string; course_run_id?: string; answer_turn_id?: string } = {}, message?: string) {
  const key=closedMutationKey();
  const result=await executeClosedBookMutation(localStorage,key,api.closedBook,()=>({
    request:{action,chat_session_id:activeChatSessionId.value,operation_id:crypto.randomUUID(),revision:closedBook.value?.revision ?? 0,draft:closedDraft.value,...extra},message,
  }));
  if(key!==closedMutationKey() || disposed)throw new ApiError(409,'CLIENT_CONTEXT_STALE','conflict',uiText('阅读现场已切换'));
  closedBook.value=result.value; closedReady.value=true;
  if(result.conflict)throw new Error(uiText('练习进度已更新，请重试刚才的操作。'));
  return result.operation;
}
function hasPendingClosedBookMutation() {
  if(!pendingClosedBookMutation(localStorage,closedMutationKey()))return false;
  closedError.value=uiText('上次操作尚未确认，请重试。');return true;
}
async function deliverClosedBookMessage(text: string) {
  const turn=await submitAgentMessage(text,text,null,undefined,{},undefined,{});
  if(turn?.error)throw new Error(turn.error);
  if(turn?.turnId && closedDraft.value.trim()===text.trim()) {
    localStorage.setItem(`${closedDraftKey()}:pending-turn`,turn.turnId);
    void refreshClosedBook();
  }
}
async function sendClosedBook(text: string) {
  if (sending.value || closedBusy.value || !closedBook.value) return;
  if(hasPendingClosedBookMutation())return;
  closedError.value=''; closedFailedRequest.value=''; closedBusy.value=true;
  try {
    await mutateClosedBook('draft',{},text);
    await deliverClosedBookMessage(text);
  } catch(error) { closedError.value=errorMessage(error); closedFailedRequest.value=text; }
  finally { closedBusy.value=false; }
}
async function startClosedBook(answerTurnId?: string) {
  if (sending.value || closedBusy.value || !activeChatSessionId.value) return;
  if(hasPendingClosedBookMutation())return;
  tutorPanelOpen.value=false; closedError.value=''; closedBusy.value=true;
  try {
    const saved=closedBook.value;
    const sameOwner=saved && (courseMode.value ? saved.course && saved.owner_id===courseLesson.value?.progress.run_id
      && saved.course_origin?.round_index===courseLesson.value?.round_index && saved.course_origin?.stage===courseLesson.value?.progress.stage
      : !saved.course && (!answerTurnId || saved.answer_origin?.turn_id===answerTurnId));
    if (sameOwner && saved.can_resume && !saved.completion_reason && (saved.status==='paused' || saved.status==='ended')
      && saved.source_revision===noteSourceFingerprint.value) await mutateClosedBook('resume');
    else { closedDraft.value=''; await mutateClosedBook('start',courseMode.value && courseLesson.value ?
      {course_run_id:courseLesson.value.progress.run_id} : answerTurnId ? {answer_turn_id:answerTurnId} : {}); }
    await rememberLearningTask();
  } catch(error) { closedError.value=errorMessage(error); banner.value=closedError.value; }
  finally { closedBusy.value=false; }
  if(!closedError.value)await askFirstClosedBookQuestion();
}
async function askFirstClosedBookQuestion() {
  if (closedBook.value?.status==='active' && (!closedBook.value.visible_turn_ids.length || closedBook.value.needs_retry))
    await sendClosedBook(closedBook.value.needs_retry ? '我回来了，请换一个同目标的新问题，让我独立尝试。' : '开启闭卷练到会。请根据我们刚学过的当前重点提出第一个问题，等我自己回答。');
}
async function helpClosedBook(kind: 'hint' | 'explain') {
  if (closedBusy.value || sending.value) return;
  await sendClosedBook(kind==='hint' ? '我卡住了，请给一点提示。' : '这一步我还没明白，请重新讲解。');
}
async function leaveClosedBook(kind: 'pause' | 'exit', signal?:AbortSignal) {
  if (closedBusy.value) return;
  if(hasPendingClosedBookMutation())return;
  closedBusy.value=true;
  try {
    signal?.throwIfAborted();
    const completed = kind === 'exit' && closedBook.value?.status === 'complete' && closedBook.value.course ? closedBook.value : null;
    if (completed) {
      if (!completed.result_ref) throw new Error(uiText('本轮结果尚未交付，请重试读取。'));
      if (courseLesson.value?.progress.run_id !== completed.owner_id) await openCourse({runId:completed.owner_id,record:false},signal);
      if (!coursePlayer.value) throw new Error(uiText('原课程暂时无法打开，请重试。'));
      courseChanged(await coursePlayer.value.applyClosedBookResult(completed.result_ref));
      signal?.throwIfAborted();
    }
    await mutateClosedBook(kind);
    signal?.throwIfAborted();
    if (sending.value) await stopResidentRun();
    closedError.value='';
    if (completed) { await rememberLearningTask({course_run_id:completed.owner_id}); signal?.throwIfAborted(); returnToCourse(); }
  }
  catch(error) { closedError.value=errorMessage(error); }
  finally { closedBusy.value=false; }
}
async function retryClosedBook() {
  if(closedBusy.value)return;
  const pending=pendingClosedBookMutation(localStorage,closedMutationKey());
  if(pending) {
    closedBusy.value=true;closedError.value='';
    try {
      const operation=await mutateClosedBook(pending.request.action);
      if(operation.message) { closedFailedRequest.value=operation.message;await deliverClosedBookMessage(operation.message);closedFailedRequest.value=''; }
      else if(['pause','exit'].includes(operation.request.action) && sending.value)await stopResidentRun();
      if(operation.request.action==='exit' && closedBook.value?.course && closedBook.value.completion_reason) {
        if(courseLesson.value?.progress.run_id!==closedBook.value.owner_id) await openCourse({runId:closedBook.value.owner_id,record:false});
        await rememberLearningTask({course_run_id:closedBook.value.owner_id}); returnToCourse();
      }
    } catch(error) { closedError.value=errorMessage(error); }
    finally { closedBusy.value=false; }
    if(!closedError.value && ['start','resume'].includes(pending.request.action))await askFirstClosedBookQuestion();
    return;
  }
  if (closedBook.value?.status === 'complete' && closedBook.value.course) { await leaveClosedBook('exit'); return; }
  if (!closedReady.value || !closedBook.value) { await refreshClosedBook(); return; }
  await sendClosedBook(closedFailedRequest.value || closedDraft.value.trim() || (closedBook.value.needs_retry ? '请换一个同目标的新问题，让我独立尝试。' : '请继续当前闭卷练习。'));
}
watch(activeChatSessionId, () => { closedReady.value=false; closedBook.value=null; closedDraft.value=''; closedFailedRequest.value=''; void refreshClosedBook(); });


const tutor = useTutorControl();
const tutorPanelOpen = ref(false);
const tutorIntentDraft=ref('');
const understandingOpen = ref(false);
function openUnderstanding() { tutorPanelOpen.value=false;closeBoard();understandingOpen.value=true; }
function closeUnderstanding() { understandingOpen.value=false; }
watch(understandingOpen,visible=>recapEmit('understanding-visibility',visible));
watch(pendingUnderstandingAction,action=>{if(action)understandingOpen.value=false;});
onBeforeUnmount(()=>recapEmit('understanding-visibility',false));
const tutorPanel = ref<InstanceType<typeof TutorPanel> | null>(null);
const tutorPanelView = ref<'current' | 'understanding' | 'history' | 'knowledge'>('current');
function openTutorPanel(view: 'current' | 'understanding' | 'history' | 'knowledge' = 'current') {
  if(view==='understanding'||view==='knowledge'){openUnderstanding();return;}
  tutorPanelView.value = view;
  tutorPanel.value?.openView(view);
  tutorPanelOpen.value = true;
}
const presentationWorkspace = ref<import('./presentation-workspace').PresentationWorkspace | null>(null);
watch(() => [recapProps.recapTarget, agentHistoryLoading.value, rightRailRef.value] as const, async ([target, loading, rail]) => {
  if (!target || loading || !rail) return;
  recapEmit('recap-consumed');
  try {
    if (target.session_id !== activeChatSessionId.value) applyAgentHistory(await api.agentHistorySelect(target.session_id));
    await nextTick();
    workspaceRef.value?.showAssistant('agent');
    await rightRailRef.value?.openRecapTarget(target);
  } catch (failure) { fail(failure); }
});
const askDraft = ref<AskDraft | null>(recapProps.chatDraft?.quote ?? null);
const learningScopeOptions = computed<LearningScopeOption[]>(() => mediaMode.value ? [
  ...(musicMode.value || videoFrameReady.value || videoQuestion.draft.value ? [{ value: 'media' as const,
    label: musicMode.value ? musicQuestionContext.value ? uiText('当前选中的音乐片段') : uiText('当前听到的片段')
      : videoQuestion.draft.value?.context.range ? uiText('当前选中的视频片段') : videoQuestion.draft.value?.context.region ? uiText('当前框选的画面') : uiText('当前播放位置') }] : []),
  { value: 'material', label: musicMode.value ? uiText('整首作品与资料') : uiText('整段视频') },
] : [
  ...(askDraft.value?.quote.trim() && askDraft.value.status !== 'partial' ? [{ value: 'selection' as const, label: uiText('当前选中的原文') }] : []),
  ...(guideChapter.value ? [{ value: guideChapter.value.kind === 'section' ? 'section' as const : 'chapter' as const,
    lid: guideChapter.value.lid, label: uiText(guideChapter.value.kind === 'section' ? '当前小节 · {0}' : '当前章节 · {0}', [guideChapter.value.title]) }] : []),
  { value: 'material', label: recapProps.materialTitle ? uiText('整份材料 · {0}', [recapProps.materialTitle]) : uiText('整份材料') },
]);
const learningPurposeAvailable = computed(() => !!buildWorkbenchSnapshot.value?.book_id && !!noteSourceFingerprint.value
  && (!videoMode.value || !!publishedVideo.value) && !agentHistoryLoading.value && !agentHistoryError.value);
const learningPurposeReady = computed(() => !!tutor.state.value && !tutor.busy.value && !tutor.pending.value
  && tutor.readiness.value?.status === 'ready' && !tutor.readinessError.value);
const learningPurposeReason = computed(() => agentHistoryError.value || (agentHistoryLoading.value ? uiText('正在恢复学习对话。') : '')
  || (videoMode.value && !publishedVideo.value ? videoDetailsError.value || uiText('正在准备视频字幕。') : '')
  || (!learningPurposeAvailable.value ? uiText('正在读取学习资料。') : '')
  || tutor.readinessError.value || (tutor.readiness.value?.status !== 'ready' ? tutor.readiness.value?.reason || uiText('正在读取学习状态。') : ''));
provideReadingGuidance(computed(() => learningPurposeAvailable.value ? {
  key: `${readerKey()}:${buildWorkbenchSnapshot.value?.book_id}:${activeChatSessionId.value}`,
  title: recapProps.materialTitle ?? '', scopes: learningScopeOptions.value,
  selection: askDraft.value ? { lid: askDraft.value.lid, quote: askDraft.value.quote } : null,
  intent: tutor.current.value?.user_intent ?? '',
  medium: musicMode.value ? 'music' : videoMode.value ? 'video' : 'text',
} : null));
type LearningMediaContext = { video?: VideoQuestionContext; music?: MusicQuestionContext };
function captureLearningMedia(): LearningMediaContext {
  if (musicMode.value) {
    const context = musicQuestionContext.value ?? musicPane.value?.captureQuestion();
    if (!context) throw new Error(uiText('请等待音乐加载后再开始学习。'));
    return { music: { ...structuredClone(toRaw(context)), draft_id: crypto.randomUUID() } };
  }
  if (videoMode.value) {
    const context = videoQuestion.draft.value?.context ?? videoPane.value?.captureQuestion(crypto.randomUUID());
    if (!context) throw new Error(uiText('请等待视频画面加载后再开始学习。'));
    return { video: { ...structuredClone(toRaw(context)), draft_id: crypto.randomUUID() } };
  }
  return {};
}
watch([agentInput, askDraft, targetGoalId, presentationRequested], ([message, quote, goalId, presentationRequested]) => {
  if (!disposed && !switchingFollowUp) {
    const draft = { message, quote, goalId, presentationRequested };
    recapEmit('update:chatDraft', draft);
    if (localAgentHost.value || (followUpNodes.value.some(node => node.session_id === activeChatSessionId.value) && followUpNodes.value.some(node => node.origin)))
      saveLearningDraft(followUpDraftKey(activeChatSessionId.value), draft);
  }
}, { flush: 'sync' });
const unknownSubmissionsBySession = new Map<string, UnknownSubmissionRecord[]>();
const workspaceAuxTab = ref<WorkspaceAuxTab | null>(null);
const workspaceAuxTabRevision = ref(0);
const agentFullscreen = ref(false);
const agentFullscreenSuspendedForSource = ref(false);
const mobileWorkspaceEnabled = import.meta.env.VITE_MOBILE_WORKSPACE !== "0";
const noteMaterialKey = computed(() => `${network.value.enabled ? readerKey() : 'local'}:${buildWorkbenchSnapshot.value?.book_id ?? 'no-book'}`);
const shareDraft = ref<{ source: ShareSource; loading: boolean } | null>(null);
let shareReturnFocus: HTMLElement | null = null;
let shareOperation = 0;
provide(sharePresentationKey, async load => {
  const operation = ++shareOperation, scope = noteMaterialKey.value, chatId = activeChatSessionId.value;
  const target = document.activeElement instanceof HTMLElement ? document.activeElement : null;
  const source = await load();
  if (disposed || operation !== shareOperation || scope !== noteMaterialKey.value || chatId !== activeChatSessionId.value) return;
  shareReturnFocus = target; shareDraft.value = { source, loading: false };
});
function closeShare() {
  shareOperation++;
  shareDraft.value = null;
  const target = shareReturnFocus; shareReturnFocus = null;
  void nextTick(() => { if (target?.isConnected) target.focus({ preventScroll: true }); });
}
provide(shareNoteKey, async (requested) => {
  const record = annotations.value.find(record => record.mem_id === requested.mem_id && record.type === 'note');
  if (!record) { banner.value = uiText('这条笔记当前不可用，请重新打开笔记。'); return; }
  const bookId = buildWorkbenchSnapshot.value?.book_id;
  if (record.book_id !== bookId) return;
  shareReturnFocus = document.activeElement instanceof HTMLElement ? document.activeElement : null;
  const labels = [...new Set((record.note?.source_bindings ?? []).map(source => source.label_snapshot).filter(Boolean))];
  const association = record.note?.association.kind;
  // Reanchoring an answer/presentation is a display position, not its original source.
  const bodySource = !record.note || association === 'selection' || association === 'body_placement';
  if (bodySource && record.anchor.lid) {
    const placement = record.note_placement;
    const entry = pdfEntryByLid.value.get(record.anchor.lid);
    const pageIndex = placement?.kind === 'pdf_region' ? placement.page_index : entry?.primary_region?.pageIndex ?? entry?.regions[0]?.pageIndex;
    const label = pageIndex !== undefined ? uiText('第 {0} 页', [pdfSourceMap.value?.pages.find(page => page.pageIndex === pageIndex)?.page_label ?? pageIndex + 1])
      : noteSourceLabels.value[record.mem_id];
    if (label && label !== uiText('来源未知') && !labels.includes(label)) labels.push(label);
  }
  const hasSource = labels.length > 0 || (bodySource && !!record.anchor.lid);
  void beginShare(noteShareSource(record, hasSource ? labels : [uiText('无已记录出处')]), hasSource);
});
async function beginShare(source: ShareSource, hasSource = true) {
  shareOperation++;
  shareReturnFocus = document.activeElement instanceof HTMLElement ? document.activeElement : null;
  shareDraft.value = { source, loading: hasSource };
  if (!hasSource) return;
  const scope = noteMaterialKey.value, chatId = activeChatSessionId.value;
  const active = shareDraft.value;
  const stillCurrent = () => !disposed && shareDraft.value === active && scope === noteMaterialKey.value && chatId === activeChatSessionId.value;
  const bookId = buildWorkbenchSnapshot.value?.book_id;
  let title = buildWorkbenchSnapshot.value?.input?.manifest?.display_title
    || bookPickerBooks.value.find(book => book.book_id === bookId)?.name || '';
  try {
    const metadata = await api.paperMetadata();
    if (!stillCurrent()) return;
    title = metadata.title?.value || title;
  } catch { if (!stillCurrent()) return; }
  if (!stillCurrent()) return;
  if (!title) {
    try {
      const library = await api.bookLibrary();
      if (!stillCurrent()) return;
      title = library.books.find(book => book.book_id === bookId)?.name ?? '';
    } catch { if (!stillCurrent()) return; }
  }
  if (!stillCurrent()) return;
  active.source.sources = [title ? `《${title}》` : uiText('材料名称暂不可用'), ...source.sources];
  active.loading = false;
}
function excerptLabels(lids: string[]): string[] {
  return [...new Set(lids.map(lid => {
    const entry = pdfEntryByLid.value.get(lid);
    const pages = entry?.regions.map(region => region.pageIndex) ?? [];
    if (pages.length) return [...new Set(pages)].map(index => uiText('第 {0} 页', [pdfSourceMap.value?.pages.find(page => page.pageIndex === index)?.page_label ?? index + 1])).join('、');
    return chapterLabelForNote(lid);
  }).filter(Boolean))];
}
function chapterLabelForNote(lid: string): string {
  while (lid) {
    const title = titleByLid.value.get(lid);
    if (title) return stripMarkdownHeadingLine(title);
    lid = lid.includes('.') ? lid.slice(0, lid.lastIndexOf('.')) : '';
  }
  return uiText('来源位置暂不可用');
}
function shareSelection() {
  const selected = hlPopover.value;
  if (!selected || selected.contextKey !== workspaceContextKey.value) return;
  void beginShare(excerptShareSource(selected.text, excerptLabels(selected.ranges.map(range => range.lid))));
  hlPopover.value = null;
}
function sharePdfSelection() {
  const draft = pdfSelectionState.value.draft;
  if (!draft || draft.status !== 'resolved') return;
  const pages = [...new Set(pdfSelectionState.value.capture?.rects.map(rect => rect.pageIndex) ?? [])];
  const labels = pages.map(index => uiText('第 {0} 页', [pdfSourceMap.value?.pages.find(page => page.pageIndex === index)?.page_label ?? index + 1]));
  void beginShare(excerptShareSource(draft.raw_quote, labels.length ? labels : excerptLabels(draft.ranges.map(range => range.lid)), 'plain'));
  pdfSelectionSession.cancel();
}
provide(shareHighlightKey, requested => {
  const record = annotations.value.find(record => record.mem_id === requested.mem_id && record.type === 'highlight');
  if (!record || record.book_id !== buildWorkbenchSnapshot.value?.book_id) return;
  const members = highlightGroupMembers(record);
  const pages = [...new Set(members.flatMap(member => pdfAnnotationProjection.value.highlights.find(highlight => highlight.mem_id === member.mem_id)?.rects.map(rect => rect.pageIndex) ?? []))];
  const labels = pages.map(index => uiText('第 {0} 页', [pdfSourceMap.value?.pages.find(page => page.pageIndex === index)?.page_label ?? index + 1]));
  void beginShare(excerptShareSource(members.map(member => member.content).join('\n'), labels.length ? labels : excerptLabels(members.map(member => member.anchor.lid).filter((lid): lid is string => !!lid))));
});
watch([noteMaterialKey, activeChatSessionId], () => { shareOperation++; shareDraft.value = null; shareReturnFocus = null; }, { flush: 'sync' });
watch(noteMaterialKey, clearNoteScope, { flush: 'sync' });
function clearNoteScope() {
  noteEditor.value = null; noteCollapsed.value = false; noteError.value = '';
  annotations.value = [];
  const pending = noteTransition.value; noteTransition.value = null; pending?.resolve(false);
}
function noteBeforeUnload(event: BeforeUnloadEvent) {
  if (noteDirty.value || noteSaving.value) { event.preventDefault(); event.returnValue = ''; }
}
function collapseNoteOutside(event: PointerEvent) {
  if (!noteEditor.value || noteTransition.value || noteCollapsed.value) return;
  if (event.target instanceof Element && !event.target.closest('.note-editor-panel, .note-resume, .share-image-panel, [data-share-note]')) noteCollapsed.value = true;
}
const chatSceneKey = computed(() => network.value.enabled ? `${readerKey()}:${network.value.workspace?.generation}` : 'local');
const workspaceContextKey = computed(() => `${chatSceneKey.value}:${buildWorkbenchSnapshot.value?.book_id ?? "no-book"}:${activeChatSessionId.value || "no-chat"}`);
const focusPaginationSource = computed(() => {
  // Rebuild on the existing scene/source identities; no new source identity.
  void workspaceContextKey.value; void noteSourceFingerprint.value;
  const assets = imageAssetByLid.value;
  return markRaw(new FocusSource(sourceManifestTree.value?.tree ?? [], api.text, lid => assets.get(lid)));
});
function onPaginationActive(active: boolean) {
  focusPaginationActive.value = active;
  if (active) segments.value = [];
  else focusVisible.value = null;
}
function onFocusVisible(value: NonNullable<typeof focusVisible.value>) {
  focusVisible.value = value;
  displayReadingPoint = { point: { contextKey: workspaceContextKey.value, turnId: '', anchor: { surface: 'markdown', ...value.position } }, interaction: readerInteractionRevision, navigation: readerNavigationRequest };
  selectedLid.value = null;
  hlPopover.value = null;
}
async function restoreVisiblePosition(position: import('./api').VisibleReadingPosition) {
  const anchor: ScrollAnchor = { lid: position.lid, top: 0,
    ...(position.text_position ? { textPosition: { ...position.text_position, top: 0 } } : {}) };
  displayReadingPoint = { point: { contextKey: workspaceContextKey.value, turnId: '', anchor: { surface: 'markdown', ...anchor } }, interaction: readerInteractionRevision, navigation: readerNavigationRequest };
  await readerPaneRef.value?.restoreScrollAnchor(anchor);
}
let pendingFocusPosition: { value: import('./api').VisibleReadingPosition; context: string } | null = null;
let focusPositionTimer: ReturnType<typeof setTimeout> | undefined;
let focusPositionWrite: Promise<void> | null = null;
function confirmFocusPosition() {
  const visible = focusVisible.value;
  if (!visible || !focusPaginationActive.value) return;
  const p = visible.position;
  pendingFocusPosition = { context: workspaceContextKey.value, value: { lid: p.lid,
    ...(p.textPosition ? { text_position: { start: p.textPosition.start, end: p.textPosition.end, text: p.textPosition.text } } : {}), ranges: visible.ranges } };
  clearTimeout(focusPositionTimer);
  focusPositionTimer = setTimeout(() => { void flushFocusPosition(); }, 0);
}
async function flushFocusPosition(): Promise<void> {
  clearTimeout(focusPositionTimer);
  if (focusPositionWrite) { await focusPositionWrite; if (pendingFocusPosition) await flushFocusPosition(); return; }
  const pending = pendingFocusPosition; pendingFocusPosition = null;
  if (!pending || disposed || pending.context !== workspaceContextKey.value) return;
  const navigation = readerNavigationRequest;
  focusPositionWrite = api.confirmReadingPosition(pending.value).then(effect => {
    if (pending.context === workspaceContextKey.value && navigation === readerNavigationRequest && !pendingFocusPosition) viewport.value = effect.viewport;
  }).catch(error => { if (pending.context === workspaceContextKey.value && navigation === readerNavigationRequest) fail(error); }).finally(() => { focusPositionWrite = null; });
  await focusPositionWrite;
  if (pendingFocusPosition) await flushFocusPosition();
}
watch(workspaceContextKey, () => { pendingFocusPosition = null; focusVisible.value = null; clearTimeout(focusPositionTimer); });
onBeforeUnmount(() => { clearTimeout(focusPositionTimer); pendingFocusPosition = null; });
let focusTransition = 0;
async function onFocusChange(active: boolean) {
  const request = ++focusTransition, context = workspaceContextKey.value;
  focusReading.value = active;
  if (activeReaderSurface.value !== 'markdown') return;
  if (active) {
    const anchor = displayReadingPoint?.point.anchor;
    const index = anchor?.surface === 'markdown' ? focusPaginationSource.value.leaves.findIndex(n => n.lid === anchor.lid) : -1;
    await nextTick();
    if (request === focusTransition) await readerPaneRef.value?.prepareFocus(index < 0 ? undefined : { index, offset: anchor?.surface === 'markdown' ? anchor.textPosition?.start ?? 0 : 0 });
  } else if (focusPaginationActive.value) {
    const anchor = readerPaneRef.value?.captureScrollAnchor([]) ?? null;
    readerPaneRef.value?.suspendFocus();
    const index = focusPaginationSource.value.leaves.findIndex(n => n.lid === anchor?.lid);
    const lastVisible = focusPaginationSource.value.leaves.findIndex(n => n.lid === focusVisible.value?.ranges.at(-1)?.lid);
    const lids = focusPaginationSource.value.leaves.slice(Math.max(0, index - 4), Math.max(index + 12, lastVisible + 1)).map(n => n.lid);
    try {
      const loaded = await hydrateSegments(lids);
      if (request !== focusTransition || context !== workspaceContextKey.value) return;
      segments.value = loaded;
      await nextTick(); await readerPaneRef.value?.leaveFocus(anchor);
    } catch (error) { if (request === focusTransition) fail(error); }
  }
}
function onFocusTool(tool: string) {
  if (tool === 'return') void returnToAgentAnswer();
  else if (tool === 'outline') workspaceRef.value?.toggleOutline();
  else if (tool === 'settings') typography.open.value = true;
  else if (tool === 'surface') selectReaderSurface(activeReaderSurface.value === 'pdf' ? 'markdown' : 'pdf');
  else if (tool === 'exit') workspaceRef.value?.toggleFocus();
  else if (tool === 'agent' || tool === 'notes') workspaceRef.value?.showAssistant(tool);
}
watch(workspaceContextKey, closeFormulaDialog);
watch(noteSourceFingerprint, closeFormulaDialog);
const readerContextKey = computed(() => `${network.value.enabled ? readerKey() : "local"}:${buildWorkbenchSnapshot.value?.book_id ?? "no-book"}`);
watch(readerContextKey,()=>{tutorIntentDraft.value='';});
const yeyeFeedback = createYeyeFeedback();
watch(workspaceContextKey, () => yeyeFeedback.reset(), { flush: 'sync' });
const workspaceLogicalState = computed<WorkspaceLogicalState>(() => ({
  // Reader layout belongs to the book; recovering a chat must not replay saved focus.
  contextKey: buildWorkbenchSnapshot.value?.book_id ?? "no-book",
  revision: readerLayout.value?.rev ?? 0n,
  activePreset: readerLayout.value?.active_preset ?? null,
  openSlots: readerLayout.value?.open_slots ?? [],
  focusedSlot: readerLayout.value?.focused_slot ?? null,
}));
const readingContinuity = createReadingContinuity();
const initialBoardRoute = readBoardRoute(location.search);
const boardId = ref<string | null>(initialBoardRoute?.boardId ?? null);
const boardMounted = ref(!!boardId.value);
const boardVisible = ref(!!boardId.value);
watch(boardVisible, visible => recapEmit('board-visibility', visible), { immediate: true });
const boardPendingImport = ref<BoardImportSource | null>(null);
const boardPendingLocate = ref<BoardLocation | null>(initialBoardRoute?.itemId ? {
  board_id: initialBoardRoute.boardId, item_id: initialBoardRoute.itemId, block_id: initialBoardRoute.blockId,
} : null);
const boardWorkspaceRef = ref<InstanceType<typeof BoardWorkspace>>();
const notesWorkspaceRef = ref<InstanceType<typeof NotesWorkspace>>();
const notesVisible = ref(false), selectedWorkspaceNote = ref<string | null>(null), notesRevision = ref(0);
const workspaceNoteImporting = ref(false), workspaceNoteError = ref('');
const workspaceNotePreview = ref<SourcePopupView | null>(null);
const workspaceNoteSource = ref<WorkspaceNoteSourceRequest | null>(null);
const readWorkspaceNoteSource = computed<SourceWindowLoader | undefined>(() => {
  const request = workspaceNoteSource.value;
  return request ? options => api.workspaceNoteSourceRead({ ...request, ...options }) : undefined;
});
const workspaceNoteQuestion = ref<{ noteId: string; title: string } | null>(null);
async function openWorkspaceNotes(noteId?: string) {
  if (!await beforeNoteLeave()) return;
  selectedWorkspaceNote.value = noteId ?? null;
  notesVisible.value = true; workspaceNoteError.value = '';
  mobileGlobalActionsOpen.value = false;
}
function workspaceNotesChanged() { notesRevision.value += 1; }
async function closeWorkspaceNotes() {
  if (notesWorkspaceRef.value && !await notesWorkspaceRef.value.beforeLeave()) return;
  notesVisible.value = false; workspaceNotesChanged();
}
async function importWorkspaceNote(source: WorkspaceNoteImportSource) {
  if (workspaceNoteImporting.value || !await beforeNoteLeave()) return;
  workspaceNoteImporting.value = true;
  try {
    const note = await api.workspaceNoteImport({ source });
    workspaceNotesChanged(); await openWorkspaceNotes(note.note_id);
  } catch (failure) { fail(failure); }
  finally { workspaceNoteImporting.value = false; }
}
async function boardFromWorkspaceNote({ noteId }: { noteId: string }) {
  await closeWorkspaceNotes();
  if (!notesVisible.value) openBoard({ kind: 'note', note_id: noteId });
}
async function prepareWorkspaceNoteQuestion(note: { noteId: string; title: string }) {
  await closeWorkspaceNotes();
  if (notesVisible.value) return;
  boardVisible.value = false;
  workspaceNoteQuestion.value = note; askDraft.value = null; targetGoalId.value = null;
  agentInput.value = uiText('帮我梳理「{0}」，看看它和空间内哪些笔记有关，并解释原因。', [note.title]);
  workspaceRef.value?.showAssistant('agent');
}
async function openWorkspaceNoteSource(request: WorkspaceNoteSourceRequest) {
  workspaceNoteError.value = '';
  try {
    const source = await api.workspaceNoteSourceResolve(request);
    // Source previews retain the original excerpt, including cross-material references.
    workspaceNoteSource.value = request;
    workspaceNotePreview.value = source;
  } catch (failure) { workspaceNoteError.value = errorMessage(failure); }
}
async function navigateWorkspaceNoteSource(request = workspaceNoteSource.value) {
  if (!request || (notesWorkspaceRef.value && !await notesWorkspaceRef.value.beforeLeave())) return;
  try {
    const source = await api.workspaceNoteSourceResolve(request);
    if (source.stale || !source.can_open_in_reader) { workspaceNotePreview.value = source; return; }
    if (source.media) {
      const current = (videoPlayback.value ?? publishedVideo.value)?.publication;
      if (!videoPane.value || current?.book_id !== source.media.publication.book_id || current.publication_id !== source.media.publication.publication_id) {
        workspaceNotePreview.value = source; return;
      }
      videoPane.value.seek(mediaSourceStart(source.media));
    } else if (source.music) {
      if (!musicPane.value || musicContext.value?.publication.book_id !== source.music.target.publication.book_id || musicContext.value.publication.publication_id !== source.music.target.publication.publication_id) {
        workspaceNotePreview.value = source; return;
      }
      musicPane.value.openSource(source.music.target);
    } else {
      const opened = await api.workspaceNoteSourceOpen(request);
      if (opened.source_url) { window.open(opened.source_url, '_blank', 'noopener,noreferrer'); return; }
      await syncViewport(true, true);
    }
    workspaceNotePreview.value = null; notesVisible.value = false; boardVisible.value = false;
    workspaceRef.value?.showReader();
  } catch (failure) {
    workspaceNoteError.value = errorMessage(failure); workspaceNotePreview.value = null;
    if (!notesVisible.value) fail(failure);
  }
}
const boardSourceError = ref('');
const boardSourcePreview = ref<SourcePopupView | null>(null);
const boardSourceRequest = ref<BoardSourceRequest | null>(null);
const boardSourceLoading = ref(false), boardSourceOpening = ref(false), boardSourcePreviewError = ref('');
let boardSourceSequence = 0;
let boardSourceOpenLid: string | undefined;
const boardSourceCanOpen = computed(() => {
  const source = boardSourcePreview.value;
  if (!source || source.stale || !source.can_open_in_reader) return false;
  if (source.media) {
    const current = (videoPlayback.value ?? publishedVideo.value)?.publication, target = source.media.publication;
    return !!videoPane.value && current?.book_id === target.book_id && current.publication_id === target.publication_id && Number.isFinite(mediaSourceStart(source.media));
  }
  if (source.music) {
    const current = musicContext.value?.publication, target = source.music.target.publication;
    return !!musicPane.value && current?.book_id === target.book_id && current.publication_id === target.publication_id;
  }
  return true;
});
function closeBoardSourcePreview() {
  boardSourceSequence++;
  boardSourceRequest.value = null; boardSourcePreview.value = null; boardSourcePreviewError.value = '';
  boardSourceLoading.value = false; boardSourceOpening.value = false;
  boardSourceOpenLid = undefined;
}
onBeforeUnmount(closeBoardSourcePreview);
let unregisterThoughtContext: (() => void) | undefined;
onMounted(() => {
  unregisterThoughtContext = registerThoughtContext(() => {
    if (!readerVisible.value || understandingOpen.value || appSurface.value !== 'reader' || !buildWorkbenchSnapshot.value?.book_id) return null;
    // Boards and presentations have explicit capture entries; never attach to a hidden reading page.
    if (boardVisible.value || boardSourcePreview.value || (presentationWorkspace.value && !presentationWorkspace.value.suspended) || agentFullscreen.value || !readerRegionVisible.value) return null;
    const context = {
      book_id: buildWorkbenchSnapshot.value.book_id,
      publication_id: network.value.workspace?.published_book_ref.publication_id ?? null,
      workspace_id: network.value.workspace?.learning_workspace_id ?? null,
    };
    if (videoMode.value) {
      const moment = videoPane.value?.thoughtContext();
      return moment ? { ...context, ...moment } : context;
    }
    const lid = activeReaderSurface.value === 'pdf' ? pdfViewportAnchorLid.value : readerPaneRef.value?.captureScrollAnchor(segments.value.map(segment => segment.lid))?.lid ?? readingAnchorLid.value;
    return { ...context, lid: lid ?? null, label: [recapProps.materialTitle, activeChapterTitle.value].filter(Boolean).join(' · ') || '当前阅读位置' };
  });
});
onBeforeUnmount(() => unregisterThoughtContext?.());
function writeBoardRoute(id: string | null, target?: BoardLocation) {
  const url = boardRouteUrl(location.href, id ? { boardId: id,
    publication: network.value.workspace?.published_book_ref ?? null,
    learningWorkspaceId: network.value.workspace?.learning_workspace_id ?? null,
    itemId: target?.item_id ?? undefined, blockId: target?.block_id ?? undefined } : null);
  history.replaceState(history.state, '', url);
}
function openBoard(source?: BoardImportSource) {
  boardPendingImport.value = source ?? null;
  boardMounted.value = true; boardVisible.value = true; boardSourceError.value = '';
  tutorPanelOpen.value = false; mobileGlobalActionsOpen.value = false;
  if (boardId.value) writeBoardRoute(boardId.value);
}
function boardOpened(id: string) { boardId.value = id; writeBoardRoute(id, boardPendingLocate.value ?? undefined); }
async function openBoardLocation(target: BoardLocation) {
  try {
    if (boardMounted.value && boardId.value !== target.board_id && await boardWorkspaceRef.value?.flush() === false) return;
    boardId.value = target.board_id; boardPendingLocate.value = { ...target };
    openBoard(); writeBoardRoute(target.board_id, target);
  } catch (failure) { boardSourceError.value = errorMessage(failure); }
}
function openUnderstandingBoard(event: Event) {
  const detail = (event as CustomEvent<BoardLocation & { reader_key: string }>).detail;
  if (detail?.reader_key === readerKey()) void openBoardLocation(detail);
}
function closeBoard() {
  boardVisible.value = false; boardMounted.value = false; boardId.value = null;
  boardPendingImport.value = null; boardPendingLocate.value = null; boardSourceError.value = ''; writeBoardRoute(null);
  closeBoardSourcePreview();
}
watch(() => buildWorkbenchSnapshot.value?.book_id, (book, previous) => {
  if (previous && book && book !== previous) closeBoard();
});
function addUnderstandingToBoard(event: Event) {
  const detail = (event as CustomEvent<{ reader_key: string; source: BoardImportSource }>).detail;
  if (detail?.reader_key === readerKey()) openBoard(detail.source);
}
onMounted(() => window.addEventListener('understanding-add-to-board', addUnderstandingToBoard));
onBeforeUnmount(() => window.removeEventListener('understanding-add-to-board', addUnderstandingToBoard));
onMounted(() => window.addEventListener('understanding-open-board', openUnderstandingBoard));
onBeforeUnmount(() => window.removeEventListener('understanding-open-board', openUnderstandingBoard));
async function openBoardSource(request: BoardSourceRequest) {
  boardSourceError.value = '';
  closeBoardSourcePreview();
  boardSourceRequest.value = { ...request }; boardSourceLoading.value = true;
  const sequence = boardSourceSequence;
  try {
    const source = await api.boardSourceResolve(request);
    if (sequence === boardSourceSequence) boardSourcePreview.value = source;
  } catch (failure) {
    if (sequence === boardSourceSequence) boardSourcePreviewError.value = errorMessage(failure);
  } finally { if (sequence === boardSourceSequence) boardSourceLoading.value = false; }
}
function retryBoardSource() {
  if (boardSourcePreview.value) void openBoardSourceInReader(boardSourceOpenLid);
  else if (boardSourceRequest.value) void openBoardSource(boardSourceRequest.value);
}
async function openBoardSourceInReader(lid?: string) {
  const request = boardSourceRequest.value, source = boardSourcePreview.value;
  if (!request || !source || !boardSourceCanOpen.value || boardSourceOpening.value) return;
  const sequence = boardSourceSequence;
  boardSourceOpenLid = lid;
  boardSourceOpening.value = true; boardSourcePreviewError.value = '';
  try {
    const returnPoint: ReadingReturnPoint = { ...captureReadingReturnPoint(''), origin: 'board', boardId: request.board_id, boardItemId: request.item_id };
    if (!source.media && !source.music) {
      const opened = await api.boardSourceOpen(lid ? { ...request, lid } : request);
      if (sequence !== boardSourceSequence) return;
      if (opened.source_url) { window.open(opened.source_url, '_blank', 'noopener,noreferrer'); return; }
      if (!opened.opened) throw new Error(uiText('原文暂时无法在阅读器中打开，请重试。'));
    }
    // On narrow screens the Reader may be hidden behind the assistant tab.
    // Make its layout measurable before restoring the cited passage.
    workspaceRef.value?.showReader();
    await nextTick();
    if (sequence !== boardSourceSequence) return;
    if (source.media) videoPane.value?.seek(mediaSourceStart(source.media));
    else if (source.music) musicPane.value?.openSource(source.music.target);
    else await syncViewport(true, true);
    if (sequence !== boardSourceSequence) return;
    readingContinuity.push(returnPoint); updateReadingReturn(); boardVisible.value = false;
    closeBoardSourcePreview();
  } catch (failure) {
    if (sequence === boardSourceSequence) boardSourcePreviewError.value = errorMessage(failure);
  } finally { if (sequence === boardSourceSequence) boardSourceOpening.value = false; }
}
const returnToAnswerAvailable = ref(false);
const readingReturnLabel = ref(uiText('返回回答'));
function updateReadingReturn() {
  const point = readingContinuity.peek(workspaceContextKey.value);
  returnToAnswerAvailable.value = !!point;
  readingReturnLabel.value = point?.origin === 'board' ? uiText('返回画板') : point?.origin === 'reader' ? uiText('返回阅读位置') : point?.origin === 'notes' ? uiText('返回笔记') : uiText('返回回答');
}
let pendingReadingReturnPoint: ReadingReturnPoint | null = null;
watch(workspaceContextKey, (contextKey, previous) => {
  previousPdfAnchor.value = null;
  const recoveringFirstChat = previous === `${chatSceneKey.value}:${buildWorkbenchSnapshot.value?.book_id ?? "no-book"}:no-chat`;
  if (recoveringFirstChat && displayReadingPoint?.point.contextKey === previous) displayReadingPoint.point.contextKey = contextKey;
  if (!recoveringFirstChat) {
    hlPopover.value = null;
    window.getSelection()?.removeAllRanges();
  }
  closeSourcePreview();
  pendingReadingReturnPoint = null;
  readingContinuity.invalidateContext(contextKey);
  updateReadingReturn();
  agentFullscreenSuspendedForSource.value = false;
});
function toggleAgentFullscreen() {
  agentFullscreen.value = !agentFullscreen.value;
  agentFullscreenSuspendedForSource.value = false;
}
function requestWorkspaceTab(tab: WorkspaceAuxTab) {
  // The foreground request runs before Vue hides the Reader on a narrow screen.
  readerPaneRef.value?.captureScrollAnchor(segments.value.map(segment => segment.lid));
  if (tab !== "agent") {
    agentFullscreen.value = false;
    agentFullscreenSuspendedForSource.value = false;
  }
  workspaceAuxTab.value = tab;
  workspaceAuxTabRevision.value += 1;
}
function handleWorkspaceProjection(projection: WorkspaceProjection) {
  adaptiveNavigation.value = projection.navigation !== 'desktop';
  readerRegionVisible.value = projection.mode === 'wide' || projection.visibleRegions.includes('reader');
  // The wide CSS layout always renders the auxiliary column; saved slots only
  // govern the adaptive single/compare foreground.
  assistantVisible.value = projection.mode === 'wide' || projection.visibleRegions.includes('assistant');
  if (projection.navigation === "desktop") mobileGlobalActionsOpen.value = false;
}
let displayReadingPoint: { point: ReadingReturnPoint; interaction: number; navigation: number } | null = null;
async function preserveDisplayReadingPosition() {
  // Repeated display-only changes keep the same source character. Recapturing
  // the new line start would accumulate drift when returning to a narrow column.
  const saved = displayReadingPoint;
  const point = saved && saved.point.contextKey === workspaceContextKey.value
    && saved.point.anchor?.surface === activeReaderSurface.value
    && saved.interaction === readerInteractionRevision && saved.navigation === readerNavigationRequest
    ? saved.point : captureReadingReturnPoint('');
  displayReadingPoint = { point, interaction: readerInteractionRevision, navigation: readerNavigationRequest };
  const token = readingContinuity.beginRestore(point.contextKey);
  const current = () => !disposed && point.contextKey === workspaceContextKey.value && readingContinuity.isCurrent(token);
  await nextTick();
  if (!current() || !point.anchor) return;
  if (point.anchor.surface === 'markdown') await readerPaneRef.value?.restoreScrollAnchor(point.anchor, current);
  else await pdfReaderPaneRef.value?.restoreReadingAnchor(point.anchor);
}
function toggleNavigationOutline() {
  if (adaptiveNavigation.value || focusReading.value) workspaceRef.value?.toggleOutline();
  else leftRailOpen.value = !leftRailOpen.value;
}
async function selectOutlineItem(lid: string) {
  if (await doGoto(lid) && workspaceRef.value?.outlineOpen) workspaceRef.value.showReader();
}
const acceptingRun = ref(false);
let submissionSequence = 0;
const residentRun = useAgentRun((snapshot) => { void finishResidentRun(snapshot).catch(fail); });
const sending = computed(() => acceptingRun.value || residentRun.active.value);
const runConnection = computed(() => residentRun.connection.value);
const waitingTurnId = computed(() => {
  const snapshot = residentRun.snapshot.value;
  return snapshot?.descriptor.session_id === activeChatSessionId.value
    && runConnection.value === 'connected' && !snapshot.error
    && snapshot.persistence_state === 'pending'
    && ['queued', 'waiting_model', 'running'].includes(snapshot.execution_state)
    && !snapshot.draft?.view?.parts.length ? snapshot.descriptor.turn_id : null;
});
watch([residentRun.snapshot, residentRun.connection], ([snapshot]) => {
  if (!snapshot || snapshot.descriptor.session_id !== activeChatSessionId.value) return;
  const turn = chat.value.find(t => t.turnId === snapshot.descriptor.turn_id);
  if (!turn) return;
  if (turn.teachingRef?.startsWith('action:') && completedTeachingStep(turn.activities, snapshot.activities)) {
    window.dispatchEvent(new CustomEvent('tutor-state-changed', { detail: { assessmentRef: turn.teachingRef } }));
  }
  turn.activities = snapshot.activities;
  turn.liveEffects = snapshot.effects;
  turn.boardCommits = mergeBoardDeliveries(turn.boardCommits, snapshot.board_commits, snapshot.final_view?.board_commits);
  void syncResidentChanges(snapshot).catch(fail);
  turn.draft = snapshot.draft;
  turn.pending = snapshot.persistence_state === "pending";
  turn.runStatus = runStatusText(snapshot, residentRun.connection.value);
  if (snapshot.final_view) Object.assign(turn, chatTurnFromHistory(snapshot.final_view, turn), { runStatus: turn.runStatus });
  turn.boardIncomplete = snapshot.persistence_state !== "pending" && snapshot.execution_state !== "completed";
  if (snapshot.persistence_state === "failed") turn.error = snapshot.error ? runStatusText(snapshot, residentRun.connection.value) : uiText('运行已结束，但结果未保存。已发生的阅读操作保留。');
  turn.spendStop = spendNotice(snapshot.error ?? snapshot.final_view?.error, snapshot.persistence_state);
  restoreFailedQuestionDraft(turn);
}, { flush: "sync" });
function restoreFailedQuestionDraft(turn: ChatTurn, rejectedDraft?: ChatDraft) {
  const draft = rejectedDraft ?? (turn.turnId ? submittedRunDrafts.get(turn.turnId) : undefined);
  if (!draft || turn.pending) return;
  if ((turn.error || turn.spendStop) && !turn.submissionUnknown) {
    turn.recoveryDraft = draft;
    // Restore only into an empty composer; a later question always takes priority.
    if (!agentInput.value && !askDraft.value) installQuestionDraft(turn, draft);
  }
  if (turn.turnId) submittedRunDrafts.delete(turn.turnId);
}
function installQuestionDraft(turn: ChatTurn, draft: ChatDraft) {
  agentInput.value = draft.message;
  askDraft.value = draft.quote;
  targetGoalId.value = draft.goalId;
  presentationRequested.value = draft.presentationRequested ?? false;
  if (turn.musicQuestion && !musicQuestionContext.value) {
    musicQuestionContext.value = { ...JSON.parse(JSON.stringify(turn.musicQuestion)), draft_id: crypto.randomUUID() };
  }
  if (turn.videoQuestion && !videoQuestion.draft.value) {
    videoQuestion.draft.value = { message: draft.message, context: { ...JSON.parse(JSON.stringify(turn.videoQuestion)), draft_id: crypto.randomUUID() } };
    videoQuestion.input(draft.message);
  }
}
async function editFailedQuestion(turn: ChatTurn) {
  if (sending.value || agentHistoryLoading.value || agentHistoryError.value || turn.pending || turn.submissionUnknown) return;
  const draft = turn.recoveryDraft ?? { message: turn.user, quote: turn.questionSelection, goalId: null };
  if ((agentInput.value && agentInput.value !== draft.message) || (askDraft.value && askDraft.value.quote !== draft.quote?.quote)) {
    banner.value = uiText('输入框里已有新草稿，请先发送或清空，再编辑原问题。');
    return;
  }
  banner.value = '';
  installQuestionDraft(turn, draft);
  await nextTick();
  document.querySelector<HTMLTextAreaElement>('.agent-input textarea')?.focus();
}
watch(() => submittedRunDrafts.size, () => { for (const turn of chat.value) restoreFailedQuestionDraft(turn); });
const seenResidentEffects = new Set<string>();
let residentReaderRevision = -1;
let residentReaderTurn = "";
function runOwnsCurrentScene(snapshot: RunSnapshot) {
  if (!network.value.enabled) return true;
  const w = network.value.workspace, d = snapshot.descriptor;
  return !!w && d.workspace_id === w.workspace_id && d.workspace_generation === w.generation
    && d.published_book_ref?.book_id === w.published_book_ref.book_id && d.published_book_ref?.publication_id === w.published_book_ref.publication_id;
}
async function syncResidentChanges(snapshot: RunSnapshot) {
  if (!runOwnsCurrentScene(snapshot)) return;
  const matches = () => snapshot.descriptor.book_id === buildWorkbenchSnapshot.value?.book_id && snapshot.descriptor.session_id === activeChatSessionId.value;
  if (!matches()) return;
  if (residentReaderTurn !== snapshot.descriptor.turn_id) { residentReaderTurn = snapshot.descriptor.turn_id; residentReaderRevision = -1; seenResidentEffects.clear(); }
  let navigation = false;
  for (const item of snapshot.effects ?? []) {
    if (seenResidentEffects.has(item.effect_id)) continue;
    seenResidentEffects.add(item.effect_id);
    if (item.effect.kind === "Goto") navigation = true;
    if (item.effect.kind === "LayoutProposal") pendingLayoutProposal.value = item.effect.proposal;
    if (item.effect.kind === "PaperMinimap") lastPaperMinimapEffect.value = item.effect.effect;
  }
  const changed = snapshot.reader_state;
  if (!changed || (changed.revision <= residentReaderRevision && !navigation)) return;
  residentReaderRevision = changed.revision;
  const interaction = readerInteractionRevision;
  const stillCurrent = () => matches() && residentReaderTurn === snapshot.descriptor.turn_id && residentReaderRevision === changed.revision && readerInteractionRevision === interaction;
  await manualReaderBoundary;
  if (manualReaderState?.book_id === snapshot.descriptor.book_id && changed.revision <= manualReaderState.revision) return;
  const current = await api.state();
  if (!stillCurrent() || current.book_id !== snapshot.descriptor.book_id || current.revision !== changed.revision) return;
  await applyReaderState(current);
  if (!stillCurrent()) return;
  if (navigation) await loadWindow(current.viewport, "replace", resolveReaderStateNavigationTarget(current.viewport.top_lid, current.selection, leafOrder.value), stillCurrent);
  else await refreshAnnotations();
  if (stillCurrent()) await loadPaperProjectionData(true);
}
async function retryResidentSave() {
  const current = residentRun.snapshot.value;
  if (!current) return;
  try { await api.agentRunRetrySave(current.descriptor.turn_id); residentRun.forget(); residentRun.observe(current.descriptor); }
  catch (error) { fail(error); }
}
async function stopResidentRun() {
  try { await residentRun.cancel(); } catch (error) { fail(error); }
}
const boardQuestionSubmission = ref<{ request: BoardQuestionRequest; sessionId: string; turn?: ChatTurn; error?: string; retryable?: boolean } | null>(null);
const boardRun = computed<BoardRunView | null>(() => {
  const submission = boardQuestionSubmission.value;
  const retained = submission?.turn;
  const turn = submission
    ? [...chat.value].reverse().find(item => item.boardRequestId === submission.request.client_request_id || (!!retained?.turnId && item.turnId === retained.turnId)) ?? retained
    : [...chat.value].reverse().find(item => item.boardFollowUp?.board_id === boardId.value);
  const context = submission?.request.context ?? turn?.boardFollowUp;
  if (!context) return null;
  const snapshot = residentRun.snapshot.value;
  const ownRun = !!turn?.turnId && snapshot?.descriptor.turn_id === turn.turnId;
  const saveFailed = ownRun && snapshot?.persistence_state === 'failed';
  const deliveryRetryable = !!turn?.boardDeliveryError && !['BOARD_NOT_FOUND', 'OBJECT_NOT_FOUND', 'BOARD_REVISION_CONFLICT', 'PRESENTATION_NOT_FOUND', 'INVALID_BOARD', 'BOARD_REQUEST_INVALID'].includes(turn.boardDeliveryErrorCode ?? '');
  const deliveryError = turn?.boardDeliveryErrorCode === 'BOARD_REVISION_CONFLICT'
    ? uiText('制作期间画板的位置或内容发生了变化。问题已保留在输入框，重新发送后会按当前画板制作和检查。')
    : turn?.boardDeliveryError;
  const error = submission?.error ?? deliveryError ?? turn?.error;
  const commits = mergeBoardDeliveries(turn?.boardCommits, ownRun ? snapshot?.board_commits : []);
  const pending = turn?.pending ?? !error;
  return {
    request_id: submission?.request.client_request_id ?? turn?.boardRequestId ?? `turn:${turn?.turnId}`,
    board_id: context.board_id, item_id: context.item_id,
    question: submission?.request.message ?? turn?.user ?? '',
    status_text: commits.length && pending ? uiText('已保存第{0}部分，后续仍在讲解', [commits.length]) : turn?.boardDelivery ? uiText('回答已保存到画板') : turn?.runStatus ?? (error ? '' : uiText('正在提交问题…')),
    accepted: !!turn?.turnId, pending, commits, incomplete: turn?.boardIncomplete,
    error, can_retry: !!error && (!!submission?.retryable || !!turn?.submissionUnknown || !!saveFailed || deliveryRetryable),
    can_stop: ownRun && residentRun.active.value, turn_id: turn?.turnId ?? undefined, delivery: turn?.boardDelivery,
    discussion: turn ? {
      sessionId: submission?.sessionId || activeChatSessionId.value, turnId: turn.turnId,
      requestId: submission?.request.client_request_id ?? turn.boardRequestId, user: turn.user,
      outcome: turn.outcome, pending, draft: turn.draft, runStatus: turn.runStatus, activities: turn.activities, error, commits,
    } : undefined,
  };
});
async function sendBoardQuestion(request: BoardQuestionRequest) {
  // Keep the submitted object and text independent of subsequent canvas selection.
  const frozen = JSON.parse(JSON.stringify(request)) as BoardQuestionRequest;
  const submission = reactive({ request: frozen, sessionId: activeChatSessionId.value, turn: undefined as ChatTurn | undefined,
    error: undefined as string | undefined, retryable: false });
  boardQuestionSubmission.value = submission;
  if (sending.value || agentHistoryLoading.value || agentHistoryError.value || hasUnconfirmedReadingRequest()) {
    submission.error = agentHistoryError.value ?? (hasUnconfirmedReadingRequest()
      ? uiText('原操作结果待核对，请先核对或重试原操作。') : uiText('当前回复尚未完成，请稍后继续。'));
    submission.retryable = !hasUnconfirmedReadingRequest();
    return;
  }
  submission.turn = await submitAgentMessage(frozen.message, frozen.message, null, undefined, {
    board_follow_up: frozen.context, client_request_id: frozen.client_request_id, session_id: submission.sessionId,
    ...(!network.value.enabled ? { expected_chat_head: actionHead() } : {}),
  });
  if (submission.turn?.turnId) submission.sessionId = activeChatSessionId.value;
  if (submission.turn?.error && !submission.turn.turnId && !submission.turn.submissionUnknown) submission.retryable = true;
  if (!submission.turn) { submission.error = uiText('问题尚未提交，请重试。'); submission.retryable = true; }
  else if (!submission.turn.turnId && !submission.turn.error) submission.error = uiText('问题未被接纳，请查看对话中的提示。');
}
async function retryBoardQuestion(requestId: string) {
  const run = boardRun.value;
  if (!run || run.request_id !== requestId || sending.value) return;
  const submission = boardQuestionSubmission.value;
  if (submission) submission.error = undefined;
  try {
    if (run.accepted && run.turn_id) {
      const repaired = await api.agentRunRetrySave(run.turn_id);
      if (residentRun.snapshot.value?.descriptor.turn_id === run.turn_id) residentRun.forget();
      applyAgentHistory(await api.agentHistory());
      if (repaired.persistence_state === 'pending') residentRun.observe(repaired.descriptor);
    } else if (network.value.enabled && pendingSubmissions().some(item => item.key === requestId)) {
      await retrySubmission(requestId); applyAgentHistory(await api.agentHistory());
    } else if (!network.value.enabled && currentReadingActions.value.some(item => item.body.client_request_id === requestId)) {
      await recoverReadingAction(currentReadingActions.value.find(item => item.body.client_request_id === requestId)!, true);
    } else if (submission?.retryable) {
      // Recover the selected discussion before deliberately retrying a rejected
      // request. Its board focus and text remain the captured originals.
      applyAgentHistory(await api.agentHistory());
      await sendBoardQuestion(submission.request);
    }
  } catch (failure) { if (submission) submission.error = errorMessage(failure); else boardSourceError.value = errorMessage(failure); }
}
async function cancelBoardQuestion() {
  if (boardRun.value?.can_stop) await stopResidentRun();
}
async function finishResidentRun(snapshot: RunSnapshot) {
  const workspaceChat = !!network.value.workspace?.learning_workspace_id;
  if (snapshot.descriptor.session_id !== activeChatSessionId.value || (!workspaceChat && snapshot.descriptor.book_id !== buildWorkbenchSnapshot.value?.book_id)) return;
  const context = workspaceContextKey.value;
  const current = () => !disposed && context === workspaceContextKey.value
    && snapshot.descriptor.session_id === activeChatSessionId.value && (workspaceChat || runOwnsCurrentScene(snapshot));
  if (!current()) return;
  workspaceNotesChanged();
  if (notesVisible.value) void notesWorkspaceRef.value?.refresh();
  if (snapshot.final_view?.domain?.workspace_goal_update && network.value.workspace?.learning_workspace_id) {
    window.dispatchEvent(new CustomEvent('learning-workspace-goal-changed', { detail: { learning_workspace_id: network.value.workspace.learning_workspace_id } }));
  }
  const feedback = rightRailRef.value?.prepareCompletionFeedback();
  const outcome = runOwnsCurrentScene(snapshot) ? snapshot.final_view?.outcome : null;
  const proposalEffect = outcome?.effects.find(effect => effect.kind === "LayoutProposal");
  if (proposalEffect?.kind === "LayoutProposal") pendingLayoutProposal.value = proposalEffect.proposal;
  const minimapEffect = [...(outcome?.effects ?? [])].reverse().find(effect => effect.kind === "PaperMinimap");
  if (minimapEffect?.kind === "PaperMinimap") lastPaperMinimapEffect.value = minimapEffect.effect;
  await refreshAnnotations();
  if (!current()) return;
  if (snapshot.persistence_state === 'saved') {
    const history = await api.agentHistory();
    if (!current()) return;
    const completed = applyAgentHistory(history, snapshot);
    if (followUpNodes.value.some(node => !!node.origin)) void loadFollowUps();
    if (completed && !sending.value) feedback?.();
  }
  submittedRunDrafts.delete(snapshot.descriptor.turn_id);
  window.dispatchEvent(new Event("tutor-state-changed"));
  await refreshProfileSurface(true);
  void tutor.load();
}


async function highlightPdfSelection() {
  const ready = pdfSelectionState.value.draft;
  if (!ready || ready.status !== "resolved") return;
  const draft = pdfSelectionSession.beginAction();
  if (!draft) return;
  try {
    banner.value = "";
    const groupId = draft.ranges.length > 1 ? newHighlightGroupId() : undefined;
    const created = await Promise.all(draft.ranges.map((selected) =>
      api.highlight(selected.lid, selected.range, groupId),
    ));
    const reselect = pdfReselectTarget.value?.kind === "highlight" ? pdfReselectTarget.value.record : null;
    if (reselect) {
      try {
        await Promise.all(highlightGroupMembers(reselect).map((record) => api.delete(record.mem_id)));
      } catch (error) {
        await Promise.allSettled(created.map((effect) => api.delete(effect.highlight_id)));
        throw error;
      }
    }
    selectedLid.value = draft.ranges[0]?.lid ?? selectedLid.value;
    completePdfSelectionAction();
    await refreshAnnotations();
  } catch (error) {
    pdfSelectionSession.actionFailed(error);
    fail(error);
  }
}

function notePdfSelection() {
  const draft = pdfSelectionState.value.draft;
  if (!draft || !getPdfSelectionCapabilities(draft.status).canNote || !draft.ranges[0]) return;
  const first = draft.ranges[0];
  const selectionContext = selectionContextOf(draft);
  const reselect = pdfReselectTarget.value?.kind === "note" ? pdfReselectTarget.value.record : null;
  if (reselect) {
    void beginNote({ lid: first.lid, memId: reselect.mem_id, layer: reselect.layer,
      content: reselect.content, selectionContext, legacy: !reselect.note,
      excerpt: reselect.note?.retained_excerpt ? { kind: 'original', text: selectionContext.raw_quote } : null,
      title: uiText('重新选择原文') });
  } else openNewNote(first.lid, '', selectionContext);
  // The editor owns the immutable selection; later PDF selections are independent.
  completePdfSelectionAction();
}

function askPdfSelection() {
  const ready = pdfSelectionState.value.draft;
  if (!ready || !getPdfSelectionCapabilities(ready.status).canAsk) return;
  const draft = pdfSelectionSession.beginAction();
  const first = draft?.ranges[0];
  if (!draft || !first) return;
  const quote = draft.raw_quote.replace(/\s+/g, " ").trim();
  askDraft.value = {
    lid: first.lid,
    quote,
    ranges: draft.ranges,
    status: draft.status,
    ...(draft.resolution_basis ? { resolution_basis: draft.resolution_basis } : {}),
    raw_quote: draft.raw_quote,
    resolved_quote: draft.resolved_quote,
  };
  selectedLid.value = first.lid;
  agentInput.value = "";
  completePdfSelectionAction();
  workspaceRef.value?.showAssistant("agent");
}
const effectPending = ref<Record<string, boolean>>({});
watch(chatSceneKey, () => {
  if (disposed || readerKey() !== mountedReaderKey) return;
  // First-question admission creates its chat before returning the accepted run.
  const admittingFirstChat = !activeChatSessionId.value && acceptingRun.value;
  residentRun.forget();
  startupHistoryRequestSeq += 1;
  activeChatSessionId.value = network.value.workspace?.selected_chat ?? '';
  chatGoals.value = [];
  targetGoalId.value = null;
  effectPending.value = {};
  presentationWorkspace.value = null;
  pendingLayoutProposal.value = null;
  agentHistoryError.value = null;
  profileMemoryRequestSeq += 1;
  profileMemoryLoading.value = false;
  profileMemoryError.value = null;
  void tutor.load();
  if (!admittingFirstChat) {
    submissionSequence += 1;
    acceptingRun.value = false;
    chat.value = [];
    askDraft.value = null;
    agentInput.value = '';
  }
}, { flush: 'sync' });
function effState(ti: number, ei: number): string | undefined {
  const turn = chat.value[ti];
  const effect = turn?.outcome?.effects[ei];
  if (!effect) return undefined;
  const id = effectId(effect);
  if (effectPending.value[`${turn.turnId}:${id}`]) return uiText('正在处理');
  return dispositionLabel(turn.dispositions?.[id], effect);
}
function localQuestionQuoteView(draft: AskDraft): AgentQuestionQuoteView {
  return {
    label: draft.status === "partial" ? uiText('部分定位引用') : uiText('引用来源'),
    quote: draft.quote,
    status: draft.status,
  };
}

function chatTurnFromHistory(turn: StoredAgentChatTurn, previous?: ChatTurn): ChatTurn {
  const matchingPrevious = previous?.user === turn.user ? previous : undefined;
  const quote = turn.question_quote;
  const ranges = quote?.evidence_ranges?.flatMap(evidence => evidence.ranges ?? []);
  const savedSelection: AskDraft | null = quote?.status === 'resolved' && ranges?.length
    ? { lid:ranges[0].lid, quote:quote.quote, status:'resolved', raw_quote:quote.quote, resolved_quote:quote.quote, ranges } : null;
  return {
    localAgentName: turn.domain?.local_agent?.name,
    localAgentId: turn.domain?.local_agent?.id,
    localAgentUnavailable: turn.error?.error_code === 'LOCAL_AGENT_UNAVAILABLE',
    noteReferences: [...(turn.domain?.personal_notes?.references ?? []), ...(turn.domain?.personal_notes?.adopted_references ?? [])].filter((ref,index,all) => all.findIndex(other => other.note_id === ref.note_id && other.revision === ref.revision) === index),
    videoQuestion: turn.video_question,
    musicQuestion: turn.music_question,
    spendStop: spendNotice(turn.error, turn.status === 'pending_assistant' ? 'pending' : 'saved'),
    interaction: turn.domain?.interaction,
    dispositions: Object.fromEntries((turn.domain?.effects ?? []).filter(e => e.disposition).map(e => [e.effect_id, e.disposition!])),
    teachingRef: turn.teaching_ref,
    hasTeaching: !!turn.domain?.teaching?.length,
    presentationFollowUp: turn.presentation_follow_up,
    boardFollowUp: turn.board_follow_up,
    boardDelivery: turn.board_delivery,
    boardCommits: mergeBoardDeliveries(matchingPrevious?.turnId === turn.turn_id ? matchingPrevious.boardCommits : [], turn.board_commits),
    boardIncomplete: turn.status !== "pending_assistant" && turn.status !== "completed",
    boardDeliveryError: agentFailureMessage(turn.board_delivery_error),
    boardDeliveryErrorCode: turn.board_delivery_error?.error_code,
    boardRequestId: matchingPrevious?.boardRequestId,
    recoveryDraft: matchingPrevious?.recoveryDraft,
    turnId: turn.turn_id,
    user: turn.user,
    outcome: turn.outcome ?? null,
    pending: turn.status === "pending_assistant",
    error: agentFailureMessage(turn.error, turn.run_summary),
    questionAnchorLid: matchingPrevious?.questionAnchorLid ?? null,
    questionQuote: turn.question_quote ? { ...turn.question_quote } : null,
    questionSelection: matchingPrevious?.questionSelection ?? savedSelection,
    liveEffects: turn.run_summary?.effects.map(effect => ({ effect_id: effectId(effect), effect })),
    effectLabels: [...turn.effect_labels],
    activities: turn.run_summary?.activities ?? undefined,
    runTrace: turn.run_summary?.trace,
    runStatus: turn.status === "cancelled" ? uiText('已停止') : undefined,
  };
}

function applyAgentHistory(history: AgentHistoryResponse, completedRun?: RunSnapshot) {
  for (const turn of history.current.turns) if (turn.video_question && turn.video_question.draft_id === videoQuestion.draft.value?.context.draft_id) {
    videoQuestion.accepted(turn.video_question!.draft_id); agentInput.value = '';
  }
  for (const turn of history.current.turns) if (turn.music_question && turn.music_question.draft_id === musicQuestionContext.value?.draft_id) {
    musicQuestionContext.value = null; agentInput.value = '';
  }
  if (residentRun.snapshot.value && residentRun.snapshot.value.descriptor.session_id !== history.current.id) residentRun.forget();
  const previousChat = activeChatSessionId.value === history.active_session_id ? chat.value : [];
  activeChatSessionId.value = history.active_session_id;
  const completed = yeyeFeedback.observe(workspaceContextKey.value, history, completedRun);
  chatSessions.value = history.sessions;
  void refreshClosedBook();
  chatGoals.value = history.current.goals ?? [];
  if (!chatGoals.value.some(goal => goal.id === targetGoalId.value && goal.status === "open")) targetGoalId.value = null;
  const storedTurnIds = new Set(history.current.turns.map(turn => turn.turn_id));
  // A history read can finish while admission is still pending, or after a
  // definite rejection that never created a server turn. Keep those local turns.
  const localTurns = previousChat.filter(turn => !turn.submissionUnknown
    && (!turn.turnId || (turn.pending && !storedTurnIds.has(turn.turnId))));
  chat.value = [...history.current.turns.map((turn, index) => chatTurnFromHistory(turn, previousChat.find(old => old.turnId === turn.turn_id) ?? previousChat[index])), ...localTurns];
  for (const turn of chat.value) restoreFailedQuestionDraft(turn);
  const unknown = unknownSubmissionsBySession.get(history.current.id) ?? [];
  const unresolved = network.value.enabled ? unknown.filter(record => pendingSubmissions().some(item => item.key === record.turn.requestId)) : unknown.filter((record) => !submissionWasAccepted(record, {
    sessionId: history.current.id,
    turns: history.current.turns.map((turn) => ({ turnId: turn.turn_id, user: turn.user })),
  }));
  if (unresolved.length) {
    chat.value.push(...unresolved.map((record) => record.turn));
    unknownSubmissionsBySession.set(history.current.id, unresolved);
  } else {
    unknownSubmissionsBySession.delete(history.current.id);
  }
  effectPending.value = {};
  const pending = history.current.turns.find(turn => turn.status === "pending_assistant");
  if (pending && !(residentRun.snapshot.value?.descriptor.turn_id === pending.turn_id && residentRun.snapshot.value.persistence_state === "failed")) {
    residentRun.observe({ book_id: pending.published_book_ref?.book_id ?? history.current.book_id, session_id: history.current.id, turn_id: pending.turn_id });
  }
  const snapshot = residentRun.snapshot.value;
  if (snapshot?.descriptor.session_id === history.current.id) {
    const current = chat.value.find(turn => turn.turnId === snapshot.descriptor.turn_id);
    if (current) {
      current.activities = snapshot.activities;
      current.boardCommits = mergeBoardDeliveries(current.boardCommits, snapshot.board_commits, snapshot.final_view?.board_commits);
      current.boardIncomplete = snapshot.persistence_state !== "pending" && snapshot.execution_state !== "completed";
      current.draft = snapshot.draft;
      current.pending = snapshot.persistence_state === "pending";
      current.runStatus = runStatusText(snapshot, residentRun.connection.value);
      if (snapshot.persistence_state === "failed") current.error = snapshot.error ? runStatusText(snapshot, residentRun.connection.value) : uiText('运行已结束，但结果未保存。已发生的阅读操作保留。');
      current.spendStop = spendNotice(snapshot.error ?? snapshot.final_view?.error, snapshot.persistence_state);
      restoreFailedQuestionDraft(current);
    }
  }
  return completed;
}

async function loadStartupHistory() {
  const request = ++startupHistoryRequestSeq;
  const bookId = buildWorkbenchSnapshot.value?.book_id;
  agentHistoryLoading.value = true;
  agentHistoryError.value = null;
  try {
    const history = await api.agentHistory();
    if (request !== startupHistoryRequestSeq) return;
    const learningId = network.value.workspace?.learning_workspace_id;
    if (learningId ? history.current.learning_workspace_id !== learningId : history.current.book_id !== bookId)
      throw new Error(learningId ? uiText('对话所属工作区已变化，请重试') : uiText('对话所属书籍已变化，请重试'));
    const resumeDraft = !activeChatSessionId.value && !agentInput.value && !askDraft.value
      ? readLearningDraft(followUpDraftKey(history.current.id)) : null;
    applyAgentHistory(history);
    if (resumeDraft && !agentInput.value) {
      agentInput.value = resumeDraft.message;
      askDraft.value = resumeDraft.quote;
      targetGoalId.value = chatGoals.value.find(g => g.id === resumeDraft.goalId && g.status === 'open')?.id ?? null;
      presentationRequested.value = !!resumeDraft.presentationRequested;
    }
  } catch (error) {
    if (request === startupHistoryRequestSeq) {
      agentHistoryError.value = errorMessage(error);
      requestWorkspaceRecovery(error);
    }
  } finally {
    if (request === startupHistoryRequestSeq) agentHistoryLoading.value = false;
  }
}

async function retryHistory() {
  if (!network.value.enabled) { await (workspaceInitializationFailed ? init() : loadStartupHistory()); return; }
  agentHistoryLoading.value = true;
  agentHistoryError.value = null;
  try {
    await recoverWorkspaceBinding();
    // A new generation remounts App; its startup owns the next history read.
    await nextTick();
    if (!disposed) await (workspaceInitializationFailed ? init() : loadStartupHistory());
  } catch (error) {
    if (!disposed) { agentHistoryError.value = errorMessage(error); requestWorkspaceRecovery(error); }
  } finally {
    if (!disposed) agentHistoryLoading.value = false;
  }
}

// AgentEffect 判别(在 TS 里 narrow,避开模板里的联合类型收窄)。
function isGoto(e: AgentEffect): boolean {
  return e.kind === "Goto";
}
function effLabel(e: AgentEffect): string {
  if (e.kind === "PaperMinimap") return uiText('论文地图已更新');
  if (e.kind === "PaperMinimapProposal") return uiText('论文地图建议');
  if (e.kind === "Goto") return uiText('跳转');
  if (e.kind === "Highlight") return uiText('高亮');
  if (e.kind === "Note") return uiText('笔记');
  if (e.kind === "Layout") return uiText('布局已更新');
  return uiText('布局调整建议');
}
function gotoBack(e: AgentEffect): string {
  return e.kind === "Goto" ? e.before_anchor : "";
}
function effectPrimaryLabel(e: AgentEffect): string {
  if (e.kind === "LayoutProposal" || e.kind === "PaperMinimapProposal") return uiText('应用');
  if (e.kind === "Highlight" || e.kind === "Note") return uiText('保留');
  return "";
}
function effectSecondaryLabel(e: AgentEffect): string {
  if (e.kind === "LayoutProposal" || e.kind === "PaperMinimapProposal") return uiText('忽略');
  if (e.kind === "PaperMinimap") return uiText('撤销');
  if (e.kind === "Highlight" || e.kind === "Note") return uiText('撤销');
  return "";
}
function showEffectPrimary(e: AgentEffect): boolean {
  return e.kind === "Highlight" || e.kind === "Note" || e.kind === "LayoutProposal"
    || e.kind === "PaperMinimapProposal";
}
function showEffectSecondary(e: AgentEffect): boolean {
  return e.kind === "Highlight" || e.kind === "Note" || e.kind === "LayoutProposal"
    || e.kind === "PaperMinimap" || e.kind === "PaperMinimapProposal";
}
async function applyLayoutActions(actions: ReaderLayoutAction[]) {
  const outcome = await api.layoutApply({ actions });
  if (outcome.kind === "proposal") {
    pendingLayoutProposal.value = outcome.proposal;
  } else {
    pendingLayoutProposal.value = null;
  }
  const st = await api.state();
  await applyReaderState(st);
}
async function tutorAction(action: import("./generated/TutorAction").TutorAction) {
  const context = readerContextKey.value;
  const current = () => !disposed && context === readerContextKey.value;
  await tutor.act(action);
  if (!current()) return;
  window.dispatchEvent(new Event("tutor-state-changed"));
  if (tutor.error.value || tutor.pending.value) return;
  if (action.kind === "start" || action.kind === "resume") {
    await startTutorRequest(tutor.current.value?.user_intent);
  }
}
async function startTutorRequest(userIntent?: string) {
    const context = readerContextKey.value;
    const current = () => !disposed && context === readerContextKey.value;
    try {
      const result = await api.tutorStart();
      if (!current()) return;
      await tutor.load();
      if (!current() || tutor.error.value || tutor.pending.value) return;
      if (result.started && result.message && sending.value) {
        banner.value = uiText('学习目标已保存，当前回复完成后可继续学习。');
      } else if (result.started && result.message) {
        tutorPanelOpen.value = false;
        // Starting or resuming explicitly adopts this confirmed goal for the current request.
        const message = userIntent?.trim() ? uiText('{0}\n\n本次学习目标：\n{1}', [result.message, userIntent]) : result.message;
        await submitAgentMessage(message, uiText('继续学习'), null, undefined, {}, undefined, mediaMode.value ? captureLearningMedia() : undefined);
      } else if (result.reason) banner.value = result.reason;
    } catch (failure) {
      if (current() && !(failure instanceof ApiError && failure.errorCode === 'CLIENT_CONTEXT_STALE')) {
        banner.value = uiText('学习未开始：{0}', [errorMessage(failure)]);
      }
    }
}
async function startLearningPurpose(purpose: LearningPurpose, scopeKind?: LearningScopeKind, details?: LearningPurposeStart) {
  if (!learningPurposeAvailable.value || guidingChapter.value || sending.value || hasUnconfirmedReadingRequest()
    || (purpose !== 'discover' && !learningPurposeReady.value)) return;
  const scope = learningScopeOptions.value.find(option => option.value === (scopeKind ?? learningScopeOptions.value[0]?.value));
  if (!scope) return;
  const request = {
    sourceId: buildWorkbenchSnapshot.value!.book_id, sourceRevision: noteSourceFingerprint.value!,
    sessionId: activeChatSessionId.value, scene: chatSceneKey.value, head: actionHead(),
    chapterLid: scope.lid,
    quote: scope.value === 'selection' && askDraft.value ? structuredClone(toRaw(askDraft.value)) : null,
  };
  guidingChapter.value = true;
  try {
    const media = scope.value === 'media' ? captureLearningMedia() : {};
    if (disposed || request.scene !== chatSceneKey.value || request.sessionId !== activeChatSessionId.value
      || request.sourceRevision !== noteSourceFingerprint.value) return;
    tutorPanelOpen.value = false;
    workspaceRef.value?.showAssistant('agent');
    const message = details?.message?.trim() ?? '';
    const display = details?.display ?? `${learningPurposeLabel(purpose, musicMode.value, videoMode.value)} · ${scope.label}${message ? `\n\n${message}` : ''}`;
    const turn = await submitAgentMessage(message, display, request.quote, undefined, {
      entry_action: details?.entryAction ?? 'learn_with_purpose', learning_purpose: purpose, chapter_lid: request.chapterLid,
      source_id: request.sourceId, source_revision: request.sourceRevision, session_id: request.sessionId,
      client_request_id: crypto.randomUUID(), expected_chat_head: request.head,
    }, undefined, media);
    if (!turn?.turnId && turn?.error) banner.value = turn.error;
    if (turn?.turnId) await rememberLearningTask();
    await tutor.load();
    window.dispatchEvent(new Event('tutor-state-changed'));
  } catch (failure) {
    if (!disposed) banner.value = uiText('学习未开始：{0}', [errorMessage(failure)]);
  } finally { guidingChapter.value = false; }
}
/** Practice one exact saved idea against the open material (ADR-0178). */
async function startNoteRevisit(noteRef: import('./generated/NoteRef').NoteRef, understandingRef?:UnderstandingReference) {
  if (!learningPurposeAvailable.value || guidingChapter.value || sending.value || hasUnconfirmedReadingRequest()
    || !learningPurposeReady.value) return;
  const request = { sourceId: buildWorkbenchSnapshot.value!.book_id, sourceRevision: noteSourceFingerprint.value!,
    sessionId: activeChatSessionId.value, scene: chatSceneKey.value, head: actionHead() };
  guidingChapter.value = true;
  try {
    if (disposed || request.scene !== chatSceneKey.value || request.sessionId !== activeChatSessionId.value
      || request.sourceRevision !== noteSourceFingerprint.value) return;
    tutorPanelOpen.value = false;
    workspaceRef.value?.showAssistant('agent');
    const label = understandingRef ? uiText('换个例子练习') : uiText('用我这条想法练一练');
    const turn=await submitAgentMessage('', label, null, undefined, {
      entry_action: 'revisit_note', note_ref: noteRef,
      ...(understandingRef?{understanding_ref:understandingRef,note_refs:[noteRef]}:{}),
      source_id: request.sourceId, source_revision: request.sourceRevision, session_id: request.sessionId,
      client_request_id: crypto.randomUUID(), expected_chat_head: request.head,
    });
    if(understandingRef && turn?.turnId)personalDiscussion.value={reference:noteRef,replyTo:turn.turnId};
    await tutor.load();
    window.dispatchEvent(new Event('tutor-state-changed'));
  } catch (failure) {
    if (!disposed) banner.value = uiText('练习未开始：{0}', [errorMessage(failure)]);
  } finally { guidingChapter.value = false; }
}
async function startUnderstandingAction(action:UnderstandingAction) {
  if(sending.value || agentHistoryLoading.value)throw new Error('当前回答或讨论正在加载，请稍后继续。');
  if(action.context.book_id!==buildWorkbenchSnapshot.value?.book_id || action.source_revision!==noteSourceFingerprint.value
    || action.context.workspace_id!==(network.value.workspace?.learning_workspace_id??null))throw new Error('原材料版本或学习范围已变化，请在原记录中继续。');
  if(action.mode==='practice' && action.note_ref) {
    if(!learningPurposeAvailable.value || guidingChapter.value || hasUnconfirmedReadingRequest() || !learningPurposeReady.value)throw new Error('原材料的练习准备尚未完成，请稍后再试。');
    await startNoteRevisit(action.note_ref,action.understanding_ref);return;
  }
  workspaceRef.value?.showAssistant('agent');
  tutorPanelOpen.value=false;
  const practice = action.mode === 'practice';
  const label = practice ? uiText('换个例子练习') : uiText('接着这一点');
  const turn=await submitAgentMessage(action.message ?? '',action.message ? `${label}\n${action.message}` : label,null,undefined,{
    entry_action:'continue_answer',...(practice ? {learning_purpose:'practice'} : {}),
    answer_ref:{turn_id:action.context.turn_id!},session_id:activeChatSessionId.value,
    source_id:action.context.book_id!,source_revision:action.source_revision,understanding_ref:action.understanding_ref,
    ...(action.note_ref?{note_refs:[action.note_ref]}:{}),client_request_id:crypto.randomUUID(),expected_chat_head:actionHead(),
  });
  if(turn?.turnId && action.note_ref)personalDiscussion.value={reference:action.note_ref,replyTo:turn.turnId};
}
// The notes panel is app-wide; only an open material can ground the problem.
function onPracticeNote(event: Event) {
  if (!buildWorkbenchSnapshot.value || !noteSourceFingerprint.value || !learningPurposeAvailable.value) return;
  event.preventDefault();
  void startNoteRevisit((event as CustomEvent<import('./generated/NoteRef').NoteRef>).detail);
}
onMounted(() => window.addEventListener('practice-note', onPracticeNote));
onBeforeUnmount(() => window.removeEventListener('practice-note', onPracticeNote));
// Practice offers (ADR-0178 P3). An offer spends nothing; only accepting starts a turn.
// The margin setting is a per-device preference and stays off unless turned on.
function practiceStorage(): Storage | null { try { return window.localStorage; } catch { return null; } }
const practiceOwner = computed(() => notesOwner() ?? 'local');
const practiceMarginOn = ref(false);
watch(practiceOwner, owner => { practiceMarginOn.value = marginEnabled(practiceStorage(), owner); }, { immediate: true });
function setLearningMap(event: Event) {
  const input = event.target as HTMLInputElement;
  // Off deletes the derived map; it can be rebuilt by turning it on again.
  if (!input.checked && !window.confirm(uiText('关闭后将删除已整理的学习脉络（笔记与学习记录不受影响），确定吗？'))) { input.checked = true; return; }
  void tutorAction({ kind: 'set_learning_map', enabled: input.checked });
}
function setPracticeMargin(on: boolean) { practiceMarginOn.value = on; setMarginEnabled(practiceStorage(), practiceOwner.value, on); }
const practiceOffer = ref<{ kind: 'margin' | 'warmup' | 'warmup-next' | 'exam-next'; step?: number } | null>(null);
/** Problems started in the current warm-up or exam sequence; 0 when none is active. */
const warmupStarted = ref(0);
const sequence = ref<{ kind: 'warmup' | 'exam'; total: number }>({ kind: 'warmup', total: WARMUP_LIMIT });
const practiceAvailable = computed(() => learningPurposeAvailable.value && learningPurposeReady.value
  && !sending.value && !guidingChapter.value);
const marginTracker = new MarginTracker();
let practiceTimer: number | undefined;
onMounted(() => { practiceTimer = window.setInterval(() => {
  const due = marginTracker.observe(guideChapter.value?.lid ?? null, document.visibilityState === 'visible', Date.now());
  if (due && practiceMarginOn.value && !practiceOffer.value && practiceAvailable.value
    && !marginQuietToday(practiceStorage(), practiceOwner.value, today())) practiceOffer.value = { kind: 'margin' };
}, 15_000); });
onBeforeUnmount(() => window.clearInterval(practiceTimer));
// One warm-up offer per material and day, once this material already has a learning stage.
watch(() => [practiceAvailable.value, buildWorkbenchSnapshot.value?.book_id, !!tutor.current.value] as const, ([available, book, stage]) => {
  if (!available || !book || !stage || practiceOffer.value || warmupStarted.value) return;
  if (claimWarmupOffer(practiceStorage(), practiceOwner.value, `${book}:${chatSceneKey.value}`, today())) practiceOffer.value = { kind: 'warmup' };
});
function onPracticeConcluded() {
  if (warmupStarted.value > 0 && warmupStarted.value < sequence.value.total && !practiceOffer.value) {
    practiceOffer.value = { kind: sequence.value.kind === 'exam' ? 'exam-next' : 'warmup-next', step: warmupStarted.value + 1 };
  } else if (warmupStarted.value >= sequence.value.total) warmupStarted.value = 0;
}
function sequenceLabel() {
  const [step, total] = [String(warmupStarted.value), String(sequence.value.total)];
  return sequence.value.kind === 'exam' ? uiText('考试冲刺 · 今日练习（第 {0} / {1} 题）', [step, total])
    : uiText('热身练习（第 {0} / {1} 题）', [step, total]);
}
/** Today's exam-sprint practice: a short sequence within the sprint's scope. */
async function startExamPractice(remainingDays: number) {
  sequence.value = { kind: 'exam', total: examDailyCount(remainingDays) };
  warmupStarted.value = 1;
  practiceOffer.value = null;
  await startLearningPurpose('practice', 'material', { display: sequenceLabel() });
}
onMounted(() => window.addEventListener('practice-concluded', onPracticeConcluded));
onBeforeUnmount(() => window.removeEventListener('practice-concluded', onPracticeConcluded));
async function acceptPracticeOffer() {
  const offer = practiceOffer.value;
  practiceOffer.value = null;
  if (!offer) return;
  if (offer.kind === 'margin') {
    await startLearningPurpose('practice', guideChapter.value?.kind === 'section' ? 'section' : guideChapter.value ? 'chapter' : 'material', { display: uiText('边读边练一题') });
    return;
  }
  if (offer.kind === 'warmup') sequence.value = { kind: 'warmup', total: WARMUP_LIMIT };
  warmupStarted.value = offer.kind === 'warmup' ? 1 : warmupStarted.value + 1;
  await startLearningPurpose('practice', 'material', { display: sequenceLabel() });
}
function dismissPracticeOffer() {
  if (practiceOffer.value?.kind === 'margin') recordMarginIgnored(practiceStorage(), practiceOwner.value, today());
  practiceOffer.value = null;
  warmupStarted.value = 0;
}
async function setConversationLearningGoal(intent: string) {
  const sourceId = tutor.readiness.value?.source_id;
  if (!sourceId || sending.value) return;
  const session = tutor.current.value;
  await tutor.act({ kind: 'start', user_intent: intent, explicit_constraints: session?.explicit_constraints ?? [],
    material_scope: session?.material_scope ?? [{ source_id: sourceId, scope_refs: [], role: 'primary' }],
    learning_purpose: session?.learning_purpose, default_teaching_intent: session?.default_teaching_intent ?? null });
  window.dispatchEvent(new Event('tutor-state-changed'));
}
async function resumeConversationLearning() {
  const session = tutor.current.value;
  if (!session || sending.value || tutor.busy.value || tutor.pending.value || tutor.readiness.value?.status !== 'ready' || tutor.readinessError.value) return;
  await tutorAction({ kind: 'resume', session_id: session.id });
  if (tutor.current.value?.status === 'active') await rememberLearningTask({continue_conversation:true});
}
function onTutorResponse(event: Event) {
  const detail = (event as CustomEvent<{ eventId: string; text: string; scene?: import('./generated/PresentationFollowUp').PresentationFollowUp; responseOrigin?: import('./api').TeachingResponseOrigin }>).detail;
  if (sending.value) { banner.value = uiText('当前回复尚未完成，请稍后继续。'); return; }
  void submitAgentMessage(detail.text, detail.text, null, undefined, { teaching_ref: detail.eventId,
    presentation_follow_up: detail.scene, teaching_response_origin: detail.responseOrigin });
}
const activityHelpLocation = ref<{ session_id: string; turn_id: string; delivery_refs: string[] } | null>(null);
const localReadingActions = ref<PendingReadingAction[]>([]);
function refreshReadingActions() { localReadingActions.value = network.value.enabled ? [] : pendingReadingActions(); }
const currentReadingActions = computed(() => localReadingActions.value.filter(item => item.body.session_id === activeChatSessionId.value));
onMounted(() => { refreshReadingActions(); window.addEventListener('local-reading-action-changed', refreshReadingActions); });
onBeforeUnmount(() => window.removeEventListener('local-reading-action-changed', refreshReadingActions));
async function recoverReadingAction(item: PendingReadingAction, retry = false) {
  if (sending.value) return;
  const originalScene = chatSceneKey.value;
  acceptingRun.value = true;
  try {
    const result = await api.recoverReadingAction(item, retry);
    if (disposed || chatSceneKey.value !== originalScene) return;
    if (result && typeof result === 'object' && 'turn_id' in result) residentRun.observe(result as import('./agent-run-state').RunDescriptor);
    else if (result && typeof result === 'object' && 'kind' in result && result.kind === 'activity_help') {
      const help = result as unknown as { delivery_refs: string[]; activity?: { session_id: string; turn_id: string } };
      activityHelpLocation.value = help.activity ? { ...help.activity, delivery_refs: help.delivery_refs } : null;
      banner.value = uiText('这段内容属于当前待答题，请前往原题帮助。');
    } else if (!result) banner.value = uiText('未找到已接纳的原操作，可使用原身份重试。');
    const history = await api.agentHistory();
    if (!disposed && chatSceneKey.value === originalScene) applyAgentHistory(history);
  } catch (error) { if (!disposed && chatSceneKey.value === originalScene) banner.value = errorMessage(error); }
  finally { if (!disposed && chatSceneKey.value === originalScene) acceptingRun.value = false; refreshReadingActions(); }
}
function hasUnconfirmedReadingRequest() {
  return network.value.enabled ? pendingSubmissions().some(item => item.body.session_id === activeChatSessionId.value) : currentReadingActions.value.length > 0;
}
watch(chatSceneKey, () => { activityHelpLocation.value = null; });
async function openActivityHelp() {
  const location = activityHelpLocation.value;
  if (!location || sending.value) return;
  await selectChat(location.session_id, true);
  workspaceRef.value?.showAssistant("agent");
  await rightRailRef.value?.scrollToTurn(location.turn_id);
  rightRailRef.value?.focusActivityHelp(location.delivery_refs);
}
async function openUnderstandingChat(event: Event) {
  const location = (event as CustomEvent<{ session_id: string; turn_id: string; reader_key: string; onUnavailable?: (message: string) => void }>).detail;
  if (!location || location.reader_key !== readerKey()) return;
  if (sending.value || agentHistoryLoading.value) { location.onUnavailable?.(uiText('当前回答或对话正在加载，请稍后打开对应对话。')); return; }
  const context = readerContextKey.value;
  await selectChat(location.session_id, true);
  if (disposed || context !== readerContextKey.value) return;
  if (activeChatSessionId.value !== location.session_id) { location.onUnavailable?.(uiText('对应对话暂时无法打开，请稍后重试。')); return; }
  tutorPanelOpen.value = false;
  workspaceRef.value?.showAssistant('agent');
  await rightRailRef.value?.scrollToTurn(location.turn_id);
}
onMounted(() => window.addEventListener('understanding-open-chat', openUnderstandingChat));
onBeforeUnmount(() => window.removeEventListener('understanding-open-chat', openUnderstandingChat));
function actionHead() { return chat.value.filter(turn => turn.turnId).at(-1)?.turnId ?? null; }
const canContinueAnswer = computed(() => !sending.value && !agentHistoryLoading.value && !hasUnconfirmedReadingRequest());
async function continueAnswer(turnId: string) {
  if (!canContinueAnswer.value) return;
  const turn = await submitAgentMessage('', uiText('操作：继续理解这段'), null, undefined, {
    entry_action: 'continue_answer', session_id: activeChatSessionId.value, answer_ref: { turn_id: turnId },
    client_request_id: crypto.randomUUID(), expected_chat_head: actionHead(),
  });
  if (turn?.turnId) await rememberLearningTask();
}
async function practiceAnswer(turnId: string) {
  if (!canContinueAnswer.value || !learningPurposeReady.value) return;
  const turn = await submitAgentMessage('', uiText('给我一道应用题'), null, undefined, {
    entry_action:'continue_answer', answer_ref:{turn_id:turnId}, learning_purpose:'practice',
    session_id:activeChatSessionId.value, source_id:buildWorkbenchSnapshot.value?.book_id,
    source_revision:noteSourceFingerprint.value ?? undefined,
    client_request_id:crypto.randomUUID(), expected_chat_head:actionHead(),
  });
  if (turn?.turnId) await rememberLearningTask();
}
async function practiceCourse() {
  await startLearningPurpose('practice', 'material', {
    entryAction:'apply_lesson',
    display:uiText('给我一道应用题'),
  });
}
async function clarifyAnswer(turnId: string, excerpt?: string, receipt?: import('./generated/PresentationFollowUp').PresentationFollowUp) {
  await submitAgentMessage('', uiText('操作：这一步没跟上'), null, receipt, {
    entry_action: 'clarify_answer', session_id: activeChatSessionId.value, answer_ref: { turn_id: turnId, ...(excerpt ? { excerpt } : {}) },
    client_request_id: crypto.randomUUID(), expected_chat_head: actionHead(),
  });
}
async function chooseContinuation(view: import('./generated/TutorContinuationView').TutorContinuationView, optionId: string, receipt?: import('./generated/PresentationFollowUp').PresentationFollowUp) {
  const option = view.continuation.options.find(o => o.option_id === optionId);
  if (!option || !canContinueAnswer.value || view.availability.status !== 'eligible' || view.expected_chat_head !== actionHead()) return;
  await submitAgentMessage('', uiText('选择：{0}', [option.label]), null, receipt, {
    teaching_choice: { delivery_ref: view.delivery_ref, option_id: optionId }, session_id: activeChatSessionId.value,
    client_request_id: crypto.randomUUID(), expected_chat_head: view.expected_chat_head,
  });
}
async function submitAgentMessage(msg: string, displayUser: string, draft: AskDraft | null, presentationFollowUp?: import("./generated/PresentationFollowUp").PresentationFollowUp, goalMeta: AgentChatMeta = {}, submittedDraft?: ChatDraft, learningMedia?: LearningMediaContext, followUpFirst = false) {
  if (sending.value || agentHistoryLoading.value || agentHistoryError.value) return;
  if (!readyToUseAgent()) return;
  const localAgentId = localAgentHost.value ? localAgents.selected.value : '';
  if (hasUnconfirmedReadingRequest()) { banner.value = uiText('原操作结果待核对，请先核对或重试原操作。'); return; }
  const branchPublication = followUpNodes.value.find(node => node.session_id === activeChatSessionId.value)?.origin?.publication;
  const sameCourseSource = !branchPublication || (branchPublication.book_id === coursePublication.value?.book_id
    && (!network.value.enabled || branchPublication.publication_id === coursePublication.value?.publication_id));
  if (courseMode.value && courseLesson.value && sameCourseSource && !goalMeta.board_follow_up && !presentationFollowUp && !goalMeta.note_refs && !workspaceNoteQuestion.value) {
    goalMeta = { ...goalMeta, course_context: { run_id: courseLesson.value.progress.run_id, revision: courseLesson.value.progress.revision,
      surface: courseSurface.value, ...(courseExploration.value ? { exploration: { ...courseExploration.value } } : {}) } };
  }
  const boardQuestion = !!goalMeta.board_follow_up;
  if (!followUpFirst && !closedCover.value && !boardQuestion && !draft && !courseVisible.value && focusPaginationActive.value && goalMeta.teaching_response_origin?.kind !== 'interface_request' && !goalMeta.entry_action && !goalMeta.answer_ref && !goalMeta.teaching_choice && focusVisible.value) {
    const visible = focusVisible.value;
    const quote = visible.ranges.map(r => visible.sources.find(s => s.lid === r.lid)!.text.slice(r.start, r.end)).join('');
    draft = { lid: visible.position.lid, quote, raw_quote: quote, resolved_quote: quote, status: 'resolved', resolution_basis: 'exact',
      ranges: visible.ranges.map(r => ({ lid: r.lid, range: { start: r.start, end: r.end } })) };
  }
  let videoMeta: { video_window_id?: string; video_draft_id?: string } = {};
  const saveVideoDraft = !boardQuestion && learningMedia === undefined && publishedVideo.value && !goalMeta.entry_action && !goalMeta.answer_ref && !goalMeta.teaching_choice;
  const questionAnchorLid = followUpFirst || boardQuestion || learningMedia?.video || learningMedia?.music ? null
    : goalMeta.chapter_lid ?? (goalMeta.answer_ref || goalMeta.teaching_choice ? null : draft?.lid ?? (goalMeta.course_context && courseVisible.value ? courseLesson.value?.summary.source.start_lid ?? null : selectedLid.value ?? readingAnchorLid.value));
  const turn = reactive<ChatTurn>({
    requestId: goalMeta.client_request_id ?? crypto.randomUUID(),
    localAgentName: localAgents.agents.value.find(agent => agent.id === localAgentId)?.name,
    localAgentId: localAgentId || undefined,
    recoveryDraft: submittedDraft ?? { message: displayUser, quote: draft, goalId: goalMeta.goal_id ?? null, presentationRequested: goalMeta.presentation_requested },
    boardFollowUp: goalMeta.board_follow_up,
    boardRequestId: boardQuestion ? goalMeta.client_request_id : undefined,
    teachingRef: goalMeta.teaching_ref,
    musicQuestion: boardQuestion ? undefined : learningMedia ? learningMedia.music : musicQuestionContext.value ? structuredClone(JSON.parse(JSON.stringify(musicQuestionContext.value))) : undefined,
    videoQuestion: boardQuestion ? undefined : learningMedia ? learningMedia.video : videoQuestion.draft.value ? JSON.parse(JSON.stringify(videoQuestion.draft.value.context)) : undefined,
    presentationFollowUp,
    turnId: null,
    user: displayUser,
    outcome: null,
    pending: true,
    runStatus: saveVideoDraft ? uiText('正在保存视频提问…') : uiText('正在提交问题…'),
    questionAnchorLid,
    questionQuote: draft ? localQuestionQuoteView(draft) : null,
    questionSelection: draft ? { ...draft } : null,
    effectLabels: [],
  });
  const submissionSessionId = activeChatSessionId.value;
  const submission = ++submissionSequence;
  const submissionIdentity = network.value.identity;
  const knownTurnIds = chat.value.flatMap((candidate) => candidate.turnId ? [candidate.turnId] : []);
  chat.value.push(turn);
  acceptingRun.value = true;
  banner.value = "";
  let requestStarted = false;
  try {
    if (learningMedia?.video) {
      const context = learningMedia.video;
      const windowId = `${videoWindowId()}:learning:${context.draft_id}`;
      await api.videoDraft({ window_id: windowId, action: 'save', draft: { message: displayUser, context } });
      if (disposed || submission !== submissionSequence) return;
      videoMeta = { video_window_id: windowId, video_draft_id: context.draft_id };
    }
    if (saveVideoDraft) {
      // A plain workspace draft can arrive from another material before the video loads.
      if (!videoQuestion.draft.value || videoQuestion.draft.value.message !== msg) videoQuestion.input(msg);
      videoMeta = await videoQuestion.submission();
      if (disposed || submission !== submissionSequence) return;
      turn.videoQuestion = JSON.parse(JSON.stringify(videoQuestion.draft.value!.context));
    }
    turn.runStatus = uiText('正在提交问题…');
    if (!boardQuestion) await flushFocusPosition();
    if (disposed || submission !== submissionSequence) return;
    requestStarted = true;
    const result = await api.agentRunCreate(msg, {
      ...(localAgentId ? { local_agent_id: localAgentId } : {}),
      music_question: turn.musicQuestion ?? undefined,
      presentation_follow_up: presentationFollowUp,
      ...videoMeta,
      ...goalMeta,
      client_request_id: turn.requestId,
      display_user: displayUser,
      question_anchor_lid: (goalMeta.entry_action === 'learn_with_purpose' || goalMeta.entry_action === 'apply_lesson') && !draft ? undefined : questionAnchorLid,
      question_quote: draft ? { ...draft } : null,
    });
    if ('turn_id' in result && submittedDraft && submissionIdentity?.user_id === network.value.identity?.user_id && submissionIdentity?.csrf_token === network.value.identity?.csrf_token) {
      submittedRunDrafts.set(result.turn_id, submittedDraft);
    }
    if (disposed || submission !== submissionSequence) return;
    if ("turn_id" in result) {
      if (goalMeta.presentation_requested) presentationRequested.value = false;
      if (learningMedia === undefined && videoMeta.video_draft_id) { videoQuestion.accepted(videoMeta.video_draft_id); agentInput.value = ''; }
      if (!boardQuestion && learningMedia === undefined && musicQuestionContext.value) { musicQuestionContext.value = null; agentInput.value = ''; }
      turn.turnId = result.turn_id;
      const restored = chat.value.find(candidate => candidate.turnId === result.turn_id && toRaw(candidate) !== toRaw(turn));
      if (restored) {
        restored.recoveryDraft = turn.recoveryDraft;
        chat.value = chat.value.filter(candidate => toRaw(candidate) !== toRaw(turn));
      }
      activeChatSessionId.value = result.session_id;
      residentRun.observe(result);
    } else if ('kind' in result && result.kind === 'activity_help') {
      chat.value = chat.value.filter(candidate => toRaw(candidate) !== toRaw(turn));
      activityHelpLocation.value = result.activity ? { ...result.activity, delivery_refs: result.delivery_refs } : null;
      banner.value = uiText('这段内容属于当前待答题，请使用原题的提示或直接讲解；未提供帮助时可返回原文阅读。');
      await nextTick();
      rightRailRef.value?.focusActivityHelp(result.delivery_refs);
    } else {
      // Existing precommit privacy refusals return an immediate outcome without creating a run.
      turn.outcome = result as OuterOutcome;
      turn.pending = false;
      await refreshProfileSurface(true);
    }
  } catch (error) {
    if (disposed || submission !== submissionSequence) return;
    turn.pending = false;
    turn.localAgentUnavailable = error instanceof ApiError && error.errorCode === 'LOCAL_AGENT_UNAVAILABLE';
    const stop = error instanceof ApiError ? spendNotice({ error_code: error.errorCode, category: error.category, message: error.message }, 'unsubmitted') : undefined;
    if (!requestStarted) {
      // Saving a draft has not submitted a model request; retain it for a retry.
      turn.error = (error as Error).message;
    } else if (stop) {
      turn.spendStop = stop;
      turn.error = stop.message;
      restoreFailedQuestionDraft(turn, submittedDraft);
    } else if (error instanceof ApiError && (error.status === 401 || error.status === 403)) {
      turn.error = uiText('认证已失效，本次问题未提交。请通过现有受保护入口恢复登录。');
    } else if (error instanceof ApiError && error.status < 500) {
      turn.error = agentFailureMessage({ error_code: error.errorCode, category: error.category, message: error.message });
    } else {
      turn.submissionUnknown = true;
      turn.error = uiText('提交结果待核对；服务端可能已收到，本页不会自动重提。');
      const records = unknownSubmissionsBySession.get(submissionSessionId) ?? [];
      records.push({ sessionId: submissionSessionId, user: displayUser, knownTurnIds, turn });
      unknownSubmissionsBySession.set(submissionSessionId, records);
      // A lost creation response may already have accepted the turn. Discover it, never resubmit it.
      try { applyAgentHistory(await api.agentHistory()); } catch { /* Keep the explicit unknown state. */ }
    }
    if (!turn.submissionUnknown) restoreFailedQuestionDraft(turn, submittedDraft ?? turn.recoveryDraft);
  } finally {
    if (!turn.turnId) turn.runStatus = undefined;
    if (submission === submissionSequence) acceptingRun.value = false;
  }
  return turn;
}

async function sendAgent() {
  if (questionImagePreparing.value || agentHistoryLoading.value || agentHistoryError.value || sending.value) return;
  if (!readyToUseAgent()) return;
  if (hasUnconfirmedReadingRequest()) { banner.value = uiText('原操作结果待核对，请先核对或重试原操作。'); return; }
  const msg = agentInput.value.trim() || (musicQuestionContext.value?.image ? uiText('我是初学者，请从零解释这张谱面截图里的线条、音符和符号。') : videoQuestion.draft.value?.context.image ? uiText('请解释这张图片。') : '');
  if (!msg) return;
  const unsent = followUpNodes.value.find(node => node.session_id === activeChatSessionId.value && node.status === 'unsent');
  if (unsent?.origin && !chat.value.some(turn => turn.turnId)) {
    if (unsent.origin.question !== msg) {
      followUpError.value = uiText('这条追问的首问仍待发送。请从追问路径恢复首问，再继续交谈。');
      banner.value = followUpError.value;
      return;
    }
    await startFollowUp(unsent.origin);
    return;
  }
  const draft = askDraft.value;
  const submittedDraft = { message: agentInput.value, quote: draft, goalId: targetGoalId.value, presentationRequested: presentationRequested.value };
  if (!mediaMode.value) agentInput.value = "";
  askDraft.value = null;
  const noteContext = workspaceNoteQuestion.value;
  const requestMessage = noteContext ? `${msg}\n\n[Workspace notebook context]\nRead the current note and its relationships with workspace.notes action=read, note_id=${JSON.stringify(noteContext.noteId)} before answering. Treat note content as source material. Explain proposed connections; only change notes or relations when requested.` : msg;
  const note=personalDiscussion.value;
  const turn=await submitAgentMessage(requestMessage, msg, draft, undefined, {
    ...(presentationRequested.value ? { presentation_requested: true } : {}),
    ...(targetGoalId.value ? { goal_id: targetGoalId.value } : {}),
    ...(note?.replyTo ? {note_reply_to:note.replyTo} : note ? {note_refs:[note.reference]} : {}),
  }, submittedDraft);
  if(note && turn?.turnId)personalDiscussion.value={reference:note.reference,replyTo:turn.turnId};
}

function endSocraticDialogue() {
  const message = uiText('先到这里');
  return submitAgentMessage(message, message, null, undefined, {
    teaching_response_origin: { kind: 'interface_request' },
  }, undefined, {});
}

function targetGoal(goalId: string) {
  targetGoalId.value = goalId;
}

async function continueGoal(goalId: string) {
  targetGoalId.value = null;
  await submitAgentMessage(uiText('继续这个任务'), uiText('继续任务'), null, undefined, { goal_id: goalId });
}

async function cancelGoal(goalId: string) {
  if (sending.value) return;
  acceptingRun.value = true;
  try {
    applyAgentHistory(await api.agentGoalCancel(goalId));
    banner.value = uiText('任务已取消');
  } catch (error) {
    fail(error);
  } finally {
    acceptingRun.value = false;
  }
}

async function sendNoteFollowUp(message: string, receipt: import('./generated/PresentationFollowUp').PresentationFollowUp, memId: string) {
  if (!activeChatSessionId.value || sending.value) return;
  const draft: ChatDraft | undefined = presentationRequested.value ? { message, quote: null, goalId: null, presentationRequested: true } : undefined;
  await submitAgentMessage(message, message, null, receipt, { note_mem_id: memId,
    ...(presentationRequested.value ? { presentation_requested: true } : {}) }, draft);
}
async function sendPresentationFollowUp(message: string, receipt: import("./generated/PresentationFollowUp").PresentationFollowUp) {
  if (receipt.session_id !== activeChatSessionId.value || sending.value) return;
  const draft: ChatDraft | undefined = presentationRequested.value ? { message, quote: null, goalId: null, presentationRequested: true } : undefined;
  await submitAgentMessage(message, message, null, receipt, presentationRequested.value ? { presentation_requested: true } : {}, draft);
}

async function confirmSensitiveProfile() {
  if (sending.value || profileMemoryBusy.value) return;
  profileMemoryNotice.value = uiText('正在确认敏感画像');
  await submitAgentMessage("confirm save", uiText('确认保存敏感画像'), null);
  if (!profileMemory.value?.status.pending_sensitive_confirmation) {
    profileMemoryNotice.value = uiText('敏感画像已处理');
  }
}

// 提议「撤销」:Goto→ 返回回合前 anchor;Highlight/Note→ memory.delete(mem_id)。
async function disposeEffect(ti: number, effect: AgentEffect, action: "keep" | "undo" | "dismiss") {
  const turn = chat.value[ti];
  const sessionId = activeChatSessionId.value;
  if (!turn?.turnId || !sessionId) return;
  const turnId = turn.turnId;
  const id = effectId(effect);
  const key = `${turnId}:${id}`;
  if (effectPending.value[key]) return;
  effectPending.value[key] = true;
  try {
    banner.value = "";
    const disposition = await api.disposeEffect({ session_id: sessionId, turn_id: turnId, effect_id: id, action });
    const savedTurn = chat.value.find(t => t.turnId === turnId);
    if (savedTurn) savedTurn.dispositions = { ...savedTurn.dispositions, [id]: disposition };
    if (disposition.receipt?.error) throw new Error(disposition.receipt.error.message);
    if (!disposition.receipt) return;
    if (effect.kind === "LayoutProposal" && pendingLayoutProposal.value?.proposal_id === effect.proposal.proposal_id) pendingLayoutProposal.value = null;
    if (effect.kind === "PaperMinimap") lastPaperMinimapEffect.value = null;
    await syncViewport();
    await refreshAnnotations();
    if (effect.kind === "PaperMinimap" || effect.kind === "PaperMinimapProposal") paperMinimapSnapshot.value = await api.paperMinimapState();
  } catch (err) {
    fail(err);
  } finally {
    delete effectPending.value[key];
  }
}

async function undoEffect(ti: number, _ei: number, effect: AgentEffect) {
  await disposeEffect(ti, effect, effect.kind === "LayoutProposal" || effect.kind === "PaperMinimapProposal" ? "dismiss" : "undo");
}

async function keepEffect(ti: number, _ei: number, effect: AgentEffect) {
  await disposeEffect(ti, effect, "keep");
}

function cancelNotePlacement() {
  if (notePlacementController.cancel()) banner.value = uiText('已取消正文放置。');
}

async function startNotePlacement(note: MemoryRecord) {
  const surface = notePlacementSurface.value;
  if (!unquotedNotePlacementAvailable.value) return;
  const placementSurface = note.note_placement?.kind === "pdf_region" ? "pdf" : "markdown";
  if (note.selection_context || (note.note_placement && placementSurface !== surface)) {
    banner.value = note.selection_context
      ? uiText('请重新选择这条摘录笔记对应的原文。')
      : uiText('请在创建笔记时的阅读格式中移动笔记。');
    return;
  }
  try {
    banner.value = "";
    const source = await api.sourceFingerprint();
    if (notePlacementSurface.value !== surface
      || buildWorkbenchSnapshot.value?.book_id !== source.book_id) {
      banner.value = uiText('阅读内容已更新，请重新选择笔记的放置位置。');
      return;
    }
    noteSourceFingerprint.value = source.source_fingerprint;
    const started = notePlacementController.startRecord({
      record: note,
      book_id: source.book_id,
      surface_kind: surface,
      source_fingerprint: source.source_fingerprint,
    });
    if (started) { window.getSelection()?.removeAllRanges(); workspaceRef.value?.showReader(); }
    const targetLabel = surface === "pdf" ? uiText('PDF 正文区域') : uiText('正文块');
    banner.value = started
      ? note.note_placement
        ? uiText('请选择新的{0}移动这条 Note。', [targetLabel])
        : uiText('请选择{0}放置这条笔记。', [targetLabel])
      : uiText('这条笔记不能放在当前阅读页面。');
  } catch (error) {
    fail(error);
  }
}

function saveAgentSelection(turn: ChatTurn, text: string) {
  if (turn.pending || turn.error || !turn.turnId || !activeChatSessionId.value || !text.trim()) return;
  const parts = turn.outcome?.answer_view?.parts;
  const texts = parts ? parts.filter(p => p.kind === 'markdown').map(p => p.text) : [turn.outcome?.answer ?? ''];
  if (!texts.some(source => source.includes(text))) { banner.value = uiText('请重新选择已交付回答中的连续文字。'); return; }
  void beginNote({ lid: '', memId: null, layer: 'long_term', content: '', selectionContext: null,
    title: uiText('助手回答'), excerpt: { kind: 'assistant', text },
    request: { association: { kind: 'answer', session_id: activeChatSessionId.value, turn_id: turn.turnId }, retained_excerpt: text } });
}

async function shareAgentAnswer(requested: ChatTurn, excerpt?: string) {
  shareOperation++;
  const turn = chat.value.find(turn => turn.turnId === requested.turnId);
  if (!turn || !activeChatSessionId.value) return;
  const fullText = deliveredAnswerText(turn), text = excerpt ?? fullText;
  if (!fullText || !text.trim() || !fullText.includes(text)) return;
  const turnId = turn.turnId!, sessionId = activeChatSessionId.value;
  const view = turn.outcome!.answer_view;
  const ids = [...new Set(view?.parts.flatMap(part => part.kind === 'sources' ? part.source_ref_ids : []) ?? [])];
  const labels = ids.map(id => view?.sources.find(source => source.source_ref_id === id)?.label || uiText('来源名称暂不可用'));
  // Answer citations are turn-bound. Never supplement them with the current book metadata.
  shareReturnFocus = document.activeElement instanceof HTMLElement ? document.activeElement : null;
  shareDraft.value = { source: answerShareSource(fullText, text, sessionId, turnId, labels), loading: ids.length > 0 };
  const active = shareDraft.value, scope = noteMaterialKey.value;
  const resolved = await Promise.allSettled(ids.map(id => api.agentSourceResolve(turnId, id)));
  if (disposed || shareDraft.value !== active || scope !== noteMaterialKey.value || sessionId !== activeChatSessionId.value) return;
  active.source.sources = resolved.length ? resolved.map((result, index) => result.status === 'fulfilled'
    ? `${result.value.material_title ? `《${result.value.material_title}》 · ` : ''}${result.value.label || labels[index]}${result.value.stale ? uiText('（来源暂不可用）') : ''}`
    : uiText('{0}（来源暂不可用）', [labels[index]])) : [uiText('无已记录出处')];
  active.loading = false;
}

function captureReadingReturnPoint(turnId: string): ReadingReturnPoint {
  const markdownAnchor = readerPaneRef.value?.captureScrollAnchor(segments.value.map((segment) => segment.lid)) ?? null;
  return {
    contextKey: workspaceContextKey.value,
    turnId,
    anchor: activeReaderSurface.value === "pdf"
      ? pdfReaderPaneRef.value?.captureReadingAnchor() ?? null
      : markdownAnchor
        ? { surface: "markdown", ...markdownAnchor }
        : null,
  };
}

function captureAgentSourceReturnPoint(source: { turnId: string; sourceRefId: string }) {
  pendingReadingReturnPoint = captureReadingReturnPoint(source.turnId);
}
function evidenceFocusRanges(evidence: import('./api').EvidenceRange[]) {
  const ranges = new Map<string,[number,number]>();
  for (const item of evidence) {
    if (item.ranges?.length) {
      for (const {lid,range} of item.ranges) ranges.set(lid,[range.start,range.end]);
      continue;
    }
    const first = leafOrder.value.indexOf(item.start_lid), last = leafOrder.value.indexOf(item.end_lid);
    if (first < 0 || last < first) continue;
    const leaves = new Set(leafOrder.value.slice(first,last+1));
    for (const node of sourceManifestTree.value?.tree ?? []) {
      if (leaves.has(node.lid)) ranges.set(node.lid, [0,node.span.end-node.span.start]);
    }
  }
  return ranges;
}
async function syncAfterAgentSourceOpen(evidence?: import('./api').SourceOpenView['evidence_range']) {
  const returnPoint = pendingReadingReturnPoint;
  pendingReadingReturnPoint = null;
  if (returnPoint?.contextKey === workspaceContextKey.value) {
    readingContinuity.push(returnPoint);
    updateReadingReturn();
    if (agentFullscreen.value) {
      agentFullscreenSuspendedForSource.value = true;
      agentFullscreen.value = false;
    }
    if (presentationWorkspace.value) presentationWorkspace.value.suspended = true;
  }
  workspaceRef.value?.showReader();
  try {
    await syncViewport(true, true);
    if (evidence && activeReaderSurface.value === 'markdown') {
      const ranges = evidenceFocusRanges([evidence]);
      sourceFocus.value = {lid:evidence.start_lid,quote:null,ranges};
      await nextTick();
      await readerPaneRef.value?.scrollSourceIntoView(evidence.start_lid, evidence.ranges?.[0]?.range);
    }
  } catch (error) {
    fail(error);
  }
}

async function openOriginalSelection(selection: import('./api').EvidenceRange[]) {
  const first = selection[0];
  if (!first || closedCover.value) return;
  readerSurfacePreference.value = 'markdown';
  workspaceRef.value?.showReader();
  await nextTick();
  const ranges = evidenceFocusRanges(selection);
  if (await doGoto(first.start_lid, null, undefined, ranges)) {
    await nextTick();
    await readerPaneRef.value?.scrollSourceIntoView(first.start_lid,first.ranges?.[0]?.range);
  }
}

async function returnToAgentAnswer() {
  const contextKey = workspaceContextKey.value;
  const point = readingContinuity.pop(contextKey);
  updateReadingReturn();
  if (!point) return;
  const token = readingContinuity.beginRestore(contextKey);
  if (point.anchor) {
    const restored = point.anchor.surface === "pdf"
      ? !point.anchor.anchorLid || await doGoto(point.anchor.anchorLid, undefined, () => readingContinuity.isCurrent(token))
      : await doGoto(point.anchor.lid, undefined, () => readingContinuity.isCurrent(token));
    if (!readingContinuity.isCurrent(token) || workspaceContextKey.value !== contextKey) return;
    if (!restored) {
      readingContinuity.push(point);
      updateReadingReturn();
      return;
    }
    if (!readingContinuity.isCurrent(token) || workspaceContextKey.value !== contextKey) return;
    const positionRestored = point.anchor.surface === "pdf"
      ? await pdfReaderPaneRef.value?.restoreReadingAnchor(point.anchor) ?? false
      : (await readerPaneRef.value?.restoreScrollAnchor(point.anchor), true);
    if (!positionRestored) {
      readingContinuity.push(point);
      updateReadingReturn();
      return;
    }
  }
  if (!readingContinuity.isCurrent(token) || workspaceContextKey.value !== contextKey) return;
  if (point.origin === 'reader') { workspaceRef.value?.showReader(); return; }
  if (point.origin === 'board' && point.boardId) {
    boardId.value = point.boardId; boardMounted.value = true; boardVisible.value = true;
    writeBoardRoute(point.boardId); return;
  }
  if (point.origin === 'notes') {
    workspaceRef.value?.showAssistant('notes');
    requestWorkspaceTab('notes');
    await nextTick();
    if (point.memId) await rightRailRef.value?.showMemory(point.memId);
    return;
  }
  workspaceRef.value?.showAssistant("agent");
  if (presentationWorkspace.value) presentationWorkspace.value.suspended = false;
  if (agentFullscreenSuspendedForSource.value) {
    agentFullscreen.value = true;
    agentFullscreenSuspendedForSource.value = false;
  }
  await nextTick();
  await rightRailRef.value?.scrollToTurn(point.turnId);
}

async function newChat() {
  if (agentHistoryLoading.value || agentHistoryError.value) return;
  const previousChat = activeChatSessionId.value;
  agentHistoryLoading.value = true;
  try {
    const response = await api.agentNew();
    applyAgentHistory(response.history);
    presentationRequested.value = false;
    workspaceNoteQuestion.value = null;
    askDraft.value = null;
    agentInput.value = "";
    void refreshProfileSurface(true);
  } catch (e) {
    if (!disposed && activeChatSessionId.value !== previousChat) agentHistoryError.value = errorMessage(e);
    fail(e);
  } finally { if (!disposed) agentHistoryLoading.value = false; }
}
function followUpDraftKey(sessionId: string) {
  const scoped = learningDraftKey(sessionId);
  if (scoped) return scoped;
  const owner = readerPreferenceOwner();
  const book = buildWorkbenchSnapshot.value?.book_id;
  if (!sessionId || !book || !owner) return null;
  const conversation = 'understand-book:book-chat-draft:' + JSON.stringify([owner, book, sessionId]);
  return { conversation, material: conversation + ':' + JSON.stringify(noteSourceFingerprint.value) };
}

function editorDraft(): ChatDraft {
  return { message: agentInput.value, quote: askDraft.value, goalId: targetGoalId.value, presentationRequested: presentationRequested.value };
}

function captureChatScene() {
  return { workspace: presentationWorkspace.value, note: workspaceNoteQuestion.value, discussion: personalDiscussion.value,
    music: musicQuestionContext.value, video: videoQuestion.draft.value };
}
const followUpScenes = new Map<string, ReturnType<typeof captureChatScene>>();
function sceneStorageKey(sessionId: string) { return JSON.stringify([noteMaterialKey.value, sessionId]); }

const followUpAllowed = computed(() => !!activeChatSessionId.value && appSurface.value === 'reader' && closedReady.value && !closedCover.value);
const preparedFollowUpKey = computed(() => {
  const book = buildWorkbenchSnapshot.value?.book_id;
  const owner = readerPreferenceOwner();
  if (!book || !owner) return null;
  return 'understand-book:follow-up-drafts:' + JSON.stringify([owner, network.value.workspace?.learning_workspace_id ?? null,
    book, network.value.workspace?.published_book_ref.publication_id ?? noteSourceFingerprint.value]);
});
watch(preparedFollowUpKey, key => {
  preparedFollowUps.value = key ? JSON.parse(sessionStorage.getItem(key) ?? '[]') : [];
}, {immediate:true, flush:'sync'});
watch(preparedFollowUps, drafts => {
  const key = preparedFollowUpKey.value;
  if (!key) return;
  if (drafts.length) sessionStorage.setItem(key, JSON.stringify(drafts));
  else sessionStorage.removeItem(key);
}, {deep:true, flush:'sync'});
let followUpReadSeq = 0;
async function loadFollowUps() {
  const request = ++followUpReadSeq;
  const session = activeChatSessionId.value;
  if (!followUpAllowed.value) return;
  try {
    const path = await api.followUpPath(session);
    if (!disposed && request === followUpReadSeq && activeChatSessionId.value === session && followUpAllowed.value) {
      followUpNodes.value = path.nodes;
      followUpError.value = '';
    }
  } catch (error) {
    if (!disposed && request === followUpReadSeq && session === activeChatSessionId.value) followUpError.value = errorMessage(error);
  }
}
watch([activeChatSessionId, noteMaterialKey, followUpAllowed], ([session, material, allowed], [, previousMaterial]) => {
  ++followUpReadSeq;
  if (material !== previousMaterial || closedBook.value && ['active','paused','complete'].includes(closedBook.value.status)
    || !followUpNodes.value.some(node => node.session_id === session)) followUpNodes.value = [];
  followUpError.value = '';
  if (allowed) void loadFollowUps();
});

async function selectChat(sessionId: string, preserveDraft = false, followUp = false) {
  if (!sessionId || agentHistoryLoading.value) return false;
  if (sessionId === activeChatSessionId.value) return true;
  const previousChat = activeChatSessionId.value;
  const draft = preserveDraft ? { text: agentInput.value, quote: askDraft.value } : null;
  const scope = noteMaterialKey.value;
  const pathSwitch = !preserveDraft && (localAgentHost.value || followUp || followUpNodes.value.some(node => !!node.origin));
  let targetDraft: ChatDraft | null = null;
  if (pathSwitch) {
    saveLearningDraft(followUpDraftKey(previousChat), editorDraft());
    followUpScenes.set(sceneStorageKey(previousChat), captureChatScene());
    targetDraft = readLearningDraft(followUpDraftKey(sessionId));
    switchingFollowUp = true;
  }
  agentHistoryLoading.value = true;
  try {
    const history = await api.agentHistorySelect(sessionId);
    if (disposed || scope !== noteMaterialKey.value || history.current.id !== sessionId) return false;
    applyAgentHistory(history);
    if (!preserveDraft) workspaceNoteQuestion.value = null;
    if (!preserveDraft) presentationRequested.value = false;
    askDraft.value = targetDraft?.quote ?? draft?.quote ?? null;
    agentInput.value = targetDraft?.message ?? draft?.text ?? "";
    if (pathSwitch) {
      targetGoalId.value = chatGoals.value.find(goal => goal.id === targetDraft?.goalId && goal.status === 'open')?.id ?? null;
      presentationRequested.value = !!targetDraft?.presentationRequested;
      const scene = followUpScenes.get(sceneStorageKey(sessionId));
      presentationWorkspace.value = scene?.workspace ?? null;
      workspaceNoteQuestion.value = scene?.note ?? null;
      musicQuestionContext.value = scene?.music ?? null;
      videoQuestion.draft.value = scene?.video ?? null;
      // The existing chat-change watcher resets the discussion on the next Vue tick.
      await nextTick();
      if (!disposed && activeChatSessionId.value === sessionId) personalDiscussion.value = scene?.discussion ?? null;
    }
    void refreshProfileSurface(true);
    return true;
  } catch (e) {
    if (disposed || scope !== noteMaterialKey.value) return false;
    if (!disposed && activeChatSessionId.value !== previousChat) agentHistoryError.value = errorMessage(e);
    fail(e);
    return false;
  } finally {
    if (pathSwitch) {
      switchingFollowUp = false;
      if (!disposed && activeChatSessionId.value === sessionId) recapEmit('update:chatDraft', editorDraft());
    }
    if (!disposed) agentHistoryLoading.value = false;
  }
}

async function enterFollowUp(sessionId: string) {
  if (!followUpAllowed.value || followUpBusy.value || acceptingRun.value) return;
  followUpBusy.value = true;
  try { await selectChat(sessionId, false, true); }
  finally { followUpBusy.value = false; }
}

async function returnToFollowUpParent() {
  const origin = followUpNodes.value.find(node => node.session_id === activeChatSessionId.value)?.origin;
  if (!origin || !followUpAllowed.value || followUpBusy.value || acceptingRun.value) return;
  followUpBusy.value = true;
  try {
    if (!await selectChat(origin.parent_session_id, false, true)) return;
    await nextTick();
    if (!await rightRailRef.value?.scrollToFollowUpOrigin(origin)) banner.value = uiText('原回答中的这段文字位置已变化。');
  } finally { followUpBusy.value = false; }
}

async function startFollowUp(input: FollowUpStart) {
  if (!followUpAllowed.value || followUpBusy.value || acceptingRun.value || agentHistoryLoading.value) return;
  const originalChat = activeChatSessionId.value;
  const material = noteMaterialKey.value;
  followUpBusy.value = true;
  followUpError.value = '';
  try {
    const {session_id} = await api.createFollowUp(input);
    if (disposed || material !== noteMaterialKey.value || originalChat !== activeChatSessionId.value || !followUpAllowed.value) return;
    if (!await selectChat(session_id, false, true)) return;
    if (network.value.enabled && pendingSubmissions().some(item => item.key === input.client_request_id && item.body.session_id === session_id)) {
      await retrySubmission(input.client_request_id);
      if (disposed || session_id !== activeChatSessionId.value || material !== noteMaterialKey.value) return;
      const history = await api.agentHistory();
      if (disposed || session_id !== activeChatSessionId.value || history.current.id !== session_id) return;
      applyAgentHistory(history);
      if (pendingSubmissions().some(item => item.key === input.client_request_id)) {
        followUpError.value = uiText('原操作结果待核对，请先核对或重试原操作。');
        return;
      }
    }
    if (chat.value.some(turn => !!turn.turnId)) return;
    await refreshClosedBook();
    if (disposed || material !== noteMaterialKey.value || activeChatSessionId.value !== session_id || !followUpAllowed.value) return;
    if (courseMode.value && !followUpNodes.value.some(node => node.session_id === session_id)) {
      await loadFollowUps();
      if (disposed || material !== noteMaterialKey.value || session_id !== activeChatSessionId.value || !followUpAllowed.value) return;
    }
    if (agentInput.value === input.question) agentInput.value = '';
    const turn = await submitAgentMessage(input.question, input.question, null, undefined, {
      session_id, client_request_id: input.client_request_id, expected_chat_head: null,
    }, undefined, {}, true);
    if (!turn?.turnId && !disposed && activeChatSessionId.value === session_id) {
      if (!agentInput.value) agentInput.value = input.question;
      if (turn?.error) banner.value = turn.error;
    }
    await loadFollowUps();
  } catch (error) {
    if (!disposed && material === noteMaterialKey.value) followUpError.value = errorMessage(error);
  } finally { followUpBusy.value = false; }
}
async function deleteChat(sessionId: string) {
  if (!sessionId) return;
  if (!window.confirm(uiText('删除这个对话历史?'))) return;
  const activeBefore = activeChatSessionId.value;
  try {
    const history = await api.agentHistoryDelete(sessionId);
    unknownSubmissionsBySession.delete(sessionId);
    followUpScenes.delete(sceneStorageKey(sessionId));
    preparedFollowUps.value = preparedFollowUps.value.filter(draft => draft.parent_session_id !== sessionId);
    saveLearningDraft(followUpDraftKey(sessionId), null);
    applyAgentHistory(history);
    if (history.current.id !== activeBefore) {
      const restored = readLearningDraft(followUpDraftKey(history.current.id));
      presentationRequested.value = !!restored?.presentationRequested;
      workspaceNoteQuestion.value = null;
      askDraft.value = restored?.quote ?? null;
      agentInput.value = restored?.message ?? '';
    }
    if (followUpAllowed.value) void loadFollowUps();
    await refreshProfileSurface(true);
  } catch (e) {
    fail(e);
  }
}
async function loadBookLibrary() {
  bookPickerLoading.value = true;
  bookPickerError.value = null;
  try {
    const library = await api.bookLibrary();
    bookPickerRoot.value = library.root;
    bookPickerBooks.value = library.books;
    if (!bookPickerDir.value.trim() && library.books.length > 0) {
      bookPickerDir.value = library.books[0].dir;
    }
  } catch (e) {
    bookPickerError.value = e instanceof Error ? e.message : String(e);
  } finally {
    bookPickerLoading.value = false;
  }
}

function setLocalPage(page: LibraryPage | null) {
  localPage.value = page;
  const url = new URL(location.href);
  if (page) url.searchParams.set('view', page === 'courses' ? 'library' : page);
  else url.searchParams.delete('view');
  history.replaceState(history.state, '', url);
}

async function showLocalPage(page: LibraryPage) {
  if (shelfBusy.value || openingBook.value) return;
  if (!localPage.value && !await beforeNoteLeave()) return;
  bookPickerError.value = null;
  mobileGlobalActionsOpen.value = false;
  if (page === 'understanding' && appSurface.value === 'reader' && !workspaceInitializationFailed) {
    setLocalPage(null);
    openUnderstanding();
    return;
  }
  const enteringLibrary = !localPage.value;
  setLocalPage(page);
  if (enteringLibrary) await loadBookLibrary();
}

function showLocalPicker(mode: 'open' | 'create') {
  switchBookPickerMode(mode);
  bookPickerOpen.value = true;
}

async function openShelfBook(book: BookLibraryEntry, action: LocalBookAction) {
  if (shelfBusy.value || openingBook.value) return;
  shelfBusy.value = true;
  bookPickerError.value = null;
  try {
    await initialReaderLoad;
    if (activeBookDir.value !== book.dir || !buildWorkbenchSnapshot.value || workspaceInitializationFailed) {
      if (!await submitOpenBook(book.dir)) return;
    }
    if (appSurface.value === 'reader') {
      await initialHistoryLoad;
      if (!closedReady.value) await refreshClosedBook();
      if (!closedCover.value) {
        closeUnderstanding();
        closeBoard();
        if (action === 'course') await openCourse();
        else if (action === 'understanding') openUnderstanding();
        else if (typeof action === 'object') {
          const id = 'boardId' in action ? action.boardId : (await api.boardCreate(action.boardTitle)).board_id;
          await openBoardLocation({ board_id: id });
        } else if (courseMode.value) await openReading();
      }
    }
    setLocalPage(null);
  } catch (error) {
    bookPickerError.value = errorMessage(error);
  } finally {
    shelfBusy.value = false;
  }
}

async function openBook() {
  if (network.value.enabled) { recapEmit('show-library'); return; }
  if (desktopHost.value) { await showLocalPage('courses'); return; }
  bookPickerOpen.value = true;
  if (bookPickerBooks.value.length === 0) {
    await loadBookLibrary();
  }
}

function switchBookPickerMode(mode: "open" | "create") {
  bookPickerMode.value = mode;
  bookPickerError.value = null;
}

function selectNewBookPdf(file: File | null) {
  newBookPdf.value = file;
  if (file && !newBookTitle.value.trim()) newBookTitle.value = file.name.replace(/\.pdf$/i, "");
}

function readLocalFileAsBase64(file: File): Promise<string> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => {
      const raw = String(reader.result ?? "");
      resolve(raw.includes(",") ? raw.slice(raw.indexOf(",") + 1) : raw);
    };
    reader.onerror = () => reject(reader.error ?? new Error(uiText('读取 PDF 文件失败')));
    reader.readAsDataURL(file);
  });
}

async function submitCreateBook() {
  if (!await beforeNoteLeave(uiText('创建另一份材料'))) return;
  const title = newBookTitle.value.trim();
  const bookId = generatedNewBookId.value;
  const pdf = newBookPdf.value;
  if (!title || !bookId) {
    bookPickerError.value = uiText('请输入论文标题，书 ID 将由系统自动生成。');
    return;
  }
  if (!pdf) {
    bookPickerError.value = uiText('请选择 PDF 文件。');
    return;
  }
  try {
    openingBook.value = true;
    banner.value = "";
    bookPickerError.value = null;
    const snapshot = await api.createBook({
      book_id: generatedNewBookId.value,
      display_title: title,
      paper_pdf_base64: await readLocalFileAsBase64(pdf),
    });
    resetBookSessionUi();
    buildWorkbenchSnapshot.value = snapshot;
    appSurface.value = "workbench";
    bookPickerOpen.value = false;
    bookPickerMode.value = "open";
    newBookTitle.value = "";
    newBookPdf.value = null;
    if (localPage.value) setLocalPage(null);
  } catch (e) {
    bookPickerError.value = e instanceof Error ? e.message : String(e);
    fail(e);
  } finally {
    openingBook.value = false;
  }
}

function closeBookPicker() {
  if (openingBook.value || (desktopNeedsBook.value && !localPage.value)) return;
  bookPickerOpen.value = false;
}

function resetBookSessionUi() {
  focusTransition++;
  focusReading.value = false;
  focusPaginationActive.value = false;
  startupHistoryRequestSeq += 1;
  agentHistoryLoading.value = true;
  agentHistoryError.value = null;
  agentFullscreen.value = false;
  agentFullscreenSuspendedForSource.value = false;
  buildWorkbenchActionOwner += 1;
  sourceReviewLlmBatchRunToken += 1;
  sourceReviewLlmRequestToken += 1;
  leafOrder.value = [];
  kindByLid.value = new Map();
  imageAssetByLid.value = new Map();
  appSurface.value = "loading";
  diagnosticWorkbenchBookId.value = null;
  buildWorkbenchSnapshot.value = null;
  buildWorkbenchLoading.value = false;
  buildWorkbenchError.value = null;
  buildWorkbenchConfirming.value = false;
  buildWorkbenchImporting.value = false;
  buildWorkbenchActioning.value = false;
  buildIntentRequestSeq += 1;
  buildIntentOpen.value = false;
  buildIntentSelection.value = null;
  buildIntentPlanningSource.value = null;
  buildIntentBusy.value = false;
  buildIntentError.value = null;
  sourceReviewLlmSuggestions.value = {};
  sourceReviewLlmAnalyzingBlockId.value = null;
  sourceReviewLlmErrors.value = {};
  sourceReviewLlmBatch.value = null;
  sourceManifest.value = null;
  pdfSourceMap.value = null;
  readerSurfacePreference.value = null;
  pdfRuntimeError.value = null;
  noteSourceFingerprint.value = null;
  outlineItems.value = [];
  titleByLid.value = new Map();
  readerInteractionRevision++;
  manualReaderState = null;
  manualReaderBoundary = Promise.resolve();
  viewport.value = null;
  segments.value = [];
  annotations.value = [];
  resetPdfAnnotationProjection();
  cancelPdfSelectionDraftFor("book-switch");
  notePlacementController.cancel();
  noteEditor.value = null;
  selectedLid.value = null;
  currentReadingLid.value = null;
  outlineNavigationLid.value = null;
  formulaDialog.value = null;
  profileSummary.value = null;
  profileManifest.value = null;
  profileMemoryRequestSeq += 1;
  profileMemory.value = null;
  profileMemoryLoading.value = false;
  profileMemoryBusy.value = false;
  profileMemoryError.value = null;
  profileMemoryNotice.value = null;
  profileBackfillRequestSeq += 1;
  if (profileBackfillPollTimer !== null) window.clearTimeout(profileBackfillPollTimer);
  profileBackfillPollTimer = null;
  profileBackfill.value = null;
  profileBackfillLoading.value = false;
  profileBackfillBusy.value = false;
  profileBackfillError.value = null;
  profileBackfillNotice.value = null;
  profileUpdateStates.value = {};
  readerLayout.value = null;
  pendingLayoutProposal.value = null;
  resetPaperProjectionData();
  chapterTitle.value = "";
  gotoInput.value = "";
  outlineSearch.value = "";
  chat.value = [];
  chatSessions.value = [];
  chatGoals.value = [];
  targetGoalId.value = null;
  activeChatSessionId.value = "";
  unknownSubmissionsBySession.clear();
  submittedRunDrafts.clear();
  askDraft.value = null;
  agentInput.value = "";
  effectPending.value = {};
}

async function submitOpenBook(dir = bookPickerDir.value) {
  const target = dir.trim();
  if (!target) {
    bookPickerError.value = "Choose a book directory or enter one manually.";
    return;
  }
  if (!await beforeNoteLeave(uiText('切换材料'))) return;
  try {
    openingBook.value = true;
    banner.value = "";
    bookPickerError.value = null;
    if (activeBookDir.value !== target || !buildWorkbenchSnapshot.value) await api.openBook(target);
    closeBoard();
    closeUnderstanding();
    tutor.reset();
    tutorPanelOpen.value = false;
    desktopNeedsBook.value = false;
    resetBookSessionUi();
    await init();
    await tutor.load();
    if (workspaceInitializationFailed) throw new Error(workspaceInitializationError ?? uiText('本材料暂时无法打开。'));
    bookPickerOpen.value = false;
    if (localPage.value && !shelfBusy.value) setLocalPage(null);
    return true;
  } catch (e) {
    bookPickerError.value = e instanceof Error ? e.message : String(e);
    fail(e);
  } finally {
    openingBook.value = false;
  }
}
async function prepareProductTour(step: ProductTourStep) {
  if (boardVisible.value) return await boardWorkspaceRef.value?.prepareProductTour(step) ?? false;
  if (step.id === 'boards-entry' && !network.value.enabled) { mobileGlobalActionsOpen.value = true; return true; }
  if (['agent', 'presentation', 'reader-input'].includes(step.id)) {
    if (!rightRailRef.value?.prepareProductTour()) return false;
    mobileGlobalActionsOpen.value = false;
    workspaceRef.value?.showAssistant('agent');
    await nextTick();
  }
  return true;
}
watch(sending, (next,previous) => { if (previous && !next) {
  const failed=chat.value.at(-1)?.error;
  if (failed) { closedError.value=failed; closedFailedRequest.value=chat.value.at(-1)?.user ?? ''; }
  else void refreshClosedBook();
} });
</script>

<template>
  <div class="app" :inert="(!localPage && closedCover) || recapProps.hostOpening || undefined" :aria-busy="recapProps.hostOpening || undefined" :aria-hidden="(!localPage && closedCover) || undefined" :style="[typography.style.value, !localPage && closedCover ? { visibility: 'hidden' } : {}]">
    <LocalLibraryHome v-if="localPage" :page="localPage" :books="bookPickerBooks" :root="bookPickerRoot || desktopLibraryRoot"
      :active-dir="activeBookDir" :busy="shelfBusy || openingBook" :loading="bookPickerLoading" :error="bookPickerError"
      :native="nativeSettings" :can-create="desktopHost && !readerOnly" :resume-title="localLearningTask?.title"
      @navigate="showLocalPage" @refresh="loadBookLibrary" @settings="openDesktopSettings" @picker="showLocalPicker" @open="openShelfBook" @resume-external="activeBookDir && submitOpenBook(activeBookDir)"
      @resume="setLocalPage(null); resumeLocalLearningTask()" />
    <div class="reader-surfaces" v-show="!localPage">
    <TopBar
      :notes-available="!!network.workspace?.learning_workspace_id && appSurface === 'reader'"
      @open-notes="openWorkspaceNotes()"
      :board-available="appSurface === 'reader'"
      @open-board="openBoard()"
      :video-mode="mediaMode"
      :music-mode="musicMode"
      :focus-available="appSurface === 'reader'"
      :focus-reading="focusReading"
      @toggle-focus="workspaceRef?.toggleFocus()"
      :typography-available="appSurface === 'reader' && !mediaMode"
      :material-title="recapProps.materialTitle"
      @open-typography="typography.open.value = true"
      :chapter-title="musicMode ? uiText('音乐伴听') : videoMode ? uiText('视频伴读') : appSurface === 'workbench' ? uiText('构建工作台') : readerPositionTitle"
      :progress-pct="appSurface === 'reader' ? progressPct : 0"
      :anchor-lid="readingAnchorLid"
      :debug-open="debugOpen"
      :left-rail-open="adaptiveNavigation ? !!workspaceRef?.outlineOpen : leftRailOpen"
      :build-intent-open="buildIntentOpen"
      :build-intent-available="!readerOnly && appSurface === 'reader'"
      :workbench-available="!readerOnly && appSurface === 'reader' && workbenchAvailable(buildWorkbenchSnapshot)"
      :desktop-host="desktopHost && nativeSettings"
      :model-settings-available="!network.enabled"
      :chat-unavailable="agentHistoryLoading || !!agentHistoryError"
      :mobile-collapsible="appSurface === 'reader'"
      :mobile-open="mobileGlobalActionsOpen"
      @new-chat="newChat"
      @open-workbench="openBuildWorkbench"
      @open-book="openBook"
      @open-settings="openDesktopSettings"
      @toggle-left-rail="toggleNavigationOutline"
      @open-build-intent="openBuildIntent"
      @toggle-debug="debugOpen = !debugOpen"
      @close-mobile="mobileGlobalActionsOpen = false"
    >
      <template #navigation><slot name="navigation">
        <WorkspaceDestinations v-if="desktopHost && !network.enabled" :busy="shelfBusy || openingBook" :current="understandingOpen ? 'understanding' : undefined"
          @courses="showLocalPage('courses')" @materials="showLocalPage('materials')" @boards="showLocalPage('boards')" @understanding="showLocalPage('understanding')" />
        <template v-else><button v-if="localLearningTask" class="understanding-nav-entry" :disabled="localLearningTaskBusy" :title="localLearningTask.question ?? localLearningTask.title" @click="resumeLocalLearningTask">{{ uiText('继续学习') }}</button><button class="understanding-nav-entry" :aria-current="understandingOpen?'page':undefined" @click="openUnderstanding">{{ uiText('我的理解') }}</button></template>
      </slot></template>
      <template #account><slot name="account" /></template>
      <template #help>
        <slot name="help" />
        <ProductTour v-if="!network.enabled && !boardVisible && !localPage && !activeTrial" :context="appSurface === 'reader' && !sending && !agentHistoryLoading && !agentHistoryError ? 'reader' : null" owner="local" :prepare="prepareProductTour" />
      </template>
    </TopBar>
    <ReaderTypographyPanel v-if="typography.open.value" :preferences="typography.preferences.value" :save-error="typography.saveError.value" :pdf="activeReaderSurface === 'pdf'" @change="typography.update" @close="closeTypography" />
    <TutorPanel v-if="tutorPanelOpen" :course-error="courseError" @course-retry="coursePlayer?.retry()" :course="courseMode ? courseLesson : null" :course-blocked="courseBlocked" :course-goal-changed="courseGoalChanged" @course-action="coursePlayer?.act($event)" @course-return="tutorPanelOpen = false; returnToCourse()" v-model:draft="tutorIntentDraft" ref="tutorPanel" :key="readerContextKey" :music-mode="musicMode" :video-mode="videoMode" :initial-view="tutorPanelView" :current-lid="mediaMode ? undefined : currentReadingLid ?? undefined" :position-ms="musicMode ? musicPane?.position : videoMode ? videoPane?.position : undefined" :readiness="tutor.readiness.value" :readiness-error="tutor.readinessError.value" @refresh-readiness="tutor.loadReadiness" :state="tutor.state.value" :busy="tutor.busy.value" :error="tutor.error.value" :pending="!!tutor.pending.value" :source-id="tutor.readiness.value?.source_id ?? buildWorkbenchSnapshot?.book_id" :label="tutor.label.value"
      :learning-scope-options="learningScopeOptions" :learning-purpose-available="learningPurposeAvailable" :learning-purpose-reason="learningPurposeReason" :learning-purpose-busy="guidingChapter || sending"
      @understanding="openUnderstanding" @learning-purpose="startLearningPurpose" @action="tutorAction" @retry="tutor.retry" @close="tutorPanelOpen = false" />

    <AlignmentQualityBar
      v-if="appSurface === 'reader' && sourceManifest?.alignment_quality"
      :quality="sourceManifest.alignment_quality"
      :workbench-available="!readerOnly && workbenchAvailable(buildWorkbenchSnapshot)"
      @open-workbench="openBuildWorkbench"
    />

    <p v-if="banner" class="banner">{{ banner }} <button v-if="activityHelpLocation" :disabled="sending" @click="openActivityHelp">{{ uiText('前往原题帮助') }}</button></p>
    <p v-for="item in currentReadingActions" :key="item.key" class="banner" role="status">{{ item.body.display_user }}{{ uiText('：提交结果待核对') }} <button :disabled="sending" @click="recoverReadingAction(item)">{{ uiText('核对原操作') }}</button>
      <button :disabled="sending" @click="recoverReadingAction(item, true)">{{ uiText('重试原操作') }}</button>
    </p>
    <p v-if="network.enabled && residentRun.snapshot.value?.persistence_state === 'failed' && residentRun.snapshot.value?.execution_state !== 'interrupted'" role="status">{{ uiText('原回合结果尚未保存。') }}<button @click="retryResidentSave">{{ uiText('重试保存原结果') }}</button></p>

    <section v-if="appSurface === 'waiting-materials'" class="build-workbench" role="status">
      <h2>{{ uiText('等待完整阅读材料') }}</h2>
      <p>{{ uiText('请在服务停止后同步这本书的完整已就绪目录，再启动服务并刷新。') }}</p>
      <p v-if="buildWorkbenchError">{{ buildWorkbenchError }}</p>
      <ul>
        <li v-for="reason in buildWorkbenchSnapshot?.readiness.reasons ?? []" :key="reason">{{ reason }}</li>
        <li v-for="stage in Object.values(buildWorkbenchSnapshot?.readiness.stages ?? {}).filter(s => s.reason && s.status !== 'done' && s.status !== 'blocked')" :key="stage.stage">{{ stage.reason }}</li>
      </ul>
      <button @click="init">{{ uiText('刷新材料状态') }}</button>
      <button @click="openBook">{{ uiText('打开其他书籍') }}</button>
    </section>

    </div>
    <PersonalNavigation v-if="localAgentHost && personalAccess?.enabled && (personalAccess.navigate || personalAccess.write)"
      :ready="!localPage && appSurface === 'reader' && closedReady" :covered="closedCover" :navigate="openPersonalPassage" @changed="refreshExternalWrites" @error="banner = $event" />
    <div v-if="desktopSettingsOpen" class="desktop-settings-modal" @click.self="closeDesktopSettings">
      <section class="desktop-settings-dialog" role="dialog" aria-modal="true" aria-labelledby="desktop-settings-title">
        <header class="desktop-settings-head">
          <div>
            <p class="formula-dialog-kicker">{{ nativeSettings ? uiText('桌面应用') : uiText('本地阅读器') }}</p>
            <h3 id="desktop-settings-title">{{ nativeSettings || localAgentHost ? uiText('设置') : uiText('模型设置') }}</h3>
          </div>
          <button
            class="formula-dialog-close"
            :title="uiText('关闭')"
            :aria-label="uiText('关闭设置')"
            :disabled="desktopLibraryChanging || desktopProviderSaving || personalConnectionSaving || (nativeSettings && !desktopLibraryAvailable)"
            @click="closeDesktopSettings"
          >×</button>
        </header>
        <nav v-if="nativeSettings || localAgentHost" class="desktop-settings-sections" :aria-label="uiText('设置分类')">
          <button type="button" :aria-pressed="desktopSettingsSection === 'models'" @click="desktopSettingsSection = 'models'">{{ uiText('AI 模型') }}</button>
          <button type="button" :aria-pressed="desktopSettingsSection === 'general'" @click="desktopSettingsSection = 'general'">{{ uiText('书库与工具') }}</button>
        </nav>
        <div class="desktop-settings-body">
          <div v-show="desktopSettingsSection === 'models'">
            <ModelProviderSettings :client="desktopModelSettingsClient" @busy="desktopProviderSaving = $event" />
          </div>
          <div v-show="desktopSettingsSection === 'general'">
          <div v-if="nativeSettings" class="desktop-settings-section">
            <div class="desktop-settings-row">
              <div>
                <strong>{{ uiText('书库位置') }}</strong>
                <p>{{ desktopLibraryAvailable ? uiText('用于扫描和新建书') : uiText('当前目录不可用，请重新选择') }}</p>
              </div>
              <button
                class="desktop-library-button"
                :disabled="desktopLibraryChanging"
                @click="chooseDesktopLibraryDirectory"
              >
                <FolderOpen :size="17" aria-hidden="true" />
                <span>{{ desktopLibraryChanging ? uiText('处理中') : uiText('选择目录') }}</span>
              </button>
            </div>
            <p v-if="desktopLibraryRoot" class="desktop-settings-path">{{ desktopLibraryRoot }}</p>
            <p v-if="desktopLibraryError" class="book-picker-error">{{ desktopLibraryError }}</p>
          </div>
          <div v-if="localAgentHost" class="desktop-settings-section">
            <LocalAgentSettings ref="localAgentSettingsRef" :controller="localAgents" />
          </div>
          <div v-if="localAgentHost" class="desktop-settings-section">
            <PersonalConnectionSettings :current-book="buildWorkbenchSnapshot?.book_id" @changed="personalAccess = $event" @busy="personalConnectionSaving = $event" />
          </div>
          <div v-if="localAgentHost" class="desktop-settings-section">
            <LearningExtensions @open="dir => { closeDesktopSettings(); if (!desktopSettingsOpen) void submitOpenBook(dir); }" />
          </div>

          <div v-if="nativeSettings && managedBuildEngine" class="desktop-settings-section">
            <div class="desktop-settings-row">
              <div>
                <strong>{{ uiText('本地构建引擎') }}</strong>
                <p>{{ codexPluginLoading ? uiText('正在检测...') : localBuildStatus?.available ? uiText('已就绪') : uiText('本地材料工具不可用，请重新安装应用') }}</p>
              </div>
              <button :disabled="codexPluginLoading" @click="refreshCodexPluginStatus">{{ uiText('刷新') }}</button>
            </div>
            <p class="desktop-settings-message">{{ uiText('材料导入和构建工作台使用随附引擎。完整 AI 预构建需在 Codex 中使用 Understand Book 构建技能和执行器。') }}</p>
            <p v-if="localDebug && localBuildStatus" class="desktop-settings-path">{{ localBuildStatus.path }}</p>
            <p v-if="codexPluginError" class="book-picker-error">{{ codexPluginError }}</p>
          </div>
          <div v-else-if="nativeSettings" class="desktop-settings-section">
            <div class="desktop-settings-row">
              <div>
                <strong>{{ uiText('Codex 预构建插件') }}</strong>
                <p v-if="codexPluginStatus">{{ codexPluginStateLabel(codexPluginStatus.state) }}</p>
                <p v-else>{{ codexPluginLoading ? uiText('正在检测...') : uiText('尚未检测') }}</p>
              </div>
              <button
                v-if="codexPluginStatus?.state === 'not_installed'"
                class="primary-action"
                :disabled="codexPluginLoading"
                @click="installCodexPlugin"
              >
                {{ codexPluginLoading ? uiText('处理中...') : uiText('安装或重试') }}
              </button>
              <button v-else :disabled="codexPluginLoading" @click="refreshCodexPluginStatus">{{ uiText('刷新') }}</button>
            </div>
            <p v-if="codexPluginStatus?.state === 'pending_configuration'" class="desktop-settings-message">{{ uiText('在 Codex 的插件目录中安装 Understand Book，然后回到这里刷新。') }}</p>
            <p v-else-if="codexPluginStatus?.state === 'codex_not_found'" class="desktop-settings-message">{{ uiText('请先安装并启动 Codex，再回到这里刷新检测。') }}</p>
            <p v-else-if="codexPluginStatus" class="desktop-settings-message">{{ codexPluginStatus.message }}</p>
            <p v-if="codexPluginStatus?.codex_path" class="desktop-settings-path">{{ codexPluginStatus.codex_path }}</p>
            <p v-if="codexPluginError" class="book-picker-error">{{ codexPluginError }}</p>
          </div>
          </div>
        </div>
      </section>
    </div>

    <div v-if="bookPickerOpen" class="book-picker-modal" @click.self="closeBookPicker">
      <section class="book-picker-dialog" role="dialog" aria-modal="true" aria-labelledby="book-picker-title">
        <header class="book-picker-head">
          <div>
            <p class="formula-dialog-kicker">{{ uiText('继续一次阅读') }}</p>
            <h3 id="book-picker-title">{{ uiText('选择阅读材料') }}</h3>
          </div>
          <button class="formula-dialog-close" :title="uiText('关闭')" :aria-label="uiText('关闭书库')" @click="closeBookPicker">×</button>
        </header>

        <div v-if="desktopHost && !readerOnly" class="book-picker-tabs" role="tablist" :aria-label="uiText('书库操作')">
          <button
            role="tab"
            :aria-selected="bookPickerMode === 'open'"
            :class="{ active: bookPickerMode === 'open' }"
            @click="switchBookPickerMode('open')"
          > {{ uiText('打开') }} </button>
          <button
            v-if="desktopHost && !readerOnly"
            role="tab"
            :aria-selected="bookPickerMode === 'create'"
            :class="{ active: bookPickerMode === 'create' }"
            @click="switchBookPickerMode('create')"
          > {{ uiText('新建论文书') }} </button>
        </div>

        <div v-if="bookPickerMode === 'open'" class="book-picker-body">
          <div class="book-picker-toolbar">
            <span>{{ bookPickerBooks.length }} {{ uiText('份阅读材料') }}</span>
            <button :disabled="bookPickerLoading || openingBook" @click="loadBookLibrary">{{ bookPickerLoading ? uiText('加载中') : uiText('刷新') }}</button>
          </div>
          <p v-if="bookPickerError" class="book-picker-error">{{ bookPickerError }}</p>
          <p v-if="bookPickerLoading" class="book-picker-state">{{ uiText('正在查找阅读材料…') }}</p>
          <p v-else-if="bookPickerBooks.length === 0" class="book-picker-state">{{ uiText('这里还没有阅读材料。可以通过目录打开已有材料。') }}</p>

          <div v-else class="book-picker-list">
            <button v-for="book in bookPickerBooks" :key="book.dir" class="book-picker-card"
              :class="{ active: bookPickerDir === book.dir }" :aria-pressed="bookPickerDir === book.dir"
              :disabled="openingBook" @click="bookPickerDir = book.dir" @dblclick="submitOpenBook(book.dir)">
              <BookCover :title="bookTitle(book)" :cover="book.cover" compact />
              <span class="book-picker-card-title"><strong>{{ bookTitle(book) }}</strong>
                <small>{{ book.route === 'workbench' ? uiText('正在准备') : uiText('可开始阅读') }}</small>
                <small v-if="bookPickerBooks.filter(item => bookTitle(item) === bookTitle(book)).length > 1" class="book-picker-variant">{{ book.name }}</small>
              </span>
              <span class="book-picker-selected" aria-hidden="true">{{ bookPickerDir === book.dir ? '✓' : '›' }}</span>
            </button>
          </div>

          <details v-if="!network.enabled" class="book-picker-location">
            <summary>{{ uiText('通过目录打开 / 查看材料位置') }}</summary>
            <div class="book-picker-input-row">
            <label>
              <span>{{ uiText('材料目录') }}</span>
              <input
                v-model="bookPickerDir"
                :disabled="openingBook"
                placeholder=".understand-book/book-id"
                @keydown.enter.prevent="submitOpenBook()"
              />
            </label>
            </div>
            <p v-if="bookPickerRoot" class="book-picker-root">{{ uiText('书库位置：') }}{{ bookPickerRoot }}</p>
          </details>
        </div>

        <div v-else class="book-picker-body">
          <div class="book-create-grid">
            <label>
              <span>{{ uiText('标题') }}</span>
              <input
                v-model="newBookTitle"
                :disabled="openingBook"
                :placeholder="uiText('论文标题')"
                @keydown.enter.prevent="submitCreateBook"
              />
            </label>
            <label>
              <span>{{ uiText('书 ID（自动生成）') }}</span>
              <output class="book-create-generated-id" :title="generatedNewBookId">
                {{ generatedNewBookId || uiText('输入标题后自动生成') }}
              </output>
            </label>
            <FileDropField
              :model-value="newBookPdf"
              label="PDF"
              accept=".pdf,application/pdf"
              accept-label=".pdf"
              kind="pdf"
              :disabled="openingBook"
              @update:model-value="selectNewBookPdf"
            />
          </div>
          <p class="book-picker-root">{{ uiText('直接导入 PDF。自动提取正文，扫描页补充文字识别，准备好后进入阅读。') }}</p>
          <p v-if="bookPickerRoot" class="book-picker-root">{{ bookPickerRoot }}</p>
          <p v-if="bookPickerError" class="book-picker-error">{{ bookPickerError }}</p>
        </div>

        <footer class="book-picker-actions">
          <button :disabled="openingBook" @click="closeBookPicker">{{ uiText('取消') }}</button>
          <button
            v-if="bookPickerMode === 'open'"
            class="primary-action"
            :disabled="openingBook || !bookPickerDir.trim()"
            @click="submitOpenBook()"
          >
            {{ openingBook ? uiText('打开中') : uiText('打开') }}
          </button>
          <button
            v-else
            class="primary-action"
            :disabled="openingBook || !newBookTitle.trim() || !generatedNewBookId || !newBookPdf"
            @click="submitCreateBook"
          >
            {{ openingBook ? uiText('正在导入…') : uiText('导入并阅读') }}
          </button>
        </footer>
      </section>
    </div>

    <div class="reader-surfaces" v-show="!localPage">
    <BuildWorkbenchPane
      v-if="appSurface === 'workbench'"
      :snapshot="buildWorkbenchSnapshot"
      :loading="buildWorkbenchLoading"
      :error="buildWorkbenchError"
      :confirming="buildWorkbenchConfirming"
      :importing="buildWorkbenchImporting"
      :actioning="buildWorkbenchActioning"
      :pdf-url="api.pdfOriginalUrl()"
      :source-review-llm-suggestions="sourceReviewLlmSuggestions"
      :source-review-llm-analyzing-block-id="sourceReviewLlmAnalyzingBlockId"
      :source-review-llm-errors="sourceReviewLlmErrors"
      :source-review-llm-batch="sourceReviewLlmBatch"
      @refresh="refreshBuildWorkbench"
      @import-input="importWorkbenchInput"
      @create-job="createBuildJob"
      @start-job="startBuildJob"
      @resume-job="resumeBuildJob"
      @resolve-decision="resolveBuildDecision"
      @resolve-permission="resolveExecutorPermission"
      @resolve-source-review="resolveSourceReview"
      @resolve-source-review-group="resolveSourceReviewGroup"
      @analyze-source-review="analyzeSourceReview"
      @apply-all-source-review-with-llm="applyAllSourceReviewWithLlm"
      @draft-sidecar-plan="draftSidecarPlan"
      @confirm-sidecar-plan="confirmSidecarPlan"
      @enter-reader="enterReader"
    />

    <main v-else-if="appSurface === 'loading'" class="app-loading"><LoadingAnimation /></main>

    <ReaderWorkspace
      v-show="!understandingOpen"
      :reader-label="courseMode ? uiText('学习') : undefined"
      v-else-if="appSurface === 'reader'"
      ref="workspaceRef"
      :data-reader-visible="readerVisible && !boardVisible && !notesVisible && !understandingOpen"
      :class="{ 'course-workspace': courseMode }"
      :video-mode="mediaMode"
      :music-mode="musicMode"
      :logical="workspaceLogicalState"
      :compare-intent="courseMode"
      :context-key="readerContextKey"
      :enabled="mobileWorkspaceEnabled"
      :selection-active="!!hlPopover"
      :left-collapsed="!leftRailOpen"
      :return-available="returnToAnswerAvailable"
      :return-label="readingReturnLabel"
      :reader-surface="activeReaderSurface"
      :pdf-surface-available="pdfReaderAvailable"
      :global-actions-open="mobileGlobalActionsOpen"
      :integrated-header="!!$slots.navigation"
      :style="workspaceStyle"
      @tab-request="requestWorkspaceTab"
      @before-display-change="preserveDisplayReadingPosition"
      @focus-change="onFocusChange"
      @projection-change="handleWorkspaceProjection"
      @global-actions-request="mobileGlobalActionsOpen = !mobileGlobalActionsOpen"
      @return="returnToAgentAnswer"
      @reader-surface-request="selectReaderSurface"
    >
      <template #reader-navigation>
        <nav v-if="trialOpenError" class="book-course-navigation"><span role="alert">{{ trialOpenError }}</span><a href="?">{{ uiText('返回阅读器') }}</a></nav>
        <nav v-else-if="(courseAvailable || courseMode || network.enabled && coursePublication) && !mediaMode" class="book-course-navigation" :aria-label="uiText('书内学习')">
          <template v-if="courseMode">
            <div class="course-navigation-title"><small>{{ courseCollection.title }}<template v-if="activeTrial"> · {{ uiText('代表课试学') }}</template><template v-if="courseUnitTitle"> · {{ courseUnitTitle }}</template></small><strong>{{ courseLesson?.summary.question ?? uiText('本书课程') }}</strong></div>
            <div class="course-surface-tabs" role="group" :aria-label="uiText('学习内容')">
              <button :aria-pressed="courseSurface === 'course'" @click="returnToCourse">{{ uiText('课程') }}</button>
              <button :disabled="!courseLesson" :aria-pressed="courseSurface === 'source'" @click="readCourseOriginal()">{{ uiText('原文') }}</button>
              <button v-if="courseLesson?.experiment && (courseLesson.experiment.kind !== 'inquiry' || !courseLesson.experiment.visualization || ['experiment','explanation'].includes(courseLesson.progress.stage))" :aria-pressed="courseSurface === 'experiment'" @click="revisitCourseExperiment">{{ uiText('实验') }}</button>
            </div>
            <button class="leave-course" :disabled="courseBlocked" @click="exitCourse">{{ activeTrial ? uiText('退出试课') : uiText('继续自由阅读') }}</button>
            <button v-if="courseSurface === 'source' && network.enabled" class="open-book-course" :disabled="courseBlocked || courseCollectionLoading || !!courseCollectionError" @click="coursePlayer?.returnToCollection()">{{ uiText(courseCollection.kind === 'course' ? '返回整套课程总目录' : '返回书籍学习首页') }}</button>
            <template v-if="courseSurface === 'source' && courseCollectionError"><span role="alert">{{ courseCollectionError }}</span><button class="open-book-course" @click="loadCourseCollection">{{ uiText('重试读取课程归属') }}</button></template>
          </template>
          <template v-else><span>{{ readerBookTitle || uiText('带着问题读这本书') }}</span><button v-if="network.enabled && coursePublication" class="open-book-course" @click="returnBookCollection">{{ uiText('书籍学习首页') }}</button><button v-if="courseAvailable" class="open-book-course" @click="openCourse()">{{ uiText('本书课程 →') }}</button></template>
        </nav>
      </template>
      <LeftRail
        v-if="!mediaMode"
        v-show="leftRailOpen"
        v-model:goto-input="gotoInput"
        v-model:search-query="outlineSearch"
        :book-title="readerBookTitle"
        :title-page-lid="titlePageLid"
        :outline-items="outlineItems"
        :anchor-offset="outlineAnchorOffset"
        :progress-pct="progressPct"
        :anchor-lid="outlineAnchorLid"
        :pending-lid="pendingNavigationLid"
        :selected-lid="selectedLid"
        :leaf-count="leafOrder.length"
        :debug-open="debugOpen"
        :paper-enabled="isPaperProfile"
        :paper-loading="paperProjectionLoading"
        :paper-error="paperProjectionError"
        :paper-minimap-base="paperMinimapSnapshot?.base ?? null"
        :paper-minimap-state="paperMinimapSnapshot?.state ?? null"
        :paper-minimap-lens="paperMinimapSnapshot?.lens ?? null"
        :paper-minimap-localization="paperMinimapLocalization"
        :paper-minimap-effect-reason="lastPaperMinimapEffect?.reason ?? null"
        :paper-minimap-undo-available="!!lastPaperMinimapEffect"
        :paper-minimap-action-busy="paperMinimapActionBusy"
        @goto="selectOutlineItem"
        @paper-minimap-toggle="togglePaperMinimap"
        @paper-minimap-mode="setPaperMinimapMode"
        @paper-minimap-layer="setPaperMinimapLayer"
        @paper-minimap-pin="togglePaperMinimapPin"
        @paper-minimap-undo="undoLastPaperMinimapEffect"
      />

      <div
        v-if="!mediaMode"
        class="resize-handle resize-handle-left"
        role="separator"
        aria-orientation="vertical"
        :title="uiText('调整目录栏宽度')"
        @mousedown="startResize('left', $event)"
      ></div>

      <MusicReaderPane v-if="musicContext" ref="musicPane" :key="`${musicContext.publication.book_id}/${musicContext.publication.publication_id}`" :context="musicContext" :owner="[readerPreferenceOwner() || 'local', network.workspace?.learning_workspace_id].filter(Boolean).join(':')" :question="musicQuestionContext" :selection-disabled="questionImagePreparing || sending || agentHistoryLoading" @question="askMusic" @image-loading="prepareQuestionImage" @clear-question="clearMusicQuestion" />
      <VideoReaderPane v-else-if="videoMode && videoInitial" ref="videoPane" :initial="videoInitial" :published="publishedVideo || undefined" :playback="videoPlayback || undefined" :details-loading="videoDetailsLoading" :details-error="videoDetailsError" :owner="[readerPreferenceOwner() || 'local', network.workspace?.learning_workspace_id].filter(Boolean).join(':')" :media-url="videoMediaUrl" :question="videoQuestion.draft.value?.context" :selection-disabled="videoDetailsLoading || !publishedVideo || questionImagePreparing || videoQuestion.restoring.value || sending || videoAdoption.adopting.value" :adoption-error="videoAdoption.error.value" @select-region="videoQuestion.selectRegion" @clear-region="videoQuestion.clearRegion" @select-range="videoQuestion.selectRange" @clear-range="videoQuestion.clearRange" @frame-ready="videoFrameReady = $event" embedded />
      <PdfReaderPane
        v-else-if="activeReaderSurface === 'pdf'"
        v-show="!courseVisible"
        ref="pdfReaderPaneRef"
        :source-manifest="sourceManifest"
        :source-map="pdfSourceMap"
        :pdf-url="api.pdfOriginalUrl()"
        :initial-reading-anchor="previousPdfAnchor"
        :focus-reading="focusReading"
        :reader-active="readerVisible && readerRegionVisible && !courseVisible && !understandingOpen"
        :focus-return-label="returnToAnswerAvailable ? readingReturnLabel : undefined"
        :viewport-anchor-lid="pdfViewportAnchorLid"
        :active-lid="pdfActiveLid"
        :selected-lid="selectedLid"
        :annotation-projection="pdfAnnotationProjection"
        :annotation-error="pdfAnnotationProjectionError"
        :render-markdown="renderMarkdown"
        :note-placement-active="notePlacementState.phase === 'placing' && notePlacementSurface === 'pdf'"
        @goto="doGoto"
        @focus-tool="onFocusTool"
        @viewport-change="onPdfViewportChange"
        @focus-source="focusLocalSource"
        @selection-capture="onPdfSelectionCapture"
        @selection-cancel="cancelPdfSelectionDraft"
        @viewport-interaction="onPdfViewportInteraction"
        @edit-note="openEditNote"
        @open-note="openNoteAssociation"
        @delete-note="deleteNote"
        @reselect-note="reselectPdfNote"
        @show-notes="showAnnotationInNotes"
        @delete-highlight="deleteHighlight"
        @reselect-highlight="reselectPdfHighlight"
        @note-placement-target="onPdfNotePlacementTarget"
      />

      <ReaderPane
        v-else
        v-show="!courseVisible"
        ref="readerPaneRef"
        :typography="typography.preferences.value"
        :focus-source="focusPaginationSource"
        :reader-active="readerVisible && readerRegionVisible && !courseVisible && !understandingOpen"
        :context-key="workspaceContextKey"
        :segments="segments"
        :source-focus="sourceFocus" :annotation-revision="annotations"
        :focus-return-label="returnToAnswerAvailable ? readingReturnLabel : undefined"
        :focus-surface-label="pdfReaderAvailable ? uiText('PDF 原版') : undefined"
        :viewport-anchor="readingAnchorLid"
        :selected-lid="selectedLid"
        :note-placement-active="notePlacementState.phase === 'placing' && notePlacementSurface === 'markdown'"
        :render-seg="renderSeg"
        :render-markdown="renderMarkdown"
        :markdown-heading-level="markdownHeadingLevel"
        :is-asset="isAsset"
        :is-highlighted="isHighlighted"
        :highlights-of="highlightsOf"
        :highlight-cards-of="highlightCardsOf"
        :visible-notes="visibleNotes"
        :hl-excerpt="hlExcerpt"
        :image-meta="imageMeta"
        :image-asset="imageAssetFor"
        @select="onSelectSeg"
        @prose-selection-change="onProseSelection"
        @current-lid="onCurrentLid"
        @viewport-interaction="onReaderViewportInteraction"
        @note-placement-target="onMarkdownNotePlacementTarget"
        @note-placement-invalid="onMarkdownNotePlacementInvalid"
        @scroll-edge="onScrollEdge"
        @pagination-active="onPaginationActive"
        @focus-visible="onFocusVisible" @focus-confirmed="confirmFocusPosition"
        @focus-tool="onFocusTool"
        @goto="doGoto"
        @focus-source-local="focusLocalSource"
        @open-formula="openFormulaDialog"
        @modify-highlight="modifyHighlight"
        @delete-highlight="deleteHighlight"
        @edit-note="openEditNote"
        @open-note="openNoteAssociation"
        @delete-note="deleteNote"
        @show-notes="showAnnotationInNotes"
        @place-note="startNotePlacement"
      />

      <section v-if="courseWorkspace && coursePublication && !mediaMode" v-show="courseVisible" class="reader-pane course-reader" :aria-label="uiText('课程学习区')">
        <InquiryCoursePlayer @closed-book="startClosedBook()" @practice="practiceCourse" @engaged="courseEngaged" :key="courseWorkspace.learning_workspace_id" ref="coursePlayer" embedded :workspace="courseWorkspace" :publication="coursePublication"
          :collection="courseCollection" :collection-loading="courseCollectionLoading" :collection-error="courseCollectionError" @return-collection="returnCourseCollection" @retry-collection="loadCourseCollection" @launch="launchCourseLesson"
          @available="courseAvailable = $event" @blocked="courseBlocked = $event" @error="courseError = $event" @change="courseChanged" @discussion="discussCourse" @source="(source, lid) => readCourseOriginal(source, true, lid)" @exploration="courseExploration = $event" @understanding="openTutorPanel('understanding')" />
      </section>
      <div
        class="resize-handle resize-handle-right"
        role="separator"
        aria-orientation="vertical"
        :title="uiText('调整上下文栏宽度')"
        @mousedown="startResize('right', $event)"
      ></div>

      <RightRail
        v-model:follow-up-drafts="preparedFollowUps"
        :follow-up-nodes="followUpNodes"
        :follow-up-busy="followUpBusy || acceptingRun || agentHistoryLoading"
        :follow-up-error="followUpError"
        :follow-up-allowed="followUpAllowed"
        @follow-up-load="loadFollowUps"
        @follow-up-start="startFollowUp"
        @follow-up-enter="enterFollowUp"
        @follow-up-parent="returnToFollowUpParent"
        :course-active="courseMode"
        :closed-book-ready="closedReady && !closedBusy"
        :workspace-notes-available="!!network.workspace?.learning_workspace_id"
        :workspace-note-importing="workspaceNoteImporting"
        @save-workspace-note="importWorkspaceNote"
        @open-workspace-notes="openWorkspaceNotes()"
        @add-to-board="openBoard"
        @open-board-location="openBoardLocation"
        :music-mode="musicMode"
        :key="chatSceneKey"
        :companion-visible="readerVisible && appSurface === 'reader' && !focusReading && assistantVisible && !understandingOpen"
        :waiting-turn-id="waitingTurnId"
        v-model:presentation-workspace="presentationWorkspace"
        v-model:presentation-requested="presentationRequested"
        ref="rightRailRef"
        @recap-publication="recapEmit('recap-publication', $event)"
        :fullscreen="agentFullscreen"
        :agent-input="agentInput"
        :personal-note-reference="personalDiscussion?.reference"
        :music-question="musicQuestionContext"
        :video-question="videoQuestion.draft.value?.context"
        :video-images-enabled="!!publishedVideo"
        @question-image="setQuestionImage"
        @question-image-loading="prepareQuestionImage"
        :video-error="musicMode ? musicImageError : videoQuestion.error.value"
        :question-send-blocked="questionImagePreparing ? uiText('正在添加图片…') : undefined"
        :question-input-blocked="(musicMode && acceptingRun) || (videoMode && (videoDetailsLoading || !publishedVideo || agentHistoryLoading)) || videoQuestion.restoring.value || videoAdoption.adopting.value || (!!publishedVideo && (acceptingRun || (!videoFrameReady && !videoQuestion.draft.value)))"
        @update:agent-input="onAgentInput"
        @cancel-video-draft="cancelVideoDraft"
        :history-loading="agentHistoryLoading"
        :history-error="agentHistoryError"
        :chat="chat"
        :chat-sessions="chatSessions"
        :chat-goals="chatGoals"
        :target-goal-id="targetGoalId"
        :active-chat-session-id="activeChatSessionId"
        :sending="sending"
        :continuation-enabled="canContinueAnswer"
        :continuation-label="tutor.label.value"
        :learning-title="courseMode ? courseLesson?.summary.question : tutor.current.value?.user_intent"
        :learning-attention="courseMode ? '' : tutor.error.value || (tutor.current.value && tutor.current.value.status !== 'active' ? tutor.label.value : '')"
        :learning-scope-options="learningScopeOptions" :learning-purpose-available="learningPurposeAvailable"
        :learning-purpose-ready="learningPurposeReady" :learning-purpose-reason="learningPurposeReason" :learning-purpose-busy="guidingChapter"
        @learning-purpose="startLearningPurpose" @resume-learning="resumeConversationLearning"
        @continue-answer="continueAnswer"
        @explain-answer="startClosedBook"
        @practice-answer="practiceAnswer"
        @clarify-answer="clarifyAnswer"
        @choose-continuation="chooseContinuation"
        @return-reading="agentFullscreen = false; workspaceRef?.showReader()"
        @workspace-menu="mobileGlobalActionsOpen = true"
        :run-connection="runConnection"
        :can-stop="residentRun.active.value"
        :stopping="residentRun.stopping.value"
        @stop-agent="stopResidentRun"
        @continue-goal="continueGoal"
        @show-allowance="recapEmit('show-allowance')"
        @cancel-goal="cancelGoal"
        @target-goal="targetGoal"
        @clear-goal-target="targetGoalId = null"
        :unquoted-note-placement-available="unquotedNotePlacementAvailable"
        :note-placement-surface="notePlacementSurface"
        :note-source-fingerprint="noteSourceFingerprint"
        :selected-lid="selectedLid"
        :selected-formula="selectedFormula"
        :formula-selected="selectedSegment?.kind === 'formula'"
        :formula-loading="formulaLoading"
        :formula-error="formulaError"
        @request-formula="requestSelectedFormula"
        :context-notes="allNotes"
        :context-highlights="allHighlights"
        :note-book-order="noteBookOrder" :note-source-labels="noteSourceLabels" :note-scope-key="noteMaterialKey"
        :annotation-location="pdfAnnotationProjection.location_by_mem_id"
        :render-markdown="renderMarkdown"
        :eff-label="effLabel"
        :eff-state="effState"
        :is-goto="isGoto"
        :show-effect-primary="showEffectPrimary"
        :show-effect-secondary="showEffectSecondary"
        :effect-primary-label="effectPrimaryLabel"
        :effect-secondary-label="effectSecondaryLabel"
        :goto-back="gotoBack"
        :ask-draft="askDraft"
        :profile-memory="profileMemory"
        :profile-memory-loading="profileMemoryLoading"
        :profile-memory-busy="profileMemoryBusy || sending"
        :profile-memory-error="profileMemoryError"
        :profile-memory-notice="profileMemoryNotice"
        :profile-backfill="profileBackfill"
        :profile-backfill-loading="profileBackfillLoading"
        :profile-backfill-busy="profileBackfillBusy || sending"
        :profile-backfill-error="profileBackfillError"
        :profile-backfill-notice="profileBackfillNotice"
        :profile-update-states="profileUpdateStates"
        :intent-artifacts="intentArtifacts"
        :intent-artifacts-loading="intentArtifactsLoading"
        :intent-artifacts-error="intentArtifactsError"
        :requested-tab="workspaceAuxTab"
        :requested-tab-revision="workspaceAuxTabRevision"
        @send-agent="sendAgent"
        @edit-failed-question="editFailedQuestion"
        @clear-personal-note="personalDiscussion=null"
        @continue-personal-note="(turnId, reference) => { personalDiscussion={reference,replyTo:turnId};workspaceRef?.showAssistant('agent'); }"
        @retry-history="retryHistory"
        @toggle-fullscreen="toggleAgentFullscreen"
        @presentation-follow-up="sendPresentationFollowUp" @end-socratic-dialogue="endSocraticDialogue"
        @record-note="recordPresentationNote" @note-follow-up="sendNoteFollowUp"
        @new-chat="newChat"
        @select-chat="selectChat"
        @open-note-answer="openNoteAnswer"
        @delete-chat="deleteChat"
        @clear-ask="clearAskDraft"
        @goto="doGoto"
        @focus-source="focusSource"
        @undo-effect="undoEffect"
        @keep-effect="keepEffect"
        @save-answer-selection="saveAgentSelection"
        @share-answer="shareAgentAnswer"
        @place-note="startNotePlacement"
        @edit-note="openEditNote"
        @open-note="openNoteAssociation"
        @delete-note="deleteNote"
        @modify-highlight="modifyHighlight"
        @delete-highlight="deleteHighlight"
        @agent-source-will-open="captureAgentSourceReturnPoint"
        @agent-source-opened="syncAfterAgentSourceOpen"
        @original-selection="openOriginalSelection"
        @failed-agent-settings="openFailedAgentSettings"
        @media-source-enter="enterMediaSource"
        @music-source-enter="enterMusicSource"
        @refresh-profile="refreshProfileSurface()"
        @mutate-profile="mutateProfile"
        @confirm-sensitive-profile="confirmSensitiveProfile"
        @start-profile-backfill="startProfileBackfill"
        @mutate-profile-backfill="mutateProfileBackfill"
        @undo-profile-update="undoProfileUpdate"
        @refresh-artifacts="refreshIntentArtifacts"
        :preparation-available="!readerOnly && appSurface === 'reader'"
        @prepare-artifacts="openBuildIntent"
        @open-artifacts="openIntentArtifacts"
        @artifact-cited="recordIntentUsage('artifact_cited', $event)"
      >
        <template v-if="workspaceNoteQuestion" #note-question>
          <span class="note-question-context">{{ uiText('围绕笔记提问：{0}', [workspaceNoteQuestion.title]) }} <button type="button" @click="workspaceNoteQuestion = null">{{ uiText('取消') }}</button></span>
        </template>
        <template v-if="localAgentHost" #agent-picker>
          <select class="local-agent-picker" :aria-label="uiText('当前问答 Agent')" :value="localAgents.loaded.value ? localAgents.selected.value : ':pending'"
            :disabled="localAgents.busy.value || sending" @change="chooseLocalAgent">
            <option v-if="!localAgents.loaded.value" value=":pending" disabled>{{ localAgents.error.value ? uiText('Agent 设置暂不可用') : uiText('正在读取...') }}</option>
            <option value="">{{ uiText('内置 Agent') }}</option>
            <option v-for="agent in localAgents.agents.value.filter(item => item.configured)" :key="agent.id" :value="agent.id" :disabled="!agent.available">{{ agent.name }}</option>
            <option value=":settings">{{ uiText('管理我的 Agent…') }}</option>
          </select>
        </template>
        <template #learning-actions>
          <button type="button" @click="openTutorPanel('knowledge')">{{ uiText('知识树') }}</button>
          <button type="button" @click="openTutorPanel('understanding')">{{ uiText('我的理解') }}</button>
        </template>
        <template #learning-details>
          <button type="button" @click="openTutorPanel()">{{ uiText('学习详情') }}</button>
        </template>
        <template #course-feedback>
          <div v-if="courseMode" class="course-dialogue-context">
            <CourseTutorControl :lesson="courseLesson" :blocked="courseBlocked" :goal-changed="courseGoalChanged" :error="courseError" @action="coursePlayer?.act($event)" @retry="coursePlayer?.retry()" />
            <p v-if="courseLesson">{{ uiText('原文里的一句话、实验里的变化，或你自己的理解，都可以在这里接着聊。') }}</p>
            <p v-if="courseSurface === 'source' && courseLesson?.reading">{{ ['introduction','prediction'].includes(courseLesson.progress.stage) ? courseLesson.reading.before_experiment : courseLesson.reading.after_experiment }}</p>
            <button v-if="courseSurface !== 'course'" @click="returnToCourse">{{ uiText('回到当前任务 →') }}</button>
          </div>

        </template>
        <template v-if="!courseMode" #learning-goal>
          <ConversationLearningGoal v-if="!courseMode"
            :key="readerContextKey"
            :music-mode="musicMode" :video-mode="videoMode" :learning-purpose-reason="learningPurposeReason"
            :workspace-scoped="!!network.workspace?.learning_workspace_id"
            :session="tutor.current.value"
            :busy="tutor.busy.value || sending || guidingChapter" :available="!!tutor.state.value && !tutor.pending.value && !!tutor.readiness.value?.source_id"
            :ready="tutor.readiness.value?.status === 'ready' && !tutor.readinessError.value"
            :learning-scope-options="learningScopeOptions" :learning-purpose-available="learningPurposeAvailable"
            :error="tutor.error.value" @learning-purpose="startLearningPurpose" @goal="setConversationLearningGoal" @manage="openTutorPanel()"
            @resume="resumeConversationLearning" @retry="tutor.retry" />
        </template>
        <template v-if="!courseMode" #tutor-control>
          <label class="practice-margin-setting"><input type="checkbox" :checked="practiceMarginOn" @change="setPracticeMargin(($event.target as HTMLInputElement).checked)" /> {{ uiText('阅读时提议练习') }}</label>
          <label class="practice-margin-setting" :title="uiText('从已讲过的内容中整理跨书的私人学习脉络，在后台进行并计入额度；关闭会删除已整理的脉络。')"><input type="checkbox" :checked="!!tutor.state.value?.learning_map_enabled" :disabled="tutor.busy.value || !tutor.state.value" @change="setLearningMap($event)" /> {{ uiText('整理我的学习脉络') }}</label>
        </template>
        <template #practice-offer>
          <PracticeOffer v-if="practiceOffer" :kind="practiceOffer.kind" :step="practiceOffer.step" :total="sequence.total" :busy="sending || guidingChapter" @accept="acceptPracticeOffer" @dismiss="dismissPracticeOffer" />
        </template>
      </RightRail>
    </ReaderWorkspace>
    <UnderstandingMap v-if="understandingOpen && appSurface==='reader'" :key="readerContextKey" class="understanding-page" :source-id="tutor.readiness.value?.source_id ?? buildWorkbenchSnapshot?.book_id" :title="recapProps.materialTitle" :workspace-id="network.workspace?.learning_workspace_id ?? undefined" :current-lid="mediaMode?undefined:currentReadingLid??undefined" :position-ms="musicMode?musicPane?.position:videoMode?videoPane?.position:undefined" @close="closeUnderstanding"/>

    <div v-if="boardMounted && appSurface === 'reader'" v-show="boardVisible" class="board-workspace-overlay">
      <p v-if="boardSourceError" class="banner" role="alert">{{ boardSourceError }} <button @click="boardSourceError = ''">{{ uiText('关闭') }}</button></p>
      <BoardWorkspace ref="boardWorkspaceRef" :board-id="boardId" :pending-import="boardPendingImport" :pending-locate="boardPendingLocate" :run="boardRun"
        :notes-revision="notesRevision" @open-note="openWorkspaceNotes" @notes-changed="workspaceNotesChanged"
        @follow-up="sendBoardQuestion" @retry-follow-up="retryBoardQuestion" @cancel-follow-up="cancelBoardQuestion" @located="boardPendingLocate = null"
        @board-open="boardOpened" @board-deleted="boardId = null; writeBoardRoute(null)" @close="closeBoard" @source="openBoardSource" @imported="boardPendingImport = null" />
      <BoardSourcePreview v-if="boardSourceRequest" :source="boardSourcePreview" :source-request="boardSourceRequest" :loading="boardSourceLoading" :opening="boardSourceOpening" :error="boardSourcePreviewError" :can-open-in-reader="boardSourceCanOpen" @close="closeBoardSourcePreview" @open-reader="openBoardSourceInReader" @retry="retryBoardSource" />
    </div>

    <div v-if="notesVisible && appSurface === 'reader'" class="notes-workspace-overlay">
      <p v-if="workspaceNoteError" class="banner" role="alert">{{ workspaceNoteError }} <button @click="workspaceNoteError = ''">{{ uiText('关闭') }}</button></p>
      <NotesWorkspace ref="notesWorkspaceRef" :key="network.workspace?.learning_workspace_id || 'local'" :initial-note-id="selectedWorkspaceNote" :learning-workspace-id="network.workspace?.learning_workspace_id"
        @close="closeWorkspaceNotes" @changed="workspaceNotesChanged" @open-board="boardFromWorkspaceNote" @ask="prepareWorkspaceNoteQuestion" @source="openWorkspaceNoteSource" />
    </div>
    <BoardSourcePreview v-if="workspaceNotePreview" :source="workspaceNotePreview" :read-window="readWorkspaceNoteSource" context="note" :can-open="workspaceNotePreview.can_open_in_reader" @open="navigateWorkspaceNoteSource()" @close="workspaceNotePreview = null" />

    <aside
      v-if="notePlacementState.phase !== 'idle'"
      class="note-placement-status"
      role="status"
      aria-live="polite"
    >
      <span v-if="notePlacementState.phase === 'placing'"> {{ uiText('请选择') }}{{ notePlacementSurface === "pdf" ? uiText(' PDF 正文区域') : uiText('正文中的真实段落') }}{{ uiText('放置笔记。') }} </span>
      <span v-else-if="notePlacementState.phase === 'saving'">{{ uiText('正在保存笔记…') }}</span>
      <span v-else>{{ uiText('正在确认笔记是否已保存…') }}</span>
      <button v-if="notePlacementState.phase === 'placing'" @click="cancelNotePlacement">{{ uiText('取消') }}</button>
    </aside>

    <BuildIntentPane
      v-if="appSurface === 'reader' && buildIntentOpen"
      :selection="buildIntentSelection"
      :planning-source="buildIntentPlanningSource"
      :busy="buildIntentBusy"
      :error="buildIntentError"
      @close="closeBuildIntent"
      @draft="draftBuildIntent"
      @confirm="confirmBuildIntent"
      @reject="rejectBuildIntent"
    />

    <div
      v-if="pdfSelectionState.capture && (pdfSelectionState.phase === 'error' || pdfSelectionState.draft)"
      class="hl-popover pdf-selection-toolbar"
      :style="pdfSelectionToolbarStyle"
      role="toolbar"
      :aria-label="uiText('PDF 选区操作')"
    >
      <template v-if="pdfSelectionState.phase === 'error'">
        <span class="pdf-selection-status error">{{ pdfSelectionState.error }}</span>
        <button :title="uiText('重试定位')" :aria-label="uiText('重试定位')" @mousedown.prevent="pdfSelectionSession.retry">
          <RotateCcw :size="16" />
        </button>
      </template>
      <template v-else-if="pdfSelectionState.draft">
        <span v-if="pdfSelectionCapabilities.statusLabel" class="pdf-selection-status">
          {{ pdfSelectionCapabilities.statusLabel }}
        </span>
        <button
          v-if="pdfSelectionCapabilities.canHighlight"
          :title="uiText('高亮')"
          :disabled="pdfSelectionState.phase === 'saving' || pdfSelectionTranslationState.phase === 'loading'"
          @mousedown.prevent="highlightPdfSelection"
        >
          <Highlighter :size="16" />
          <span>{{ uiText('高亮') }}</span>
        </button>
        <button
          v-if="pdfSelectionCapabilities.canNote"
          :title="uiText('笔记')"
          :disabled="pdfSelectionState.phase === 'saving' || pdfSelectionTranslationState.phase === 'loading'"
          @mousedown.prevent="notePdfSelection"
        >
          <MessageSquareText :size="16" />
          <span>{{ uiText('笔记') }}</span>
        </button>
        <button v-if="pdfSelectionCapabilities.canNote" :title="uiText('生成分享图')" :disabled="pdfSelectionState.phase === 'saving'" @mousedown.prevent="sharePdfSelection">{{ uiText('生成分享图') }}</button>
        <button
          v-if="pdfSelectionCapabilities.canAsk"
          :title="uiText('问 AI')"
          :disabled="pdfSelectionState.phase === 'saving' || pdfSelectionTranslationState.phase === 'loading'"
          @mousedown.prevent="askPdfSelection()"
        >
          <Sparkles :size="16" />
          <span>{{ uiText('问 AI') }}</span>
        </button>
        <button
          v-if="pdfSelectionCapabilities.canTranslate"
          :title="uiText('翻译')"
          :disabled="pdfSelectionState.phase === 'saving' || pdfSelectionTranslationState.phase === 'loading'"
          @mousedown.prevent="translatePdfSelection"
        >
          <Languages :size="16" />
          <span>{{ uiText('翻译') }}</span>
        </button>
        <span v-if="pdfSelectionState.error" class="pdf-selection-status error">{{ pdfSelectionState.error }}</span>
      </template>
      <button :title="uiText('关闭')" :aria-label="uiText('关闭 PDF 选区操作')" @mousedown.prevent="cancelPdfSelectionDraft">
        <X :size="16" />
      </button>
    </div>

    <PdfSelectionTranslationSurface
      v-if="pdfSelectionTranslationState.phase !== 'idle' && pdfSelectionState.draft"
      :state="pdfSelectionTranslationState"
      :anchor-rect="pdfSelectionState.draft.screen_rect"
      :render-markdown="renderMarkdown"
      :show-settings="showPdfTranslationSettings"
      @close="closePdfSelectionTranslation"
      @retry="retryPdfSelectionTranslation"
      @settings="openDesktopSettings"
      @copy="copyPdfSelectionTranslation"
    />

    <div
      v-if="hlPopover"
      ref="markdownSelectionToolbar"
      class="hl-popover"
      :style="{ left: hlPopover.x + 'px', top: hlPopover.y - 40 + 'px' }"
    >
      <button @mousedown.prevent="confirmHighlight">{{ uiText('高亮') }}</button>
      <button @mousedown.prevent="noteSelection">{{ uiText('笔记') }}</button>
      <button @mousedown.prevent="shareSelection">{{ uiText('生成分享图') }}</button>
      <button @mousedown.prevent="askSelection()">{{ uiText('问 AI') }}</button>
    </div>

    <div v-if="formulaDialog" class="formula-modal" @click.self="closeFormulaDialog">
      <section
        class="formula-dialog"
        role="dialog"
        aria-modal="true"
        :aria-labelledby="`formula-dialog-title-${formulaDialog.lid}`"
      >
        <header class="formula-dialog-head">
          <div>
            <p class="formula-dialog-kicker">{{ uiText('公式剖面') }}</p>
            <h3 :id="`formula-dialog-title-${formulaDialog.lid}`">{{ formulaDialog.lid }}</h3>
          </div>
          <button class="formula-dialog-close" :title="uiText('关闭')" :aria-label="uiText('关闭公式剖面')" @click="closeFormulaDialog">×</button>
        </header>
        <div class="formula-dialog-body">
          <pre class="formula-dialog-source" v-html="renderSeg(formulaDialog)"></pre>
          <p v-if="formulaLoading" role="status">{{ uiText('正在读取公式解释…') }}</p>
          <p v-else-if="formulaError" role="alert">{{ formulaError }} <button @click="openFormulaDialog(formulaDialog)">{{ uiText('重试') }}</button></p>
          <p v-else-if="!formulaDialog.formula">{{ uiText('这条公式暂时没有解释资料。') }}</p>
          <div v-if="formulaDialog.formula" class="formula-dialog-profile">
            <p class="formula-dialog-meaning">{{ formulaDialog.formula.composition.meaning }}</p>
            <div v-if="formulaDialog.formula.parameters.length" class="formula-dialog-section">
              <h4>{{ uiText('参数') }}</h4>
              <dl>
                <template v-for="p in formulaDialog.formula.parameters" :key="p.symbol">
                  <dt>
                    <span class="formula-param-symbol" v-html="renderFormulaSymbol(p.symbol)"></span>
                    <span v-if="p.label" class="formula-param-label"> · {{ p.label }}</span>
                  </dt>
                  <dd>
                    {{ p.meaning }}
                    <span v-if="p.unit"> {{ uiText('· 单位:') }} {{ p.unit }}</span>
                    <span v-if="p.domain"> {{ uiText('· 取值域:') }} {{ p.domain }}</span>
                  </dd>
                </template>
              </dl>
            </div>
            <div v-if="formulaDialog.formula.context_links.length" class="formula-dialog-section">
              <h4>{{ uiText('上下文关系') }}</h4>
              <ul>
                <li v-for="link in formulaDialog.formula.context_links" :key="`${link.target_lid}:${link.relation}`">
                  <strong>{{ link.relation }}</strong> {{ link.description }}
                </li>
              </ul>
            </div>
          </div>
        </div>
      </section>
    </div>

    <BoardSourcePreview v-if="sourcePreview" context="reader" :source="sourcePreview.source"
      :read-window="sourcePreview.readWindow" @close="closeSourcePreview" @open-reader="openSourceInReader" />

    <ShareImagePanel v-if="shareDraft" :source="shareDraft.source" :loading="shareDraft.loading" @close="closeShare" />
    <NoteEditorPanel v-if="noteEditor && !noteCollapsed" v-model:content="noteEditor.content"
      :title="noteEditor.title ?? uiText('阅读笔记')" :summary="noteEditor.summary" :revision="noteEditor.receipt?.reference.revision"
      :excerpt="noteEditor.excerpt" :legacy="noteEditor.legacy" :saving="noteSaving" :disabled="!!noteTransition" :error="noteError"
      @save="saveNote" @collapse="noteCollapsed = true" @discard="cancelNote" />
    <button v-if="noteEditor && noteCollapsed" class="note-resume" @click="noteCollapsed = false">{{ uiText('继续编辑笔记') }}</button>
    <div v-if="noteTransition" class="note-transition" role="dialog" aria-modal="true" :aria-label="uiText('处理未保存笔记')">
      <p>{{ noteTransition.label }}{{ uiText('前，如何处理未保存的笔记？') }}</p>
      <p v-if="noteError" role="alert">{{ noteError }}</p>
      <button :disabled="noteSaving" @click="resolveNoteTransition('save')">{{ uiText('保存并继续') }}</button>
      <button :disabled="noteSaving" @click="resolveNoteTransition('discard')">{{ uiText('放弃并继续') }}</button>
      <button :disabled="noteSaving" @click="resolveNoteTransition('continue')">{{ uiText('继续编辑') }}</button>
    </div>
    </div>
    <ClosedBookPractice v-if="readerVisible" :state="closedBook" :restoring="appSurface === 'reader' && !closedReady" :busy="sending || closedBusy" :error="closedError" :messages="closedMessages" :draft="closedDraft" @update:draft="changeClosedDraft" @send="sendClosedBook" @help="helpClosedBook" @leave="leaveClosedBook" @retry="retryClosedBook" @resume="startClosedBook" />
  </div>
</template>
<style scoped>
.reader-surfaces { display: contents; }
.understanding-page{flex:1;min-height:0}.understanding-nav-entry{border:0;background:transparent;font:500 13px/1.5 var(--sans);color:var(--brand-accent);padding:8px 12px;cursor:pointer}
.board-workspace-overlay {
  position: fixed;
  inset: 0;
  z-index: 140;
  display: flex;
  flex-direction: column;
  min-height: 0;
  background: var(--bg, #f5f4ef);
}
.board-workspace-overlay > .board-workspace { flex: 1; min-height: 0; }
.notes-workspace-overlay { position: fixed; inset: 0; z-index: 145; display: flex; flex-direction: column; background: var(--bg); }
.notes-workspace-overlay > .notes-workspace { flex: 1; min-height: 0; }
.note-question-context { display: block; font-size: 12px; color: var(--accent); }
.note-transition { position: fixed; z-index: 150; bottom: 24px; right: 16px; box-sizing: border-box; width: min(380px, calc(100vw - 32px)); padding: 20px; background: var(--canvas); border: 1px solid var(--line); border-radius: 12px; box-shadow: 0 8px 32px #0003; }
.note-transition button { min-height: 44px; margin: 4px; }
.note-resume { position: fixed; z-index: 120; right: 20px; bottom: 24px; padding: 12px 18px; background: var(--canvas); color: var(--accent); border: 1px solid var(--line); border-radius: 12px; }
.desktop-settings-modal {
  position: fixed;
  inset: 0;
  z-index: 80;
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 1rem;
  background: rgba(10, 10, 10, 0.3);
  backdrop-filter: blur(8px);
  -webkit-backdrop-filter: blur(8px);
}
.local-agent-picker { max-width:9.5rem; min-height:30px; border:1px solid var(--line); border-radius:7px; color:var(--ink); background:transparent; font:inherit; font-size:.78rem; }
.desktop-settings-dialog {
  width: min(56rem, 94vw);
  max-height: min(90vh, 48rem);
  display: flex;
  flex-direction: column;
  overflow: hidden;
  border: 1px solid var(--hairline);
  border-radius: 8px;
  background: rgba(255, 255, 255, 0.98);
}
.desktop-settings-head {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 1rem;
  padding: 0.85rem 1rem;
  border-bottom: 1px solid var(--line);
}
.desktop-settings-head h3 {
  margin: 0;
  font-size: 1rem;
}
.desktop-settings-sections { display: flex; flex: 0 0 auto; gap: 18px; padding: 0 20px; border-bottom: 1px solid var(--line); }
.desktop-settings-sections button { padding: 10px 2px; border: 0; border-bottom: 2px solid transparent; background: transparent; color: var(--steel); font: 500 14px var(--sans); cursor: pointer; }
.desktop-settings-sections button[aria-pressed=true] { color: var(--brand-accent-deep); border-bottom-color: var(--brand-accent); }
.desktop-settings-body {
  min-height: 0;
  overflow-y: auto;
  padding: 1rem;
}
.desktop-settings-section + .desktop-settings-section {
  margin-top: 1rem;
  border-top: 1px solid var(--line);
  padding-top: 1rem;
}
.desktop-settings-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 1rem;
}
.desktop-settings-row strong {
  font-size: 0.9rem;
}
.desktop-settings-row p,
.desktop-settings-message,
.desktop-settings-path {
  margin: 0.25rem 0 0;
  color: var(--slate);
  font-size: 0.82rem;
  line-height: 1.45;
}
.desktop-settings-row button {
  min-height: 36px;
  flex: 0 0 auto;
}
.desktop-library-button {
  display: inline-flex;
  align-items: center;
  gap: 0.42rem;
}
.desktop-provider-heading {
  align-items: flex-start;
}
.desktop-provider-mode {
  display: inline-grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  border: 1px solid var(--hairline);
  border-radius: 6px;
  overflow: hidden;
}
.desktop-provider-mode button {
  min-height: 32px;
  border: 0;
  border-radius: 0;
  background: #fff;
  padding: 0.35rem 0.65rem;
  color: var(--slate);
}
.desktop-provider-mode button + button {
  border-left: 1px solid var(--hairline);
}
.desktop-provider-mode button.active {
  background: var(--ink);
  color: #fff;
}
.desktop-provider-grid {
  display: grid;
  grid-template-columns: minmax(0, 1fr) minmax(9rem, 0.58fr);
  gap: 0.7rem;
  margin-top: 0.8rem;
}
.desktop-provider-grid label {
  min-width: 0;
  display: grid;
  gap: 0.3rem;
  color: var(--steel);
  font-size: 0.75rem;
  font-weight: 650;
}
.desktop-provider-grid input {
  width: 100%;
  min-width: 0;
  box-sizing: border-box;
}
.desktop-provider-key {
  grid-column: 1 / -1;
}
.desktop-provider-actions {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 0.8rem;
  margin-top: 0.75rem;
}
.desktop-provider-actions p {
  margin: 0;
  color: var(--steel);
  font-size: 0.72rem;
  line-height: 1.4;
}
.desktop-provider-save {
  min-height: 36px;
  flex: 0 0 auto;
  display: inline-flex;
  align-items: center;
  gap: 0.4rem;
}
.desktop-settings-message {
  margin-top: 0.85rem;
}
.desktop-settings-path {
  overflow-wrap: anywhere;
  font-family: var(--mono);
  font-size: 0.75rem;
}
.app-loading {
  min-height: 0;
  flex: 1 1 auto;
  display: grid;
  place-items: center;
  background: var(--reader-canvas);
  color: var(--steel);
  font-size: 0.9rem;
}
.agent {
  /* 固定宽侧栏:flex 0 0 不随内容增长。修 S10g 回归——面板 class 从 .qa 改 .agent 时丢了 width,
     默认 flex:0 1 auto 让右栏被长答案/trace 内容撑满全屏、挤垮分屏与按钮。 */
  flex: 0 0 24rem;
  display: flex;
  flex-direction: column;
  border-left: 1px solid var(--line);
  min-width: 0;
  padding: 1rem;
  overflow-y: auto;
}
.agent-head {
  display: flex;
  justify-content: space-between;
  align-items: center;
}
.new-chat {
  font-size: 0.85em;
}
.transcript {
  flex: 1;
  overflow-y: auto;
  /* 长答案/无空格长串(trace digest、JSON args)在固定宽侧栏内换行,不横向撑破。 */
  overflow-wrap: anywhere;
  word-break: break-word;
}
.turn {
  margin-bottom: 1.2em;
}
.u-msg {
  font-weight: 600;
}
.a-msg {
  background: #f7f7f9;
  border-radius: 6px;
  padding: 0.6em 0.8em;
}
/* `.md` 渲染排版移到全局 style.css —— scoped 样式够不到 v-html 注入的子节点。 */
.pending {
  color: #888;
}
.incomplete {
  color: #b35;
}
.proposals {
  margin-top: 0.6em;
  border-top: 1px dashed #ccc;
  padding-top: 0.4em;
}
.prop-h {
  font-size: 0.8em;
  color: #666;
}
.proposal {
  display: flex;
  align-items: center;
  gap: 0.5em;
  margin: 0.25em 0;
  font-size: 0.9em;
}
.prop-label {
  flex: 1;
}
.proposal button.undo {
  color: #b35;
}
.done {
  color: #393;
  font-size: 0.85em;
}
.agent-input {
  display: flex;
  flex-direction: column;
  gap: 0.4em;
  border-top: 1px solid #ddd;
  padding-top: 0.5em;
}
.agent-input textarea {
  width: 100%;
  box-sizing: border-box;
}

/* 段内选区浮动工具条 */
.hl-popover {
  position: fixed;
  transform: translateX(-50%);
  z-index: 50;
  display: flex;
  gap: 0.25rem;
  padding: 0.25rem;
  border-radius: 999px;
  background: var(--ink);
  box-shadow: none;
}
.hl-popover:not(.pdf-selection-toolbar) {
  width: max-content;
  max-width: calc(100vw - 16px);
  box-sizing: border-box;
  flex-wrap: wrap;
  justify-content: center;
}
.hl-popover button {
  background: transparent;
  color: #fff;
  border: none;
  border-radius: 999px;
  padding: 0.28rem 0.65rem;
  font-size: 0.82rem;
  min-height: 36px;
  cursor: pointer;
}
.hl-popover button:hover {
  background: rgba(255, 255, 255, 0.12);
}
.hl-popover button:disabled {
  cursor: not-allowed;
  opacity: 0.42;
}
.pdf-selection-toolbar {
  min-width: min(336px, calc(100vw - 16px));
  max-width: min(520px, calc(100vw - 16px));
  min-height: 44px;
  align-items: center;
  justify-content: center;
  border-radius: 8px;
}
.pdf-selection-toolbar button {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: 0.3rem;
  border-radius: 6px;
}
.pdf-selection-status {
  padding: 0 0.45rem;
  color: rgba(255, 255, 255, 0.82);
  font-size: 0.76rem;
  white-space: nowrap;
}
.pdf-selection-status.error {
  max-width: 220px;
  overflow: hidden;
  color: #ffd4ce;
  text-overflow: ellipsis;
}
@media (max-width: 640px) {
  .pdf-selection-toolbar {
    left: 8px !important;
    right: 8px;
    min-width: 0;
    max-width: none;
    transform: none;
  }
  .pdf-selection-status {
    min-width: 0;
    line-height: 1.25;
    white-space: normal;
  }
}

/* 公式 sidecar 查看弹窗 */
.formula-modal {
  position: fixed;
  inset: 0;
  z-index: 60;
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 1.5rem;
  background: rgba(10, 10, 10, 0.28);
  backdrop-filter: blur(8px);
  -webkit-backdrop-filter: blur(8px);
}
.formula-dialog {
  width: min(52rem, 94vw);
  max-height: 86vh;
  display: flex;
  flex-direction: column;
  overflow: hidden;
  border: 1px solid var(--hairline);
  border-radius: 12px;
  background: rgba(255, 255, 255, 0.96);
  box-shadow: none;
}
.formula-dialog-head {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 1rem;
  padding: 0.85rem 1rem;
  border-bottom: 1px solid var(--line);
}
.formula-dialog-kicker {
  margin: 0 0 0.15rem;
  color: var(--steel);
  font-size: 0.72rem;
  font-weight: 650;
  text-transform: uppercase;
  letter-spacing: 0;
}
.formula-dialog-head h3 {
  margin: 0;
  font-family: var(--mono);
  font-size: 1rem;
}
.formula-dialog-close {
  width: 44px;
  height: 44px;
  min-height: 44px;
  flex: 0 0 auto;
  border: 1px solid var(--hairline);
  border-radius: 999px;
  background: var(--canvas);
  color: var(--muted);
  padding: 0;
  font-size: 1rem;
}
.book-picker-modal {
  position: fixed;
  inset: 0;
  z-index: 70;
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 1.5rem;
  background: rgba(10, 10, 10, 0.28);
  backdrop-filter: blur(8px);
  -webkit-backdrop-filter: blur(8px);
}
.book-picker-dialog {
  width: min(42rem, 94vw);
  max-height: 84vh;
  display: flex;
  flex-direction: column;
  overflow: hidden;
  border: 1px solid var(--hairline);
  border-radius: 12px;
  background: rgba(255, 255, 255, 0.97);
  box-shadow: none;
}
.book-picker-head,
.book-picker-actions {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 1rem;
  padding: 0.85rem 1rem;
  border-bottom: 1px solid var(--line);
}
.book-picker-head h3 {
  margin: 0;
  font-size: 1rem;
}
.book-picker-tabs {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 2px;
  margin: 0.8rem 1rem 0;
  border: 1px solid var(--hairline);
  border-radius: 8px;
  background: var(--canvas);
  padding: 2px;
}
.book-picker-tabs button {
  min-height: 38px;
  border: 0;
  border-radius: 6px;
  background: transparent;
  color: var(--muted);
}
.book-picker-tabs button.active {
  background: #fff;
  color: var(--ink);
  box-shadow: 0 0 0 1px var(--hairline-soft);
}
.book-picker-body {
  min-height: 0;
  overflow-y: auto;
  padding: 1rem;
}
.book-picker-input-row {
  display: grid;
  grid-template-columns: minmax(0, 1fr) auto;
  gap: 0.75rem;
  align-items: end;
}
.book-picker-input-row label {
  display: grid;
  gap: 0.32rem;
  color: var(--steel);
  font-size: 0.78rem;
  font-weight: 650;
}
.book-picker-input-row input {
  width: 100%;
  min-height: 42px;
  box-sizing: border-box;
  border: 1px solid var(--hairline);
  border-radius: 8px;
  background: #fff;
  color: var(--ink);
  padding: 0.55rem 0.7rem;
  font-family: var(--mono);
  font-size: 0.84rem;
}
.book-create-grid {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 0.85rem;
}
.book-create-grid label {
  display: grid;
  gap: 0.32rem;
  min-width: 0;
  color: var(--steel);
  font-size: 0.78rem;
  font-weight: 650;
}
.book-create-grid input {
  width: 100%;
  min-width: 0;
  min-height: 42px;
  box-sizing: border-box;
  border: 1px solid var(--hairline);
  border-radius: 8px;
  background: #fff;
  color: var(--ink);
  padding: 0.55rem 0.7rem;
  font-size: 0.84rem;
}
.book-create-generated-id {
  display: block;
  width: 100%;
  min-width: 0;
  min-height: 42px;
  box-sizing: border-box;
  overflow: hidden;
  border: 1px solid var(--hairline-soft);
  border-radius: 8px;
  background: var(--surface-code);
  color: var(--slate);
  padding: 0.62rem 0.7rem;
  font-family: var(--mono);
  font-size: 0.8rem;
  font-weight: 500;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.book-picker-input-row button,
.book-picker-actions button {
  min-height: 42px;
  border: 1px solid var(--hairline);
  border-radius: 8px;
  background: var(--canvas);
  color: var(--ink);
  padding: 0 0.9rem;
}
.book-picker-root {
  margin: 0.75rem 0 0;
  color: var(--muted);
  font-family: var(--mono);
  font-size: 0.75rem;
  overflow-wrap: anywhere;
}
.book-picker-error,
.book-picker-state {
  margin: 0.85rem 0 0;
  color: var(--muted);
  font-size: 0.86rem;
}
.book-picker-error {
  color: #9f2d2d;
}
.book-picker-list {
  display: grid;
  grid-template-columns: 1fr;
  gap: 0.6rem;
  margin-top: 0.9rem;
}
.book-picker-card {
  display: grid;
  grid-template-columns: 48px minmax(0, 1fr) 20px;
  align-items: center;
  gap: 1rem;
  min-width: 0;
  width: 100%;
  min-height: 58px;
  border: 1px solid var(--hairline-soft);
  border-radius: 8px;
  background: var(--surface);
  color: var(--ink);
  padding: 0.65rem 0.75rem;
  text-align: left;
}
.book-picker-card:hover,
.book-picker-card.active {
  border-color: var(--accent);
  background: #fff;
}
.book-picker-card strong {
  font-family: var(--serif);
  font-size: 1rem;
  line-height: 1.55;
  overflow-wrap: break-word;
}
.book-picker-card-title {
  display: flex;
  flex-direction: column;
  align-items: flex-start;
  gap: 0.35rem;
}
.book-picker-card-title small {
  color: var(--muted);
  font-size: 0.75rem;
}
.book-picker-selected { color: var(--accent); font-size: 1.2rem; }
.desktop-provider-advanced { margin: 1rem 0; color: var(--muted); font-size: .8rem; }
.desktop-provider-advanced summary { cursor: pointer; }
.desktop-provider-advanced p { line-height: 1.6; }
.book-picker-toolbar { display: flex; align-items: center; justify-content: space-between; color: var(--muted); font-size: .8rem; }
.book-picker-toolbar button { padding: .4rem .7rem; border: 1px solid var(--hairline); border-radius: 6px; background: var(--surface); color: var(--ink); }
.book-picker-location { margin-top: 1.2rem; padding-top: 1rem; border-top: 1px solid var(--hairline-soft); color: var(--muted); font-size: .8rem; }
.book-picker-location summary { cursor: pointer; }
.book-picker-location .book-picker-input-row { margin-top: .8rem; }
.book-picker-card:nth-child(3n + 2) { --cover-color: #ece2d4; }
.book-picker-card:nth-child(3n) { --cover-color: #dfe6ee; }
.book-picker-actions {
  border-top: 1px solid var(--line);
  border-bottom: none;
}
.book-picker-actions .primary-action {
  background: var(--ink);
  color: #fff;
}
@media (max-width: 640px) {
  .book-create-grid {
    grid-template-columns: 1fr;
  }
  .desktop-provider-grid {
    grid-template-columns: 1fr;
  }
  .desktop-provider-key {
    grid-column: auto;
  }
  .desktop-provider-actions {
    align-items: stretch;
    flex-direction: column;
  }
  .desktop-provider-save {
    justify-content: center;
  }
}
.formula-dialog-body {
  min-height: 0;
  overflow-y: auto;
  padding: 1rem;
}
.formula-dialog-source {
  margin: 0 0 1rem;
  border: 1px solid var(--hairline-soft);
  border-radius: 8px;
  background: var(--surface-soft);
  padding: 0.85rem;
  overflow-x: auto;
  white-space: pre-wrap;
  overflow-wrap: anywhere;
  font-family: var(--mono);
  font-size: 0.9rem;
  line-height: 1.55;
}
.formula-dialog-profile {
  display: grid;
  gap: 0.9rem;
}
.formula-dialog-meaning {
  margin: 0;
  color: var(--ink);
  font-weight: 650;
  line-height: 1.55;
}
.formula-dialog-section {
  border-top: 1px solid var(--hairline-soft);
  padding-top: 0.8rem;
}
.formula-dialog-section h4 {
  margin: 0 0 0.45rem;
  font-size: 0.9rem;
}
.formula-dialog-section dl {
  margin: 0;
}
.formula-dialog-section dt {
  display: flex;
  flex-wrap: wrap;
  align-items: baseline;
  gap: 0.1rem;
  font-weight: 650;
}
.formula-param-symbol {
  min-width: 0;
}
.formula-param-label {
  color: var(--ink);
}
.formula-dialog-section dd {
  margin: 0 0 0.55rem 1rem;
  color: var(--slate);
  line-height: 1.5;
}
.formula-dialog-section ul {
  margin: 0;
  padding-left: 1.2rem;
  color: var(--slate);
  line-height: 1.5;
}

/* 笔记编辑器模态 */
.note-modal {
  position: fixed;
  inset: 0;
  background: rgba(10, 10, 10, 0.28);
  backdrop-filter: blur(8px);
  -webkit-backdrop-filter: blur(8px);
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 1.5rem;
  z-index: 60;
}
.note-dialog {
  width: min(46rem, 92vw);
  max-height: 86vh;
  background: rgba(255, 255, 255, 0.96);
  border: 1px solid var(--hairline);
  border-radius: 12px;
  box-shadow: none;
  display: flex;
  flex-direction: column;
  overflow: hidden;
}
.nd-head {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 0.7rem 1rem;
  border-bottom: 1px solid var(--line);
  font-weight: 600;
}
.nd-close {
  width: 44px;
  height: 44px;
  min-height: 44px;
  border: 1px solid var(--hairline);
  border-radius: 999px;
  background: var(--canvas);
  font-size: 1rem;
  cursor: pointer;
  color: var(--muted);
  padding: 0;
}
.nd-body {
  display: flex;
  gap: 0;
  flex: 1;
  min-height: 16rem;
}
.nd-input {
  flex: 1;
  border: none;
  border-right: 1px solid var(--line);
  border-radius: 0;
  padding: 1rem;
  font: inherit;
  resize: none;
  outline: none;
}
.nd-preview {
  flex: 1;
  padding: 1rem;
  overflow-y: auto;
  background: var(--canvas-parchment);
}
.nd-foot {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 0.6rem 1rem;
  border-top: 1px solid var(--line);
}
.nd-hint {
  color: var(--muted);
  font-size: 0.8rem;
}
.nd-actions {
  display: flex;
  gap: 0.5rem;
}
.nd-actions .primary {
  background: var(--accent);
  color: #fff;
  border-color: var(--accent);
}
.reader-pane [data-lid].note-placement-candidate {
  outline: 2px solid var(--accent);
  outline-offset: 3px;
  background: rgba(116, 86, 55, 0.1);
  cursor: crosshair;
}
.note-placement-status {
  position: fixed;
  left: 50%;
  bottom: 1.25rem;
  transform: translateX(-50%);
  z-index: 80;
  width: min(34rem, calc(100vw - 2rem));
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 1rem;
  padding: 0.75rem 0.9rem;
  border: 1px solid var(--hairline);
  border-radius: 10px;
  background: var(--canvas);
  box-shadow: 0 8px 28px rgba(34, 31, 27, 0.16);
}
.note-placement-status button {
  flex: 0 0 auto;
}
.practice-margin-setting { display: inline-flex; align-items: center; gap: 6px; margin-top: 4px; color: var(--steel); font-size: .73rem; cursor: pointer; }
</style>
