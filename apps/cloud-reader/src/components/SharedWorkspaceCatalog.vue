<script setup lang="ts">
import { computed, onBeforeUnmount, ref, watch } from 'vue';
import { ArrowLeft, ArrowRight, BookOpen } from '@lucide/vue';
import { t as uiText } from '@understand-book/web/reader/i18n';
import { ApiError } from '@understand-book/web/reader/api';
import { bookTitle } from '@understand-book/web/reader/book-title';
import { network, type LearningWorkspace, type PublishedBookRef } from '@understand-book/web/reader/reader-context';
import { sharedWorkspaces } from '../network-client';
import { sharedBookKey, sharedBookLabel, type SharedBook, type SharedRelease, type SharedReleasePage } from '@understand-book/web/reader/shared-workspaces';
import { inquiryCourse, type CourseCatalog } from '@understand-book/web/reader/inquiry-course';
import type { CourseLessonSummary } from '@understand-book/web/reader/generated/CourseLessonSummary';
import type { CourseProgress } from '@understand-book/web/reader/generated/CourseProgress';
import type { CourseLaunchTarget, LearningTaskView } from '@understand-book/web/reader/learning-tasks';
import { learningCollections, type LearningCollection, type LearningCollectionTarget } from '@understand-book/web/reader/learning-collections';
import { examSprints, sprintRequest, type ExamSprintInput } from '../exam-sprints';
import LearningCatalogCard from '@understand-book/web/reader/components/LearningCatalogCard.vue';
import MaterialCover from '@understand-book/web/reader/components/MaterialCover.vue';
import SharedWorkspaceManifest from '@understand-book/web/reader/components/SharedWorkspaceManifest.vue';

