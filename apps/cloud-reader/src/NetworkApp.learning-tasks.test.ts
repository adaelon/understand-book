// @vitest-environment happy-dom
import { flushPromises, mount } from '@vue/test-utils';
import { defineComponent } from 'vue';
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import { installIdentity, network, workspaceStorageKey, type LearningWorkspace, type NetworkWorkspace, type PublishedBookRef } from './network-context';
import { saveLearningSelection } from '@understand-book/web/reader/learning-workspace-state';
import type { CourseLaunchTarget, LearningTaskView } from '@understand-book/web/reader/learning-tasks';
import { learningCollectionUrl, readLearningCollection, type LearningCollectionTarget } from '@understand-book/web/reader/learning-collections';
import type { SharedRelease } from '@understand-book/web/reader/shared-workspaces';
import SharedWorkspaceCatalog from './components/SharedWorkspaceCatalog.vue';

type ReaderLocation = { publication: PublishedBookRef | undefined; learningId: string | null | undefined };
const observation = vi.hoisted(() => ({
  resumed: [] as { task: LearningTaskView; location: ReaderLocation }[],
  courses: [] as { target: Omit<CourseLaunchTarget, 'publication'>; location: ReaderLocation }[],
  reading: [] as ReaderLocation[],
  resumeFailures: 0,
  courseFailures: 0,
  resumePending: null as Promise<void> | null,
}));
vi.mock('@understand-book/web/reader/App.vue', () => ({ default: defineComponent({
  name: 'App', props: ['readerVisible', 'learningWorkspace', 'hostOpening'], emits: ['show-library', 'learning-collection', 'course-launch'],
  methods: {
    beforeNoteLeave: async () => true,
    closeBoard() {},
    closeUnderstanding() {},
    async openReading() {
      observation.reading.push({ publication: network.value.workspace ? { ...network.value.workspace.published_book_ref } : undefined,
        learningId: network.value.workspace?.learning_workspace_id });
    },
    async resumeLearningTask(task: LearningTaskView) {
      observation.resumed.push({ task: JSON.parse(JSON.stringify(task)), location: {
        publication: network.value.workspace ? { ...network.value.workspace.published_book_ref } : undefined,
        learningId: network.value.workspace?.learning_workspace_id,
      } });
      if (observation.resumeFailures > 0) {
        observation.resumeFailures--;
        throw new Error('原学习任务暂时无法恢复，请重试。');
      }
      await observation.resumePending;
    },
    async openCourse(target: Omit<CourseLaunchTarget, 'publication'>) {
      observation.courses.push({ target: JSON.parse(JSON.stringify(target)), location: {
        publication: network.value.workspace ? { ...network.value.workspace.published_book_ref } : undefined,
        learningId: network.value.workspace?.learning_workspace_id,
      } });
      if (observation.courseFailures > 0) {
        observation.courseFailures--;
        throw new Error('下一课暂时无法打开，请重试。');
      }
    },
  },
  template: '<div class="test-task-reader"><slot name="navigation"/><button @click="$emit(\'show-library\')">资料</button></div>',
}) }));
import NetworkApp from './NetworkApp.vue';

const identity = { user_id: 'learning-task-reader', csrf_token: 'csrf' };
const firstBook = { book_id: '基础概念', publication_id: 'foundation-v1' };
const secondBook = { book_id: '应用实验', publication_id: 'application-v2' };
const books = [firstBook, secondBook].map(published_book_ref => ({ published_book_ref, title: published_book_ref.book_id }));
type RecordedRequest = { method: string; url: string; body: Record<string, unknown> };
let requests: RecordedRequest[];
let unexpected: string[];
let spaces: LearningWorkspace[];
let tasks: LearningTaskView[];
let tasksUnavailable: boolean;
let savedReading: boolean;
let workspace: NetworkWorkspace;
let wrapper: ReturnType<typeof mount> | undefined;
let host: HTMLDivElement | undefined;

