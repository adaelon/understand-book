<script setup lang="ts">
import { t as uiText } from '@understand-book/web/reader/i18n';
import { useAccountPresence } from './account-presence';

import LanguageSwitcher from '@understand-book/web/reader/components/LanguageSwitcher.vue';
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue';
import App from '@understand-book/web/reader/App.vue';
import BoardLibrary from '@understand-book/web/reader/components/BoardLibrary.vue';
import WorkspaceDestinations from '@understand-book/web/reader/components/WorkspaceDestinations.vue';
import LearningHomeTask from '@understand-book/web/reader/components/LearningHomeTask.vue';
import { inquiryCourse } from '@understand-book/web/reader/inquiry-course';
import type { CourseLaunchTarget, LearningTaskView } from '@understand-book/web/reader/learning-tasks';
import { readLearningCollection, learningCollectionUrl, type LearningCollectionTarget } from '@understand-book/web/reader/learning-collections';
import UnderstandingEntry from '@understand-book/web/reader/components/UnderstandingEntry.vue';
import ProductTour from '@understand-book/web/reader/components/ProductTour.vue';
import type { ProductTourContext, ProductTourStep } from '@understand-book/web/reader/product-tour';
import type { BoardSummary } from '@understand-book/web/reader/generated/BoardSummary';
import NotesWorkspace from '@understand-book/web/reader/components/NotesWorkspace.vue';
import BoardSourcePreview from '@understand-book/web/reader/components/BoardSourcePreview.vue';
import type { SourcePopupView, SourceWindowLoader, WorkspaceNoteSourceRequest } from '@understand-book/web/reader/api';
import { pendingNoteSource, noteSourceError } from '@understand-book/web/reader/personal-notes';
import { isTauri } from '@tauri-apps/api/core';
import DesktopAuthorization from './components/DesktopAuthorization.vue';
import DesktopSignIn from './components/DesktopSignIn.vue';
import VideoReaderPane from '@understand-book/web/reader/components/VideoReaderPane.vue';
import { preloadVideoPlayback } from '@understand-book/web/reader/video-playback';
import type { PreparingVideo } from '@understand-book/web/reader/video-api';
const videos = ref<PreparingVideo[]>([]), selectedVideo = ref<PreparingVideo | null>(null);
import MaterialCover from '@understand-book/web/reader/components/MaterialCover.vue';
import { ArrowUpRight, ArrowRight, Plus, ChevronDown, BookOpen, Layers, Settings2, UserRound } from '@lucide/vue';
import type { SharedBook } from '@understand-book/web/reader/shared-workspaces';
import YeyeMascot from '@understand-book/web/reader/components/YeyeMascot.vue';
import brandLogo from '@understand-book/web/reader/assets/brand/understand-book-logo.svg';
import { bookTitle } from '@understand-book/web/reader/book-title';
import LoadingAnimation from '@understand-book/web/reader/components/LoadingAnimation.vue';
import AccountAllowance from './components/AccountAllowance.vue';
import AccountSettings from './components/AccountSettings.vue';
import AccountMcp from './components/AccountMcp.vue';
import LearningRecordsPanel from './components/LearningRecordsPanel.vue';
import SharedWorkspaceCatalog from './components/SharedWorkspaceCatalog.vue';
import SharedWorkspaceInstance from './components/SharedWorkspaceInstance.vue';
import ExamSprintPanel from './components/ExamSprintPanel.vue';
import PersonalMaterials from './components/PersonalMaterials.vue';
import { readMcpSource, sourceParams, type McpSource } from '@understand-book/web/reader/mcp-source';
import { readBoardRoute, boardRouteUrl } from '@understand-book/web/reader/board-route';
import AccountAccess from './components/AccountAccess.vue';
import { takeAccountLink, type AccountView } from './account-forms';
import AgentPresentation from '@understand-book/web/reader/components/AgentPresentation.vue';
import SourceExcerpt from '@understand-book/web/reader/components/SourceExcerpt.vue';
import { api, ApiError } from '@understand-book/web/reader/api';
import { invalidateLinkedWindows, isLinkedReaderChange } from './network-presentation';
import { network, installIdentity, installWorkspace, forgetNetwork, sceneKey, readerKey, savedWorkspace, saveWorkspace, type ChatDraft, type NetworkIdentity, type NetworkWorkspace, type PublishedBookRef, type LearningWorkspace } from './network-context';
import { abortable, networkFetch, recoverWorkspaceBinding, requestWorkspaceRecovery, recoverSubmissions, unconfirmedSubmissions, retrySubmission, readReadingResumption, learningWorkspaces, openLearningMaterial, openLibraryMaterial, type ReadingResumption, type PendingSubmission } from './network-client';
import { savedLearningSelection, saveLearningSelection, learningDraftKey, readLearningDraft, saveLearningDraft } from '@understand-book/web/reader/learning-workspace-state';
import { targetPublication, type RecapTarget } from '@understand-book/web/reader/session-recap';
const username = ref(''), password = ref(''), error = ref(''), busy = ref(false), checking = ref(true);
type BookOpening = { kind: 'book'; reference: PublishedBookRef; learningId: string | null; members?: PublishedBookRef[]; title: string; board?: { id?: string; title: string }; course?: { lessonId: string; runId?: string; chatSessionId?: string }; task?: LearningTaskView; reading?: boolean }
  | { kind: 'resume'; record: ReadingResumption; title: string };
const openingRequest = ref<BookOpening | null>(null), opening = ref(false), openingError = ref('');
let openingController: AbortController | null = null;
function showFailure(failure: unknown) {
  // A superseded request has nothing to report in the newly selected book or chat.
  if (failure instanceof ApiError && failure.errorCode === 'CLIENT_CONTEXT_STALE') return;
  error.value = failure instanceof Error ? failure.message : String(failure);
}
const sourceTarget = ref<McpSource | null>(null), sourceError = ref('');
let pendingSource: McpSource | null = null;
let pendingBoard = readBoardRoute(location.search);
try { pendingSource = readMcpSource(location.search); } catch (failure) { sourceError.value = (failure as Error).message; }
let mcpRequested = new URLSearchParams(location.search).get('account') === 'mcp';
const accountLink = takeAccountLink();
const accountView = ref<AccountView | null>(accountLink.view), resetToken = ref(accountLink.token);
useAccountPresence(() => !checking.value && !accountView.value);
accountLink.token = '';
function showAccount(view: AccountView | null) {
  accountView.value = view; password.value = ''; resetToken.value = '';
  const url = new URL(location.href); url.hash = '';
  if (view) url.searchParams.set('account', view); else url.searchParams.delete('account');
  history.replaceState(history.state, '', url);
}
function accountLogin(email?: string, notice?: string) {
  showAccount(null); if (email) username.value = email; error.value = notice ?? '';
  checking.value = false;
}
function passwordChanged() {
  forgetNetwork(); books.value = []; pendingItems.value = []; settingsOpen.value = false;
  accountLogin(undefined, uiText('密码已更新，请使用新密码重新登录。'));
  channel?.postMessage('changed');
}
type CatalogBook = SharedBook & { is_default?: boolean };
const books = ref<CatalogBook[]>([]);
const materialBooks = computed(() => books.value.map(book => {
  const video = videos.value.find(video => video.publication && samePublication(video.publication, book.published_book_ref));
  return { ...book, title: book.title?.trim() || video?.title, media_kind: video ? 'video' as const : book.media_kind };
}));
const pendingVideos = computed(() => videos.value.filter(video => !video.publication
  || !books.value.some(book => samePublication(book.published_book_ref, video.publication!))));