const props = defineProps<{ books: SharedBook[]; workspaces?: LearningWorkspace[]; learningTasks?: LearningTaskView[] }>();
const target = defineModel<LearningCollectionTarget | null>('target', { default: null });
const emit = defineEmits<{
  started: [workspace: LearningWorkspace, message?: string, preparedCourse?: boolean, target?: CourseLaunchTarget];
  reading: [publication: PublishedBookRef, learningWorkspaceId?: string];
}>();
type ReleaseView = { release: SharedRelease; workspace?: LearningWorkspace; catalog?: CourseCatalog };
type GoalOptions = { goal: string; sprint: boolean; examDate: string; syllabus: string; minutes: number | null; entryMode: 'diagnostic' | 'question'; question: string };
const page = ref<SharedReleasePage | null>(null), detailPage = ref<SharedReleasePage | null>(null);
const views = ref<ReleaseView[]>([]), related = ref<SharedRelease[]>([]);
const prepared = ref<Record<string, LearningWorkspace>>({}), options = ref<Record<string, GoalOptions>>({});
const loading = ref(false), busy = ref(false), error = ref('');
const catalogElement = ref<HTMLElement | null>(null);
const operations = new Map<string, string>();
let request = 0, pendingRead: AbortController | null = null;
const materialBook = (reference: PublishedBookRef) => props.books.find(book => sharedBookKey(book.published_book_ref) === sharedBookKey(reference));
const bookLabel = (reference: PublishedBookRef) => bookTitle({ title: (materialBook(reference) ?? props.books.find(book => book.published_book_ref.book_id === reference.book_id))?.title, book_id: reference.book_id });
const releaseBooks = (release: SharedRelease) => [...new Set((release.manifest.inquiry_course?.lessons ?? []).map(lesson => lesson.source.publication.book_id))];
const overview = (view: ReleaseView) => view.catalog?.release_version === view.release.version ? view.catalog.course ?? view.release.manifest.inquiry_course : view.release.manifest.inquiry_course;
const lessons = (view: ReleaseView) => overview(view)?.lessons ?? [];
const collections = computed(() => learningCollections(props.books, page.value?.items ?? []));
const materialReleases = computed(() => (page.value?.items ?? []).filter(release => !collections.value.some(item => item.kind === 'course' ? item.release.release_id === release.release_id : item.releases.some(course => course.release_id === release.release_id))));
const selectedBook = computed(() => target.value?.kind === 'book' ? target.value.publication : null);
const detailTitle = computed(() => selectedBook.value ? bookLabel(selectedBook.value) : views.value[0]?.release.manifest.name ?? uiText('专题课程'));
const courseCount = computed(() => views.value.filter(view => lessons(view).length).length);
const lessonCount = computed(() => views.value.reduce((count, view) => count + lessons(view).length, 0));
const completedCount = computed(() => views.value.reduce((count, view) => count + lessons(view).filter(lesson => progressFor(view, lesson.id)?.status === 'completed').length, 0));
const hasProgress = computed(() => views.value.some(view => !!view.workspace));
const bookVersions = computed(() => {
  if (!selectedBook.value) return [];
  const versions = [selectedBook.value, ...props.books.map(book => book.published_book_ref), ...views.value.flatMap(view => lessons(view).map(lesson => lesson.source.publication))];
  return versions.filter((publication, index) => publication.book_id === selectedBook.value!.book_id && versions.findIndex(other => sharedBookKey(other) === sharedBookKey(publication)) === index);
});
function workspaceFor(releaseId: string) {
  const explicit = props.workspaces?.find(workspace => workspace.learning_workspace_id === target.value?.learningWorkspaceId && workspace.shared_release?.release_id === releaseId);
  if (explicit) return explicit;
  const task = props.learningTasks?.filter(task => task.status !== 'unavailable').sort((a, b) => b.selected_at - a.selected_at)
    .find(task => props.workspaces?.some(workspace => !workspace.archived && workspace.learning_workspace_id === task.learning_workspace_id && workspace.shared_release?.release_id === releaseId));
  return props.workspaces?.find(workspace => !workspace.archived && workspace.learning_workspace_id === task?.learning_workspace_id)
    ?? props.workspaces?.find(workspace => !workspace.archived && workspace.shared_release?.release_id === releaseId)
    ?? prepared.value[releaseId];
}
function courseTarget(release: SharedRelease): LearningCollectionTarget {
  const workspace = workspaceFor(release.release_id);
  return { kind: 'course', releaseId: release.release_id, version: workspace?.shared_release?.version ?? release.version,
    ...(workspace ? { learningWorkspaceId: workspace.learning_workspace_id } : {}) };
}
function openCollection(collection: LearningCollection) {
  if (collection.kind === 'course') { target.value = courseTarget(collection.release); return; }
  const task = props.learningTasks?.filter(task => task.status !== 'unavailable' && task.published_book_ref.book_id === collection.bookId).sort((a, b) => b.selected_at - a.selected_at)[0];
  const workspace = task?.learning_workspace_id ? props.workspaces?.find(workspace => workspace.learning_workspace_id === task.learning_workspace_id) : collection.releases.map(release => workspaceFor(release.release_id)).find(Boolean);
  target.value = { kind: 'book', publication: task?.published_book_ref ?? collection.book.published_book_ref,
    ...(workspace ? { learningWorkspaceId: workspace.learning_workspace_id } : {}) };
}
function settings(view: ReleaseView) { return options.value[`${view.release.release_id}:${view.release.version}`]!; }
function prepareOptions(view: ReleaseView) {
  const key = `${view.release.release_id}:${view.release.version}`;
  options.value[key] ??= { goal: view.release.manifest.public_goal, sprint: false, examDate: '', syllabus: '', minutes: null, entryMode: 'diagnostic', question: '' };
}
function canStart(view: ReleaseView, lesson?: CourseLessonSummary) {
  if (view.workspace) return !view.workspace.archived && (!lesson || !view.catalog || view.catalog.release_version === view.release.version || !!progressFor(view, lesson.id));
  const choice = settings(view);
  return !view.release.archived && view.release.availability.status === 'ready' && !!choice?.goal.trim() && (!choice.sprint || !!choice.examDate);
}
function availabilityLabel(release: SharedRelease) {
  if (workspaceFor(release.release_id)) return uiText('已有学习记录');
  if (release.archived) return uiText('已归档，暂不接受新的学习');
  if (release.availability.status === 'ready') return release.manifest.inquiry_course?.lessons.length ? uiText('可以开始学习') : uiText('资料已就绪');
  if (release.availability.audience.some(user => user.missing_grants.length)) return uiText('尚未开通完整访问，请查看说明');
  return uiText('部分材料还在准备，请查看说明');
}
const failureMessage = (failure: unknown) => failure instanceof Error ? failure.message : uiText('学习目录暂时无法读取');
function fail(failure: unknown, ticket: number) { if (ticket === request && !(failure instanceof ApiError && failure.errorCode === 'CLIENT_CONTEXT_STALE')) error.value = failureMessage(failure); }
async function readCatalog<T>(read: (signal: AbortSignal) => Promise<T>): Promise<T> {
  pendingRead?.abort();
  const controller = new AbortController(); pendingRead = controller;
  const timeout = setTimeout(() => controller.abort(new Error(uiText('课程读取超时，请重试。'))), 15_000);
  try { return await read(controller.signal); }
  finally { clearTimeout(timeout); if (pendingRead === controller) pendingRead = null; }
}
async function frozenView(workspace: LearningWorkspace, version?: number): Promise<ReleaseView> {
  const [saved, catalog] = await Promise.all([sharedWorkspaces.instance(workspace.learning_workspace_id), inquiryCourse.catalog(workspace.learning_workspace_id)]);
  if (!saved.instance) throw new Error(uiText('原学习路线暂时无法读取，请重试。'));
  const release = version && version !== saved.instance.version
    ? await readCatalog(signal => sharedWorkspaces.read(saved.instance!.release_id, version, signal)) : saved.instance.release;
  return { release, workspace, catalog };
}
async function load(offset = 0) {
  const ticket = ++request; loading.value = true; error.value = '';
  try { const value = await readCatalog(signal => sharedWorkspaces.list(offset, signal)); if (ticket === request) page.value = value; }
  catch (failure) { fail(failure, ticket); }
  finally { if (ticket === request) loading.value = false; }
}
async function loadDetail(offset = 0) {
  const selection = target.value;
  if (!selection) { views.value = []; related.value = []; detailPage.value = null; return; }
  const ticket = ++request; loading.value = true; error.value = ''; views.value = []; related.value = [];
  try {
    const explicit = selection.learningWorkspaceId ? props.workspaces?.find(workspace => workspace.learning_workspace_id === selection.learningWorkspaceId) : undefined;
    if (selection.learningWorkspaceId && !explicit) throw new Error(uiText('原学习记录暂时无法读取，请重试。'));
    let resolved: ReleaseView[], relatedReleases: SharedRelease[] = [];
    if (selection.kind === 'course') {
      const workspace = explicit ?? workspaceFor(selection.releaseId);
      resolved = [workspace ? await frozenView(workspace, selection.version) : { release: await readCatalog(signal => sharedWorkspaces.read(selection.releaseId, selection.version, signal)) }];
      if (ticket === request) detailPage.value = null;
    } else {
      const value = await readCatalog(signal => sharedWorkspaces.list(offset, signal, selection.publication.book_id));
      const own = value.items.filter(release => releaseBooks(release).length === 1 && releaseBooks(release)[0] === selection.publication.book_id);
      relatedReleases = value.items.filter(release => releaseBooks(release).length > 1);
      const explicitView = explicit?.shared_release ? await frozenView(explicit) : undefined;
      resolved = await Promise.all(own.map(async release => {
        if (explicitView?.release.release_id === release.release_id) return explicitView;
        const workspace = workspaceFor(release.release_id);
        return workspace ? frozenView(workspace) : { release };
      }));
      if (explicitView && !resolved.some(view => view.release.release_id === explicitView.release.release_id)) {
        if (releaseBooks(explicitView.release).length === 1 && releaseBooks(explicitView.release)[0] === selection.publication.book_id) resolved.unshift(explicitView);
        else if (releaseBooks(explicitView.release).includes(selection.publication.book_id)) relatedReleases = [explicitView.release, ...relatedReleases.filter(release => release.release_id !== explicitView.release.release_id)];
      }
      if (ticket === request) detailPage.value = value;
    }
    if (ticket === request) { resolved.forEach(prepareOptions); views.value = resolved; related.value = relatedReleases; }
  } catch (failure) { fail(failure, ticket); }
  finally { if (ticket === request) loading.value = false; }
}
function progressFor(view: ReleaseView, lessonId: string): CourseProgress | undefined {
  const progress = view.catalog?.progress.filter(item => item.lesson_id === lessonId && item.release_version === view.release.version && item.goal_revision === view.workspace?.goal.revision) ?? [];
  const task = props.learningTasks?.filter(task => task.learning_workspace_id === view.workspace?.learning_workspace_id && task.status !== 'unavailable' && progress.some(item => item.run_id === task.course_run_id)).sort((a, b) => b.selected_at - a.selected_at)[0];
  return progress.find(item => item.run_id === task?.course_run_id) ?? progress.find(item => item.status === 'active' || item.status === 'paused') ?? progress[0];
}
function statusLabel(progress?: CourseProgress) { return progress?.status === 'completed' ? uiText('已完成') : progress?.status === 'paused' ? uiText('已暂停') : progress?.status === 'skipped' ? uiText('已跳过') : progress ? uiText('进行中') : uiText('未开始'); }
function lessonAction(view: ReleaseView, lesson: CourseLessonSummary) {
  const progress = progressFor(view, lesson.id);
  return progress?.status === 'completed' || progress?.status === 'skipped' ? uiText('回看本课') : progress ? uiText('继续本课') : uiText('开始本课');
}
function units(view: ReleaseView) {
  const course = overview(view);
  const groups = (course?.units ?? []).map(unit => ({ ...unit, lessons: lessons(view).filter(lesson => lesson.unit_id === unit.id) })).filter(unit => unit.lessons.length);
  const remaining = lessons(view).filter(lesson => !groups.some(unit => unit.id === lesson.unit_id));
  if (remaining.length) groups.push({ id: 'remaining', title: uiText('课程目录'), lessons: remaining });
  return groups;
}
async function existingTarget(workspace: LearningWorkspace, release: SharedRelease, lessonId: string): Promise<CourseLaunchTarget> {
  const catalog = await inquiryCourse.catalog(workspace.learning_workspace_id);
  const lesson = (catalog.release_version === release.version ? catalog.course : release.manifest.inquiry_course)?.lessons.find(lesson => lesson.id === lessonId);
  if (!lesson) throw new Error(uiText('所选课程内容已变化，请刷新目录。'));
  const progress = catalog.progress.filter(item => item.lesson_id === lessonId && item.release_version === release.version && item.goal_revision === workspace.goal.revision);
  const task = props.learningTasks?.filter(item => item.learning_workspace_id === workspace.learning_workspace_id && item.status !== 'unavailable' && progress.some(run => run.run_id === item.course_run_id)).sort((a, b) => b.selected_at - a.selected_at)[0];
  const run = progress.find(item => item.run_id === task?.course_run_id) ?? progress.find(item => item.status === 'active' || item.status === 'paused') ?? progress[0];
  if (!run && release.version !== catalog.release_version) throw new Error(uiText('旧版本的这节课尚未留下学习记录。'));
  return { publication: lesson.source.publication, lessonId, ...(run ? { runId: run.run_id } : {}), ...(task?.chat_session_id ? { chatSessionId: task.chat_session_id } : {}) };
}
async function start(view: ReleaseView, lesson?: CourseLessonSummary) {
  if (busy.value || loading.value || !canStart(view, lesson)) return;
  const choice = settings(view), release = view.release, existing = view.workspace ?? workspaceFor(release.release_id);
  const sprint: ExamSprintInput | undefined = choice.sprint ? { exam_date: choice.examDate, syllabus: choice.syllabus, daily_minutes: choice.minutes || null, utc_offset_minutes: -new Date().getTimezoneOffset(), entry_mode: choice.entryMode } : undefined;
  const key = JSON.stringify([release.release_id, release.version, choice.goal.trim(), sprint]);
  if (!operations.has(key)) operations.set(key, crypto.randomUUID());
  const ticket = request; busy.value = true; error.value = '';
  try {
    if (existing) {
      const launch = lesson ? await existingTarget(existing, release, lesson.id) : undefined;
      if (ticket === request) emit('started', existing, undefined, !!launch, launch);
      return;
    }
    const value = sprint ? await examSprints.useCourse(release.release_id, release.version, operations.get(key)!, choice.goal.trim(), sprint)
      : await sharedWorkspaces.use(release.release_id, release.version, operations.get(key)!, choice.goal.trim());
    if (ticket === request) {
      const workspace = { ...value.workspace, shared_release: { release_id: value.instance.release_id, version: value.instance.version } };
      prepared.value[release.release_id] = workspace; view.workspace = workspace; view.release = value.instance.release;
      const chosen = lesson ? value.instance.release.manifest.inquiry_course?.lessons.find(item => item.id === lesson.id) : undefined;
      if (lesson && !chosen) throw new Error(uiText('所选课程内容已变化，请刷新目录。'));
      const launch = chosen ? { publication: chosen.source.publication, lessonId: chosen.id } : undefined;
      emit('started', workspace, sprint ? sprintRequest(choice.entryMode, choice.question) : undefined, !!launch, launch);
    }
  } catch (failure) { fail(failure, ticket); }
  finally { if (ticket === request) busy.value = false; }
}
function readBook(publication: PublishedBookRef, workspaceId?: string) { emit('reading', publication, workspaceId); }
function showUnits() {
  const unit = catalogElement.value?.querySelector<HTMLElement>('.course-unit');
  unit?.scrollIntoView({ block: 'start' });
  unit?.querySelector<HTMLElement>('h4')?.focus({ preventScroll: true });
}
function refresh() { return target.value ? loadDetail(detailPage.value?.offset ?? 0) : load(page.value?.offset ?? 0); }
watch(target, () => { if (target.value) void loadDetail(); else { ++request; pendingRead?.abort(); loading.value = false; error.value = ''; views.value = []; if (!page.value) void load(); } });
watch(() => props.workspaces?.map(workspace => `${workspace.learning_workspace_id}:${workspace.shared_release?.version}:${workspace.goal.revision}:${workspace.archived}`).join('|'), () => { if (target.value) void loadDetail(detailPage.value?.offset ?? 0); });
watch(() => `${network.value.epoch}:${network.value.identity?.user_id}`, () => { page.value = null; prepared.value = {}; options.value = {}; operations.clear(); busy.value = false; if (target.value) void loadDetail(); else void load(); }, { immediate: true });
onBeforeUnmount(() => { request++; pendingRead?.abort(); });
</script>

