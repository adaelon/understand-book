<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from 'vue';
import App from './App.vue';
import VideoReaderPane from './components/VideoReaderPane.vue';
import type { PreparingVideo } from './video-api';
const videos = ref<PreparingVideo[]>([]), selectedVideo = ref<PreparingVideo | null>(null);
import BookCover from './components/BookCover.vue';
import YeyeMascot from './components/YeyeMascot.vue';
import brandLogo from './assets/brand/understand-book-logo.svg';
import { bookTitle } from './book-title';
import LoadingAnimation from './components/LoadingAnimation.vue';
import AccountAllowance from './components/AccountAllowance.vue';
import AccountSettings from './components/AccountSettings.vue';
import AccountMcp from './components/AccountMcp.vue';
import LearningRecordsPanel from './components/LearningRecordsPanel.vue';
import SharedWorkspaceCatalog from './components/SharedWorkspaceCatalog.vue';
import SharedWorkspaceInstance from './components/SharedWorkspaceInstance.vue';
import PersonalPreparations from './components/PersonalPreparations.vue';
import { readMcpSource, sourceParams, type McpSource } from './mcp-source';
import AccountAccess from './components/AccountAccess.vue';
import { takeAccountLink, type AccountView } from './account-forms';
import AgentPresentation from './components/AgentPresentation.vue';
import SourceExcerpt from './components/SourceExcerpt.vue';
import { api, ApiError } from './api';
import { invalidateLinkedWindows, isLinkedReaderChange } from './network-presentation';
import { network, installIdentity, installWorkspace, forgetNetwork, sceneKey, readerKey, workspaceStorageKey, type ChatDraft, type NetworkIdentity, type NetworkWorkspace, type PublishedBookRef, type LearningWorkspace } from './network-context';
import { networkFetch, recoverWorkspaceBinding, recoverSubmissions, pendingSubmissions, retrySubmission, readReadingResumption, learningWorkspaces, openLearningMaterial, type ReadingResumption, type PendingSubmission } from './network-client';
import { savedLearningSelection, saveLearningSelection, learningDraftKey, readLearningDraft, saveLearningDraft } from './learning-workspace-state';
import { targetPublication, type RecapTarget } from './session-recap';
const username = ref(''), password = ref(''), error = ref(''), busy = ref(false), checking = ref(true);
function showFailure(failure: unknown) {
  // A superseded request has nothing to report in the newly selected book or chat.
  if (failure instanceof ApiError && failure.errorCode === 'CLIENT_CONTEXT_STALE') return;
  error.value = failure instanceof Error ? failure.message : String(failure);
}
const sourceTarget = ref<McpSource | null>(null), sourceError = ref('');
let pendingSource: McpSource | null = null;
try { pendingSource = readMcpSource(location.search); } catch (failure) { sourceError.value = (failure as Error).message; }
let mcpRequested = new URLSearchParams(location.search).get('account') === 'mcp';
const accountLink = takeAccountLink();
const accountView = ref<AccountView | null>(accountLink.view), resetToken = ref(accountLink.token);
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
  forgetNetwork(); books.value = []; pending.value = 0; settingsOpen.value = false;
  accountLogin(undefined, '密码已更新，请使用新密码重新登录。');
  channel?.postMessage('changed');
}
type CatalogBook = { published_book_ref: PublishedBookRef; title?: string | null; is_default?: boolean; cover?: import('./api').BookCoverSource | null };
const books = ref<CatalogBook[]>([]);
const learningSpaces = ref<LearningWorkspace[]>([]), learningError = ref(''), learningLoading = ref(false);
const selectedLearningId = ref<string | null>(network.value.workspace?.learning_workspace_id ?? savedLearningSelection());
const selectedLearning = computed(() => learningSpaces.value.find(space => space.learning_workspace_id === selectedLearningId.value) ?? null);
const currentLearningGoal = computed(() => {
  const scene = network.value.workspace?.learning_workspace_goal;
  const saved = learningSpaces.value.find(space => space.learning_workspace_id === network.value.workspace?.learning_workspace_id)?.goal;
  return saved && (!scene || saved.revision >= scene.revision) ? saved : scene;
});
const createGoal = ref(''), goalDraft = ref(''), editingGoal = ref(false);
const recordsOpen = ref(false), legacyRecordsOpen = ref(false);
const sharedOpen = ref(false);
const memberBooks = computed(() => selectedLearning.value?.members.map(member => ({ ...member,
  book: books.value.find(book => samePublication(book.published_book_ref, member.published_book_ref)),
})) ?? []);
const availableBooks = computed(() => books.value.filter(book => !selectedLearning.value?.members.some(member => samePublication(member.published_book_ref, book.published_book_ref))));
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
}
function updateLearning(space: LearningWorkspace) {
  learningSpaces.value = [space, ...learningSpaces.value.filter(item => item.learning_workspace_id !== space.learning_workspace_id)];
}
async function createLearning() {
  if (!createGoal.value.trim() || busy.value) return;
  busy.value = true; error.value = '';
  try { const space = await learningWorkspaces.create(createGoal.value.trim()); updateLearning(space); selectLearning(space); createGoal.value = ''; }
  catch (failure) { showFailure(failure); }
  finally { busy.value = false; }
}
async function startSharedLearning(space: LearningWorkspace) {
  updateLearning(space); sharedOpen.value = false; await enterLearning(space);
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
  if (readerApp.value && !await readerApp.value.beforeNoteLeave('切换工作区')) return;
  selectLearning(space);
  if (!space.members.some(item => item.readable)) return;
  try {
    if (!network.value.workspace) await restoreAvailableScene();
    const previous = network.value.workspace?.learning_workspace_positions?.[space.learning_workspace_id];
    const member = space.members.find(item => item.readable && previous && samePublication(item.published_book_ref, previous)) ?? space.members.find(item => item.readable);
    if (member) await openBook(member.published_book_ref, space.learning_workspace_id);
  } catch (failure) { showFailure(failure); }
}
async function restoreAvailableScene() {
  try { await recoverWorkspaceBinding(); }
  catch (failure) {
    if (!(failure instanceof ApiError && failure.status === 404)) throw failure;
    sessionStorage.removeItem(workspaceStorageKey());
  }
}
function manageCurrentLearning() {
  selectedLearningId.value = network.value.workspace?.learning_workspace_id ?? null;
  saveLearningSelection(selectedLearningId.value); void showLibrary();
}
const libraryOpen = ref(!network.value.workspace);
const resumption = ref<ReadingResumption | null>(null), resumptionError = ref(''), resumptionLoading = ref(false);
const resumptionBook = computed(() => books.value.find(book =>
  book.published_book_ref.book_id === resumption.value?.published_book_ref.book_id
  && book.published_book_ref.publication_id === resumption.value?.published_book_ref.publication_id));