function learningSpace(id = 'original-learning'): LearningWorkspace {
  return {
    learning_workspace_id: id, goal: { text: '从概念到应用', constraints: [], revision: 1, created_at: 1 },
    goal_history: [], members: [firstBook, secondBook].map(published_book_ref => ({ published_book_ref, readable: true, added_at: 1 })),
    archived: false, created_at: 1, updated_at: 1,
  };
}
function learningTask(kind: LearningTaskView['kind'] = 'course'): LearningTaskView {
  return {
    learning_workspace_id: 'original-learning', published_book_ref: secondBook,
    course_run_id: kind === 'course' ? 'original-course-run' : null, chat_session_id: 'original-chat',
    selected_at: 20, kind, title: '用第二份材料解释实验现象', status: 'paused',
    lesson_id: kind === 'course' ? 'application-lesson' : null, stage: kind === 'course' ? 'explanation' : null,
    closed_book_id: kind === 'closed_book' ? 'original-explanation-practice' : null,
    question: '这次观察如何支持你的判断？', message: null,
  };
}

function originalCourse(version = 2): SharedRelease {
  const availability = { status: 'ready' as const, materials: [firstBook, secondBook].map(published_book_ref => ({
    published_book_ref, available_capabilities: ['text', 'tutor'], missing_capabilities: [], publication_available: true,
  })), audience: [{ user_id: identity.user_id, ready: true, missing_grants: [] }] };
  return {
    release_id: 'original-course-release', version, latest_version: 5, archived: false, created_at: 1,
    readiness_at_publication: availability, availability,
    manifest: { name: version === 2 ? '从概念到应用的完整路线' : '新版路线：更新后的课程', publisher: '课程组', public_goal: '通过实验理解原理',
      applicability: '自主学习', prerequisites: '基础阅读', topic_links: [], suggested_route: [],
      materials: [firstBook, secondBook].map(published_book_ref => ({ published_book_ref, required_capabilities: ['text', 'tutor'] })),
      inquiry_course: { language: 'zh-CN', units: [{ id: 'observation', title: '观察与解释' }], routes: [],
        lessons: [firstBook, secondBook].map((publication, index) => ({ id: index ? 'application-lesson' : 'foundation-lesson',
          unit_id: 'observation', question: index ? '如何解释实验现象？' : '怎样认识基本概念？', objective: '用证据解释',
          prerequisites: '基础阅读', estimated_minutes: 20, duration_basis: '阅读和解释',
          source: { publication, source_revision: '1', start_lid: '1.1', end_lid: '1.8', label: '学习材料', structure_refs: [] },
        })),
      },
    },
  };
}

function courseContext(): NonNullable<LearningTaskView['course_context']> {
  return { release_id: 'original-course-release', release_version: 2, collection_kind: 'course',
    collection_title: '从概念到应用的完整路线', unit_title: '观察与解释', lesson_number: 2, lesson_count: 2 };
}

