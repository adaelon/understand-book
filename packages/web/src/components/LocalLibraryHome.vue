<script setup lang="ts">
import { computed, ref, watch } from 'vue';
import { ArrowLeft, BookOpen, FolderOpen, Plus, Settings } from '@lucide/vue';
import type { BookLibraryEntry } from '../api';
import { bookTitle } from '../book-title';
import { t } from '../i18n';
import type { PublishedBookRef } from '../reader-context';
import type { SharedBook } from '../shared-workspaces';
import brandLogo from '../assets/brand/understand-book-logo.svg';
import BoardLibrary from './BoardLibrary.vue';
import LanguageSwitcher from './LanguageSwitcher.vue';
import LearningCatalogCard from './LearningCatalogCard.vue';
import MaterialCover from './MaterialCover.vue';
import WorkspaceDestinations from './WorkspaceDestinations.vue';

export type LibraryPage = 'courses' | 'materials' | 'boards' | 'understanding';
export type LocalBookAction = 'read' | 'course' | 'understanding' | { boardId: string } | { boardTitle: string };
const props = defineProps<{
  page: LibraryPage; books: BookLibraryEntry[]; root: string; activeDir: string | null;
  loading: boolean; busy: boolean; error: string | null; canCreate: boolean; native: boolean; resumeTitle?: string;
}>();
const emit = defineEmits<{
  navigate: [page: LibraryPage]; refresh: []; settings: []; picker: [mode: 'open' | 'create'];
  open: [book: BookLibraryEntry, action: LocalBookAction]; resume: []; resumeExternal: [];
}>();
const selectedDir = ref('');
const selected = computed(() => props.books.find(book => book.dir === selectedDir.value));
watch(() => props.page, () => { selectedDir.value = ''; });
const readable = computed(() => props.books.filter(book => book.route === 'reader'));
const externalBook = computed(() => props.activeDir && !props.books.some(book => book.dir === props.activeDir) ? props.activeDir : null);
const visibleBooks = computed(() => props.page === 'understanding' ? readable.value : props.books);
const duplicateTitles = computed(() => {
  const seen = new Set<string>(), duplicates = new Set<string>();
  for (const book of props.books) {
    const title = bookTitle(book);
    if (seen.has(title)) duplicates.add(title);
    seen.add(title);
  }
  return duplicates;
});
// A personal board belongs to a book ID. When the library contains two copies, prefer the one already in use.
const boardBooks = computed(() => {
  const books = new Map<string, BookLibraryEntry>();
  for (const book of readable.value) {
    if (!books.has(book.book_id) || book.dir === props.activeDir) books.set(book.book_id, book);
  }
  return books;
});
const boardMaterials = computed<SharedBook[]>(() => Array.from(boardBooks.value.values(), book => ({
  title: bookTitle(book), published_book_ref: { book_id: book.book_id, publication_id: 'local' },
})));
function boardAction(reference: PublishedBookRef, action: LocalBookAction) {
  const book = boardBooks.value.get(reference.book_id);
  if (book) emit('open', book, action);
}
function description(book: BookLibraryEntry) {
  if (duplicateTitles.value.has(bookTitle(book))) return book.dir;
  if (props.page === 'materials') return book.route === 'workbench' ? t('正在准备') : t('可开始阅读');
  if (book.route === 'workbench') return t('打开工作台查看准备状态');
  if (book.has_course) return t('先读原文，也可以选择已提供的单元开始学习。');
  return t('从原文开始，带着问题阅读。');
}
function choose(book: BookLibraryEntry) {
  if (props.page === 'courses' && book.route === 'reader') selectedDir.value = book.dir;
  else emit('open', book, props.page === 'understanding' ? 'understanding' : 'read');
}
function navigate(page: LibraryPage) { selectedDir.value = ''; emit('navigate', page); }
</script>