watch(() => network.value.epoch, () => {
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
      resumptionError.value = unavailable ? '原书籍或阅读记录已不可用，请选择其他材料。' : '接续记录暂时无法读取，请重试。';
    }
  } finally { if (scope === sceneKey()) resumptionLoading.value = false; }
}
async function showLibrary() {
  if (readerApp.value && !await readerApp.value.beforeNoteLeave('返回书架')) return;
  libraryOpen.value = true; error.value = '';
  try { await catalog(); await loadResumption(); }
  catch (failure) { showFailure(failure); }
}
async function continueReading() {
  if (!resumption.value || busy.value) return;
  busy.value = true; error.value = '';
  try { await recoverWorkspaceBinding(resumption.value); await recover(); selectedLearningId.value = network.value.workspace?.learning_workspace_id ?? null; saveLearningSelection(selectedLearningId.value); libraryOpen.value = false; suspended.value = false; }
  catch (failure) {
    showFailure(failure);
    await loadResumption();
  } finally { busy.value = false; }
}
function bookLabel(book: CatalogBook) {
  const title = bookTitle({ title: book.title, book_id: book.published_book_ref.book_id });
  const versions = books.value.filter(item => item.published_book_ref.book_id === book.published_book_ref.book_id);
  if (versions.length === 1) return title;
  return `${title} · 版本 ${versions.indexOf(book) + 1}${book.is_default ? '（默认）' : ''}`;
}
const readerApp = ref<InstanceType<typeof App> | null>(null);
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
  if (readerApp.value && !await readerApp.value.beforeNoteLeave('打开来源')) return;
  busy.value = true; sourceError.value = '';
  try {
    if (network.value.workspace) await api.openBook(JSON.stringify(source.published_book_ref));
    else installWorkspace(await networkFetch<NetworkWorkspace>('POST', '/workspaces', {
      attachment_id: network.value.attachment, published_book_ref: source.published_book_ref,
    }));
    sourceTarget.value = source; libraryOpen.value = false; suspended.value = false;
  } catch (failure) {
    if (failure instanceof ApiError && failure.errorCode === 'CLIENT_CONTEXT_STALE') return;
    sourceError.value = failure instanceof ApiError && [403, 404].includes(failure.status)
      ? '该来源材料未获授权或该版本已不可用，请联系管理员开通，或选择其他材料。'
      : '来源暂时无法打开，请重试。';
  } finally { busy.value = false; }
}
async function openAccountDestination() {
  if (mcpRequested) { showMcp(); mcpRequested = false; }
  if (pendingSource && !sourceError.value) await openPendingSource();
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
  if (readerApp.value && !await readerApp.value.beforeNoteLeave('切换发布')) return;
  openingRecap.value = true; error.value = '';
  try {
    await api.openBook(JSON.stringify(publication));
    await api.agentHistorySelect(target.session_id);
    recapTarget.value = target;
  } catch (failure) { showFailure(failure); }
  finally { openingRecap.value = false; }
}
const linkedRequested = new URLSearchParams(location.search).get('linked') === '1';
const linkedPresentation = ref<{ session_id: string; turn_id: string; reference: import('./generated/PresentationRef').PresentationRef } | null>(null);
const linkedSource = ref<import('./api').SourcePopupView | null>(null);
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
    error.value = '来源已定位，请回原阅读窗口查看';
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
  if (event.data.user !== network.value.identity?.user_id) { error.value = '附属窗口账号已改变，请从原窗口重新打开'; return; }
  installWorkspace(event.data.workspace, event.data.attachment, true);
  linkedPresentation.value = event.data;
}
async function followUp(message: string, receipt: import('./generated/PresentationFollowUp').PresentationFollowUp) {
  const scope = sceneKey();
  try { await api.agentRunCreate(message, { presentation_follow_up: receipt }); if (scope === sceneKey()) error.value = '问题已提交到原对话，可回原窗口查看回答'; }
  catch (failure) { if (scope === sceneKey()) showFailure(failure); }
}
function pageHide() {
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
const pending = ref(0);
const pendingItems = ref<PendingSubmission[]>([]);
watch(() => network.value.identity?.user_id, () => {
  videos.value = []; selectedVideo.value = null;
  pendingItems.value = []; pending.value = 0; linkedPresentation.value = null; books.value = []; learningSpaces.value = [];
  selectedLearningId.value = savedLearningSelection(); sharedOpen.value = false;
}, { flush: 'sync' });
let identityCheck: Promise<void> | null = null;
const channel = typeof BroadcastChannel === 'undefined' ? null : new BroadcastChannel('understand-book:authentication');
network.value = { ...network.value, enabled: true };
async function catalog() {
  books.value = (await networkFetch<{ books: typeof books.value }>('GET', '/library')).books;
  videos.value = (await networkFetch<{ videos: PreparingVideo[] }>('GET', '/videos')).videos;
  await loadLearningSpaces();
}
async function restore() {
  await recoverWorkspaceBinding();
  await recover();
}
async function recover() {
  try { await recoverSubmissions(); pendingChanged(); }
  catch (failure) { showFailure(failure); }
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
          await loadResumption(); await recover();
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
      if (network.value.identity) showFailure(failure);
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
    await loadResumption();
    await openAccountDestination();
  } catch { error.value = '登录失败，请核对账号、密码或稍后重试。'; }
  finally { password.value = ''; busy.value = false; checking.value = false; }
}
async function logout() {
  if (readerApp.value && !await readerApp.value.beforeNoteLeave('退出登录')) return;
  const n = network.value, w = n.workspace;
  const detach = w ? networkFetch('POST', `/workspaces/${w.workspace_id}/detach`, { attachment_id: n.attachment, generation: w.generation, expected_revision: w.revision }).catch(() => {}) : Promise.resolve();
  forgetNetwork(); books.value = []; pending.value = 0; error.value = '';
  busy.value = true;
  try {
    // Clear private UI immediately, but detach before revoking its Cookie.
    await detach;
    const response = await fetch('/api/auth/logout', { method: 'POST', credentials: 'same-origin',
      headers: { 'Content-Type': 'application/json', 'X-CSRF-Token': n.identity!.csrf_token }, body: '{}' });
    if (!response.ok) throw new Error('logout');
  } catch {
    error.value = '退出请求未确认，请核对网络后重新登录。';
  } finally { busy.value = false; }
  channel?.postMessage('changed');
}
async function openBook(reference: PublishedBookRef, learningId: string | null = selectedLearningId.value) {
  if (readerApp.value && !await readerApp.value.beforeNoteLeave('切换材料')) return;
  busy.value = true; error.value = '';
  try {
    if (!network.value.workspace) await restoreAvailableScene();
    if (network.value.workspace) await openLearningMaterial(reference, learningId);
    else installWorkspace(await networkFetch<NetworkWorkspace>('POST', '/workspaces', { attachment_id: network.value.attachment, published_book_ref: reference, learning_workspace_id: learningId }));
    selectedLearningId.value = learningId; saveLearningSelection(learningId);
    libraryOpen.value = false; suspended.value = false;
  } catch (failure) { showFailure(failure); }
  finally { busy.value = false; }
}
function pendingChanged() { pendingItems.value = pendingSubmissions(); pending.value = pendingItems.value.length; }
function onVisible() { if (document.visibilityState !== 'hidden') void checkIdentity(); }
function onLearningGoalChanged(event: Event) {
  if ((event as CustomEvent<{ learning_workspace_id: string }>).detail?.learning_workspace_id === network.value.workspace?.learning_workspace_id) void loadLearningSpaces();
}
async function onAccountChange(event: MessageEvent) {
  if (event.data === 'profile-changed') { void checkIdentity(); return; }
  const previous = identityCheck;
  forgetNetwork(); books.value = []; pending.value = 0;
  if (previous) await previous;
  void checkIdentity();
}
onMounted(() => {
  void checkIdentity();
  window.addEventListener('pagehide', pageHide);
  window.addEventListener('message', receiveLinked);
  window.addEventListener('submission-pending-changed', pendingChanged);
  window.addEventListener('learning-workspace-goal-changed', onLearningGoalChanged);
  channel?.addEventListener('message', onAccountChange);
  document.addEventListener('visibilitychange', onVisible);
  window.addEventListener('pageshow', onVisible); window.addEventListener('online', onVisible); window.addEventListener('focus', onVisible);
});
onBeforeUnmount(() => {
  window.removeEventListener('pagehide', pageHide);
  window.removeEventListener('message', receiveLinked);
  window.removeEventListener('submission-pending-changed', pendingChanged);
  window.removeEventListener('learning-workspace-goal-changed', onLearningGoalChanged);
  channel?.close(); document.removeEventListener('visibilitychange', onVisible);
  window.removeEventListener('pageshow', onVisible); window.removeEventListener('online', onVisible); window.removeEventListener('focus', onVisible);
});
</script>
<template>
  <AccountAccess v-if="accountView" :key="accountView" :view="accountView" :token="resetToken" @login="accountLogin" @reset="passwordChanged" @forgot="showAccount('forgot-password')" />
  <main v-else-if="checking && !network.workspace" class="network-loading"><LoadingAnimation /></main>
  <main v-else-if="!network.identity" class="network-login">
    <a class="brand-signature" href="/home.html"><img :src="brandLogo" class="brand-logo" width="36" height="36" alt="" /><span>Understand Book</span></a>
    <h1>回到你的阅读</h1>
    <form @submit.prevent="login">
      <label>邮箱或账号<input v-model="username" autocomplete="username" required></label>
      <label>密码<input v-model="password" type="password" autocomplete="current-password" required></label>
      <button :disabled="busy">登录</button>
    </form>
    <p><button :disabled="busy" @click="showAccount('register')">注册账号</button> <button :disabled="busy" @click="showAccount('forgot-password')">忘记密码</button></p>
    <p role="status">{{ error }}</p>
  </main>
  <div v-else class="network-shell">
    <details v-if="network.linked" class="network-account network-linked-account">
      <summary>账号</summary>
      <div class="network-account-menu">
        <span>{{ network.identity.email || network.identity.user_id }}</span>
        <button @click="settingsOpen = true">个人设置</button> <button @click="showMcp">MCP 接入</button> <button @click="allowanceOpen = true">使用额度</button>
        <a v-if="network.identity.capabilities?.admin" href="/admin/" target="_blank" rel="noopener">运营后台</a>
        <button @click="logout">退出登录</button>
      </div>
    </details>
    <div v-if="pending || error || sourceError" class="network-notices">
      <span v-if="sourceError" role="alert">{{ sourceError }} <button v-if="pendingSource" :disabled="busy" @click="openPendingSource">重试打开来源</button></span>
      <span v-if="pending" role="status">{{ pending }} 个问题提交待核对 <button @click="recover">核对原问题</button></span>
      <span v-for="item in pendingItems" :key="item.key">{{ item.body.display_user || item.body.message }} <button @click="retryOriginal(item.key)">重试原提交</button></span>
      <span v-if="error" role="alert">{{ error }}</span>
    </div>
    <AgentPresentation v-if="!suspended && network.linked && linkedPresentation && network.workspace" :key="key" :session-id="linkedPresentation.session_id" :turn-id="linkedPresentation.turn_id" :reference="linkedPresentation.reference" @follow-up="followUp" @source="showLinkedSource" />
    <VideoReaderPane v-else-if="!suspended && selectedVideo" :key="`${selectedVideo.video.media_id}/${selectedVideo.video.media_revision}`" :initial="selectedVideo" :owner="network.identity.user_id" @close="selectedVideo = null" @ready="reference => { selectedVideo = null; openBook(reference); }" />
    <main v-else-if="openingRecap" class="network-loading"><LoadingAnimation /></main>
    <section v-if="!selectedVideo && !libraryOpen && network.workspace?.learning_workspace_id && currentLearningGoal" class="workspace-goal-bar" aria-label="工作区大目标">
      <div><span>工作区大目标</span><p>{{ currentLearningGoal.text }}</p></div>
      <button v-if="!network.linked" :disabled="busy" @click="manageCurrentLearning">工作区与资料</button>
    </section>
    <App ref="readerApp" v-if="!suspended && !network.linked && !openingRecap && network.workspace" v-show="!selectedVideo && !libraryOpen" :reader-visible="!selectedVideo && !libraryOpen" :key="appKey" v-model:chat-draft="chatDraft" :recap-target="recapTarget" :mcp-source="sourceTarget" @mcp-source-consumed="sourceConsumed" @mcp-source-error="sourceFailed" @show-library="showLibrary" @show-allowance="allowanceOpen = true" @recap-consumed="recapTarget = null" @recap-publication="openRecapPublication">
      <template #account>
        <details class="network-account">
          <summary>账号</summary>
          <div class="network-account-menu">
            <span>{{ network.identity.email || network.identity.user_id }}</span>
            <button @click="settingsOpen = true">个人设置</button> <button @click="showMcp">MCP 接入</button> <button @click="allowanceOpen = true">使用额度</button>
            <a v-if="network.identity.capabilities?.admin" href="/admin/" target="_blank" rel="noopener">运营后台</a>
            <span v-if="network.identity.capabilities?.presentation.authoring === false">演示制作暂不可用</span>
            <button @click="logout">退出登录</button>
          </div>
        </details>
      </template>
    </App>
    <p v-if="linkedRequested && !linkedPresentation" role="status">请保持原阅读窗口打开；附属连接失效后请从原窗口重新打开。</p>
    <main v-if="!selectedVideo && !linkedRequested && (libraryOpen || !network.workspace)" class="network-library">
      <header class="network-library-head">
        <div><p class="network-library-kicker brand-signature"><img :src="brandLogo" class="brand-logo" width="28" height="28" alt="" />你的学习</p><h1>学习工作区</h1></div>
        <p><span class="account-email">{{ network.identity.email || network.identity.user_id }}</span> <button @click="settingsOpen = true">个人设置</button> <button @click="showMcp">MCP 接入</button> <button @click="allowanceOpen = true">使用额度</button> <a v-if="network.identity.capabilities?.admin" href="/admin/" target="_blank" rel="noopener">运营后台</a> <button @click="logout">退出登录</button></p>
      </header>
      <section class="learning-spaces" aria-label="学习工作区" :aria-busy="learningLoading">
        <form class="learning-create" @submit.prevent="createLearning">
          <label>这次想学会什么？<input v-model="createGoal" placeholder="比如：学习深度学习，或期末考试冲刺" required :disabled="busy" /></label>
          <button :disabled="busy || !createGoal.trim()">创建工作区</button>
        </form>
        <p v-if="learningError" role="alert">工作区暂时无法读取：{{ learningError }} <button @click="loadLearningSpaces">重试工作区</button></p>
        <div class="learning-space-list">
          <article v-for="space in learningSpaces" :key="space.learning_workspace_id" class="learning-space-card" :class="{ selected: selectedLearningId === space.learning_workspace_id }">
            <p>{{ space.goal.text }}</p><small>{{ space.members.length }} 份资料{{ space.archived ? ' · 已归档' : '' }}</small>
            <div><button :disabled="busy" @click="selectLearning(space)">管理工作区</button><button :disabled="busy || space.archived" @click="enterLearning(space)">打开工作区</button></div>
          </article>
        </div>
      </section>
      <section class="shared-entry">
        <button :aria-expanded="sharedOpen" @click="sharedOpen = !sharedOpen">{{ sharedOpen ? '收起共享工作区' : '浏览共享预构建工作区' }}</button>
        <SharedWorkspaceCatalog v-if="sharedOpen" :books="books" @started="startSharedLearning" />
      </section>
      <section v-if="selectedLearning" class="learning-details" aria-label="当前工作区">
        <header><div><span class="network-library-kicker">工作区大目标 · 第 {{ selectedLearning.goal.revision }} 版</span><h2>{{ selectedLearning.goal.text }}</h2></div>
          <button :disabled="busy || selectedLearning.archived" @click="goalDraft = selectedLearning.goal.text; editingGoal = true">修改大目标</button></header>
        <form v-if="editingGoal" class="learning-goal-edit" @submit.prevent="reviseGoal">
          <label>工作区大目标<textarea v-model="goalDraft" required :disabled="busy" /></label>
          <button :disabled="busy || !goalDraft.trim()">保存大目标</button><button type="button" :disabled="busy" @click="editingGoal = false">取消修改</button>
        </form>
        <p v-if="selectedLearning.goal.constraints.length">{{ selectedLearning.goal.constraints.join('；') }}</p>
        <SharedWorkspaceInstance :workspace="selectedLearning" :books="books" @updated="updateSharedLearning" />
        <PersonalPreparations :key="selectedLearning.learning_workspace_id" :workspace="selectedLearning" @ready="catalog" @open="reference => openBook(reference, selectedLearning!.learning_workspace_id)" @allowance="allowanceOpen = true" />
        <p v-if="!memberBooks.length" class="learning-empty">工作区已创建。加入资料后，即可围绕这个目标开始阅读与讨论。</p>
        <h3>工作区资料</h3>
        <div class="learning-members">
          <article v-for="member in memberBooks" :key="JSON.stringify(member.published_book_ref)" class="learning-member">
            <strong>{{ member.book ? bookLabel(member.book) : member.published_book_ref.book_id }}</strong>
            <span v-if="!member.readable">资料暂不可读</span>
            <div><button :disabled="busy || !member.readable || selectedLearning.archived" @click="openBook(member.published_book_ref)">{{ network.workspace?.learning_workspace_id === selectedLearningId && samePublication(network.workspace.published_book_ref, member.published_book_ref) ? '继续阅读' : '开始阅读' }}</button>
              <button :disabled="busy || selectedLearning.archived" @click="changeMember(member.published_book_ref, false)">移出资料</button></div>
          </article>
        </div>
        <details v-if="!selectedLearning.archived" class="learning-add" :open="!memberBooks.length">
          <summary>加入资料</summary>
          <div v-for="book in availableBooks" :key="JSON.stringify(book.published_book_ref)" class="learning-add-book"><span>{{ bookLabel(book) }}</span><button :disabled="busy" @click="changeMember(book.published_book_ref, true)">加入工作区</button></div>
          <p v-if="!availableBooks.length">当前可用资料均已加入。</p>
        </details>
        <button class="learning-records-toggle" :disabled="busy" :aria-expanded="recordsOpen" @click="recordsOpen = !recordsOpen">{{ recordsOpen ? '收起已有学习记录' : '带入已有学习记录' }}</button>
        <LearningRecordsPanel v-if="recordsOpen" :target="selectedLearning" :key="selectedLearning.learning_workspace_id" @refresh-goal="loadLearningSpaces" />
        <div class="learning-archive"><p>{{ selectedLearning.archived ? '工作区已归档，学习记录与来源保留。恢复后可继续学习。' : '归档后暂停主动教学和新增后台整理，学习记录与来源保留。' }}</p><button :disabled="busy" @click="archiveLearning">{{ selectedLearning.archived ? '恢复工作区' : '归档工作区' }}</button></div>
        <button class="learning-legacy" :disabled="busy" @click="selectLearning(null)">返回书架</button>
      </section>
      <section class="learning-records-entry">
        <button :aria-expanded="legacyRecordsOpen" @click="legacyRecordsOpen = !legacyRecordsOpen">{{ legacyRecordsOpen ? '收起旧记录' : '旧聊天与学习记录' }}</button>
        <LearningRecordsPanel v-if="legacyRecordsOpen" />
      </section>
      <section v-if="!selectedLearning && resumption && resumptionBook" class="reading-resumption" aria-label="阅读接续" :aria-busy="resumptionLoading">
        <div class="resumption-cover"><BookCover :title="bookTitle({ title: resumptionBook.title, book_id: resumption.published_book_ref.book_id })" :cover="resumptionBook.cover" /></div>
        <div class="resumption-content">
          <p class="network-library-kicker">接着上次读</p>
          <h2>{{ bookLabel(resumptionBook) }}</h2>
          <p class="resumption-position">已保存位置 · {{ resumption.position_label || '原阅读位置' }}</p>
          <p v-if="resumption.position_excerpt" class="resumption-excerpt">{{ resumption.position_excerpt }}</p>
          <div v-if="resumption.last_question" class="resumption-question"><span>上次的问题</span><p>{{ resumption.last_question }}</p></div>
          <button class="resumption-continue" :disabled="busy || resumptionLoading" @click="continueReading">{{ resumptionLoading ? '正在更新…' : '继续阅读' }}</button>
        </div>
        <YeyeMascot class="resumption-mascot" :size="144" :motion="busy || resumptionLoading || resumptionError ? 'static' : 'idle'" />
      </section>
      <p v-if="resumptionLoading && !resumptionBook" class="resumption-status" role="status">正在读取接续记录…</p>
      <p v-else-if="resumptionError" class="resumption-status" role="status">{{ resumptionError }} <button :disabled="busy" @click="showLibrary">重试</button></p>
      <h2 v-if="!selectedLearning" class="library-section-title">书架</h2>
      <div v-if="!selectedLearning" class="network-library-grid">
        <button v-for="video in videos" :key="video.video.media_id" class="network-book-card" @click="video.publication ? openBook(video.publication) : selectedVideo = video"><strong>{{ video.title }}</strong><span>{{ video.english_ready ? "视频 · 英文已发布" : "视频 · 转写准备中" }}</span></button>
        <button v-for="book in books" :key="book.published_book_ref.publication_id" class="network-book-card" :disabled="busy" @click="openBook(book.published_book_ref)">
          <BookCover :title="bookTitle({ title: book.title, book_id: book.published_book_ref.book_id })" :cover="book.cover" />
          <strong>{{ bookLabel(book) }}</strong>
        </button>
      </div>
      <p v-if="!books.length && !videos.length">账号已创建，等待管理员开通材料。</p>
    </main>
    <AccountMcp v-if="mcpOpen" @close="closeMcp" />
    <AccountSettings v-if="settingsOpen" @close="settingsOpen = false" @bound="channel?.postMessage('profile-changed')" @password-changed="passwordChanged" />
    <AccountAllowance v-if="allowanceOpen" :key="key" @close="allowanceOpen = false" />
    <aside v-if="linkedSource" class="network-source" aria-label="演示来源">
      <strong>{{ linkedSource.heading_path?.join(' / ') || linkedSource.label }}</strong>
      <SourceExcerpt :source="linkedSource" />
      <p v-if="linkedSource.stale">来源已变化，显示原引用快照。</p>
      <button v-if="linkedSource.can_open_in_reader" @click="openLinkedSource">在原阅读区打开</button>
      <button @click="linkedSource = null">关闭来源</button>
    </aside>
  </div>