<template>
  <section ref="catalogElement" class="shared-catalog" :aria-label="uiText('书籍与专题课程')" :aria-busy="loading || busy">
    <div class="heading"><div><span class="eyebrow">{{ uiText('学习') }}</span><h2>{{ target ? detailTitle : uiText('读一本书，或走完一套课程') }}</h2><p v-if="!target">{{ uiText('先找到想学的内容，再从一个单元、一节课开始。') }}</p></div><button :disabled="loading || busy" @click="refresh">{{ uiText('刷新目录') }}</button></div>
    <button v-if="target" class="back-link" :disabled="busy" @click="target = null"><ArrowLeft :size="16" />{{ uiText('返回学习首页') }}</button>
    <p v-if="error" class="catalog-error" role="alert">{{ error }} <button :disabled="loading || busy" @click="refresh">{{ uiText('重试') }}</button></p><p v-if="loading" role="status">{{ uiText('正在读取学习目录…') }}</p>
    <template v-if="target && !loading">
      <div v-if="selectedBook" class="book-home-intro"><div class="detail-cover"><MaterialCover :title="detailTitle" :cover="materialBook(selectedBook)?.cover" /></div><div><span class="eyebrow">{{ uiText('书籍学习首页') }}</span><p>{{ uiText('先读原文，也可以选择已提供的单元开始学习。') }}</p><div class="book-home-actions"><button v-if="lessonCount" class="primary browse-lessons" :disabled="busy" @click="showUnits">{{ uiText('跟着课程学') }}<ArrowRight :size="16" /></button><button :class="{primary: !lessonCount}" :disabled="busy" @click="readBook(selectedBook, target?.learningWorkspaceId)"><BookOpen :size="16" />{{ uiText('阅读原文') }}</button></div></div></div>
      <button v-else-if="lessonCount" class="primary browse-lessons" :disabled="busy" @click="showUnits">{{ uiText('选择一课开始') }}<ArrowRight :size="16" /></button>
      <details v-if="bookVersions.length > 1" class="book-versions"><summary>{{ uiText('已提供的书籍版本') }} · {{ bookVersions.length }}</summary><div v-for="publication in bookVersions" :key="sharedBookKey(publication)"><code>{{ publication.publication_id }}</code><button :disabled="busy" @click="readBook(publication)">{{ uiText('阅读此版本') }}</button></div></details>
      <div v-if="lessonCount" class="collection-progress"><span>{{ selectedBook ? uiText('当前目录：{0} 条课程路线 · {1} 节课', [courseCount, lessonCount]) : uiText('共 {0} 节课', [lessonCount]) }}</span><span>{{ hasProgress ? uiText('已完成 {0} / {1} 节', [completedCount, lessonCount]) : uiText('尚未开始') }}</span><progress v-if="hasProgress" :max="lessonCount" :value="completedCount" :aria-label="uiText('当前目录学习进度')" /></div>
      <p v-if="selectedBook" class="scope-note">{{ uiText('本书已提供的课程') }}</p><p v-if="selectedBook && !views.length && !error" class="empty-catalog">{{ uiText('当前页尚未提供本书的单元课，可以先阅读原文。') }}</p>
      <article v-for="view in views" :key="`${view.release.release_id}:${view.release.version}`" class="release-section">
        <header class="release-heading"><div><span class="eyebrow">{{ lessons(view).length ? (selectedBook ? uiText('本书课程') : uiText('整套课程')) : uiText('资料与建议路线') }}</span><h3 v-if="selectedBook">{{ view.release.manifest.name }}</h3></div><small>{{ uiText('第 {0} 版', [view.release.version]) }}<span v-if="view.workspace"> · {{ uiText('你的学习版本') }}</span></small></header>
        <p v-if="view.catalog && view.catalog.release_version !== view.release.version" class="scope-note">{{ uiText('正在查看原课程版本。') }} <button :disabled="busy" @click="target = {kind: 'course', releaseId: view.release.release_id, version: view.catalog.release_version, learningWorkspaceId: view.workspace?.learning_workspace_id}">{{ uiText('查看当前学习版本') }}</button></p><div class="release-goal"><span class="eyebrow">{{ uiText('学习目标') }}</span><p class="accepted-goal">{{ view.workspace?.goal.text ?? settings(view).goal }}</p><p v-if="view.workspace" class="scope-note">{{ uiText('接着已保存的进度学习。进入后可以调整目标或备考设置。') }}</p><p v-else class="scope-note">{{ uiText('选择一课开始，即按这个目标学习；作答和进度会为你保存。') }}</p></div>
        <p v-if="view.workspace?.archived" class="status">{{ uiText('原学习工作区已归档，可恢复后继续') }}</p><p v-else-if="view.release.availability.status !== 'ready' || view.release.archived" class="status">{{ availabilityLabel(view.release) }}</p>
        <details v-if="!view.workspace" class="goal-options"><summary>{{ uiText('调整目标或准备考试') }}</summary><form @submit.prevent>
          <label>{{ uiText('你的学习目标') }}<textarea v-model="settings(view).goal" required :disabled="busy || view.release.archived" /></label><label><input v-model="settings(view).sprint" type="checkbox" :disabled="busy" />{{ uiText('用于考试冲刺') }}</label>
          <template v-if="settings(view).sprint"><label>{{ uiText('考试日期') }}<input v-model="settings(view).examDate" type="date" required :disabled="busy" /></label><label>{{ uiText('已知考试范围') }}<textarea v-model="settings(view).syllabus" :disabled="busy" :placeholder="uiText('老师划定的章节、题型或考纲，暂无可留空。')" /></label><label>{{ uiText('每天可用分钟数（选填）') }}<input v-model.number="settings(view).minutes" type="number" min="1" max="1440" :disabled="busy" /></label><fieldset :disabled="busy"><legend>{{ uiText('从哪里开始') }}</legend><label><input v-model="settings(view).entryMode" type="radio" value="diagnostic" />{{ uiText('先做短诊断') }}</label><label><input v-model="settings(view).entryMode" type="radio" value="question" />{{ uiText('直接提问，跳过诊断') }}</label></fieldset><label v-if="settings(view).entryMode === 'question'">{{ uiText('第一道问题（选填）') }}<textarea v-model="settings(view).question" :disabled="busy" :placeholder="uiText('可以现在输入，或进入阅读后选中原文提问。')" /></label></template>
        </form></details>
        <p v-if="!view.workspace && settings(view).sprint && !settings(view).examDate" class="status">{{ uiText('请先填写考试日期。') }}</p>
        <div v-if="!selectedBook && lessons(view).length" class="associated-books"><span class="eyebrow">{{ uiText('关联书籍与资料') }}</span><button v-for="material in view.release.manifest.materials" :key="sharedBookKey(material.published_book_ref)" :disabled="busy" @click="readBook(material.published_book_ref, view.workspace?.learning_workspace_id)">{{ sharedBookLabel(material.published_book_ref, books, view.release.manifest.materials) }}<BookOpen :size="14" /></button></div>
        <section v-for="(unit, unitIndex) in units(view)" :key="unit.id" class="course-unit"><header><span>{{ String(unitIndex + 1).padStart(2, '0') }}</span><h4 tabindex="-1">{{ unit.title }}</h4><small>{{ uiText('{0} 节课', [unit.lessons.length]) }}</small></header><ol><li v-for="lesson in unit.lessons" :key="lesson.id" class="lesson-row" :data-lesson="lesson.id"><div class="lesson-copy"><span class="lesson-status">{{ statusLabel(progressFor(view, lesson.id)) }}</span><h5>{{ lesson.question }}</h5><p>{{ lesson.objective }}</p><dl><div><dt>{{ uiText('建议基础') }}</dt><dd>{{ lesson.prerequisites || view.release.manifest.prerequisites }}</dd></div><div><dt>{{ uiText('预计投入') }}</dt><dd>{{ uiText('约 {0} 分钟', [lesson.estimated_minutes]) }}</dd></div></dl><details v-if="lesson.duration_basis" class="lesson-duration"><summary>{{ uiText('课时说明') }}</summary><p>{{ lesson.duration_basis }}</p></details></div><div class="lesson-actions"><small v-if="view.catalog && view.catalog.release_version !== view.release.version && !progressFor(view, lesson.id)">{{ uiText('旧版本的这节课尚未留下学习记录。') }}</small><button class="primary" :disabled="busy || !canStart(view, lesson)" @click="start(view, lesson)">{{ lessonAction(view, lesson) }}<ArrowRight :size="16" /></button><button :disabled="busy" @click="readBook(lesson.source.publication, view.workspace?.learning_workspace_id)">{{ uiText('阅读本课原文') }}</button></div></li></ol></section>
        <button v-if="!lessons(view).length" class="primary material-use" :disabled="busy || !canStart(view)" @click="start(view)">{{ view.workspace ? uiText('继续已有学习') : uiText('使用这些资料') }}</button>
        <details class="course-details" :open="view.release.availability.status !== 'ready'"><summary>{{ uiText('课程与资料说明') }}</summary><SharedWorkspaceManifest :release="view.release" :books="books" /></details>
      </article>
      <section v-if="related.length" class="related-courses"><h3>{{ uiText('关联专题课程') }}</h3><p class="scope-note">{{ uiText('这些课程结合本书与其他资料，进度属于各自的整套课程。') }}</p><button v-for="release in related" :key="release.release_id" :disabled="busy" @click="target = courseTarget(release)"><span><strong>{{ release.manifest.name }}</strong><small>{{ release.manifest.public_goal }}</small></span><ArrowRight :size="17" /></button></section>
      <nav v-if="detailPage && detailPage.total > detailPage.limit" class="pagination" :aria-label="uiText('本书课程分页')"><button :disabled="busy || !detailPage.offset" @click="loadDetail(Math.max(0, detailPage.offset - detailPage.limit))">{{ uiText('上一页') }}</button><span>{{ detailPage.offset + 1 }}–{{ detailPage.offset + detailPage.items.length }} / {{ detailPage.total }}</span><button :disabled="busy || detailPage.offset + detailPage.limit >= detailPage.total" @click="loadDetail(detailPage.offset + detailPage.limit)">{{ uiText('下一页') }}</button></nav>
    </template>
    <template v-else-if="!target && page && !loading">
      <p v-if="page.total > page.limit" class="scope-note">{{ uiText('课程数量按当前展示页统计，翻页可继续发现其他路线。') }}</p>
      <div class="collection-list">
        <LearningCatalogCard v-for="collection in collections" :key="collection.kind === 'book' ? collection.bookId : collection.release.release_id"
          :data-kind="collection.kind" :kind="collection.kind === 'book' ? uiText('书籍') : uiText('专题课程')"
          :title="collection.kind === 'book' ? bookLabel(collection.book.published_book_ref) : collection.release.manifest.name"
          :cover="collection.kind === 'book' ? collection.book.cover : undefined"
          :description="collection.kind === 'course' ? collection.release.manifest.public_goal : collection.releases.length ? uiText('已提供 {0} 条课程路线 · {1} 节课', [collection.releases.length, collection.releases.reduce((count, release) => count + (release.manifest.inquiry_course?.lessons.length ?? 0), 0)]) : uiText('从原文开始，带着问题阅读。')"
          :action="collection.kind === 'book' ? uiText('查看这本书') : uiText('查看整套课程')" :disabled="busy" @open="openCollection(collection)" />
      </div>
      <section v-if="materialReleases.length" class="material-routes"><h3>{{ uiText('资料与建议路线') }}</h3><div v-for="release in materialReleases" :key="release.release_id"><div><strong>{{ release.manifest.name }}</strong><p>{{ release.manifest.public_goal }}</p></div><button :disabled="busy" @click="target = courseTarget(release)">{{ uiText('查看资料与路线') }}</button></div></section>
      <p v-if="!collections.length && !materialReleases.length" class="empty-catalog">{{ uiText('暂无可用书籍或专题课程。可以先添加自己的资料。') }}</p>
      <nav v-if="page.total > page.limit" class="pagination" :aria-label="uiText('公开学习目录分页')"><button :disabled="busy || !page.offset" @click="load(Math.max(0, page.offset - page.limit))">{{ uiText('上一页') }}</button><span>{{ page.offset + 1 }}–{{ page.offset + page.items.length }} / {{ page.total }}</span><button :disabled="busy || page.offset + page.limit >= page.total" @click="load(page.offset + page.limit)">{{ uiText('下一页') }}</button></nav>
    </template>
  </section>
