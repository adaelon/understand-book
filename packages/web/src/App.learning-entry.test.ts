// @vitest-environment happy-dom
import { flushPromises, mount } from '@vue/test-utils';
import { defineComponent } from 'vue';
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import App from './App.vue';
import type { CourseAction } from './generated/CourseAction';
import type { CourseLessonView } from './generated/CourseLessonView';
import type { ClosedBookPractice } from './generated/ClosedBookPractice';
import type { LearningTaskView } from './learning-tasks';
import { network, submittedRunDrafts } from './network-context';
import { notesPanel, pendingNoteSource } from './personal-notes';
import { inquiryCourse } from './inquiry-course';

const mocks = vi.hoisted(() => ({
  showReader: vi.fn(), showAssistant: vi.fn(), showCourseTask: vi.fn(), applyClosedBookResult: vi.fn(),
  order: [] as string[],
  api: Object.fromEntries([
    'videoPlayback', 'videoContext', 'musicContext', 'videoDraft', 'desktopStatus', 'buildWorkbench', 'manifest', 'assetManifest',
    'sourceFingerprint', 'sourceManifest', 'state', 'profileManifest', 'text', 'recall', 'agentHistory', 'agentHistorySelect', 'agentNew',
    'profileMemory', 'profileBackfill', 'intentUsageEvent', 'intentArtifacts', 'bookLibrary', 'paperMetadata',
    'tutorState', 'tutorReadiness', 'tutorMutate', 'tutorStart', 'closedBook', 'selectLearningTask', 'learningTasks', 'agentRunCreate', 'agentRun',
  ].map(key => [key, vi.fn()])),
}));
vi.mock('./api', async original => ({ ...await original<typeof import('./api')>(), api: mocks.api }));
vi.mock('./inquiry-course', () => ({ inquiryCourse: { catalog: vi.fn(), read: vi.fn(), mutate: vi.fn() } }));
vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));
vi.mock('./components/PdfReaderPane.vue', () => ({ default: { name: 'PdfReaderPane', template: '<div />' } }));

const publication = { book_id: 'startup', publication_id: 'local' };
const deliveredAnswer = { turn_id: 'selected-explanation', user: '为什么要比较完成时间？', status: 'completed', effect_labels: [],
  outcome: { answer: '两项同时开始，全部完成需要等待更慢的那一项。', effects: [] } };
const deliveredQuestion = { turn_id: 'delivered-first-question', user: '我来解释一下', status: 'completed', effect_labels: [],
  outcome: { answer: '如果两项分别需要 5 分钟和 8 分钟，什么时候全部完成？', effects: [] } };
const historyFor = (id = 'startup-chat') => ({ active_session_id: id, sessions: [],
  current: { id, book_id: 'startup', turns: [deliveredAnswer, ...(id === 'original-chat' ? [deliveredQuestion] : [])], goals: [] } });
let lesson: CourseLessonView;
let closedByChat: Record<string, ClosedBookPractice | null>;
let wrapper: ReturnType<typeof mount> | undefined;
type LearningEntryApi = {
  openCourse(target: { lessonId?: string; runId?: string }, signal?: AbortSignal): Promise<unknown>;
  openReading(signal?:AbortSignal):Promise<void>;
  resumeLearningTask(task: LearningTaskView, signal?: AbortSignal): Promise<void>;
};

function practiceState(overrides: Partial<ClosedBookPractice> = {}): ClosedBookPractice {
  return {
    id: 'original-practice', owner_id: 'startup-chat', course: false, chat_session_id: 'startup-chat',
    source_id: 'startup', source_revision: 'startup', goal_revision: null, revision: 4, status: 'active', can_resume: true,
    target: '解释当前问题', criteria: '独立解释原因并应用', attempt: 1, assisted: false, needs_retry: false,
    visible_turn_ids: [], draft: '', completion_reason: '', learner_quote: '', ...overrides,
  };
}
const coursePlayer = defineComponent({
  name: 'InquiryCoursePlayer', props: ['workspace', 'publication', 'embedded'],
  emits: ['available', 'blocked', 'error', 'change', 'engaged', 'practice', 'closed-book'],
  methods: {
    async openLesson(lessonId?: string, runId?: string) {
      mocks.order.push(`open-course:${lessonId}:${runId}`);
      this.$emit('change', structuredClone(lesson));
      return structuredClone(lesson);
    },
    async resumeSaved() { this.$emit('change', structuredClone(lesson)); return structuredClone(lesson); },
    showTask: mocks.showCourseTask,
    showExperiment() {},
    async act(action: CourseAction) {
      if (action.kind === 'resume') lesson.progress.status = 'active';
      if (action.kind === 'pause') lesson.progress.status = 'paused';
      lesson.progress.revision++;
      this.$emit('change', structuredClone(lesson));
    },
    async applyClosedBookResult(resultRef: string) {
      const value = await mocks.applyClosedBookResult(resultRef);
      this.$emit('blocked', false); this.$emit('error', ''); this.$emit('change', value);
      return value;
    },
  },
  template: '<div class="test-course-player" />',
});