beforeEach(() => {
  sessionStorage.clear(); localStorage.clear(); history.replaceState({}, '', '/');
  requests = []; unexpected = []; spaces = [learningSpace()]; tasks = [learningTask()]; tasksUnavailable = false; savedReading = false;
  observation.resumed = []; observation.courses = []; observation.reading = []; observation.resumeFailures = 0; observation.courseFailures = 0;
  observation.resumePending = null;
  installIdentity(identity);
  saveLearningSelection('original-learning');
  sessionStorage.setItem(workspaceStorageKey(), JSON.stringify({ workspace_id: 'this-window', view: 'library' }));
  workspace = {
    workspace_id: 'this-window', generation: 3, revision: 8, selected_chat: 'unrelated-latest-chat',
    published_book_ref: firstBook, learning_workspace_id: 'original-learning', reader: { viewport: { top_lid: '1.16' } },
    learning_workspace_positions: { 'original-learning': firstBook },
  } as unknown as NetworkWorkspace;
  vi.stubGlobal('fetch', vi.fn(async (input: string, init: RequestInit = {}) => {
    const url = String(input), method = init.method ?? 'GET';
    const body = init.body ? JSON.parse(String(init.body)) : {};
    requests.push({ method, url, body });
    if (url === '/api/auth/me') return Response.json(identity);
    if (url === '/api/account/presence') return Response.json({ ok: true });
    if (url === '/api/library') return Response.json({ books });
    if (url === '/api/videos') return Response.json({ videos: [] });
    if (url === '/api/learning-workspaces' && method === 'GET') return Response.json({ workspaces: spaces });
    if (url === '/api/me/learning-tasks') return tasksUnavailable
      ? Response.json({ message: '学习任务读取暂时失败' }, { status: 503 }) : Response.json({ tasks });
    if (url.startsWith('/api/workspace-releases?')) return Response.json({ items: [], total: 0, limit: 12, offset: 0 });
    if (url === '/api/workspace-releases/original-course-release/versions/2') return Response.json(originalCourse());
    const scope = /^\/api\/learning-workspaces\/([^/]+)\/shared-release(?:\/course)?$/.exec(url)?.[1];
    const shared = spaces.find(space => space.learning_workspace_id === scope)?.shared_release;
    if (url.endsWith('/shared-release')) return Response.json({ instance: shared ? {
      release_id: shared.release_id, version: shared.version, latest_version: 5, release: originalCourse(shared.version), history: [{ version: 2, selected_at: 1 }],
    } : null });
    if (url.endsWith('/shared-release/course')) return Response.json(shared ? {
      release_id: shared.release_id, release_version: shared.version, course: originalCourse(shared.version).manifest.inquiry_course,
      progress: [{ run_id: 'original-course-run', release_id: shared.release_id, release_version: 2, lesson_id: 'application-lesson',
        revision: 4, goal_revision: 1, stage: 'explanation', status: 'paused' }],
    } : { course: null, progress: [], release_version: null });
    if (url.endsWith('/exam-sprint')) return Response.json({ sprint: null });
    if (url.endsWith('/preparations')) return Response.json({ items: [] });
    if (url === '/api/account/storage') return Response.json({ items: [], can_upload: false, available_bytes: 0, used_bytes: 0,
      reserved_bytes: 0, total_bytes: 0, limit_bytes: null, max_file_bytes: null, over_limit: false, expires_at: null, retained_until: null });
    if (url === '/api/me/reading-resumption' || url === '/api/workspaces/this-window/resumption') return Response.json(savedReading ? {
      workspace_id: workspace.workspace_id, published_book_ref: workspace.published_book_ref, selected_chat: workspace.selected_chat,
      learning_workspace_id: workspace.learning_workspace_id, position_label: '段落 · 原阅读位置', position_excerpt: '这里是上次读到的内容。', last_question: '原阅读问题',
    } : null);
    if (url === '/api/workspaces/this-window' || url === '/api/workspaces/this-window/attach') return Response.json(workspace);
    if (url === '/api/workspaces/this-window/book/open') {
      workspace = { ...workspace, generation: workspace.generation + 1, revision: workspace.revision + 1,
        published_book_ref: body.published_book_ref as PublishedBookRef,
        learning_workspace_id: body.learning_workspace_id as string | null, selected_chat: null };
      return Response.json(workspace);
    }
    unexpected.push(`${method} ${url}`);
    throw new Error(`Unexpected ${method} ${url}`);
  }));
});

afterEach(async () => {
  wrapper?.unmount(); wrapper = undefined;
  host?.remove(); host = undefined;
  await flushPromises();
  vi.unstubAllGlobals(); vi.restoreAllMocks();
  sessionStorage.clear(); localStorage.clear();
  network.value = { ...network.value, enabled: false, identity: null, workspace: null };
  expect(unexpected).toEqual([]);
});

async function start() {
  network.value = { ...network.value, identity: null, workspace: null };
  host = document.createElement('div'); document.body.append(host);
  wrapper = mount(NetworkApp, { attachTo: host, global: { stubs: { MaterialCover: true, YeyeMascot: true, teleport: true } } });
  await flushPromises();
  expect(wrapper.find('.network-library').exists()).toBe(true);
}
function bookOpens() { return requests.filter(request => request.url.endsWith('/book/open')); }
function newInstances() { return requests.filter(request => request.method === 'POST' && (request.url === '/api/learning-workspaces' || request.url === '/api/workspaces' || request.url.endsWith('/use'))); }

