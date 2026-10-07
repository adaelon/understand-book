<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from 'vue';
import App from './App.vue';
import BookCover from './components/BookCover.vue';
import AgentPresentation from './components/AgentPresentation.vue';
import SourceExcerpt from './components/SourceExcerpt.vue';
import { api } from './api';
import { invalidateLinkedWindows, isLinkedReaderChange } from './network-presentation';
import { network, installIdentity, installWorkspace, forgetNetwork, sceneKey, type ChatDraft, type NetworkIdentity, type NetworkWorkspace, type PublishedBookRef } from './network-context';
import { networkFetch, recoverWorkspaceBinding, recoverSubmissions, pendingSubmissions, retrySubmission, type PendingSubmission } from './network-client';
import { targetPublication, type RecapTarget } from './session-recap';
const username = ref(''), password = ref(''), error = ref(''), busy = ref(false), checking = ref(true);
type CatalogBook = { published_book_ref: PublishedBookRef; is_default?: boolean; cover?: import('./api').BookCoverSource | null };
const books = ref<CatalogBook[]>([]);
function bookLabel(book: CatalogBook) {
  const versions = books.value.filter(item => item.published_book_ref.book_id === book.published_book_ref.book_id);
  if (versions.length === 1) return book.published_book_ref.book_id;
  return `${book.published_book_ref.book_id} · 版本 ${versions.indexOf(book) + 1}${book.is_default ? '（默认）' : ''}`;
}
const key = computed(sceneKey);
const chatDraft = ref<ChatDraft | null>(null);
// A transport generation changes on reattachment; the user's unsent question does not.
watch(() => {
  const n = network.value, w = n.workspace;
  return JSON.stringify([n.epoch, n.identity?.user_id, w?.workspace_id, w?.published_book_ref, w?.selected_chat]);
}, () => { chatDraft.value = null; }, { flush: 'sync' });
const recapTarget = ref<RecapTarget | null>(null), openingRecap = ref(false);
async function openRecapPublication(target: RecapTarget) {
  const publication = targetPublication(target);
  if (!publication) return;
  openingRecap.value = true; error.value = '';
  try {
    await api.openBook(JSON.stringify(publication));
    await api.agentHistorySelect(target.session_id);
    recapTarget.value = target;
  } catch (failure) { error.value = failure instanceof Error ? failure.message : String(failure); }
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
  catch (failure) { if (scope === sceneKey()) error.value = failure instanceof Error ? failure.message : String(failure); }
}
async function openLinkedSource() {
  const turn = linkedPresentation.value?.turn_id, source = linkedSource.value, scope = sceneKey();
  if (!turn || !source) return;
  try { await api.agentSourceOpen(turn, source.source_ref_id); if (scope === sceneKey()) {
    error.value = '来源已定位，请回原阅读窗口查看';
    window.opener?.postMessage({ kind: 'reader-linked-reader-changed' }, location.origin); window.opener?.focus();
  } }
  catch (failure) { if (scope === sceneKey()) error.value = failure instanceof Error ? failure.message : String(failure); }
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
  catch (failure) { if (scope === sceneKey()) error.value = failure instanceof Error ? failure.message : String(failure); }
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
  pendingItems.value = []; pending.value = 0; linkedPresentation.value = null; books.value = [];
}, { flush: 'sync' });
let identityCheck: Promise<void> | null = null;
const channel = typeof BroadcastChannel === 'undefined' ? null : new BroadcastChannel('understand-book:authentication');
network.value = { ...network.value, enabled: true };
async function catalog() { books.value = (await networkFetch<{ books: typeof books.value }>('GET', '/library')).books; }
async function restore() {
  await recoverWorkspaceBinding();
  await recover();
}
async function recover() {
  try { await recoverSubmissions(); pendingChanged(); }
  catch (failure) { error.value = failure instanceof Error ? failure.message : String(failure); }
}
async function retryOriginal(key: string) {
  try { await retrySubmission(key); pendingChanged(); }
  catch (failure) { error.value = failure instanceof Error ? failure.message : String(failure); }
}
async function checkIdentity() {
  if (identityCheck) return identityCheck;
  identityCheck = (async () => {
    try {
      error.value = '';
      const identity = await networkFetch<NetworkIdentity>('GET', '/auth/me');
      if (!network.value.identity) { installIdentity(identity); await catalog(); if (linkedRequested) window.opener?.postMessage({ kind: 'reader-linked-ready' }, location.origin); }
      else if (identity.user_id !== network.value.identity.user_id || identity.csrf_token !== network.value.identity.csrf_token) {
        forgetNetwork(); books.value = []; installIdentity(identity); await catalog();
      }
      if (!linkedRequested) await restore();
      suspended.value = false;
      if (network.value.workspace && network.value.linked) {
        const current = await networkFetch<NetworkWorkspace>('GET', `/workspaces/${network.value.workspace.workspace_id}`);
        installWorkspace(current); await recover();
      }
      window.dispatchEvent(new Event('workspace-recovered'));
    } catch (failure) {
      if (network.value.identity) error.value = failure instanceof Error ? failure.message : String(failure);
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
  } catch { error.value = '登录失败，请核对账号、密码或稍后重试。'; }
  finally { password.value = ''; busy.value = false; checking.value = false; }
}
async function logout() {
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
async function openBook(reference: PublishedBookRef) {
  busy.value = true; error.value = '';
  try {
    installWorkspace(await networkFetch<NetworkWorkspace>('POST', '/workspaces', { attachment_id: network.value.attachment, published_book_ref: reference }));
  } catch (failure) { error.value = failure instanceof Error ? failure.message : String(failure); }
  finally { busy.value = false; }
}
function pendingChanged() { pendingItems.value = pendingSubmissions(); pending.value = pendingItems.value.length; }
function onVisible() { if (document.visibilityState !== 'hidden') void checkIdentity(); }
function onAccountChange() { forgetNetwork(); books.value = []; pending.value = 0; void checkIdentity(); }
onMounted(() => {
  void checkIdentity();
  window.addEventListener('pagehide', pageHide);
  window.addEventListener('message', receiveLinked);
  window.addEventListener('submission-pending-changed', pendingChanged);
  channel?.addEventListener('message', onAccountChange);
  document.addEventListener('visibilitychange', onVisible);
  window.addEventListener('pageshow', onVisible); window.addEventListener('online', onVisible); window.addEventListener('focus', onVisible);
});
onBeforeUnmount(() => {
  window.removeEventListener('pagehide', pageHide);
  window.removeEventListener('message', receiveLinked);
  window.removeEventListener('submission-pending-changed', pendingChanged);
  channel?.close(); document.removeEventListener('visibilitychange', onVisible);
  window.removeEventListener('pageshow', onVisible); window.removeEventListener('online', onVisible); window.removeEventListener('focus', onVisible);
});
</script>
<template>
  <main v-if="!network.identity" class="network-login">
    <h1>回到你的阅读</h1>
    <p v-if="checking">正在恢复登录…</p>
    <form v-else @submit.prevent="login">
      <label>账号<input v-model="username" autocomplete="username" required></label>
      <label>密码<input v-model="password" type="password" autocomplete="current-password" required></label>
      <button :disabled="busy">登录</button>
    </form>
    <p role="status">{{ error }}</p>
  </main>
  <div v-else class="network-shell">
    <details v-if="network.linked" class="network-account network-linked-account">
      <summary>账号</summary>
      <div class="network-account-menu">
        <span>{{ network.identity.user_id }}</span>
        <button @click="logout">退出登录</button>
      </div>
    </details>
    <div v-if="pending || error" class="network-notices">
      <span v-if="pending" role="status">{{ pending }} 个问题提交待核对 <button @click="recover">核对原问题</button></span>
      <span v-for="item in pendingItems" :key="item.key">{{ item.body.display_user || item.body.message }} <button @click="retryOriginal(item.key)">重试原提交</button></span>
      <span v-if="error" role="alert">{{ error }}</span>
    </div>
    <AgentPresentation v-if="!suspended && network.linked && linkedPresentation && network.workspace" :key="key" :session-id="linkedPresentation.session_id" :turn-id="linkedPresentation.turn_id" :reference="linkedPresentation.reference" @follow-up="followUp" @source="showLinkedSource" />
    <p v-else-if="openingRecap" role="status">正在打开原发布…</p>
    <App v-else-if="!suspended && network.workspace" :key="key" v-model:chat-draft="chatDraft" :recap-target="recapTarget" @recap-consumed="recapTarget = null" @recap-publication="openRecapPublication">
      <template #account>
        <details class="network-account">
          <summary>账号</summary>
          <div class="network-account-menu">
            <span>{{ network.identity.user_id }}</span>
            <span v-if="network.identity.capabilities?.presentation.authoring === false">演示制作暂不可用</span>
            <button @click="logout">退出登录</button>
          </div>
        </details>
      </template>
    </App>
    <p v-else-if="linkedRequested" role="status">请保持原阅读窗口打开；附属连接失效后请从原窗口重新打开。</p>
    <main v-else class="network-library">
      <header class="network-library-head">
        <div><p class="network-library-kicker">你的书架</p><h1>选择阅读材料</h1></div>
        <p>{{ network.identity.user_id }} <button @click="logout">退出登录</button></p>
      </header>
      <div class="network-library-grid">
        <button v-for="book in books" :key="book.published_book_ref.publication_id" class="network-book-card" :disabled="busy" @click="openBook(book.published_book_ref)">
          <BookCover :title="book.published_book_ref.book_id" :cover="book.cover" />
          <strong>{{ bookLabel(book) }}</strong>
        </button>
      </div>
      <p v-if="!books.length">暂无获授权的材料。</p>
    </main>
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
.network-login { max-width: 32rem; margin: 12vh auto; padding: 1.5rem; }
.network-library { flex: 1; min-height: 0; overflow: auto; width: 100%; padding: clamp(20px, 5vw, 64px); box-sizing: border-box; }
.network-library-head { max-width: 1100px; margin: 0 auto 32px; display: flex; justify-content: space-between; align-items: center; gap: 20px; }
.network-library-head h1 { margin: 6px 0 0; font-size: clamp(24px, 3vw, 32px); }
.network-library-head > p { display: flex; align-items: center; gap: 12px; font-size: 13px; }
.network-library-kicker { margin: 0; color: var(--muted); font-size: 13px; }
.network-library-grid { max-width: 1100px; margin: auto; display: grid; grid-template-columns: repeat(auto-fill, minmax(145px, 1fr)); gap: 28px 24px; }
.network-book-card { display: flex; flex-direction: column; align-items: stretch; border: 0; border-radius: 8px; padding: 8px !important; min-width: 0; background: transparent; color: var(--ink); text-align: left; cursor: pointer; }
.network-book-card:hover { background: #00000005; }
.network-book-card:focus-visible { outline: 2px solid var(--accent); outline-offset: 4px; }
.network-book-card:disabled { opacity: .65; cursor: wait; }
.network-book-card strong { display: block; margin-top: 14px; font-size: 14px; line-height: 1.6; overflow-wrap: anywhere; }
.network-book-card:nth-child(3n + 2) { --cover-color: #ece2d4; }
.network-book-card:nth-child(3n) { --cover-color: #dfe6ee; }
@media (max-width: 540px) {
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
@media (max-width: 1023px) { .network-account-menu { position: static; } }
.network-source { position: fixed; bottom: 1rem; right: 1rem; width: min(620px, calc(100vw - 2rem)); box-sizing: border-box; max-height: calc(100dvh - 2rem); overflow: auto; padding: 1rem; z-index: 110; background: var(--canvas); border: 1px solid var(--line); border-radius: 8px; box-shadow: 0 12px 40px #0003; }
.network-source > strong { display: block; margin-bottom: 12px; }
.network-linked-account .network-account-menu { position: absolute; top: auto; bottom: 100%; }
</style>