</template>
<style scoped>
.workspace-goal-bar { flex: none; display: flex; align-items: center; gap: 16px; justify-content: space-between; padding: 8px 24px; background: var(--surface-soft, #f4f0e8); border-bottom: 1px solid var(--line); }
.workspace-goal-bar > div { min-width: 0; }
.workspace-goal-bar span { color: var(--muted); font-size: 11px; }
.workspace-goal-bar p { margin: 2px 0; font-size: 14px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.workspace-goal-bar button { flex: none; background: transparent; border: 1px solid var(--line); border-radius: 8px; color: var(--ink); }
.learning-spaces, .learning-details, .library-section-title { max-width: 1100px; margin: 0 auto 28px; }
.shared-entry { max-width: 1100px; margin: 0 auto 28px; }.shared-entry>button { padding: 10px 15px; min-height: 44px; border: 1px solid var(--line); border-radius: 9px; background: var(--canvas); color: var(--ink); cursor: pointer; }.learning-archive { display: flex; flex-wrap: wrap; align-items: center; gap: 12px; margin: 18px 0; }.learning-archive p { color: var(--muted); font-size: 12px; line-height: 1.7; margin: 0; }
.learning-records-entry { max-width: 1100px; margin: 0 auto 28px; }.learning-records-entry > button { padding: 9px 14px; min-height: 44px; border: 1px solid var(--line); border-radius: 9px; background: var(--canvas); color: var(--ink); cursor: pointer; }.learning-records-toggle { margin-right: 12px; }
.learning-create { display: flex; gap: 12px; align-items: end; padding: 20px; border-radius: 18px; background: var(--surface-soft, #f4f0e8); }
.learning-create label { flex: 1; margin: 0; color: var(--muted); font-size: 13px; }
.learning-create input, .learning-goal-edit textarea { box-sizing: border-box; width: 100%; min-width: 0; border: 1px solid var(--line); border-radius: 8px; background: var(--canvas); color: var(--ink); padding: 10px; }
.learning-spaces button, .learning-details button { border: 1px solid var(--line); border-radius: 9px; background: var(--canvas); color: var(--ink); cursor: pointer; }
.learning-create > button { flex: none; background: var(--brand-accent); color: var(--brand-on-accent); }
.learning-spaces button:disabled, .learning-details button:disabled { opacity: .5; cursor: default; }
.learning-space-list { margin-top: 16px; display: grid; grid-template-columns: repeat(auto-fit, minmax(220px, 1fr)); gap: 12px; }
.learning-space-card { min-width: 0; padding: 18px; border: 1px solid var(--line); border-radius: 14px; }
.learning-space-card.selected { border-color: var(--brand-accent); background: var(--surface-soft); }
.learning-space-card p { margin: 0 0 8px; line-height: 1.6; overflow-wrap: anywhere; }
.learning-space-card small { color: var(--muted); }
.learning-space-card > div, .learning-member > div { display: flex; flex-wrap: wrap; gap: 8px; margin-top: 12px; font-size: 13px; }
.learning-details { border-top: 1px solid var(--line); padding-top: 24px; }
.learning-details header { display: flex; align-items: center; justify-content: space-between; gap: 12px; }
.learning-details header > div { min-width: 0; }
.learning-details header > button { flex: none; }
.learning-details h2 { margin: 6px 0 12px; font-weight: 500; font-size: 22px; overflow-wrap: anywhere; }
.learning-details h3, .library-section-title { font-size: 16px; font-weight: 500; }
.learning-goal-edit { max-width: 680px; }
.learning-goal-edit textarea { min-height: 80px; font: inherit; }
.learning-goal-edit button + button { margin-left: 8px; }
.learning-empty { color: var(--muted); line-height: 1.8; }
.learning-members { display: grid; grid-template-columns: repeat(auto-fit, minmax(220px, 1fr)); gap: 12px; }
.learning-member { min-width: 0; padding: 16px; background: var(--surface-soft); border: 1px solid var(--line); border-radius: 12px; overflow-wrap: anywhere; }
.learning-member > span { display: block; color: var(--muted); font-size: 12px; margin-top: 6px; }
.learning-add { margin: 20px 0; }
.learning-add summary { cursor: pointer; padding: 12px 0; }
.learning-add-book { display: flex; align-items: center; justify-content: space-between; gap: 12px; padding: 8px 0; border-bottom: 1px solid var(--line); }
.learning-add-book span { min-width: 0; overflow-wrap: anywhere; }
.learning-add-book button { flex: none; }
@media (max-width: 540px) {
  .workspace-goal-bar { padding: 6px 12px; gap: 8px; }
  .workspace-goal-bar button { min-height: 44px; font-size: 12px; }
  .learning-create { flex-direction: column; align-items: stretch; }
  .learning-create input, .learning-goal-edit textarea { font-size: 16px; }
  .learning-space-list, .learning-members { grid-template-columns: minmax(0, 1fr); }
  .learning-spaces button, .learning-details button { min-height: 44px; }
}
.network-loading { height: 100%; flex: 1; min-height: 0; display: grid; place-items: center; background: var(--reader-canvas); }
.network-login { max-width: 32rem; margin: 12vh auto; padding: 1.5rem; }
.network-login h1, .network-library-head h1 { font-family: var(--font-display); font-weight: 400; font-synthesis: none; line-height: 1.45; }
.network-library { flex: 1; min-height: 0; overflow: auto; width: 100%; padding: clamp(20px, 5vw, 64px); box-sizing: border-box; }
.network-library-head { max-width: 1100px; margin: 0 auto 32px; display: flex; justify-content: space-between; align-items: center; gap: 20px; }
.network-library-head h1 { margin: 6px 0 0; font-size: clamp(24px, 3vw, 32px); }
.network-library-head > p { display: flex; flex-wrap: wrap; align-items: center; gap: 12px; font-size: 13px; min-width: 0; max-width: 100%; }
.account-email { overflow-wrap: anywhere; min-width: 0; max-width: 100%; }
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
.network-library-grid { max-width: 1100px; margin: auto; display: grid; grid-template-columns: repeat(auto-fill, minmax(145px, 1fr)); gap: 28px 24px; }
.network-book-card { display: flex; flex-direction: column; align-items: stretch; border: 0; border-radius: 8px; padding: 8px !important; min-width: 0; background: transparent; color: var(--ink); text-align: left; cursor: pointer; }
.network-book-card:hover { background: #00000005; }
.network-book-card:focus-visible { outline: 2px solid var(--accent); outline-offset: 4px; }
.network-book-card:disabled { opacity: .65; cursor: wait; }
.network-book-card strong { display: block; margin-top: 14px; font-size: 14px; line-height: 1.6; overflow-wrap: anywhere; }
.network-book-card:nth-child(3n + 2) { --cover-color: #ece2d4; }
.network-book-card:nth-child(3n) { --cover-color: #dfe6ee; }
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
  .network-library-head { align-items: start; flex-direction: column; gap: 8px; margin-bottom: 20px; }
  .network-library-grid { grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 22px 12px; }
}
label { display: grid; gap: .5rem; margin: 1rem 0; }
input, button { font: inherit; padding: .6rem; }
.network-shell { height: 100%; min-height: 0; display: flex; flex-direction: column; }
.network-shell > :deep(.app) { flex: 1; min-height: 0; height: auto; }
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