beforeEach(() => {
  vi.resetAllMocks(); mocks.order = [];
  localStorage.clear(); sessionStorage.clear(); window.history.replaceState({}, '', '/');
  network.value = { ...network.value, enabled: false, workspace: null, identity: null };
  closedByChat = {};
  lesson = {
    summary: { id: 'original-lesson', unit_id: 'timing', question: '什么时候全部完成？', objective: '理解所有并行任务的整体资源边界',
      prerequisites: '会比较两个量', estimated_minutes: 8, duration_basis: '一轮问题',
      source: { publication, source_revision: 'startup', start_lid: '1.1', end_lid: '1.1', label: '当前段落', structure_refs: ['1'] } },
    progress: { run_id: 'original-course-run', release_id: 'course-release', release_version: 1, lesson_id: 'original-lesson', revision: 4, goal_revision: 1, stage: 'explanation', status: 'active' },
    introduction: '比较两项完成时间。', prediction: { question: '先预测总时间。', options: [] }, prediction_response: null,
    experiment: null, preset: null, observed: [], explanation: '比较较晚完成的一项。', task: null, answer: null,
    hint: null, solution: null, assessment_status: null, support_delivered: [], source_excerpt: null, concepts: [],
    reflection_prompt: '为什么更慢的一项决定总时间？', lab: null, exploration: null, expressions: [], reviews: [], reading: null,
    round_index: 0, round_titles: ['比较两项'], round_history: [], decision: null, activity_response: null, inquiry_kind: null,
    resolved_questions: [], math_exploration: null,
  };
  vi.mocked(inquiryCourse.catalog).mockResolvedValue({ release_id: 'course-release', release_version: 1,
    course: { lessons: [lesson.summary] } as never, progress: [lesson.progress] });
  mocks.applyClosedBookResult.mockImplementation(async () => {
    lesson.progress.revision++;
    return structuredClone(lesson);
  });
  const api = mocks.api;
  api.videoContext.mockResolvedValue(null); api.musicContext.mockResolvedValue(null); api.videoPlayback.mockResolvedValue(null); api.videoDraft.mockResolvedValue({ draft: null });
  api.desktopStatus.mockResolvedValue({ desktop_host: false, reader_only: true, active_book: true });
  api.buildWorkbench.mockResolvedValue({ book_id: 'startup', readiness: { route: 'reader' }, input: { manifest: null }, source_review: { unresolved: [] } });
  api.manifest.mockResolvedValue({ tree: [
    { lid: '1', kind: 'chapter', children: ['1.1'], span: { start: 0, end: 22 }, title: '# 比较完成时间' },
    { lid: '1.1', kind: 'paragraph', children: [], span: { start: 5, end: 22 } },
  ], stats_by_lid: {} });
  api.assetManifest.mockResolvedValue({ images: [] }); api.sourceFingerprint.mockResolvedValue({ book_id: 'startup', source_fingerprint: 'startup' }); api.sourceManifest.mockResolvedValue(null);
  api.state.mockResolvedValue({ book_id: 'startup', revision: 0,
    viewport: { anchor_lid: '1.1', top_lid: '1.1', bottom_lid: '1.1', width: 1, visible_lids: ['1.1'] },
    profile: { profile_id: 'technical_learning', profile_version: '1' }, layout: { rev: 0, open_slots: [] } });
  api.profileManifest.mockResolvedValue({ profile_id: 'technical_learning', slots: [] }); api.text.mockImplementation(async (lid: string) => ({ lid, text: '两项同时开始，等待较晚结束的一项。' })); api.recall.mockResolvedValue([]);
  api.agentHistory.mockResolvedValue(historyFor());
  api.agentNew.mockImplementation(async()=>{mocks.order.push('create-chat');return {ok:true,history:historyFor('first-course-chat')};});
  api.agentHistorySelect.mockImplementation(async (id: string) => { mocks.order.push(`select-chat:${id}`); return historyFor(id); });
  api.profileMemory.mockResolvedValue({}); api.profileBackfill.mockResolvedValue({ jobs: [], sessions: [] });
  api.intentArtifacts.mockResolvedValue({ overlay: null }); api.intentUsageEvent.mockResolvedValue({}); api.bookLibrary.mockResolvedValue({ root: 'books', books: [] });
  api.tutorState.mockResolvedValue({ control: { enabled: true, revision: 0, current_tutor_session_id: null }, sessions: {} });
  api.tutorReadiness.mockResolvedValue({ status: 'ready', source_id: 'startup', source_revision: 'startup', limitations: [], reason: '可用',
    teaching_assets: { status: 'stale', teaching_map_revision: null, limitations: [], reason: '教学资料构建失败' } });
  api.selectLearningTask.mockResolvedValue({});
  api.learningTasks.mockResolvedValue({ tasks: [] });
  api.agentRunCreate.mockResolvedValue({ answer: '请用一个新的例子解释原因。', effects: [] });
  api.closedBook.mockImplementation(async (request: Record<string, unknown>) => {
    const chat = String(request.chat_session_id);
    if (request.action === 'state') return structuredClone(closedByChat[chat] ?? null);
    if (request.action === 'start') closedByChat[chat] = practiceState({ chat_session_id: chat, course: !!request.course_run_id,
      owner_id: String(request.course_run_id ?? chat), answer_origin: request.answer_turn_id
        ? { turn_id: String(request.answer_turn_id), question: deliveredAnswer.user, answer: deliveredAnswer.outcome.answer } : undefined });
    const state = closedByChat[chat];
    if (!state) throw new Error('练习不存在');
    if (request.action === 'resume') state.status = 'active';
    if (request.action === 'pause') state.status = 'paused';
    if (request.action === 'exit') state.status = 'ended';
    state.revision++;
    return structuredClone(state);
  });
});