it.each(['course', 'closed_book'] as const)('resumes the exact second material and saved %s task instead of the first member or latest chat', async kind => {
  tasks = [learningTask(kind)];
  await start();
  expect(wrapper!.get('.learning-home-task').text()).toContain(tasks[0].title);
  await wrapper!.get('.learning-home-task .task-primary').trigger('click');
  await flushPromises();

  expect(bookOpens()).toHaveLength(1);
  expect(bookOpens()[0].body).toMatchObject({ published_book_ref: secondBook, learning_workspace_id: 'original-learning' });
  expect(observation.resumed).toEqual([{ task: tasks[0], location: { publication: secondBook, learningId: 'original-learning' } }]);
  expect(observation.courses).toEqual([]);
  expect(newInstances()).toEqual([]);
  expect(wrapper!.find('.book-opening').exists()).toBe(false);
  expect(wrapper!.get('.test-task-reader').isVisible()).toBe(true);
});

it('keeps a failed task restoration actionable and retries the same original task without creating a new instance', async () => {
  observation.resumeFailures = 1;
  await start();
  await wrapper!.get('.learning-home-task .task-primary').trigger('click');
  await flushPromises();

  expect(wrapper!.get('.book-opening').attributes('role')).toBe('alert');
  expect(wrapper!.get('.book-opening').text()).toContain('原学习任务暂时无法恢复');
  expect(observation.resumed).toHaveLength(1);
  const retry = wrapper!.get('.book-opening').findAll('button').find(button => button.text() === '重试打开');
  expect(retry).toBeTruthy();
  await retry!.trigger('click');
  await flushPromises();

  expect(observation.resumed).toEqual([0, 1].map(() => ({ task: tasks[0], location: { publication: secondBook, learningId: 'original-learning' } })));
  expect(bookOpens().every(request => request.body.learning_workspace_id === 'original-learning' && JSON.stringify(request.body.published_book_ref) === JSON.stringify(secondBook))).toBe(true);
  expect(newInstances()).toEqual([]);
  expect(wrapper!.find('.book-opening').exists()).toBe(false);
});

it('opens on learning without forcing a private-workspace form and exposes learning in the main navigation', async () => {
  spaces = []; tasks = []; saveLearningSelection(null);
  await start();
  expect(wrapper!.get('.learning-home-task').text()).toContain('看看可以学什么');
  expect(wrapper!.get('.shared-catalog').isVisible()).toBe(true);
  expect(wrapper!.get('.learning-create').isVisible()).toBe(false);
  expect(wrapper!.get('.network-primary-nav [aria-label="学习"]').attributes('aria-current')).toBe('page');

  await wrapper!.get('.network-primary-nav [aria-label="学习资料"]').trigger('click');
  await flushPromises();
  expect(wrapper!.find('.learning-home-task').exists()).toBe(false);
  await wrapper!.get('.network-primary-nav [aria-label="学习"]').trigger('click');
  await flushPromises();
  expect(new URLSearchParams(location.search).get('view')).toBe('learning');
  expect(wrapper!.get('.shared-catalog').isVisible()).toBe(true);
  expect(wrapper!.get('.learning-create').isVisible()).toBe(false);
  expect(newInstances()).toEqual([]);
});

it('launches a shared course from the emitted fourth target and new workspace rather than the previously selected scope', async () => {
  tasks = [];
  await start();
  const catalog = wrapper!.getComponent(SharedWorkspaceCatalog);
  expect(catalog.props('workspaces')).toEqual(spaces);
  const created = learningSpace('new-course-instance');
  const target: CourseLaunchTarget = { publication: secondBook, lessonId: 'second-material-lesson', runId: 'continued-course-run', chatSessionId: 'continued-course-chat' };
  spaces = [...spaces, created];
  catalog.vm.$emit('started', created, undefined, true, target);
  await flushPromises();

  expect(bookOpens()).toHaveLength(1);
  expect(bookOpens()[0].body).toMatchObject({ published_book_ref: secondBook, learning_workspace_id: created.learning_workspace_id });
  expect(observation.courses).toEqual([{
    target: { lessonId: target.lessonId, runId: target.runId, chatSessionId: target.chatSessionId },
    location: { publication: secondBook, learningId: created.learning_workspace_id },
  }]);
  expect(observation.resumed).toEqual([]);
  expect(newInstances()).toEqual([]);
});