const materialFor = (reference: PublishedBookRef) => materialBooks.value.find(book => samePublication(book.published_book_ref, reference));
const materialTitle = (reference: PublishedBookRef) => bookLabel(materialFor(reference) ?? { published_book_ref: reference });
const learningSpaces = ref<LearningWorkspace[]>([]), learningError = ref(''), learningLoading = ref(false);
const selectedLearningId = ref<string | null>(network.value.workspace?.learning_workspace_id ?? savedLearningSelection());
const selectedLearning = computed(() => learningSpaces.value.find(space => space.learning_workspace_id === selectedLearningId.value) ?? null);
const createGoal = ref(''), goalDraft = ref(''), editingGoal = ref(false), createOpen = ref(false);
const materialsOpen = ref(false);
const recordsOpen = ref(false), legacyRecordsOpen = ref(false);
const sharedOpen = ref(true);
const memberBooks = computed(() => selectedLearning.value?.members.map(member => ({ ...member,
  book: materialFor(member.published_book_ref),
})) ?? []);
const availableBooks = computed(() => materialBooks.value.filter(book => !selectedLearning.value?.members.some(member => samePublication(member.published_book_ref, book.published_book_ref))));
function samePublication(left: PublishedBookRef, right: PublishedBookRef) { return left.book_id === right.book_id && left.publication_id === right.publication_id; }
async function loadLearningSpaces() {
  learningLoading.value = true; learningError.value = '';
  try { learningSpaces.value = (await learningWorkspaces.list()).workspaces; }
  catch (failure) { if (!(failure instanceof ApiError && failure.errorCode === 'CLIENT_CONTEXT_STALE')) learningError.value = failure instanceof Error ? failure.message : String(failure); }
  finally { learningLoading.value = false; }
}
function selectLearning(space: LearningWorkspace | null) {
  selectedLearningId.value = space?.learning_workspace_id ?? null; saveLearningSelection(selectedLearningId.value);
  editingGoal.value = false; goalDraft.value = ''; error.value = '';
  recordsOpen.value = false; legacyRecordsOpen.value = false;
  materialsOpen.value = false;
}
function updateLearning(space: LearningWorkspace) {
  learningSpaces.value = [space, ...learningSpaces.value.filter(item => item.learning_workspace_id !== space.learning_workspace_id)];
}
async function createLearning() {
  if (!createGoal.value.trim() || busy.value) return;
  busy.value = true; error.value = '';
  try { const space = await learningWorkspaces.create(createGoal.value.trim()); updateLearning(space); selectLearning(space); coursesHome.value = false; sharedOpen.value = false; createGoal.value = ''; createOpen.value = false; }
  catch (failure) { showFailure(failure); }
  finally { busy.value = false; }
}
async function startSharedLearning(space: LearningWorkspace, message?: string, preparedCourse = false, target?: CourseLaunchTarget) {
  updateLearning(space); sharedOpen.value = false;
  selectLearning(space);
  if (preparedCourse && !message) {
    try {
      if (target) {
        const task = learningTasks.value.find(item => item.learning_workspace_id === space.learning_workspace_id && item.course_run_id === target.runId && item.status !== 'unavailable');
        if (task) await continueLearningTask(task);
        else await openBookCourse(target, space.learning_workspace_id);
      }
      else await enterLearning(space);
    } catch (failure) { showFailure(failure); }
    return;
  }
  await enterLearning(space);
  if (message && network.value.workspace?.learning_workspace_id === space.learning_workspace_id && !libraryOpen.value) {
    await nextTick(); readerApp.value?.prepareExamQuestion(message);
  }
}
async function beginExamLearning(message: string) {
  const space = selectedLearning.value; if (!space) return;
  await enterLearning(space);
  if (network.value.workspace?.learning_workspace_id === space.learning_workspace_id && !libraryOpen.value) {
    await nextTick(); readerApp.value?.prepareExamQuestion(message);
  }
}
async function beginExamPractice(remainingDays: number) {
  const space = selectedLearning.value; if (!space) return;
  await enterLearning(space);
  if (network.value.workspace?.learning_workspace_id === space.learning_workspace_id && !libraryOpen.value) {
    await nextTick(); await readerApp.value?.startExamPractice(remainingDays);
  }
}
async function updateSharedLearning(space: LearningWorkspace) {
  updateLearning(space);
  const current = network.value.workspace;
  if (current?.learning_workspace_id !== space.learning_workspace_id
    || space.members.some(member => member.readable && samePublication(member.published_book_ref, current.published_book_ref))) return;
  const next = space.members.find(member => member.readable && member.published_book_ref.book_id === current.published_book_ref.book_id)
    ?? space.members.find(member => member.readable);
  if (!next) return;
  busy.value = true; error.value = '';
  try { await openLearningMaterial(next.published_book_ref, space.learning_workspace_id); }
  catch (failure) { showFailure(failure); }
  finally { busy.value = false; }
}
async function archiveLearning() {
  const space = selectedLearning.value; if (!space || busy.value) return;
  busy.value = true; error.value = '';
  try { updateLearning(await learningWorkspaces.archive(space.learning_workspace_id, !space.archived)); }
  catch (failure) { showFailure(failure); }
  finally { busy.value = false; }
}
async function reviseGoal() {
  const space = selectedLearning.value;
  if (!space || !goalDraft.value.trim() || busy.value) return;
  busy.value = true; error.value = '';
  try {
    updateLearning(await learningWorkspaces.goal(space.learning_workspace_id, goalDraft.value.trim(), space.goal.constraints)); editingGoal.value = false;
    if (network.value.workspace?.learning_workspace_id === space.learning_workspace_id) {
      const latest = await networkFetch<NetworkWorkspace>('GET', `/workspaces/${network.value.workspace.workspace_id}`); installWorkspace(latest);
      window.dispatchEvent(new Event('tutor-state-changed'));
    }
  } catch (failure) { showFailure(failure); }
  finally { busy.value = false; }
}
async function changeMember(reference: PublishedBookRef, add: boolean) {
  const space = selectedLearning.value; if (!space || busy.value) return;
  busy.value = true; error.value = '';
  try { updateLearning(await learningWorkspaces.member(space.learning_workspace_id, reference, add)); }
  catch (failure) { showFailure(failure); }
  finally { busy.value = false; }
}
async function enterLearning(space: LearningWorkspace) {
  if (busy.value) return;
  selectLearning(space);
  const task = learningTasks.value.find(item => item.learning_workspace_id === space.learning_workspace_id && item.status !== 'unavailable');
  if (task) { await continueLearningTask(task); return; }
  if (space.shared_release) {
    try {
      const catalog = await inquiryCourse.catalog(space.learning_workspace_id);
      const run = catalog.progress.find(item => item.goal_revision === space.goal.revision && ['active', 'paused'].includes(item.status));
      const lesson = catalog.course?.lessons.find(item => item.id === run?.lesson_id) ?? catalog.course?.lessons[0];
      if (lesson) { await openBookCourse({ publication: lesson.source.publication, lessonId: lesson.id, runId: run?.run_id }, space.learning_workspace_id); return; }
    } catch (failure) { showFailure(failure); return; }
  }
  if (resumption.value?.learning_workspace_id === space.learning_workspace_id) { await continueReading(); return; }
  const members = space.members.filter(item => item.readable).map(item => item.published_book_ref);
  if (members.length) await openBook(members[0], space.learning_workspace_id, members);
}
function manageCurrentLearning() {
  selectedLearningId.value = network.value.workspace?.learning_workspace_id ?? null;
  saveLearningSelection(selectedLearningId.value); void showLibrary();
}
const libraryOpen = ref(!network.value.workspace || ['boards', 'courses', 'learning', 'materials'].includes(new URLSearchParams(location.search).get('view') ?? ''));
const coursesHome = ref(!['boards', 'materials'].includes(new URLSearchParams(location.search).get('view') ?? ''));
const collectionTarget = ref<LearningCollectionTarget | null>(readLearningCollection(location.search));
watch(collectionTarget, target => {
  if (libraryOpen.value && coursesHome.value) history.replaceState(history.state, '', learningCollectionUrl(location.href, target));
});
const boardsHome = ref(new URLSearchParams(location.search).get('view') === 'boards');
const readerBoardOpen = ref(false);
const readerUnderstandingOpen = ref(false);
const boardsActive = computed(() => libraryOpen.value ? boardsHome.value : readerBoardOpen.value);
const resumption = ref<ReadingResumption | null>(null), resumptionError = ref(''), resumptionLoading = ref(false);
const learningTasks = ref<LearningTaskView[]>([]), learningTasksLoading = ref(false), learningTasksError = ref('');
let learningTasksRead = 0;
async function loadLearningTasks() {
  const ticket = ++learningTasksRead, owner = network.value.identity?.user_id;
  if (!owner) return;
  learningTasksLoading.value = true; learningTasksError.value = '';
  try {
    const value = await networkFetch<{ tasks: LearningTaskView[] }>('GET', '/me/learning-tasks');
    if (ticket === learningTasksRead && owner === network.value.identity?.user_id) learningTasks.value = value.tasks;
  } catch (failure) {
    if (ticket === learningTasksRead && owner === network.value.identity?.user_id) learningTasksError.value = uiText('学习任务暂时无法读取，请重试。');
  } finally { if (ticket === learningTasksRead) learningTasksLoading.value = false; }
}
async function continueLearningTask(task: LearningTaskView) {
  if (busy.value || task.status === 'unavailable') return;
  await runBookOpening({ kind: 'book', reference: task.published_book_ref, learningId: task.learning_workspace_id, title: task.title, task });
}
async function showCourses() {
  await showLearningCollection(null);
}
async function showLearningCollection(target: LearningCollectionTarget | null) {
  if (busy.value) return;
  if (readerApp.value && !await readerApp.value.beforeNoteLeave(uiText('打开学习'))) return;
  boardsHome.value = false; coursesHome.value = true; sharedOpen.value = true; selectedVideo.value = null;
  libraryOpen.value = true; error.value = ''; collectionTarget.value = target;
  if (target?.learningWorkspaceId) {
    const space = learningSpaces.value.find(space => space.learning_workspace_id === target.learningWorkspaceId);
    if (space) selectLearning(space);
  }
  const url = learningCollectionUrl(boardRouteUrl(location.href, null).href, target); history.replaceState(history.state, '', url);
  const id = network.value.workspace?.workspace_id ?? savedWorkspace()?.workspace_id;
  if (id) saveWorkspace(id, 'library');
  try { await catalog(); } catch (failure) { showFailure(failure); }
}
async function readCollectionBook(reference: PublishedBookRef, learningId?: string) {
  await runBookOpening({ kind: 'book', reference, learningId: learningId ?? null, title: materialTitle(reference), reading: true });
}
async function useOwnMaterials() {
  await showLibrary();
  createOpen.value = !learningSpaces.value.length;
  materialsOpen.value = true;
  await nextTick();
  document.querySelector<HTMLElement>(createOpen.value ? '.learning-create input' : '.materials-heading')?.focus();
}
async function scrollCourseCatalog() {
  sharedOpen.value = true;
  await nextTick();
  document.querySelector('.shared-catalog')?.scrollIntoView({ behavior: 'smooth', block: 'start' });
}
const resumptionBook = computed(() => materialBooks.value.find(book =>
  book.published_book_ref.book_id === resumption.value?.published_book_ref.book_id
  && book.published_book_ref.publication_id === resumption.value?.published_book_ref.publication_id));