afterEach(() => {
  wrapper?.unmount(); wrapper = undefined;
  network.value = { ...network.value, enabled: false, workspace: null, identity: null };
  pendingNoteSource.value = null;
  Object.assign(notesPanel, { open: false, view: 'library', draftId: null, reference: null });
  submittedRunDrafts.clear(); localStorage.clear(); sessionStorage.clear();
  window.history.replaceState({}, '', '/'); vi.restoreAllMocks(); vi.unstubAllGlobals();
});

async function start() {
  wrapper = mount(App, { shallow: true, global: { renderStubDefaultSlot: true, stubs: {
    InquiryCoursePlayer: coursePlayer,
    ReaderWorkspace: defineComponent({ name: 'ReaderWorkspace', template: '<div><slot /></div>',
      setup(_, { expose }) { expose({ showReader: mocks.showReader, showAssistant: mocks.showAssistant }); } }),
    ReaderPane: defineComponent({ template: '<div />', setup(_, { expose }) { expose({ captureScrollAnchor: () => null }); } }),
  } } });
  await flushPromises();
  return wrapper;
}
function closedMutations(action?: string) {
  return mocks.api.closedBook.mock.calls.map(([request]) => request).filter(request => action ? request.action === action : request.action !== 'state');
}
async function openCourse() {
  await (wrapper!.vm as unknown as LearningEntryApi).openCourse({ lessonId: lesson.summary.id, runId: lesson.progress.run_id });
  await flushPromises();
}

it('creates a private conversation for a first course before saving its continuation', async () => {
  mocks.api.agentHistory.mockResolvedValue({active_session_id:'',sessions:[],current:{id:'',book_id:'startup',turns:[],goals:[]}});
  await start();
  expect(mocks.api.agentNew).not.toHaveBeenCalled();
  await openCourse();
  expect(mocks.api.agentNew).toHaveBeenCalledTimes(1);
  expect(mocks.order.indexOf('create-chat')).toBeLessThan(mocks.order.findIndex(value=>value.startsWith('open-course:')));
  expect(mocks.api.selectLearningTask).toHaveBeenCalledWith({chat_session_id:'first-course-chat',course_run_id:'original-course-run'});
  expect(mocks.api.agentRunCreate).not.toHaveBeenCalled();
});