it('shows a retryable task-read failure without presenting an empty learning history, then resumes the recovered task', async () => {
  tasksUnavailable = true;
  await start();
  const home = wrapper!.get('.learning-home-task');
  expect(home.get('[role="alert"]').text()).toContain('学习任务暂时无法读取');
  expect(home.text()).not.toContain('选一本书，或开始一套课程');
  tasksUnavailable = false;
  await home.get('[role="alert"] button').trigger('click');
  await flushPromises();

  expect(wrapper!.find('.learning-home-task [role="alert"]').exists()).toBe(false);
  expect(wrapper!.get('.learning-home-task').text()).toContain(tasks[0].title);
  expect(requests.filter(request => request.url === '/api/me/learning-tasks')).toHaveLength(2);
  await wrapper!.get('.learning-home-task .task-primary').trigger('click');
  await flushPromises();
  expect(observation.resumed[0].task).toEqual(tasks[0]);
  expect(newInstances()).toEqual([]);
});

it('retains the known task when a foreground refresh fails and replaces it only after an explicit successful retry', async () => {
  await start();
  const original = tasks[0];
  tasksUnavailable = true;
  window.dispatchEvent(new Event('focus'));
  await flushPromises();
  expect(wrapper!.get('.learning-home-task').text()).toContain(original.title);
  expect(wrapper!.get('.learning-home-task [role="alert"]').text()).toContain('学习任务暂时无法读取');
  tasks = [{ ...original, title: '实验反馈后的下一步', selected_at: 30 }];
  tasksUnavailable = false;
  await wrapper!.get('.learning-home-task [role="alert"] button').trigger('click');
  await flushPromises();
  expect(wrapper!.get('.learning-home-task').text()).toContain(tasks[0].title);
  expect(wrapper!.get('.learning-home-task').text()).not.toContain(original.title);
  expect(wrapper!.find('.learning-home-task [role="alert"]').exists()).toBe(false);
});

const collectionTargets: LearningCollectionTarget[] = [
  { kind: 'book', publication: secondBook, learningWorkspaceId: 'original-learning' },
  { kind: 'course', releaseId: 'original-course-release', version: 2, learningWorkspaceId: 'original-learning' },
];

it.each(collectionTargets)('browses an explicit $kind collection URL after refresh without restoring the old reader or creating an instance', async target => {
  if (target.kind === 'course') spaces[0]!.shared_release = { release_id: target.releaseId, version: target.version };
  savedReading = true;
  sessionStorage.setItem(workspaceStorageKey(), JSON.stringify({ workspace_id: 'this-window', view: 'reader' }));
  history.replaceState({}, '', learningCollectionUrl(location.href, target));
  await start();

  expect(wrapper!.getComponent(SharedWorkspaceCatalog).props('target')).toEqual(target);
  expect(wrapper!.find('.shared-catalog [role="alert"]').exists()).toBe(false);
  expect(wrapper!.get('.shared-catalog .heading h2').text()).toBe(target.kind === 'book' ? '应用实验' : '从概念到应用的完整路线');
  expect(readLearningCollection(location.search)).toEqual(target);
  expect(new URLSearchParams(location.search).get('view')).toBe('learning');
  expect(wrapper!.find('.test-task-reader').exists()).toBe(false);
  expect(wrapper!.find('.learning-home-task').exists()).toBe(false);
  expect(requests.some(request => request.url === '/api/workspaces/this-window/resumption')).toBe(true);
  expect(requests.filter(request => request.method === 'POST' && request.url !== '/api/account/presence')).toEqual([]);
  expect(bookOpens()).toEqual([]);
  expect(newInstances()).toEqual([]);
  expect(observation.resumed).toEqual([]);
  expect(observation.courses).toEqual([]);
});