watch(() => network.value.epoch, () => {
  cancelBookOpening();
  resumption.value = null; resumptionError.value = ''; resumptionLoading.value = false; libraryOpen.value = true;
}, { flush: 'sync' });
async function loadResumption() {
  resumptionError.value = ''; resumptionLoading.value = true;
  const scope = sceneKey();
  try { const value = await readReadingResumption(); if (scope === sceneKey()) resumption.value = value; }
  catch (failure) {
    if (scope === sceneKey()) {
      const unavailable = failure instanceof ApiError && failure.status === 404;
      if (unavailable) resumption.value = null;
      resumptionError.value = unavailable ? uiText('原书籍或阅读记录已不可用，请选择其他材料。') : uiText('接续记录暂时无法读取，请重试。');
    }
  } finally { if (scope === sceneKey()) resumptionLoading.value = false; }
}
async function showLibrary() {
  if (readerApp.value && !await readerApp.value.beforeNoteLeave(uiText('返回书架'))) return;
  boardsHome.value = false; coursesHome.value = false; sharedOpen.value = false; selectedVideo.value = null; collectionTarget.value = null;
  const url = learningCollectionUrl(boardRouteUrl(location.href, null).href, null); url.searchParams.set('view', 'materials'); history.replaceState(history.state, '', url);
  libraryOpen.value = true; error.value = '';
  const id = network.value.workspace?.workspace_id ?? savedWorkspace()?.workspace_id;
  if (id) saveWorkspace(id, 'library');
  try { await catalog(); await loadResumption(); }
  catch (failure) { showFailure(failure); }
}
async function showBoards() {
  if (readerApp.value && !await readerApp.value.beforeNoteLeave(uiText('打开学习画板'))) return;
  boardsHome.value = true; coursesHome.value = false; libraryOpen.value = true; selectedVideo.value = null; error.value = ''; collectionTarget.value = null;
  const url = learningCollectionUrl(boardRouteUrl(location.href, null).href, null); url.searchParams.set('view', 'boards'); history.replaceState(history.state, '', url);
  const id = network.value.workspace?.workspace_id ?? savedWorkspace()?.workspace_id;
  if (id) saveWorkspace(id, 'library');
  try { await catalog(); } catch (failure) { showFailure(failure); }
}
async function showCurrentReader() {
  if (!network.value.workspace || !readerApp.value) return;
  if (!await readerApp.value.beforeNoteLeave(uiText('继续阅读'))) return;
  readerApp.value.closeBoard(); readerApp.value.closeUnderstanding(); libraryOpen.value = false; selectedVideo.value = null;
  const url = new URL(location.href); url.searchParams.delete('view'); history.replaceState(history.state, '', url);
  saveWorkspace(network.value.workspace.workspace_id, 'reader');
}
async function showUnderstanding() {
  if(busy.value)return;
  if(libraryOpen.value && selectedLearning.value && selectedLearning.value.learning_workspace_id!==network.value.workspace?.learning_workspace_id) {
    await enterLearning(selectedLearning.value);
  } else { await showCurrentReader(); }
  await nextTick();
  if(!readerApp.value || libraryOpen.value || openingError.value){error.value=openingError.value||'先在工作区打开一份学习资料，就可以查看对应的理解。';return;}
  readerApp.value.openUnderstanding();
}
async function openLibraryBoard(board: BoardSummary, publication: PublishedBookRef) {
  await runBookOpening({ kind: 'book', reference: publication, learningId: board.learning_workspace_id,
    title: board.title, board: { id: board.board_id, title: board.title } });
}
async function createLibraryBoard(title: string, publication: PublishedBookRef) {
  await runBookOpening({ kind: 'book', reference: publication, learningId: null, title, board: { title } });
}
async function continueReading() {
  if (!resumption.value || busy.value) return;
  await runBookOpening({ kind: 'resume', record: resumption.value, title: materialTitle(resumption.value.published_book_ref) });
}
async function restoreLastVisit() {
  const saved = savedWorkspace();
  await loadResumption();
  if (saved && saved.view !== 'library' && saved.workspace_id === resumption.value?.workspace_id
    && !pendingSource && !pendingBoard && !mcpRequested && !boardsHome.value
    && !['courses', 'learning', 'materials'].includes(new URLSearchParams(location.search).get('view') ?? '')) await continueReading();
}
function bookLabel(book: CatalogBook) {
  const title = bookTitle({ title: materialFor(book.published_book_ref)?.title ?? book.title, book_id: book.published_book_ref.book_id });
  const versions = books.value.filter(item => item.published_book_ref.book_id === book.published_book_ref.book_id);
  if (versions.length === 1) return title;
  return uiText('{0} · 版本 {1}{2}', [title, versions.findIndex(item => samePublication(item.published_book_ref, book.published_book_ref)) + 1, book.is_default ? uiText('（默认）') : '']);
}
const readerApp = ref<InstanceType<typeof App> | null>(null);
const homeNotes = ref<LearningWorkspace | null>(null), homeNotesRef = ref<InstanceType<typeof NotesWorkspace>>();
const homeNotesError = ref(''), homeNotePreview = ref<SourcePopupView | null>(null);
const homeNoteSourceRequest = ref<WorkspaceNoteSourceRequest | null>(null);
const homeNoteSourcePublication = ref<PublishedBookRef | null>(null);
const homeNoteSourceAvailable = ref(false);
const readHomeNoteSource = computed<SourceWindowLoader | undefined>(() => {
  const request = homeNoteSourceRequest.value, scope = homeNotes.value?.learning_workspace_id;
  return request && scope ? options => api.workspaceNoteSourceRead({ ...request, ...options }, scope) : undefined;
});
let homeNoteSourceSequence = 0;
async function openHomeNotes(space: LearningWorkspace) {
  if (readerApp.value && !await readerApp.value.beforeNoteLeave()) return;
  homeNotes.value = space; homeNotesError.value = '';
}
async function closeHomeNotes() {
  if (homeNotesRef.value && !await homeNotesRef.value.beforeLeave()) return;
  homeNotes.value = null;
}
async function notebookReader(action: (app: InstanceType<typeof App>) => unknown, preferred?: PublishedBookRef | null) {
  const space = homeNotes.value;
  if (!space || !await homeNotesRef.value?.beforeLeave()) return;
  const member = space.members.find(item => item.readable && (!preferred || samePublication(item.published_book_ref, preferred)));
  if (!member) { homeNotesError.value = uiText('笔记已保存在这个空间。加入一份资料后，就可以在画板和 Agent 对话中继续使用。'); return; }
  if (space.archived) { homeNotesError.value = uiText('这个空间已归档。恢复空间后可继续讨论和整理。'); return; }
  const current = network.value.workspace;
  if (current?.learning_workspace_id !== space.learning_workspace_id || (preferred && !samePublication(current.published_book_ref, preferred)) || !space.members.some(item => item.readable && samePublication(item.published_book_ref, current.published_book_ref))) {
    await openBook(member.published_book_ref, space.learning_workspace_id);
  } else { libraryOpen.value = false; suspended.value = false; }
  await nextTick();
  if (network.value.workspace?.learning_workspace_id !== space.learning_workspace_id || !readerApp.value || openingError.value) {
    homeNotesError.value = openingError.value || uiText('暂时无法打开学习现场，请重试。'); return;
  }
  homeNotes.value = null;
  await action(readerApp.value);
}
async function homeNoteSource(request: WorkspaceNoteSourceRequest) {
  const space = homeNotes.value; if (!space) return;
  const sequence = ++homeNoteSourceSequence;
  try {
    const note = await api.workspaceNote(request.note_id, space.learning_workspace_id);
    const binding = note.source_bindings.find(item => item.source_ref_id === request.source_ref_id);
    if (!binding) return;
    const source = await api.workspaceNoteSourceResolve(request, space.learning_workspace_id).catch(error => {
      if (!(error instanceof ApiError) || ![403, 404, 409].includes(error.status)) throw error;
      return { source_ref_id: binding.source_ref_id, label: binding.label_snapshot,
        highlighted_quote: binding.preview_snapshot, context_before: '', context_after: '',
        stale: true, can_open_in_reader: false, unavailable_reason: uiText('这是保存笔记时保留的来源摘录。') };
    });
    if (sequence !== homeNoteSourceSequence || homeNotes.value !== space) return;
    homeNoteSourceRequest.value = request;
    homeNoteSourceAvailable.value = !space.archived && space.members.some(item => item.readable && item.published_book_ref.book_id === binding.book_id && (!binding.publication_id || item.published_book_ref.publication_id === binding.publication_id));
    homeNoteSourcePublication.value = space.members.find(item => item.readable && item.published_book_ref.book_id === binding.book_id && (!binding.publication_id || item.published_book_ref.publication_id === binding.publication_id))?.published_book_ref ?? null;
    homeNotePreview.value = source;
  } catch (failure) {
    if (sequence === homeNoteSourceSequence && homeNotes.value === space) homeNotesError.value = failure instanceof Error ? failure.message : String(failure);
  }
}
watch(() => network.value.epoch, () => { homeNotes.value = null; homeNotePreview.value = null; });
const key = computed(sceneKey);
const appKey = computed(readerKey);
const chatDraft = ref<ChatDraft | null>(readLearningDraft(learningDraftKey()));
let draftStorageKey = learningDraftKey();
let restoringDraft = false;
watch(chatDraft, draft => { if (!restoringDraft) saveLearningDraft(draftStorageKey, draft); }, { flush: 'sync' });
const allowanceOpen = ref(false), settingsOpen = ref(false), mcpOpen = ref(false);
function showMcp() {
  mcpOpen.value = true;
  const url = new URL(location.href); url.searchParams.set('account', 'mcp'); history.replaceState(history.state, '', url);
}
function closeMcp() {
  mcpOpen.value = false; mcpRequested = false;
  const url = new URL(location.href); if (url.searchParams.get('account') === 'mcp') url.searchParams.delete('account');
  history.replaceState(history.state, '', url);
}
function sourceConsumed() {
  pendingSource = null; sourceTarget.value = null; sourceError.value = '';
  const url = new URL(location.href); sourceParams.forEach(key => url.searchParams.delete(key));
  history.replaceState(history.state, '', url);
}
function sourceFailed(message: string) { sourceTarget.value = null; sourceError.value = message; }
async function openPendingSource() {
  if (!pendingSource || sourceTarget.value || linkedRequested) return;
  const source = pendingSource;
  if (readerApp.value && !await readerApp.value.beforeNoteLeave(uiText('打开来源'))) return;
  busy.value = true; sourceError.value = '';
  try {
    if (network.value.workspace) await api.openBook(JSON.stringify(source.published_book_ref));
    else installWorkspace(await networkFetch<NetworkWorkspace>('POST', '/workspaces', {
      attachment_id: network.value.attachment, published_book_ref: source.published_book_ref,
    }));
    sourceTarget.value = source; libraryOpen.value = false; suspended.value = false;
    saveWorkspace(network.value.workspace!.workspace_id, 'reader');
  } catch (failure) {
    if (failure instanceof ApiError && failure.errorCode === 'CLIENT_CONTEXT_STALE') return;
    sourceError.value = failure instanceof ApiError && [403, 404].includes(failure.status)
      ? uiText('该来源材料未获授权或该版本已不可用，请联系管理员开通，或选择其他材料。')
      : uiText('来源暂时无法打开，请重试。');
  } finally { busy.value = false; }
}
async function openAccountDestination() {
  if (mcpRequested) { showMcp(); mcpRequested = false; }
  if (pendingSource && !sourceError.value) await openPendingSource();
  if (pendingBoard?.publication && !linkedRequested && !busy.value) {
    const target = pendingBoard;
    await runBookOpening({ kind: 'book', reference: target.publication!, learningId: target.learningWorkspaceId, title: uiText('画板') });
    if (!openingError.value) pendingBoard = null;
  }
}
watch([() => network.value.epoch, () => network.value.identity?.user_id, () => network.value.identity?.csrf_token], () => { allowanceOpen.value = false; settingsOpen.value = false; mcpOpen.value = false; sourceTarget.value = null; }, { flush: 'sync' });
// A transport generation changes on reattachment; the user's unsent question does not.
watch(() => {
  const n = network.value, w = n.workspace;
  return JSON.stringify([n.epoch, n.identity?.user_id, w?.workspace_id, w?.learning_workspace_id, w?.published_book_ref, w?.selected_chat]);
}, () => {
  draftStorageKey = learningDraftKey(); restoringDraft = true;
  chatDraft.value = readLearningDraft(draftStorageKey); restoringDraft = false;
}, { flush: 'sync' });
const recapTarget = ref<RecapTarget | null>(null), openingRecap = ref(false);
async function openRecapPublication(target: RecapTarget) {
  const publication = targetPublication(target);
  if (!publication) return;
  if (readerApp.value && !await readerApp.value.beforeNoteLeave(uiText('切换发布'))) return;
  openingRecap.value = true; error.value = '';
  try {
    await api.openBook(JSON.stringify(publication));
    await api.agentHistorySelect(target.session_id);
    recapTarget.value = target;
  } catch (failure) { showFailure(failure); }
  finally { openingRecap.value = false; }
}
const linkedRequested = new URLSearchParams(location.search).get('linked') === '1';
const linkedPresentation = ref<{ session_id: string; turn_id: string; reference: import('@understand-book/web/reader/generated/PresentationRef').PresentationRef } | null>(null);
const linkedSource = ref<import('@understand-book/web/reader/api').SourcePopupView | null>(null);
watch(key, () => { linkedSource.value = null; invalidateLinkedWindows(); });
async function showLinkedSource(id: string) {
  const turn = linkedPresentation.value?.turn_id, scope = sceneKey();
  if (!turn) return;
  try { const result = await api.agentSourceResolve(turn, id); if (scope === sceneKey()) linkedSource.value = result; }
  catch (failure) { if (scope === sceneKey()) showFailure(failure); }
}
async function openLinkedSource() {
  const turn = linkedPresentation.value?.turn_id, source = linkedSource.value, scope = sceneKey();
  if (!turn || !source) return;
  try { await api.agentSourceOpen(turn, source.source_ref_id); if (scope === sceneKey()) {
    error.value = uiText('来源已定位，请回原阅读窗口查看');
    window.opener?.postMessage({ kind: 'reader-linked-reader-changed' }, location.origin); window.opener?.focus();
  } }
  catch (failure) { if (scope === sceneKey()) showFailure(failure); }
}
const suspended = ref(false);
async function receiveLinked(event: MessageEvent) {
  if (!linkedRequested && isLinkedReaderChange(event)) { window.dispatchEvent(new Event('linked-reader-changed')); return; }
  if (!linkedRequested || event.source !== window.opener || event.origin !== location.origin) return;
  if (event.data?.kind === 'reader-linked-expired') {
    pageHide(); suspended.value = false; return;
  }
  if (event.data?.kind !== 'reader-linked-context') return;
  if (event.data.user !== network.value.identity?.user_id) { error.value = uiText('附属窗口账号已改变，请从原窗口重新打开'); return; }
  installWorkspace(event.data.workspace, event.data.attachment, true);
  linkedPresentation.value = event.data;
}
async function followUp(message: string, receipt: import('@understand-book/web/reader/generated/PresentationFollowUp').PresentationFollowUp) {
  const scope = sceneKey();
  try { await api.agentRunCreate(message, { presentation_follow_up: receipt }); if (scope === sceneKey()) error.value = uiText('问题已提交到原对话，可回原窗口查看回答'); }
  catch (failure) { if (scope === sceneKey()) showFailure(failure); }
}
function pageHide() {
  clearRecoveryRetry();
  invalidateLinkedWindows();
  suspended.value = true;
  const n = network.value, w = n.workspace;
  if (!w || !n.identity) return;
  void fetch(`/api/workspaces/${w.workspace_id}/detach`, { method: 'POST', keepalive: true, credentials: 'same-origin',
    headers: { 'Content-Type': 'application/json', 'X-CSRF-Token': n.identity.csrf_token },
    body: JSON.stringify({ attachment_id: n.attachment, generation: w.generation, expected_revision: w.revision }),
  }).catch(() => {});
  if (n.linked) { network.value = { ...n, workspace: null, epoch: n.epoch + 1 }; linkedPresentation.value = null; }
}
const pendingItems = ref<PendingSubmission[]>([]);
watch(() => network.value.identity?.user_id, () => {
  videos.value = []; selectedVideo.value = null;
  pendingItems.value = []; linkedPresentation.value = null; books.value = []; learningSpaces.value = [];
  selectedLearningId.value = savedLearningSelection(); sharedOpen.value = false;
}, { flush: 'sync' });
let identityCheck: Promise<void> | null = null;
let recoveryRetryTimer: ReturnType<typeof setTimeout> | undefined;
let recoveryRetryDelay = 1000;
let disposed = false;
function clearRecoveryRetry() { clearTimeout(recoveryRetryTimer); recoveryRetryTimer = undefined; }
function scheduleRecoveryRetry() {
  if (disposed || recoveryRetryTimer !== undefined || document.visibilityState === 'hidden' || navigator.onLine === false
    || !network.value.identity || !network.value.workspace || network.value.linked || accountView.value || opening.value) return;
  const owner = sceneKey();
  recoveryRetryTimer = setTimeout(() => {
    recoveryRetryTimer = undefined;
    if (!disposed && owner === sceneKey() && document.visibilityState !== 'hidden' && navigator.onLine !== false) void checkIdentity();
  }, recoveryRetryDelay);
  recoveryRetryDelay = Math.min(recoveryRetryDelay * 2, 30_000);
}
watch(() => network.value.identity?.user_id, () => { clearRecoveryRetry(); recoveryRetryDelay = 1000; learningTasksRead++; learningTasks.value = []; learningTasksError.value = ''; });
let restoreOnLoad = true;
const channel = typeof BroadcastChannel === 'undefined' ? null : new BroadcastChannel('understand-book:authentication');
network.value = { ...network.value, enabled: true };
async function catalog() {
  books.value = (await networkFetch<{ books: typeof books.value }>('GET', '/library')).books;
  videos.value = (await networkFetch<{ videos: PreparingVideo[] }>('GET', '/videos')).videos;
  await loadLearningSpaces();
  await loadLearningTasks();
}
async function restore() {
  await recoverWorkspaceBinding();
  await recover();
}
async function recover() {
  try { await recoverSubmissions(); }
  catch (failure) { showFailure(failure); }
  finally { pendingChanged(); }
}
async function retryOriginal(key: string) {
  try { await retrySubmission(key); pendingChanged(); }
  catch (failure) { showFailure(failure); }
}
async function checkIdentity() {
  if (accountView.value) { checking.value = false; return; }
  if (identityCheck) return identityCheck;
  identityCheck = (async () => {
    try {
      error.value = '';
      const identity = await networkFetch<NetworkIdentity>('GET', '/auth/me');
      const sameIdentity = identity.user_id === network.value.identity?.user_id && identity.csrf_token === network.value.identity?.csrf_token;
      if (!network.value.identity) { installIdentity(identity); await catalog(); if (linkedRequested) window.opener?.postMessage({ kind: 'reader-linked-ready' }, location.origin); }
      else if (identity.user_id !== network.value.identity.user_id || identity.csrf_token !== network.value.identity.csrf_token) {
        forgetNetwork(); books.value = []; installIdentity(identity); await catalog();
      }
      else { network.value = { ...network.value, identity }; }
      if (!linkedRequested) {
        if (libraryOpen.value || !network.value.workspace) {
          if (sameIdentity) await catalog();
          checking.value = false;
          const initial = restoreOnLoad; restoreOnLoad = false;
          if (initial) await restoreLastVisit(); else await loadResumption();
          await recover();
        } else await restore();
      }
      suspended.value = false;
      if (network.value.workspace && network.value.linked) {
        const current = await networkFetch<NetworkWorkspace>('GET', `/workspaces/${network.value.workspace.workspace_id}`);
        installWorkspace(current); await loadLearningSpaces(); await recover();
      }
      window.dispatchEvent(new Event('workspace-recovered'));
      await openAccountDestination();
    } catch (failure) {
      if (!disposed && network.value.identity) { showFailure(failure); requestWorkspaceRecovery(failure); }
    } finally { checking.value = false; identityCheck = null; }
  })();
  return identityCheck;
}
async function login() {
  busy.value = true; error.value = ''; books.value = [];
  forgetNetwork();
  try {
    await networkFetch('POST', '/auth/login', { username: username.value, password: password.value });
    password.value = '';
    installIdentity(await networkFetch<NetworkIdentity>('GET', '/auth/me'));
    channel?.postMessage('changed'); await catalog();
    busy.value = false;
    restoreOnLoad = false; await restoreLastVisit();
    await openAccountDestination();
  } catch { error.value = uiText('登录失败，请核对账号、密码或稍后重试。'); }
  finally { password.value = ''; busy.value = false; checking.value = false; }
}
async function logout() {
  if (homeNotesRef.value && !await homeNotesRef.value.beforeLeave()) return;
  if (network.value.identity?.is_guest && !window.confirm(uiText('退出后，这枚内测码无法再次使用，也无法自行恢复游客记录。确定退出体验吗？'))) return;
  if (readerApp.value && !await readerApp.value.beforeNoteLeave(uiText('退出登录'))) return;
  const n = network.value, w = n.workspace;
  const detach = w ? networkFetch('POST', `/workspaces/${w.workspace_id}/detach`, { attachment_id: n.attachment, generation: w.generation, expected_revision: w.revision }).catch(() => {}) : Promise.resolve();
  forgetNetwork(); books.value = []; pendingItems.value = []; error.value = '';
  busy.value = true;
  try {
    // Clear private UI immediately, but detach before revoking its Cookie.
    await detach;
    const response = await fetch('/api/auth/logout', { method: 'POST', credentials: 'same-origin',
      headers: { 'Content-Type': 'application/json', 'X-CSRF-Token': n.identity!.csrf_token }, body: '{}' });
    if (!response.ok) throw new Error('logout');
  } catch {
    error.value = uiText('退出请求未确认，请核对网络后重新登录。');
  } finally { busy.value = false; }
  channel?.postMessage('changed');
}
watch(pendingNoteSource, async target => {
  if (!target) return;
  if (!target.book_id || !target.publication_id) {
    noteSourceError.value = '这条记录没有可打开的资料版本，原话和保留摘录仍可阅读。'; pendingNoteSource.value = null; return;
  }
  if (busy.value) { noteSourceError.value = '正在打开另一份资料，请稍后再返回记录现场。'; pendingNoteSource.value = null; return; }
  if (target.board_id && target.workspace_id && target.workspace_id === network.value.workspace?.learning_workspace_id && !libraryOpen.value && !suspended.value) return;
  if (!libraryOpen.value && !suspended.value && network.value.workspace?.published_book_ref.book_id === target.book_id && network.value.workspace?.published_book_ref.publication_id === target.publication_id && (network.value.workspace?.learning_workspace_id ?? null) === target.workspace_id) return;
  await openBook({ book_id: target.book_id, publication_id: target.publication_id }, target.workspace_id);
  if (pendingNoteSource.value === target && (openingError.value || network.value.workspace?.published_book_ref.publication_id !== target.publication_id)) {
    noteSourceError.value = `原资料暂时无法打开：${openingError.value || '尚未切换到原资料，请重试。'}`; pendingNoteSource.value = null;
  }
});
async function openBookCourse(target: CourseLaunchTarget, learningId: string | null = selectedLearningId.value) {
  await runBookOpening({ kind: 'book', reference: target.publication, learningId,
    title: materialTitle(target.publication), course: { lessonId: target.lessonId, runId: target.runId, chatSessionId: target.chatSessionId } });
}
async function openCourseUnderstanding(reference: PublishedBookRef) {
  await openBook(reference, selectedLearningId.value);
  if (!openingError.value && network.value.workspace?.published_book_ref.publication_id === reference.publication_id) {
    await nextTick(); readerApp.value?.openTutorPanel('understanding');
  }
}
async function openBook(reference: PublishedBookRef, learningId: string | null = selectedLearningId.value, members?: PublishedBookRef[]) {
  if (busy.value) return;
  if (materialFor(reference)?.media_kind === 'video') void preloadVideoPlayback().catch(() => {});
  await runBookOpening({ kind: 'book', reference, learningId, members,
    title: members ? learningSpaces.value.find(space => space.learning_workspace_id === learningId)?.goal.text ?? materialTitle(reference) : materialTitle(reference) });
}
async function runBookOpening(request: BookOpening) {
  if (busy.value) return;
  busy.value = true; error.value = ''; openingError.value = '';
  if (readerApp.value) {
    try { if (!await readerApp.value.beforeNoteLeave(uiText('打开资料'))) { busy.value = false; return; } }
    catch (failure) { busy.value = false; showFailure(failure); return; }
  }
  const controller = new AbortController();
  openingController = controller; openingRequest.value = request; opening.value = true;
  const timeout = setTimeout(() => controller.abort(new DOMException(uiText('打开资料超时，请检查连接后重试。'), 'TimeoutError')), 15_000);
  try {
    if (request.kind === 'resume') await recoverWorkspaceBinding(request.record, controller.signal);
    else await openLibraryMaterial(request.reference, request.learningId, controller.signal, request.members);
    controller.signal.throwIfAborted();
    selectedLearningId.value = network.value.workspace?.learning_workspace_id ?? null; saveLearningSelection(selectedLearningId.value);
    libraryOpen.value = false; suspended.value = false;
    saveWorkspace(network.value.workspace!.workspace_id, 'reader');
    const url = new URL(location.href); url.searchParams.delete('view'); history.replaceState(history.state, '', url);
    await nextTick();
    if (request.kind === 'book' && request.board) {
      if (!request.board.id) request.board.id = (await api.boardCreate(request.board.title)).board_id;
      await readerApp.value?.openBoardLocation({ board_id: request.board.id });
    }
    if (request.kind === 'book' && (request.course || request.task)) {
      if (!readerApp.value) throw new Error(uiText('学习现场尚未准备好，请重试。'));
      if (request.task) await abortable(readerApp.value.resumeLearningTask(request.task,controller.signal), controller.signal);
      else await abortable(readerApp.value.openCourse(request.course,controller.signal), controller.signal);
    }
    if (request.kind === 'book' && request.reading) {
      if (!readerApp.value) throw new Error(uiText('学习现场尚未准备好，请重试。'));
      await abortable(readerApp.value.openReading(controller.signal), controller.signal);
    }
    openingRequest.value = null;
    void recover();
  } catch (failure) {
    if (controller.signal.aborted && controller.signal.reason?.name === 'AbortError') return;
    if (failure instanceof ApiError && failure.errorCode === 'CLIENT_CONTEXT_STALE') { openingRequest.value = null; return; }
    openingError.value = controller.signal.aborted ? controller.signal.reason.message
      : failure instanceof ApiError && failure.errorCode === 'WORKSPACE_CAPACITY'
        ? uiText('当前阅读窗口已达到上限，请稍后重试。学习记录已保留。')
        : failure instanceof Error ? failure.message : String(failure);
    if (request.kind === 'resume') void loadResumption();
  } finally {
    clearTimeout(timeout);
    if (openingController === controller) { openingController = null; opening.value = false; busy.value = false; }
  }
}
function cancelBookOpening() {
  openingController?.abort(new DOMException(uiText('已取消打开'), 'AbortError'));
  openingRequest.value = null; openingError.value = '';
}
function retryBookOpening() {
  const request = openingRequest.value;
  if (!request || busy.value) return;
  if (request.kind === 'resume' && resumption.value) void continueReading();
  else void runBookOpening(request);
}
function pendingChanged() { pendingItems.value = unconfirmedSubmissions(); }
function onVisible() {
  clearRecoveryRetry();
  if (document.visibilityState !== 'hidden' && !opening.value) { recoveryRetryDelay = 1000; void checkIdentity(); }
}
function onLearningGoalChanged(event: Event) {
  if ((event as CustomEvent<{ learning_workspace_id: string }>).detail?.learning_workspace_id === network.value.workspace?.learning_workspace_id) void loadLearningSpaces();
}
async function onAccountChange(event: MessageEvent) {
  if (event.data === 'profile-changed') { void checkIdentity(); return; }
  const previous = identityCheck;
  forgetNetwork(); books.value = []; pendingItems.value = [];
  if (previous) await previous;
  void checkIdentity();
}
onMounted(() => {
  void checkIdentity();
  window.addEventListener('pagehide', pageHide);
  window.addEventListener('workspace-recovery-needed', scheduleRecoveryRetry);
  window.addEventListener('message', receiveLinked);
  window.addEventListener('submission-pending-changed', pendingChanged);
  window.addEventListener('learning-workspace-goal-changed', onLearningGoalChanged);
  channel?.addEventListener('message', onAccountChange);
  document.addEventListener('visibilitychange', onVisible);
  window.addEventListener('pageshow', onVisible); window.addEventListener('online', onVisible); window.addEventListener('focus', onVisible);
});
onBeforeUnmount(() => {
  disposed = true; clearRecoveryRetry();
  cancelBookOpening();
  window.removeEventListener('pagehide', pageHide);
  window.removeEventListener('workspace-recovery-needed', scheduleRecoveryRetry);
  window.removeEventListener('message', receiveLinked);
  window.removeEventListener('submission-pending-changed', pendingChanged);
  window.removeEventListener('learning-workspace-goal-changed', onLearningGoalChanged);
  channel?.close(); document.removeEventListener('visibilitychange', onVisible);
  window.removeEventListener('pageshow', onVisible); window.removeEventListener('online', onVisible); window.removeEventListener('focus', onVisible);
});
const readerHeader = computed(() => !!network.value.workspace && !network.value.linked && !linkedRequested
  && !suspended.value && !openingRecap.value && !selectedVideo.value && !libraryOpen.value && !readerBoardOpen.value);