it('opens explicit reading outside a saved course view after preserving the course and closed-book draft',async()=>{
  const app=await start();await openCourse();
  const viewKey='understand-book:learning:local:course-view:local-course:startup:local';
  expect(sessionStorage.getItem(viewKey)).toBe('course');
  closedByChat['startup-chat']=practiceState({draft:'我还在比较两项完成时间。',visible_turn_ids:['selected-explanation']});
  mocks.api.selectLearningTask.mockClear();mocks.showReader.mockClear();
  await (app.vm as unknown as LearningEntryApi).openReading();await flushPromises();
  expect(lesson.progress.status).toBe('paused');expect(lesson.progress.run_id).toBe('original-course-run');
  expect(closedMutations('exit')).toEqual([expect.objectContaining({chat_session_id:'startup-chat',draft:'我还在比较两项完成时间。'})]);
  expect(closedByChat['startup-chat']).toMatchObject({status:'ended',draft:'我还在比较两项完成时间。'});
  expect(sessionStorage.getItem(viewKey)).toBeNull();expect(app.getComponent({name:'RightRail'}).props('courseActive')).toBe(false);
  expect(mocks.showReader).toHaveBeenCalled();expect(mocks.api.selectLearningTask).not.toHaveBeenCalled();expect(mocks.api.agentNew).not.toHaveBeenCalled();
  expect(mocks.order.filter(value=>value.startsWith('open-course:'))).toHaveLength(1);
});

it('does not create a conversation or save a course task when opening is cancelled before initial history arrives', async () => {
  let releaseHistory!: (value: ReturnType<typeof historyFor>) => void;
  mocks.api.agentHistory.mockReturnValueOnce(new Promise<ReturnType<typeof historyFor>>(resolve => { releaseHistory = resolve; }));
  const app = await start(), controller = new AbortController();
  expect(app.getComponent({ name: 'RightRail' }).props('historyLoading')).toBe(true);
  const opening = (app.vm as unknown as LearningEntryApi).openCourse({ lessonId: lesson.summary.id, runId: lesson.progress.run_id }, controller.signal);
  const rejected = expect(opening).rejects.toMatchObject({ name: 'AbortError' });
  await flushPromises();
  expect(mocks.api.agentNew).not.toHaveBeenCalled();
  expect(mocks.api.selectLearningTask).not.toHaveBeenCalled();

  controller.abort(new DOMException('已取消打开', 'AbortError'));
  releaseHistory({ active_session_id: '', sessions: [], current: { id: '', book_id: 'startup', turns: [], goals: [] } });
  await rejected; await flushPromises();
  expect(mocks.api.agentNew).not.toHaveBeenCalled();
  expect(mocks.api.selectLearningTask).not.toHaveBeenCalled();
  expect(mocks.api.agentRunCreate).not.toHaveBeenCalled();
  expect(mocks.order).toEqual([]);
  expect(app.getComponent({ name: 'RightRail' }).props('courseActive')).toBe(false);
});

it('resumes an ordinary conversation outside the old course and sends its next practice without course context', async () => {
  const app = await start(); await openCourse();
  expect(app.getComponent({ name: 'RightRail' }).props('courseActive')).toBe(true);
  const previousProgress = structuredClone(lesson.progress);
  mocks.api.selectLearningTask.mockClear();
  const task: LearningTaskView = { learning_workspace_id: 'local-course:startup', published_book_ref: publication,
    course_run_id: null, chat_session_id: 'ordinary-chat', selected_at: 30, kind: 'conversation',
    title: '回到普通讨论', status: 'active', lesson_id: null, stage: null, closed_book_id: null,
    question: deliveredAnswer.user, message: null };
  await (app.vm as unknown as LearningEntryApi).resumeLearningTask(task);
  await flushPromises();

  const rail = app.getComponent({ name: 'RightRail' });
  expect(rail.props('courseActive')).toBe(false);
  expect(rail.props('activeChatSessionId')).toBe('ordinary-chat');
  expect(mocks.api.agentHistorySelect).toHaveBeenCalledWith('ordinary-chat');
  expect(mocks.api.selectLearningTask).toHaveBeenCalledWith({ chat_session_id: 'ordinary-chat', continue_conversation: true });
  expect(lesson.progress).toEqual(previousProgress);

  rail.vm.$emit('practice-answer', deliveredAnswer.turn_id);
  await flushPromises();
  expect(mocks.api.agentRunCreate).toHaveBeenCalledTimes(1);
  const meta = mocks.api.agentRunCreate.mock.calls[0][1];
  expect(meta).toMatchObject({ entry_action: 'continue_answer', answer_ref: { turn_id: deliveredAnswer.turn_id },
    learning_purpose: 'practice', session_id: 'ordinary-chat' });
  expect(meta).not.toHaveProperty('course_context');
  expect(lesson.progress).toEqual(previousProgress);
  expect(mocks.order.filter(value => value.startsWith('open-course:'))).toHaveLength(1);
  expect(closedMutations()).toEqual([]);
});