it.each(collectionTargets)('opens the current task’s original $kind collection without resuming or replacing the task', async target => {
  // The private instance can advance while an older lesson still belongs to its original release.
  if (target.kind === 'course') spaces[0]!.shared_release = { release_id: target.releaseId, version: 5 };
  tasks = [{ ...learningTask(target.kind === 'book' ? 'conversation' : 'course'),
    ...(target.kind === 'course' ? { course_context: courseContext() } : {}),
  }];
  const original = structuredClone(tasks[0]);
  await start();
  await wrapper!.get('.learning-home-task .task-collection').trigger('click');
  await flushPromises();

  expect(readLearningCollection(location.search)).toEqual(target);
  expect(wrapper!.getComponent(SharedWorkspaceCatalog).props('target')).toEqual(target);
  expect(wrapper!.find('.shared-catalog [role="alert"]').exists()).toBe(false);
  expect(wrapper!.get('.shared-catalog .heading h2').text()).toBe(target.kind === 'book' ? '应用实验' : '从概念到应用的完整路线');
  expect(wrapper!.find('.learning-home-task').exists()).toBe(false);
  expect(wrapper!.find('.test-task-reader').exists()).toBe(false);
  expect(bookOpens()).toEqual([]);
  expect(newInstances()).toEqual([]);
  expect(observation.resumed).toEqual([]);
  expect(observation.courses).toEqual([]);
  expect(requests.filter(request => request.method === 'POST' && request.url !== '/api/account/presence')).toEqual([]);
  expect(tasks).toEqual([original]);

  await wrapper!.get('.network-primary-nav [aria-label="学习"]').trigger('click');
  await flushPromises();
  expect(wrapper!.get('.learning-home-task').text()).toContain(original.title);
  expect(readLearningCollection(location.search)).toBe(null);
  expect(observation.resumed).toEqual([]);
});

it('retries a reader’s cross-book course launch with the original workspace, publication, lesson and saved chat', async () => {
  const other = learningSpace('other-learning');
  other.goal.text = '另一组学习资料';
  spaces.push(other);
  await start();
  await wrapper!.get('.learning-home-task .task-primary').trigger('click');
  await flushPromises();
  wrapper!.getComponent({ name: 'App' }).vm.$emit('show-library');
  await flushPromises();
  await wrapper!.findAll('.learning-space-select').find(button => button.text().includes(other.goal.text))!.trigger('click');
  await flushPromises();
  expect(wrapper!.findAll('.learning-space-select').find(button => button.attributes('aria-pressed') === 'true')!.text()).toContain(other.goal.text);
  await wrapper!.get('.network-current-reader').trigger('click');
  await flushPromises();
  expect(network.value.workspace?.learning_workspace_id).toBe('original-learning');
  const target: CourseLaunchTarget = { publication: firstBook, lessonId: 'next-foundation-lesson', runId: 'next-original-run', chatSessionId: 'next-original-chat' };
  observation.courseFailures = 1;
  wrapper!.getComponent({ name: 'App' }).vm.$emit('course-launch', target, 'original-learning');
  await flushPromises();

  expect(wrapper!.get('.book-opening[role="alert"]').text()).toContain('下一课暂时无法打开');
  const courseOpen = { target: { lessonId: target.lessonId, runId: target.runId, chatSessionId: target.chatSessionId },
    location: { publication: firstBook, learningId: 'original-learning' } };
  expect(observation.courses).toEqual([courseOpen]);
  expect(bookOpens()[1].body).toMatchObject({ published_book_ref: firstBook, learning_workspace_id: 'original-learning' });
  await wrapper!.get('.book-opening').findAll('button').find(button => button.text() === '重试打开')!.trigger('click');
  await flushPromises();

  expect(observation.courses).toEqual([courseOpen, courseOpen]);
  // The first attempt already switched materials; retry attaches that same reader before reopening the lesson.
  expect(bookOpens().slice(1).map(request => ({ publication: request.body.published_book_ref, learningId: request.body.learning_workspace_id })))
    .toEqual([courseOpen.location]);
  expect(observation.resumed).toHaveLength(1);
  expect(newInstances()).toEqual([]);
  expect(wrapper!.find('.book-opening').exists()).toBe(false);
  expect(wrapper!.get('.test-task-reader').isVisible()).toBe(true);
});