const productTourContext = computed<ProductTourContext | null>(() => {
  if (checking.value || busy.value || opening.value || !network.value.identity || network.value.linked || linkedRequested
    || selectedVideo.value || settingsOpen.value || allowanceOpen.value || accountView.value || suspended.value) return null;
  if (libraryOpen.value || !network.value.workspace) return boardsHome.value ? 'boards' : 'library';
  return readerApp.value?.productTourReady ? readerBoardOpen.value ? 'board' : 'reader' : null;
});
async function prepareProductTour(step: ProductTourStep) {
  if (productTourContext.value === 'reader' || productTourContext.value === 'board') return await readerApp.value?.prepareProductTour(step) ?? false;
  return true;
}
</script>
<template>
  <DesktopAuthorization />
  <AccountAccess v-if="accountView" :key="accountView" :view="accountView" :token="resetToken" @login="accountLogin" @reset="passwordChanged" @forgot="showAccount('forgot-password')" />
  <main v-else-if="checking && !network.workspace" class="network-loading"><LoadingAnimation /></main>
  <main v-else-if="!network.identity" class="network-login">
    <header class="login-heading"><a class="brand-signature" href="/home.html"><img :src="brandLogo" class="brand-logo" width="36" height="36" alt="" /><span>Understand Book</span></a><LanguageSwitcher /></header>
    <h1>{{ uiText('回到你的阅读') }}</h1>
    <DesktopSignIn v-if="isTauri()" />
    <form v-else @submit.prevent="login">
      <label>{{ uiText('邮箱或账号') }}<input v-model="username" autocomplete="username" required></label>
      <label>{{ uiText('密码') }}<input v-model="password" type="password" autocomplete="current-password" required></label>
      <button :disabled="busy">{{ uiText('登录') }}</button>
    </form>
    <p v-if="!isTauri()"><button :disabled="busy" @click="showAccount('register')">{{ uiText('注册账号') }}</button> <button :disabled="busy" @click="showAccount('forgot-password')">{{ uiText('忘记密码') }}</button></p>
    <p role="status">{{ error }}</p>
  </main>
  <div v-else class="network-shell">
    <header v-if="!network.linked && !linkedRequested && !readerHeader" class="network-page-bar">
      <WorkspaceDestinations :busy="busy" :current="!libraryOpen && readerUnderstandingOpen ? 'understanding' : boardsActive ? 'boards' : libraryOpen ? (coursesHome ? 'courses' : 'materials') : undefined" @courses="showCourses" @materials="showLibrary" @boards="showBoards" @understanding="showUnderstanding" />
      <button v-if="network.workspace" type="button" class="network-current-reader" :disabled="busy" @click="showCurrentReader">{{ materialFor(network.workspace.published_book_ref)?.media_kind === 'video' ? uiText('继续观看') : uiText('继续阅读') }}<ArrowRight :size="15" aria-hidden="true" /></button>
      <ProductTour class="network-tour" :context="productTourContext" :owner="network.identity.user_id" :prepare="prepareProductTour" :open-boards="showBoards" />
    </header>
    <aside v-if="network.identity.is_guest && !network.linked" class="guest-session-notice"><span>{{ uiText('游客体验 · 已赠送 5 元额度，示例材料已开通') }}</span><button @click="allowanceOpen = true">{{ uiText('查看剩余额度') }}</button></aside>
    <details v-if="network.linked" class="network-account network-linked-account">
      <summary>{{ uiText('账号') }}</summary>
      <div class="network-account-menu">
        <span>{{ network.identity.is_guest ? uiText('游客身份') : network.identity.email || network.identity.user_id }}</span>
        <button @click="settingsOpen = true">{{ uiText('个人设置') }}</button> <button @click="showMcp">{{ uiText('MCP 接入') }}</button> <button @click="allowanceOpen = true">{{ uiText('使用额度') }}</button>
        <a v-if="network.identity.capabilities?.admin" href="/admin/" target="_blank" rel="noopener">{{ uiText('运营后台') }}</a>
        <button @click="logout">{{ uiText('退出登录') }}</button>
      </div>
    </details>
    <div v-if="pendingItems.length || error || sourceError" class="network-notices">
      <span v-if="sourceError" role="alert">{{ sourceError }} <button v-if="pendingSource" :disabled="busy" @click="openPendingSource">{{ uiText('重试打开来源') }}</button></span>
      <span v-if="pendingItems.length" role="status">{{ pendingItems.length }} {{ uiText('个问题提交待核对') }} <button @click="recover">{{ uiText('核对原问题') }}</button></span>
      <span v-for="item in pendingItems" :key="item.key">{{ item.body.display_user || item.body.message }} <button @click="retryOriginal(item.key)">{{ uiText('重试原提交') }}</button></span>
      <span v-if="error" role="alert">{{ error }}</span>
    </div>
    <AgentPresentation v-if="!suspended && network.linked && linkedPresentation && network.workspace" :key="key" :session-id="linkedPresentation.session_id" :turn-id="linkedPresentation.turn_id" :reference="linkedPresentation.reference" @follow-up="followUp" @source="showLinkedSource" />
    <VideoReaderPane v-else-if="!suspended && selectedVideo" :key="`${selectedVideo.video.media_id}/${selectedVideo.video.media_revision}`" :initial="selectedVideo" :owner="network.identity.user_id" @close="selectedVideo = null" @ready="reference => { selectedVideo = null; openBook(reference); }" />
    <main v-else-if="openingRecap || (suspended && network.workspace && !network.linked)" class="network-loading"><LoadingAnimation /></main>
    <App :learning-workspace="learningSpaces.find(space => space.learning_workspace_id === network.workspace?.learning_workspace_id) ?? null" ref="readerApp" v-if="!suspended && !network.linked && !openingRecap && network.workspace" v-show="!selectedVideo && !libraryOpen" :reader-visible="!selectedVideo && !libraryOpen" :host-opening="opening" :material-title="materialTitle(network.workspace.published_book_ref)" :key="appKey" v-model:chat-draft="chatDraft" :recap-target="recapTarget" :mcp-source="sourceTarget" @mcp-source-consumed="sourceConsumed" @mcp-source-error="sourceFailed" @show-library="showLibrary" @learning-collection="showLearningCollection" @course-launch="openBookCourse" @show-allowance="allowanceOpen = true" @recap-consumed="recapTarget = null" @recap-publication="openRecapPublication" @board-visibility="readerBoardOpen = $event" @understanding-visibility="readerUnderstandingOpen = $event">
      <template v-if="readerHeader" #navigation>
        <WorkspaceDestinations :busy="busy" :current="readerUnderstandingOpen ? 'understanding' : undefined" @courses="showCourses" @understanding="showUnderstanding" @materials="network.workspace?.learning_workspace_id ? manageCurrentLearning() : showLibrary()" @boards="showBoards" />
      </template>
      <template v-if="readerHeader" #help>
        <ProductTour :context="productTourContext" :owner="network.identity.user_id" :prepare="prepareProductTour" :open-boards="showBoards" />
      </template>
      <template #account>
        <details class="network-account">
          <summary><UserRound aria-hidden="true" />{{ uiText('账号') }}<ChevronDown class="account-chevron" aria-hidden="true" /></summary>
          <div class="network-account-menu">
            <span>{{ network.identity.is_guest ? uiText('游客身份') : network.identity.email || network.identity.user_id }}</span>
            <button @click="settingsOpen = true">{{ uiText('个人设置') }}</button> <button @click="showMcp">{{ uiText('MCP 接入') }}</button> <button @click="allowanceOpen = true">{{ uiText('使用额度') }}</button>
            <a v-if="network.identity.capabilities?.admin" href="/admin/" target="_blank" rel="noopener">{{ uiText('运营后台') }}</a>
            <span v-if="network.identity.capabilities?.presentation.authoring === false">{{ uiText('演示制作暂不可用') }}</span>
            <button @click="logout">{{ uiText('退出登录') }}</button>
          </div>
        </details>
      </template>
    </App>
    <p v-if="linkedRequested && !linkedPresentation" role="status">{{ uiText('请保持原阅读窗口打开；附属连接失效后请从原窗口重新打开。') }}</p>
    <BoardLibrary v-if="!selectedVideo && !linkedRequested && (libraryOpen || !network.workspace) && boardsHome" :key="network.identity.user_id" :materials="materialBooks" :workspaces="learningSpaces" :busy="busy" @open="openLibraryBoard" @create="createLibraryBoard" @materials="showLibrary" />
    <main v-if="!selectedVideo && !linkedRequested && (libraryOpen || !network.workspace) && !boardsHome" class="network-library" :class="{ 'is-opening': opening, 'collection-open': coursesHome && collectionTarget }" :aria-busy="opening">
      <header class="network-library-head" data-tour="learning-purpose">
        <div><p class="network-library-kicker brand-signature"><img :src="brandLogo" class="brand-logo" width="24" height="24" alt="" />UNDERSTAND BOOK</p><h1 v-if="!coursesHome || !collectionTarget">{{ uiText(coursesHome ? '今天，从这里开始' : '学习资料') }}<span>{{ uiText('把资料变成自己的理解。') }}</span></h1></div>
        <div class="library-head-actions">
          <LanguageSwitcher />
          <button class="create-workspace-toggle" :aria-expanded="createOpen" @click="createOpen = !createOpen"><Plus :size="16" />{{ uiText('新建工作区') }}</button>
          <details class="network-account"><summary><span class="account-avatar">{{ (network.identity.is_guest ? uiText('游客身份') : network.identity.email || network.identity.user_id).slice(0, 1).toUpperCase() }}</span><span class="account-email">{{ network.identity.is_guest ? uiText('游客身份') : network.identity.email || network.identity.user_id }}</span><ChevronDown :size="14" /></summary>
            <div class="network-account-menu"><button @click="settingsOpen = true">{{ uiText('个人设置') }}</button><button @click="showMcp">{{ uiText('MCP 接入') }}</button><button @click="allowanceOpen = true">{{ uiText('使用额度') }}</button><a v-if="network.identity.capabilities?.admin" href="/admin/" target="_blank" rel="noopener">{{ uiText('运营后台') }}</a><button @click="logout">{{ uiText('退出登录') }}</button></div>
          </details>
        </div>
      </header>
      <LearningHomeTask v-if="coursesHome && !collectionTarget" :tasks="learningTasks" :books="materialBooks" :loading="learningTasksLoading" :error="learningTasksError" :busy="busy" @resume="continueLearningTask" @collection="showLearningCollection" @courses="scrollCourseCatalog" @materials="useOwnMaterials" @retry="loadLearningTasks" />
      <form v-show="createOpen" class="learning-create" @submit.prevent="createLearning">
        <label>{{ uiText('这次想学会什么？') }}<input v-model="createGoal" :placeholder="uiText('比如：学习深度学习，或期末考试冲刺')" required :disabled="busy" /></label>
        <button :disabled="busy || !createGoal.trim()">{{ uiText('创建工作区') }}<ArrowRight :size="16" /></button>
      </form>
      <div class="learning-layout" :class="{ 'has-selection': selectedLearning, 'is-courses': coursesHome }">
      <section v-show="!coursesHome" class="learning-spaces" :aria-label="uiText('学习工作区')" :aria-busy="learningLoading">
        <div class="learning-nav-heading"><span>{{ uiText('我的工作区') }}</span><span>{{ learningSpaces.length.toString().padStart(2, '0') }}</span></div>
        <p v-if="learningError" role="alert">{{ uiText('工作区暂时无法读取：') }}{{ learningError }} <button @click="loadLearningSpaces">{{ uiText('重试工作区') }}</button></p>
        <div class="learning-space-list">
          <article v-for="space in learningSpaces" :key="space.learning_workspace_id" class="learning-space-card" :class="{ selected: selectedLearningId === space.learning_workspace_id }">
            <button class="learning-space-select" :aria-pressed="selectedLearningId === space.learning_workspace_id" :disabled="busy" @click="selectLearning(space)">
              <span class="workspace-cover-stack" aria-hidden="true"><span v-for="member in space.members.slice(0, 3)" :key="JSON.stringify(member.published_book_ref)" class="workspace-mini-cover"><MaterialCover :paused="opening" :title="materialTitle(member.published_book_ref)" :cover="materialFor(member.published_book_ref)?.cover" :video="materialFor(member.published_book_ref)?.media_kind === 'video'" :music="materialFor(member.published_book_ref)?.media_kind === 'music'" compact /></span><BookOpen v-if="!space.members.length" :size="28" /></span>
              <span class="workspace-nav-copy"><strong>{{ space.goal.text }}</strong><small>{{ space.members.length }} {{ uiText('份资料') }}{{ space.archived ? uiText(' · 已归档') : '' }}</small></span>
            </button>
            <button class="workspace-enter" :disabled="busy || space.archived" @click="enterLearning(space)">{{ uiText('打开工作区') }}<ArrowUpRight :size="14" /></button>
          </article>
        </div>
        <button v-if="selectedLearning" class="learning-legacy" :disabled="busy" @click="selectLearning(null)"><Layers :size="16" />{{ uiText('全部资料') }}</button>
        <div class="shared-entry"><button @click="showCourses"><BookOpen :size="16" />{{ uiText('浏览课程') }}<ArrowUpRight :size="14" /></button><p>{{ uiText('从准备好的课程与学习路线开始。') }}</p></div>
      </section>
      <div class="learning-content">
      <SharedWorkspaceCatalog v-if="coursesHome || sharedOpen" v-model:target="collectionTarget" :books="materialBooks" :workspaces="learningSpaces" :learning-tasks="learningTasks" @reading="readCollectionBook" @started="startSharedLearning" />
      <section v-if="selectedLearning && !coursesHome" class="learning-details" :aria-label="uiText('当前工作区')">
        <header class="learning-goal-heading"><div><span class="network-library-kicker">{{ uiText('当前学习目标') }} <span class="goal-revision">{{ uiText('· 第') }} {{ selectedLearning.goal.revision }} {{ uiText('版') }}</span></span><h2>{{ selectedLearning.goal.text }}</h2><p v-if="selectedLearning.goal.constraints.length" class="goal-constraints">{{ selectedLearning.goal.constraints.join('；') }}</p></div>
          <div class="goal-actions"><button class="workspace-primary" :disabled="busy || selectedLearning.archived || !memberBooks.some(member => member.readable)" @click="enterLearning(selectedLearning)">{{ uiText('继续学习') }}<ArrowRight :size="16" /></button><button class="goal-edit-button" :disabled="busy" @click="openHomeNotes(selectedLearning)">{{ uiText('空间笔记') }}</button><button class="goal-edit-button" :disabled="busy || selectedLearning.archived" @click="goalDraft = selectedLearning.goal.text; editingGoal = true">{{ uiText('修改大目标') }}</button></div></header>
        <form v-if="editingGoal" class="learning-goal-edit" @submit.prevent="reviseGoal">
          <label>{{ uiText('工作区大目标') }}<textarea v-model="goalDraft" required :disabled="busy" /></label>
          <button :disabled="busy || !goalDraft.trim()">{{ uiText('保存大目标') }}</button><button type="button" :disabled="busy" @click="editingGoal = false">{{ uiText('取消修改') }}</button>
        </form>
        <UnderstandingEntry :key="selectedLearning.learning_workspace_id" :workspace-id="selectedLearning.learning_workspace_id" :disabled="busy || !memberBooks.some(member=>member.readable)" @open="reference=>reference?openCourseUnderstanding(reference):showUnderstanding()"/>
        <SharedWorkspaceInstance :workspace="selectedLearning" :books="materialBooks" @updated="updateSharedLearning" @understanding="openCourseUnderstanding" @launch="openBookCourse" />
        <div class="materials-heading"><div><h3>{{ uiText('工作区资料') }} <span>{{ memberBooks.length }}</span></h3><p>{{ uiText('从一份资料开始，带着问题继续探索。') }}</p></div><button :disabled="busy || selectedLearning.archived" :aria-expanded="materialsOpen" @click="materialsOpen = !materialsOpen"><Plus :size="16" />{{ uiText('添加资料') }}</button></div>
        <p v-if="!memberBooks.length" class="learning-empty"><BookOpen :size="32" />{{ uiText('工作区已创建。加入资料后，即可围绕这个目标开始阅读与讨论。') }}</p>
        <div class="learning-members" :class="{ 'few-materials': memberBooks.length <= 2 }">
          <article v-for="member in memberBooks" :key="JSON.stringify(member.published_book_ref)" class="learning-member" :class="{ 'is-video': member.book?.media_kind === 'video' }">
            <button class="member-open" :disabled="busy || !member.readable || selectedLearning.archived" @click="openBook(member.published_book_ref)">
              <span class="member-cover"><MaterialCover :paused="opening" :title="materialTitle(member.published_book_ref)" :cover="member.book?.cover" :video="member.book?.media_kind === 'video'" :music="member.book?.media_kind === 'music'" /></span>
              <span class="member-copy"><span class="member-kind">{{ member.book?.media_kind === 'music' ? uiText('音乐伴听') : member.book?.media_kind === 'video' ? uiText('课程视频') : uiText('阅读资料') }}</span><strong>{{ materialTitle(member.published_book_ref) }}</strong><span v-if="!member.readable" class="member-unavailable">{{ uiText('资料暂不可读') }}</span><span class="member-read-action">{{ network.workspace?.learning_workspace_id === selectedLearningId && samePublication(network.workspace.published_book_ref, member.published_book_ref) ? uiText('继续阅读') : uiText('开始阅读') }}<ArrowUpRight :size="16" /></span></span>
            </button>
            <details class="member-menu"><summary :aria-label="uiText('管理{0}', [materialTitle(member.published_book_ref)])"><Settings2 :size="15" /></summary><button :disabled="busy || selectedLearning.archived" @click="changeMember(member.published_book_ref, false)">{{ uiText('移出资料') }}</button></details>
          </article>
        </div>
        <div v-show="materialsOpen || !memberBooks.length" class="materials-add-panel">
        <PersonalMaterials :key="`${network.identity.user_id}:${selectedLearning.learning_workspace_id}`" :workspace="selectedLearning" @ready="catalog" @open="reference => openBook(reference, selectedLearning!.learning_workspace_id)" @allowance="allowanceOpen = true" />
        <details v-if="!selectedLearning.archived" class="learning-add" open>
          <summary>{{ uiText('加入资料') }}</summary>
          <div v-for="book in availableBooks" :key="JSON.stringify(book.published_book_ref)" class="learning-add-book"><span class="add-book-cover"><MaterialCover :paused="opening" :title="bookLabel(book)" :cover="book.cover" :video="book.media_kind === 'video'" :music="book.media_kind === 'music'" compact /></span><span>{{ bookLabel(book) }}</span><button :disabled="busy" @click="changeMember(book.published_book_ref, true)">{{ uiText('加入工作区') }}</button></div>
          <p v-if="!availableBooks.length">{{ uiText('当前可用资料均已加入。') }}</p>
        </details>
        </div>
        <ExamSprintPanel :workspace="selectedLearning" @updated="updateLearning" @begin="beginExamLearning" @practice="beginExamPractice" />
        <details class="workspace-settings"><summary><Settings2 :size="16" />{{ uiText('工作区管理与学习记录') }}<ChevronDown :size="14" /></summary><div class="workspace-settings-body">
        <button class="learning-records-toggle" :disabled="busy" :aria-expanded="recordsOpen" @click="recordsOpen = !recordsOpen">{{ recordsOpen ? uiText('收起已有学习记录') : uiText('带入已有学习记录') }}</button>
        <LearningRecordsPanel v-if="recordsOpen" :target="selectedLearning" :key="selectedLearning.learning_workspace_id" @refresh-goal="loadLearningSpaces" />
        <div class="learning-archive"><p>{{ selectedLearning.archived ? uiText('工作区已归档，学习记录与来源保留。恢复后可继续学习。') : uiText('归档后暂停主动教学和新增后台整理，学习记录与来源保留。') }}</p><button :disabled="busy" @click="archiveLearning">{{ selectedLearning.archived ? uiText('恢复工作区') : uiText('归档工作区') }}</button></div>
        <button class="learning-legacy" :disabled="busy" @click="selectLearning(null)">{{ uiText('返回书架') }}</button>
        </div></details>
      </section>
      <section v-if="!selectedLearning && !coursesHome" class="learning-records-entry">
        <button :aria-expanded="legacyRecordsOpen" @click="legacyRecordsOpen = !legacyRecordsOpen">{{ legacyRecordsOpen ? uiText('收起旧记录') : uiText('旧聊天与学习记录') }}</button>
        <LearningRecordsPanel v-if="legacyRecordsOpen" />
      </section>
      <section v-if="!coursesHome && !selectedLearning && resumption && resumptionBook" class="reading-resumption" :aria-label="uiText('阅读接续')" :aria-busy="resumptionLoading">
        <div class="resumption-cover"><MaterialCover :paused="opening" :title="bookLabel(resumptionBook)" :cover="resumptionBook.cover" :video="resumptionBook.media_kind === 'video'" :music="resumptionBook.media_kind === 'music'" /></div>
        <div class="resumption-content">
          <p class="network-library-kicker">{{ uiText('接着上次读') }}</p>
          <h2>{{ bookLabel(resumptionBook) }}</h2>
          <p class="resumption-position">{{ uiText('已保存位置 ·') }} {{ resumption.position_label || uiText('原阅读位置') }}</p>
          <p v-if="resumption.position_excerpt" class="resumption-excerpt">{{ resumption.position_excerpt }}</p>
          <div v-if="resumption.last_question" class="resumption-question"><span>{{ uiText('上次的问题') }}</span><p>{{ resumption.last_question }}</p></div>
          <button class="resumption-continue" :disabled="busy || resumptionLoading" @click="continueReading">{{ resumptionLoading ? uiText('正在更新…') : uiText('继续阅读') }}</button>
        </div>
        <YeyeMascot class="resumption-mascot" :size="144" :motion="busy || resumptionLoading || resumptionError ? 'static' : 'idle'" />
      </section>
      <p v-if="!coursesHome && resumptionLoading && !resumptionBook" class="resumption-status" role="status">{{ uiText('正在读取接续记录…') }}</p>
      <p v-else-if="!coursesHome && resumptionError" class="resumption-status" role="status">{{ resumptionError }} <button :disabled="busy" @click="showLibrary">{{ uiText('重试') }}</button></p>
      <h2 v-if="!selectedLearning && !coursesHome" class="library-section-title">{{ uiText('全部资料') }}<span>{{ uiText('你的书籍、课程与阅读收藏') }}</span></h2>
      <div v-if="!selectedLearning && !coursesHome" class="network-library-grid">
        <button v-for="video in pendingVideos" :key="video.video.media_id" class="network-book-card" :disabled="busy" @click="video.publication ? openBook(video.publication) : selectedVideo = video"><span class="shelf-cover"><MaterialCover :paused="opening" :title="video.title" video /></span><strong>{{ video.title }}</strong><span>{{ video.english_ready ? uiText('视频 · 英文已发布') : uiText('视频 · 转写准备中') }}</span></button>
        <button v-for="book in materialBooks" :key="book.published_book_ref.publication_id" class="network-book-card" :disabled="busy" @click="openBook(book.published_book_ref)">
          <span class="shelf-cover"><MaterialCover :paused="opening" :title="bookLabel(book)" :cover="book.cover" :video="book.media_kind === 'video'" :music="book.media_kind === 'music'" /></span>
          <strong>{{ bookLabel(book) }}</strong>
          <span>{{ book.media_kind === 'music' ? uiText('音乐伴听') : book.media_kind === 'video' ? uiText('课程视频') : uiText('阅读资料') }}</span>
        </button>
      </div>
      <p v-if="!coursesHome && !books.length && !videos.length">{{ uiText('添加自己的资料，或从准备好的课程开始。') }} <button @click="useOwnMaterials">{{ uiText('添加资料') }}</button></p>
      </div></div>
    </main>
    <Teleport to="body">
      <aside v-if="openingRequest && (opening || openingError)" class="book-opening" :class="{ 'book-opening-failed': !opening }" :role="opening ? 'status' : 'alert'" aria-live="polite" aria-atomic="true">
        <span v-if="opening" class="book-opening-spinner" aria-hidden="true"></span><BookOpen v-else :size="22" aria-hidden="true" />
        <div class="book-opening-copy"><strong>{{ opening ? uiText('正在打开') : uiText('暂时无法打开') }} · {{ openingRequest.title }}</strong><p>{{ opening ? uiText('正在准备阅读内容…') : openingError }}</p></div>
        <div class="book-opening-actions"><button v-if="opening" @click="cancelBookOpening">{{ uiText('取消') }}</button><template v-else><button class="book-opening-retry" @click="retryBookOpening">{{ uiText('重试打开') }}</button><button @click="cancelBookOpening">{{ uiText('返回书架') }}</button></template></div>
      </aside>
    </Teleport>
    <AccountMcp v-if="mcpOpen" @close="closeMcp" />
    <div v-if="homeNotes" class="home-notes-overlay">
      <p v-if="homeNotesError" class="home-notes-notice" role="status">{{ homeNotesError }} <button @click="homeNotesError = ''">{{ uiText('关闭') }}</button></p>
      <NotesWorkspace ref="homeNotesRef" :key="homeNotes.learning_workspace_id" :learning-workspace-id="homeNotes.learning_workspace_id" :readonly="homeNotes.archived"
        @changed="readerApp?.workspaceNotesChanged()"
        @close="closeHomeNotes" @open-board="request => notebookReader(app => app.openBoard({ kind: 'note', note_id: request.noteId }))"
        @ask="request => notebookReader(app => app.prepareWorkspaceNoteQuestion(request))" @source="homeNoteSource" />
    </div>
    <BoardSourcePreview v-if="homeNotePreview" :source="homeNotePreview" :read-window="readHomeNoteSource" context="note" :can-open="homeNoteSourceAvailable && !homeNotePreview.stale" @open="homeNotePreview = null; homeNoteSourceRequest && notebookReader(app => app.navigateWorkspaceNoteSource(homeNoteSourceRequest), homeNoteSourcePublication)" @close="homeNotePreview = null" />
    <AccountSettings v-if="settingsOpen" @close="settingsOpen = false" @bound="channel?.postMessage('profile-changed')" @password-changed="passwordChanged" />
    <AccountAllowance v-if="allowanceOpen" :key="key" @close="allowanceOpen = false" />
    <aside v-if="linkedSource" class="network-source" :aria-label="uiText('演示来源')">
      <strong>{{ linkedSource.heading_path?.join(' / ') || linkedSource.label }}</strong>
      <SourceExcerpt :source="linkedSource" />
      <p v-if="linkedSource.stale">{{ uiText('来源已变化，显示原引用快照。') }}</p>
      <button v-if="linkedSource.can_open_in_reader" @click="openLinkedSource">{{ uiText('在原阅读区打开') }}</button>
      <button @click="linkedSource = null">{{ uiText('关闭来源') }}</button>
    </aside>
  </div>