it('requests an application question for the exact delivered answer without starting closed-book practice', async () => {
  const app = await start();
  expect(mocks.api.agentRunCreate).not.toHaveBeenCalled();
  expect(closedMutations()).toEqual([]);
  app.getComponent({ name: 'RightRail' }).vm.$emit('practice-answer', deliveredAnswer.turn_id);
  await flushPromises();

  expect(mocks.api.agentRunCreate).toHaveBeenCalledTimes(1);
  expect(mocks.api.agentRunCreate.mock.calls[0][1]).toMatchObject({
    entry_action: 'continue_answer', answer_ref: { turn_id: deliveredAnswer.turn_id }, learning_purpose: 'practice',
    session_id: 'startup-chat', expected_chat_head: deliveredAnswer.turn_id, question_quote: null,
  });
  expect(closedMutations()).toEqual([]);
});

it('starts explanation practice only after the explicit answer action and preserves the selected answer origin', async () => {
  const app = await start();
  expect(closedMutations()).toEqual([]);
  expect(mocks.api.agentRunCreate).not.toHaveBeenCalled();
  expect(app.getComponent({ name: 'RightRail' }).props('closedBookReady')).toBe(true);
  app.getComponent({ name: 'RightRail' }).vm.$emit('explain-answer', deliveredAnswer.turn_id);
  await flushPromises();

  expect(closedMutations('start')).toHaveLength(1);
  expect(closedMutations('start')[0]).toMatchObject({ action: 'start', chat_session_id: 'startup-chat', answer_turn_id: deliveredAnswer.turn_id });
  expect(closedMutations('start')[0]).not.toHaveProperty('course_run_id');
  expect(mocks.api.agentRunCreate).toHaveBeenCalledTimes(1);
  expect(app.getComponent({ name: 'ClosedBookPractice' }).props('state').answer_origin.turn_id).toBe(deliveredAnswer.turn_id);
});

it('keeps course application practice in the ordinary learning flow', async () => {
  const app = await start(); await openCourse();
  app.getComponent({ name: 'InquiryCoursePlayer' }).vm.$emit('practice');
  await flushPromises();

  expect(mocks.api.agentRunCreate).toHaveBeenCalledTimes(1);
  expect(mocks.api.agentRunCreate.mock.calls[0][1]).toMatchObject({ entry_action: 'learn_with_purpose', learning_purpose: 'practice',
    course_context: { run_id: 'original-course-run' } });
  expect(closedMutations()).toEqual([]);
});

it('starts explicit course closed-book practice with the course run without overriding its current task with the whole lesson objective', async () => {
  const app = await start(); await openCourse();
  expect(closedMutations()).toEqual([]);
  app.getComponent({ name: 'InquiryCoursePlayer' }).vm.$emit('closed-book');
  await flushPromises();

  expect(closedMutations('start')).toHaveLength(1);
  expect(closedMutations('start')[0]).toMatchObject({ course_run_id: 'original-course-run', chat_session_id: 'startup-chat' });
  expect(closedMutations('start')[0]).not.toHaveProperty('target');
  expect(closedMutations('start')[0]).not.toHaveProperty('answer_turn_id');
});

it('retains a completed closed-book result when returning to the course fails and exits only after the same result is accepted', async () => {
  closedByChat['startup-chat'] = practiceState({ course: true, owner_id: 'original-course-run', status: 'complete',
    result_ref: 'delivered-result', completion_reason: '已独立解释', visible_turn_ids: [deliveredAnswer.turn_id] });
  mocks.applyClosedBookResult.mockRejectedValueOnce(new Error('练习结果尚未接回本课'));
  const app = await start(); await openCourse();
  mocks.showCourseTask.mockClear();
  app.getComponent({ name: 'ClosedBookPractice' }).vm.$emit('leave', 'exit');
  await flushPromises();

  expect(mocks.applyClosedBookResult).toHaveBeenCalledWith('delivered-result');
  expect(closedMutations('exit')).toEqual([]);
  expect(app.getComponent({ name: 'ClosedBookPractice' }).props()).toMatchObject({
    state: { id: 'original-practice', status: 'complete', result_ref: 'delivered-result' }, error: '练习结果尚未接回本课',
  });
  expect(mocks.showCourseTask).not.toHaveBeenCalled();

  app.getComponent({ name: 'ClosedBookPractice' }).vm.$emit('leave', 'exit');
  await flushPromises();
  expect(mocks.applyClosedBookResult.mock.calls).toEqual([['delivered-result'], ['delivered-result']]);
  expect(closedMutations('exit')).toHaveLength(1);
  expect(app.getComponent({ name: 'ClosedBookPractice' }).props('state').status).toBe('ended');
  expect(mocks.showCourseTask).toHaveBeenCalled();
  expect(mocks.showReader).toHaveBeenCalled();
});