<template>
  <header class="library-bar">
    <span class="signature"><img :src="brandLogo" width="25" height="25" alt="" /><strong>Understand Book</strong></span>
    <WorkspaceDestinations :current="page" :busy="busy" @courses="navigate('courses')" @materials="navigate('materials')" @boards="navigate('boards')" @understanding="navigate('understanding')" />
    <div class="library-tools">
      <LanguageSwitcher />
      <button type="button" :aria-label="native ? t('打开桌面设置') : t('模型设置')" :title="native ? t('打开桌面设置') : t('模型设置')" @click="emit('settings')"><Settings :size="18" /></button>
    </div>
  </header>
  <p v-if="error" class="library-error" role="alert">{{ error }} <button @click="emit('refresh')">{{ t('重试') }}</button></p>
  <BoardLibrary v-if="page === 'boards'" local :materials="boardMaterials" :workspaces="[]" :busy="busy || loading"
    @materials="navigate('materials')" @open="(board, ref) => boardAction(ref, { boardId: board.board_id })"
    @create="(title, ref) => boardAction(ref, { boardTitle: title })" />
  <main v-else class="library-main">
    <section class="shelf" :aria-busy="loading || busy">
      <header class="shelf-heading">
        <div>
          <span class="eyebrow">{{ page === 'courses' ? t('学习') : t('本地书库') }}</span>
          <h1>{{ page === 'materials' ? t('学习资料') : page === 'understanding' ? t('我的理解') : selected ? bookTitle(selected) : t('读一本书，或走完一套课程') }}</h1>
          <p v-if="page === 'materials'" class="root-path">{{ t('书库位置：') }}{{ root }}</p>
          <p v-else-if="page === 'understanding'">{{ t('选择一本书，查看自己的理解与进展。') }}</p>
          <p v-else-if="!selected">{{ t('先找到想学的内容，再从一个单元、一节课开始。') }}</p>
        </div>
        <div class="actions">
          <button v-if="externalBook && !loading && (page === 'materials' || page === 'courses')" :title="externalBook" :disabled="busy" @click="emit('resumeExternal')">{{ t('继续阅读') }}</button>
          <button v-if="page === 'materials'" :disabled="busy" @click="emit('picker','open')"><FolderOpen :size="16" />{{ t('打开已有资料') }}</button>
          <button v-if="page === 'materials' && canCreate" :disabled="busy" @click="emit('picker','create')"><Plus :size="16" />{{ t('新建论文书') }}</button>
          <button v-if="page === 'courses' && resumeTitle" :title="resumeTitle" :disabled="busy" @click="emit('resume')">{{ t('继续学习') }}</button>
          <button :disabled="loading || busy" @click="emit('refresh')">{{ t('刷新目录') }}</button>
        </div>
      </header>
      <p v-if="loading && !books.length" role="status">{{ t('正在读取学习目录…') }}</p>
      <template v-else-if="page === 'courses' && selected">
        <button class="back-link" :disabled="busy" @click="selectedDir = ''"><ArrowLeft :size="16" />{{ t('返回学习首页') }}</button>
        <div class="book-intro">
          <div class="detail-cover"><MaterialCover :title="bookTitle(selected)" :cover="selected.cover" /></div>
          <div><span class="eyebrow">{{ t('书籍学习首页') }}</span><p>{{ description(selected) }}</p>
            <div class="actions">
              <button :class="{ primary: !selected.has_course }" :disabled="busy" @click="emit('open', selected, 'read')"><BookOpen :size="16" />{{ t('阅读原文') }}</button>
              <button v-if="selected.has_course" class="primary" :disabled="busy" @click="emit('open', selected, 'course')">{{ t('跟着课程学') }}</button>
            </div>
          </div>
        </div>
      </template>
      <div v-else-if="visibleBooks.length" class="collection-list">
        <LearningCatalogCard v-for="book in visibleBooks" :key="book.dir" :title="bookTitle(book)" :cover="book.cover"
          :kind="t('书籍')" :description="description(book)" :disabled="busy"
          :action="page === 'understanding' ? t('我的理解') : book.route === 'workbench' ? t('打开工作台') : page === 'courses' ? t('查看这本书') : t('开始阅读')"
          @open="choose(book)" />
      </div>
      <div v-else-if="!error" class="empty-library">
        <p>{{ t('这里还没有可阅读的资料。') }}</p>
        <button v-if="page !== 'materials'" @click="navigate('materials')">{{ t('学习资料') }}</button>
      </div>
    </section>
  </main>
</template>

<style scoped>
.library-bar { display:flex; align-items:center; gap:20px; box-sizing:border-box; flex:0 0 48px; min-width:0; height:48px; padding-inline:clamp(12px,1.6vw,28px); background:var(--reader-canvas); border-bottom:1px solid var(--hairline); }
.signature { display:flex; align-items:center; gap:8px; white-space:nowrap; color:var(--charcoal); font:600 17px/1 Georgia,serif; letter-spacing:-.035em; }
.signature::after { content:''; height:18px; margin-left:8px; border-left:1px solid var(--hairline); }
.library-tools,.actions { display:flex; align-items:center; gap:8px; }
.library-tools { margin-left:auto; }
.library-tools button { display:grid; place-items:center; width:36px; height:36px; border:0; border-radius:6px; background:transparent; color:var(--steel); cursor:pointer; }
.library-main { flex:1; min-height:0; overflow:auto; padding:28px clamp(16px,4vw,64px) 56px; background:#f7f5ef; color:var(--ink); font:14px/1.6 var(--sans); }
.shelf { max-width:1320px; box-sizing:border-box; margin:0 auto; padding:28px; border:1px solid var(--line); border-radius:16px; background:var(--canvas); }
.shelf-heading,.book-intro { display:flex; gap:24px; align-items:center; }
.shelf-heading { justify-content:space-between; flex-wrap:wrap; }
.shelf-heading>div { min-width:0; }
.eyebrow { color:var(--brand-accent-deep); font-size:12px; }
h1 { margin:7px 0; font:500 25px/1.5 var(--serif,var(--sans)); overflow-wrap:anywhere; }
p { color:var(--muted); font-size:13px; overflow-wrap:anywhere; }
.root-path { margin-bottom:0; }
.actions { flex-wrap:wrap; }
.shelf button,.library-error button { display:inline-flex; align-items:center; justify-content:center; gap:8px; min-height:42px; padding:8px 14px; border:1px solid var(--line); border-radius:8px; color:var(--ink); background:var(--canvas); font:inherit; font-size:13px; cursor:pointer; }
.shelf button:disabled { opacity:.55; cursor:wait; }
.shelf .primary { background:var(--brand-accent); border-color:var(--brand-accent); color:var(--brand-on-accent); }
.collection-list { display:grid; grid-template-columns:repeat(auto-fill,minmax(min(100%,320px),1fr)); gap:18px; margin-top:24px; }
.book-intro { padding:22px 26px; border-radius:12px; background:var(--surface-soft); }
.detail-cover { width:78px; flex:none; }
.shelf .back-link { margin:6px 0 14px; border:0; padding-inline:0; color:var(--steel); }
.library-error { padding:8px 20px; color:var(--brand-error); }
@media(max-width:720px) {
  .library-bar { gap:5px; padding:0 8px; }
  .signature strong,.signature::after { display:none; }
  .library-tools { gap:2px; }
  .shelf { padding:18px; }
  .library-main { padding:16px 12px 36px; }
  .shelf-heading { gap:8px; }
  .book-intro { padding:18px; gap:16px; }
  .detail-cover { width:58px; }
}
</style>