it('accepts a reader’s return-to-collection event as browsing the same original course version and scope', async () => {
  spaces[0]!.shared_release = { release_id: 'original-course-release', version: 2 };
  await start();
  await wrapper!.get('.learning-home-task .task-primary').trigger('click');
  await flushPromises();
  const target = collectionTargets[1]!;
  wrapper!.getComponent({ name: 'App' }).vm.$emit('learning-collection', target);
  await flushPromises();

  expect(wrapper!.get('.network-library').isVisible()).toBe(true);
  expect(wrapper!.getComponent(SharedWorkspaceCatalog).props('target')).toEqual(target);
  expect(readLearningCollection(location.search)).toEqual(target);
  expect(bookOpens()).toHaveLength(1);
  expect(observation.resumed).toHaveLength(1);
  expect(observation.courses).toEqual([]);
  expect(newInstances()).toEqual([]);
});

it('keeps reader interaction blocked until the restored task is saved, then accepts returning to its collection', async () => {
  spaces[0]!.shared_release = { release_id: 'original-course-release', version: 2 };
  let restored!: () => void;
  observation.resumePending = new Promise<void>(resolve => { restored = resolve; });
  await start();
  await wrapper!.get('.learning-home-task .task-primary').trigger('click');
  await flushPromises();

  const reader = wrapper!.getComponent({ name: 'App' });
  expect(observation.resumed).toHaveLength(1);
  expect(wrapper!.get('.book-opening').attributes('role')).toBe('status');
  expect(reader.props('hostOpening')).toBe(true);

  restored();
  await flushPromises();
  expect(reader.props('hostOpening')).toBe(false);
  expect(wrapper!.find('.book-opening').exists()).toBe(false);
  reader.vm.$emit('learning-collection', collectionTargets[1]);
  await flushPromises();
  expect(wrapper!.get('.network-library').isVisible()).toBe(true);
  expect(wrapper!.getComponent(SharedWorkspaceCatalog).props('target')).toEqual(collectionTargets[1]);
  expect(newInstances()).toEqual([]);
});

it('keeps cancellation outside the blocked reader and restores interaction without waiting for late task persistence', async () => {
  let restored!: () => void;
  observation.resumePending = new Promise<void>(resolve => { restored = resolve; });
  await start();
  await wrapper!.get('.learning-home-task .task-primary').trigger('click');
  await flushPromises();
  const reader = wrapper!.getComponent({ name: 'App' });
  const cancel = wrapper!.get('.book-opening button');
  expect(reader.props('hostOpening')).toBe(true);
  expect(cancel.text()).toBe('取消');
  expect(cancel.element.closest('.test-task-reader')).toBeNull();
  expect(cancel.attributes('disabled')).toBeUndefined();
  await cancel.trigger('click');
  await flushPromises();
  expect(reader.props('hostOpening')).toBe(false);
  expect(wrapper!.find('.book-opening').exists()).toBe(false);
  restored(); await flushPromises();
  expect(reader.props('hostOpening')).toBe(false);
  expect(newInstances()).toEqual([]);
});

it('opens original text from a book collection in the same workspace without entering a course', async () => {
  const target = collectionTargets[0]!;
  history.replaceState({}, '', learningCollectionUrl(location.href, target));
  await start();
  await wrapper!.get('.book-home-intro .primary').trigger('click');
  await flushPromises();

  expect(bookOpens()).toHaveLength(1);
  expect(bookOpens()[0].body).toMatchObject({ published_book_ref: secondBook, learning_workspace_id: 'original-learning' });
  expect(observation.reading).toEqual([{ publication: secondBook, learningId: 'original-learning' }]);
  expect(observation.resumed).toEqual([]);
  expect(observation.courses).toEqual([]);
  expect(newInstances()).toEqual([]);
  expect(wrapper!.get('.test-task-reader').isVisible()).toBe(true);
});