</template>
<style scoped>
.home-notes-overlay { position: fixed; inset: 0; z-index: 145; display: flex; flex-direction: column; background: var(--reader-canvas); }
.home-notes-overlay > .notes-workspace { flex: 1; min-height: 0; }
.home-notes-notice { margin: 0; padding: 12px 24px; background: var(--surface-soft); color: var(--ink); font-size: 13px; line-height: 1.8; }
.home-notes-notice button { margin-left: 12px; border: 0; background: transparent; color: var(--brand-accent-deep); }
.network-loading { height: 100%; flex: 1; min-height: 0; display: grid; place-items: center; background: var(--reader-canvas); }
.network-login { max-width: 32rem; margin: 12vh auto; padding: 1.5rem; }
.guest-session-notice { display: flex; align-items: center; justify-content: center; gap: 16px; flex-wrap: wrap; padding: 8px 16px; font-size: 12px; color: var(--steel, #6c6a64); background: var(--surface); border-bottom: 1px solid var(--line); }
.guest-session-notice button { border: 0; background: transparent; color: inherit; text-decoration: underline; text-underline-offset: 3px; cursor: pointer; font: inherit; }
.login-heading { display: flex; align-items: center; justify-content: space-between; flex-wrap: wrap; gap: 12px; }
.network-login h1, .network-library-head h1 { font-family: var(--font-display); font-weight: 400; font-synthesis: none; line-height: 1.45; }
.network-library-kicker { margin: 0; color: var(--muted); font-size: 13px; }
.reading-resumption { max-width: 1100px; box-sizing: border-box; margin: 0 auto 40px; padding: clamp(20px, 4vw, 40px); display: flex; gap: clamp(20px, 4vw, 44px); align-items: center; border-radius: 28px; background: var(--surface, #f4f0e8); border: 1px solid var(--line); }
.resumption-cover { width: clamp(90px, 16vw, 155px); flex-shrink: 0; }
.resumption-content { flex: 1; min-width: 0; }
.resumption-content h2 { margin: 8px 0 16px; font-family: var(--serif, Georgia, serif); font-size: clamp(21px, 3vw, 30px); overflow-wrap: anywhere; }
.resumption-position { margin: 0 0 8px; font-size: 13px; color: var(--muted); }
.resumption-excerpt, .resumption-question p { margin: 0; line-height: 1.7; overflow-wrap: anywhere; white-space: pre-line; }
.resumption-excerpt { display: -webkit-box; -webkit-box-orient: vertical; -webkit-line-clamp: 3; overflow: hidden; }
.resumption-question { margin-top: 20px; }
.resumption-question span { display: block; margin-bottom: 5px; color: var(--muted); font-size: 12px; }
.resumption-continue { margin-top: 24px; padding: 10px 22px; border: 0; border-radius: 24px; background: var(--brand-accent); color: var(--brand-on-accent); cursor: pointer; }
.resumption-continue:hover:not(:disabled) { background: var(--brand-accent-deep); }
.resumption-continue:disabled { opacity: .6; cursor: wait; }
.resumption-status { max-width: 1100px; margin: 0 auto 24px; }
@media (max-width: 900px) {
  .reading-resumption .resumption-mascot { display: none; }
}
@media (max-width: 540px) {
  .reading-resumption { display: block; margin-bottom: 28px; border-radius: 24px; }
  .resumption-cover { float: right; width: 72px; margin: 0 0 12px 16px; }
  .resumption-cover :deep(.book-cover-fallback) { padding: 10px 7px; gap: 6px; }
  .resumption-cover :deep(.book-cover-name) { font-size: 10px; -webkit-line-clamp: 4; }
  .resumption-cover :deep(.book-cover-mark) { font-size: 4px; }
  .resumption-content { display: contents; }
  .resumption-question { clear: both; }
}
label { display: grid; gap: .5rem; margin: 1rem 0; }
input, button { font: inherit; padding: .6rem; }
.network-shell { height: 100%; min-height: 0; display: flex; flex-direction: column; }
.network-shell > :deep(.app) { flex: 1; min-height: 0; height: auto; position: relative; }
.network-shell > :deep(.app .board-workspace-overlay) { position: absolute; }
.network-page-bar { display: flex; align-items: center; gap: 8px; flex: 0 0 48px; height: 48px; padding-inline: clamp(12px, 1.6vw, 28px); border-bottom: 1px solid var(--line); background: var(--reader-canvas); }
.network-page-bar > nav { margin-right: auto; }
.network-current-reader { display: inline-flex; align-items: center; gap: 6px; padding: 4px 8px; border: 0; border-radius: 5px; background: transparent; color: var(--brand-accent-deep); font: 500 13px var(--sans); white-space: nowrap; }
.network-current-reader:hover { background: var(--brand-accent-soft); }
.network-page-bar :deep(.product-tour-launch) { min-height: 36px; border: 0; background: transparent; }
.account-chevron { margin-left: auto; }
@media (max-width: 640px) { .network-page-bar { padding-inline: 6px; gap: 2px; } }
.network-notices { flex: none; max-height: 25%; overflow: auto; display: flex; flex-wrap: wrap; gap: .7rem; padding: .3rem .8rem; background: var(--surface); }
.network-account { position: relative; font-size: .85rem; }
.network-linked-account { position: fixed; right: .6rem; bottom: .4rem; z-index: 100; background: var(--canvas); }
.network-account summary { cursor: pointer; padding: .4rem .6rem; }
.network-account-menu { position: absolute; right: 0; top: 100%; z-index: 100; display: grid; gap: .6rem; min-width: 12rem; padding: 1rem; background: var(--canvas); border: 1px solid var(--line); border-radius: .6rem; box-shadow: 0 8px 24px #0002; }
.network-account-menu { max-width: calc(100vw - 2rem); box-sizing: border-box; overflow-wrap: anywhere; }
@media (max-width: 1023px) { .network-account-menu { position: static; } }
.network-source { position: fixed; bottom: 1rem; right: 1rem; width: min(620px, calc(100vw - 2rem)); box-sizing: border-box; max-height: calc(100dvh - 2rem); overflow: auto; padding: 1rem; z-index: 110; background: var(--canvas); border: 1px solid var(--line); border-radius: 8px; box-shadow: 0 12px 40px #0003; }
.network-source > strong { display: block; margin-bottom: 12px; }
.network-linked-account .network-account-menu { position: absolute; top: auto; bottom: 100%; }
</style>

<style scoped src="@understand-book/web/reader/learning-library.css"></style>
<style scoped>
.learning-layout.is-courses { grid-template-columns: minmax(0, 1fr); }
.learning-layout.is-courses .learning-content { min-width: 0; }
</style>