it('removes the closed-book overlay while the reader is hidden and restores the same saved practice when reopened', async () => {
  closedByChat['startup-chat'] = practiceState({ draft: '我正在解释这个条件' });
  const app = await start();
  expect(app.getComponent({ name: 'ClosedBookPractice' }).props('state').id).toBe('original-practice');
  mocks.api.closedBook.mockClear();
  await app.setProps({ readerVisible: false });
  expect(app.findComponent({ name: 'ClosedBookPractice' }).exists()).toBe(false);
  await app.setProps({ readerVisible: true });
  expect(app.getComponent({ name: 'ClosedBookPractice' }).props()).toMatchObject({
    state: { id: 'original-practice', status: 'active' }, draft: '我正在解释这个条件',
  });
  expect(closedMutations()).toEqual([]);
  expect(mocks.api.agentRunCreate).not.toHaveBeenCalled();
});

it('resumes a paused course in the learning pane instead of covering its task with the mobile chat pane', async () => {
  lesson.progress.status = 'paused';
  lesson.progress.stage = 'prediction';
  const task: LearningTaskView = { learning_workspace_id: 'local-course:startup', published_book_ref: publication,
    course_run_id: 'original-course-run', chat_session_id: 'original-chat', selected_at: 20, kind: 'course',
    title: '继续当前预测', status: 'paused', lesson_id: 'original-lesson', stage: 'prediction', closed_book_id: null,
    question: lesson.summary.question, message: null };
  const app = await start();
  mocks.showReader.mockClear(); mocks.showAssistant.mockClear();
  await (app.vm as unknown as LearningEntryApi).resumeLearningTask(task);
  await flushPromises();

  expect(lesson.progress.status).toBe('active');
  expect(mocks.showReader).toHaveBeenCalled();
  expect(mocks.showCourseTask).toHaveBeenCalled();
  expect(mocks.showAssistant).not.toHaveBeenCalled();
  expect(mocks.api.selectLearningTask).toHaveBeenLastCalledWith({ chat_session_id: 'original-chat', course_run_id: 'original-course-run' });
  expect(mocks.api.agentRunCreate).not.toHaveBeenCalled();
});

it('restores the original chat before opening its course run and resumes a delivered closed-book question without generating it again', async () => {
  closedByChat['original-chat'] = practiceState({ chat_session_id: 'original-chat', owner_id: 'original-course-run', course: true,
    status: 'paused', visible_turn_ids: [deliveredQuestion.turn_id], draft: '我认为需要等到八分钟，因为' });
  const task: LearningTaskView = { learning_workspace_id: 'local-course:startup', published_book_ref: publication,
    course_run_id: 'original-course-run', chat_session_id: 'original-chat', selected_at: 20, kind: 'closed_book',
    title: '继续解释完成时间', status: 'paused', lesson_id: 'original-lesson', stage: 'explanation', closed_book_id: 'original-practice',
    question: deliveredQuestion.outcome.answer, message: null };
  const app = await start();
  await (app.vm as unknown as LearningEntryApi).resumeLearningTask(task);
  await flushPromises();

  expect(mocks.order).toEqual(['select-chat:original-chat', 'open-course:original-lesson:original-course-run']);
  expect(closedMutations('resume')).toHaveLength(1);
  expect(closedMutations('resume')[0]).toMatchObject({ chat_session_id: 'original-chat' });
  expect(closedMutations('start')).toEqual([]);
  expect(mocks.api.agentRunCreate).not.toHaveBeenCalled();
  expect(app.getComponent({ name: 'ClosedBookPractice' }).props()).toMatchObject({
    state: { id: 'original-practice', status: 'active', visible_turn_ids: [deliveredQuestion.turn_id] },
    draft: '我认为需要等到八分钟，因为', messages: [{ id: deliveredQuestion.turn_id, answer: deliveredQuestion.outcome.answer }],
  });
  expect(mocks.api.selectLearningTask).toHaveBeenLastCalledWith({ chat_session_id: 'original-chat', course_run_id: 'original-course-run' });
});