</template>

<style scoped>
.shared-catalog{margin:20px 0;padding:28px;border:1px solid var(--line);border-radius:16px;background:var(--canvas);color:var(--ink)}.heading,.release-heading{display:flex;gap:20px;align-items:center;justify-content:space-between}.heading>div,.release-heading>div{min-width:0}.heading>button{flex:none}h2,h3,h4,h5{margin:0;font-weight:500;overflow-wrap:anywhere}h2{font:500 25px/1.5 var(--serif,var(--sans))}h3{font-size:19px;line-height:1.5}h4{font-size:17px}h5{font-size:16px;line-height:1.6}p{line-height:1.8;overflow-wrap:anywhere}.heading p,.status,.empty-catalog,.scope-note,.release-heading small{color:var(--muted);font-size:13px}.eyebrow{display:block;margin-bottom:7px;color:var(--brand-accent-deep);font-size:12px;letter-spacing:.03em}button{display:inline-flex;align-items:center;justify-content:center;gap:8px;font:inherit;border:1px solid var(--line);border-radius:8px;color:var(--ink);background:var(--canvas);padding:10px 14px;min-height:42px;cursor:pointer}button:disabled{opacity:.5;cursor:default}button.primary{background:var(--brand-accent);border-color:var(--brand-accent);color:var(--brand-on-accent)}.back-link{border:0;padding-inline:0;margin:10px 0 18px;color:var(--steel);font-size:13px}.collection-list{display:grid;grid-template-columns:repeat(auto-fit,minmax(min(100%,320px),1fr));gap:18px;margin-top:24px}.book-home-intro{display:flex;align-items:center;gap:28px;padding:22px 26px;background:var(--surface-soft);border-radius:12px}.detail-cover{width:78px;height:116px;flex:none}.book-home-intro p{font-size:14px}.book-home-actions{display:flex;flex-wrap:wrap;gap:10px}.book-home-actions button{font-size:13px}.course-unit{scroll-margin-top:20px}.book-versions>div{display:flex;justify-content:space-between;gap:12px;padding:10px 0}.book-versions code{font-size:11px;overflow-wrap:anywhere;align-self:center}.book-versions button{flex:none;font-size:12px}.collection-progress{display:flex;flex-wrap:wrap;align-items:center;justify-content:space-between;gap:12px;margin:24px 0 8px;font-size:13px}.collection-progress progress{height:6px;width:100%;accent-color:var(--brand-accent)}.scope-note{margin:10px 0 18px}.release-section{margin-top:28px;padding-top:24px;border-top:1px solid var(--line)}.release-goal{margin-top:18px;max-width:820px}.accepted-goal{font-size:15px;margin:0}.goal-options{padding:0 14px;border:1px solid var(--line);border-radius:8px;background:var(--surface-soft)}.goal-options label{display:block;margin:12px 0;font-size:13px;line-height:1.8}.goal-options textarea{display:block;box-sizing:border-box;width:100%;min-height:75px;margin-top:6px;padding:10px;border:1px solid var(--line);border-radius:7px;background:var(--canvas);color:var(--ink);font:inherit}.goal-options input:not([type=checkbox]):not([type=radio]){display:block;max-width:100%;padding:7px;border:1px solid var(--line);border-radius:6px;background:var(--canvas);color:var(--ink);font:inherit}.goal-options input[type=checkbox],.goal-options input[type=radio]{margin-inline-end:7px}.goal-options fieldset{min-width:0;border:1px solid var(--line);border-radius:8px}.associated-books{display:flex;flex-wrap:wrap;align-items:center;gap:8px;margin:20px 0}.associated-books>.eyebrow{width:100%}.associated-books button{font-size:12px}.course-unit{margin-top:24px}.course-unit>header{display:flex;align-items:baseline;gap:12px;margin-bottom:10px}.course-unit>header>span{color:#b4986c;font:400 22px/1.4 var(--mono)}.course-unit>header>small{margin-left:auto;color:var(--muted)}.course-unit ol{list-style:none;padding:0;margin:0;border:1px solid var(--line);border-radius:12px;overflow:hidden}.lesson-row{display:flex;align-items:center;gap:24px;padding:20px 22px;border-bottom:1px solid var(--line)}.lesson-row:last-child{border-bottom:0}.lesson-copy{flex:1;min-width:0}.lesson-status{font-size:11px;color:var(--steel)}.lesson-copy h5{margin-top:5px}.lesson-copy p{font-size:13px;margin:8px 0;color:var(--muted)}dl{margin:10px 0 0;display:grid;gap:5px;font-size:12px}dl>div{display:flex;align-items:baseline;gap:10px}dt{flex:none;color:var(--muted)}dd{margin:0;line-height:1.65;overflow-wrap:anywhere}.lesson-actions{flex:none;display:flex;flex-direction:column;gap:8px}.lesson-actions button{font-size:12px}.lesson-actions button:not(.primary){border-color:transparent;color:var(--steel)}details{margin-top:16px}summary{padding:10px 0;font-size:13px;cursor:pointer}.course-details{border-top:1px solid var(--line);margin-top:20px}.material-use{margin-top:16px}.material-routes,.related-courses{margin-top:28px;padding-top:22px;border-top:1px solid var(--line)}.material-routes>div{display:flex;gap:20px;justify-content:space-between;align-items:center;padding:18px 0;border-bottom:1px solid var(--line)}.material-routes strong{font-weight:500;font-size:14px}.material-routes p{color:var(--muted);font-size:12px;margin:6px 0}.material-routes button{flex:none;font-size:12px}.related-courses>button{display:flex;justify-content:space-between;width:100%;margin-top:10px;text-align:left}.related-courses strong,.related-courses small{display:block;overflow-wrap:anywhere}.related-courses small{color:var(--muted);font-size:12px;font-weight:400;margin-top:6px}.pagination{display:flex;flex-wrap:wrap;gap:10px;align-items:center;margin-top:22px;font-size:13px}.catalog-error{color:var(--danger,#a33e25)}
.lesson-duration{margin-top:4px}.lesson-duration summary{color:var(--steel);font-size:12px}.lesson-duration p{white-space:pre-line}
@media(max-width:620px){.shared-catalog{padding:18px}.heading{align-items:flex-start;flex-direction:column;gap:10px}.heading>div{width:100%}.heading>button{font-size:12px;padding:8px 10px}h2{font-size:22px}.book-home-intro{padding:20px;gap:20px}.detail-cover{width:62px;height:94px}.book-home-intro p{font-size:13px}.release-heading{align-items:flex-start;flex-direction:column;gap:8px}.lesson-row{align-items:stretch;flex-direction:column;gap:16px;padding:18px}.lesson-actions{flex-direction:row;flex-wrap:wrap}.lesson-actions button{flex:1}.goal-options textarea{font-size:16px}.material-routes>div{align-items:flex-start;flex-direction:column;gap:10px}.book-versions>div{align-items:flex-start;flex-direction:column}.course-unit>header{gap:8px}.course-unit>header>small{white-space:nowrap}}
</style>
