// @vitest-environment happy-dom
import { flushPromises, mount } from '@vue/test-utils';
import { defineComponent } from 'vue';
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import App from './App.vue';
import type { CourseAction } from './generated/CourseAction';
import type { CourseLessonView } from './generated/CourseLessonView';
import type { ClosedBookPractice } from './generated/ClosedBookPractice';
import type { LearningTaskView } from './learning-tasks';
import { network, submittedRunDrafts } from './reader-context';
import { installReaderHost } from './reader-host';
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
const deliveredAnswer = { turn_id: 'selected-explanation', can_explain_back: true, user: '为什么要比较完成时间？', status: 'completed', effect_labels: [],
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
  name: 'InquiryCoursePlayer', props: ['workspace', 'publication', 'embedded', 'submitTeachBack'],
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

async function start(courseTrial?: { bookId: string; revision: string }) {
  wrapper = mount(App, { props: { courseTrial }, shallow: true, global: { renderStubDefaultSlot: true, stubs: {
    InquiryCoursePlayer: coursePlayer,
    ReaderWorkspace: defineComponent({ name: 'ReaderWorkspace', template: '<div><slot name="reader-navigation" /><slot /></div>',
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

it('opens the requested sample lesson without selecting a formal learning task or changing its saved view', async () => {
  const formalView = 'understand-book:learning:local:course-view:local-course:startup:local';
  sessionStorage.setItem(formalView, 'reading');
  await start({ bookId: 'startup', revision: 'pilot-2' });
  expect(inquiryCourse.catalog).toHaveBeenCalledWith('local-course-trial:startup:pilot-2');
  expect(mocks.order).toContain('open-course:original-lesson:undefined');
  expect(sessionStorage.getItem('understand-book:learning:local:course-view:local-course-trial:startup:pilot-2:local')).toBe('course');
  expect(sessionStorage.getItem(formalView)).toBe('reading');
  expect(mocks.api.selectLearningTask).not.toHaveBeenCalled();
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

it('binds the open composer, lets the reader cancel, and submits their own expression once', async () => {
  const app = await start();
  const rail = app.getComponent({ name: 'RightRail' });
  const expression = '两件事同时开始，慢的那件结束才算都完成。';
  rail.vm.$emit('explain-answer', deliveredAnswer.turn_id);
  await flushPromises();
  expect(rail.props('explainAnswerId')).toBe(deliveredAnswer.turn_id);
  rail.vm.$emit('update:agentInput', expression);
  rail.vm.$emit('cancel-explain-answer');
  await flushPromises();
  expect(rail.props('explainAnswerId')).toBeNull();
  expect(rail.props('agentInput')).toBe(expression);
  expect(mocks.api.agentRunCreate).not.toHaveBeenCalled();
  rail.vm.$emit('explain-answer', deliveredAnswer.turn_id);
  rail.vm.$emit('send-agent');
  await flushPromises();
  const [[text, meta]] = mocks.api.agentRunCreate.mock.calls;
  expect(text).toBe(expression);
  expect(meta).toMatchObject({ display_user: expression, entry_action: 'explain_back', answer_ref: { turn_id: deliveredAnswer.turn_id },
    source_id: 'startup', source_revision: 'startup', session_id: 'startup-chat', expected_chat_head: deliveredAnswer.turn_id,
    expression_id: meta.client_request_id, operation_id: expect.any(String) });
  expect(meta.client_request_id).toBeTruthy();
  expect(meta).not.toHaveProperty('retry_of');
  expect(meta).not.toHaveProperty('learning_purpose');
  expect(rail.props('explainAnswerId')).toBeNull();
  expect(rail.props('chat').filter((turn: { user: string }) => turn.user === expression)).toHaveLength(1);
  expect(closedMutations()).toEqual([]);
});

it('retries a saved failed comparison in the original chat turn after reopening the reader', async () => {
  const expression = '慢的那项决定完成时间。';
  const reply = { kind: 'revise_understanding', round_id: 'tb:previous', row_id: 'why' };
  const failed = { turn_id: 'first-expression', user: expression, status: 'failed', effect_labels: [],
    error: { error_code: 'TEACH_BACK_INVALID', category: 'protocol', message: 'invalid comparison' },
    domain: { effects: [], interaction: { answer_ref: { turn_id: deliveredAnswer.turn_id }, teach_back_reply: reply },
      teach_back_attempts: [{ expression_id: 'saved-expression-id', operation_id: 'first-attempt' }] } };
  const history = historyFor();
  mocks.api.agentHistory.mockResolvedValue({ ...history, current: { ...history.current, turns: [deliveredAnswer, failed] } });
  const app = await start();
  const rail = app.getComponent({ name: 'RightRail' });
  const original = rail.props('chat').at(-1);
  expect(original?.error).toBeTruthy();
  rail.vm.$emit('retry-explain-answer', original);
  await flushPromises();
  expect(mocks.api.agentRunCreate).toHaveBeenCalledExactlyOnceWith(expression, expect.objectContaining({
    expression_id: 'saved-expression-id', client_request_id: 'saved-expression-id', retry_of: failed.turn_id,
    expected_chat_head: failed.turn_id, operation_id: expect.any(String), answer_ref: { turn_id: deliveredAnswer.turn_id }, teach_back_reply: reply,
  }));
  expect(mocks.api.agentRunCreate.mock.calls[0][1].operation_id).not.toBe('first-attempt');
  expect(rail.props('chat').map((turn: { user: string }) => turn.user)).toEqual([deliveredAnswer.user, expression]);
  expect(closedMutations()).toEqual([]);
});

it('retries a failed course explanation with its saved experiment and course revision after reopening', async () => {
  const course = {run_id:lesson.progress.run_id, lesson_id:lesson.summary.id, round_index:0, stage:'explanation', delivery_ref:'explanation:1'};
  const exploration = {work_gflops:140, read_gb:70, compute_tflops:989, bandwidth_tb_s:3.35, target_ms:9};
  const original = {run_id:course.run_id, revision:2, surface:'course', exploration};
  const failed = {turn_id:'failed-course', user:'容量影响能否容纳。', status:'failed', effect_labels:[],
    error:{error_code:'TEACH_BACK_INVALID', category:'protocol', message:'invalid comparison'},
    domain:{effects:[], interaction:{course, answer_ref:null}, course_retry_context:original,
      teach_back_attempts:[{expression_id:'expression', operation_id:'first'}]}};
  const history = historyFor();
  mocks.api.agentHistory.mockResolvedValue({...history, current:{...history.current, turns:[failed]}});
  lesson.exploration = {...exploration, bandwidth_tb_s:6.7};
  const app = await start();
  await openCourse();
  const submit = app.getComponent(coursePlayer).props('submitTeachBack');
  await submit({text:failed.user, retry:true});
  expect(mocks.api.agentRunCreate).toHaveBeenLastCalledWith(failed.user, expect.objectContaining({
    course_context:original, client_request_id:'expression', expression_id:'expression', retry_of:failed.turn_id,
  }));
  await submit({text:'带宽决定读取时间下界。'});
  expect(mocks.api.agentRunCreate).toHaveBeenLastCalledWith('带宽决定读取时间下界。', expect.objectContaining({
    course_context:{...original,revision:lesson.progress.revision, exploration:lesson.exploration},
  }));
});

it.each(['dispute_interpretation', 'revise_understanding', 'follow_up'] as const)(
  'sends %s from a restored comparison with the selected row and a fresh expression identity', async action => {
    const card = { round_id: 'tb:earlier', source: { source_id: 'startup', source_revision: 'startup' },
      rows: [], student_graph: { nodes: [], edges: [] }, book_graph: { nodes: [], edges: [] }, side_notes: [], follow_up: { row_id: 'why', question: '为什么？' } };
    const saved = { turn_id: 'earlier', user: '先前的想法', status: 'completed', effect_labels: [], domain: {
      interaction: { answer_ref: { turn_id: deliveredAnswer.turn_id } }, teach_back_attempts: [{ expression_id: 'first', operation_id: 'submit' }],
    }, outcome: { effects: [], answer_view: { parts: [{kind: 'teach_back_comparison', comparison: card}], sources: [] } } };
    const history = historyFor();
    const next = { ...saved, turn_id: 'later', outcome: { effects: [], answer_view: { parts: [{kind: 'teach_back_comparison',
      comparison: { ...card, round_id: 'tb:later', continuation: { first_round_id: card.round_id, parent_round_id: card.round_id,
        kind: 'revise_understanding', row_id: 'why' } } }], sources: [] } } };
    const turns = action === 'follow_up' ? [deliveredAnswer, saved] : [deliveredAnswer, saved, next];
    mocks.api.agentHistory.mockResolvedValue({ ...history, current: { ...history.current, turns } });
    const rail = (await start()).getComponent({ name: 'RightRail' });
    const text = '两项都结束时，慢的那一项刚完成。';
    rail.vm.$emit('update:agentInput', action === 'follow_up' ? text : '另一个问题的草稿');
    rail.vm.$emit('comparison-response', { action, text, turnId: 'earlier', roundId: card.round_id, rowId: 'why' });
    await flushPromises();
    expect(mocks.api.agentRunCreate).toHaveBeenCalledExactlyOnceWith(text, expect.objectContaining({
      entry_action: 'explain_back', answer_ref: { turn_id: deliveredAnswer.turn_id }, expected_chat_head: turns.at(-1)!.turn_id,
      teach_back_reply: { kind: action, row_id: 'why', round_id: card.round_id },
    }));
    expect(mocks.api.agentRunCreate.mock.calls[0][1].expression_id).not.toEqual('first');
    expect(rail.props('agentInput')).toBe(action === 'follow_up' ? '' : '另一个问题的草稿');
    expect(closedMutations()).toEqual([]);
  });

it('explains a selected earlier answer while submitting against the current conversation head', async () => {
  const history = historyFor();
  const latest = { ...deliveredAnswer, turn_id: 'later-answer', user: '带宽是什么？' };
  mocks.api.agentHistory.mockResolvedValue({ ...history, current: { ...history.current, turns: [deliveredAnswer, latest] } });
  const app = await start();
  const rail = app.getComponent({ name: 'RightRail' });
  rail.vm.$emit('explain-answer', deliveredAnswer.turn_id);
  await flushPromises();
  expect(rail.props('explainAnswerId')).toBe(deliveredAnswer.turn_id);
  rail.vm.$emit('update:agentInput', '等最后完成的那项。');
  rail.vm.$emit('send-agent');
  await flushPromises();
  expect(mocks.api.agentRunCreate).toHaveBeenCalledExactlyOnceWith('等最后完成的那项。', expect.objectContaining({
    entry_action: 'explain_back', answer_ref: { turn_id: deliveredAnswer.turn_id }, expected_chat_head: latest.turn_id,
  }));
});

it('starts closed-book practice only after the explicit answer action and preserves the selected answer origin', async () => {
  const app = await start();
  expect(closedMutations()).toEqual([]);
  expect(mocks.api.agentRunCreate).not.toHaveBeenCalled();
  expect(app.getComponent({ name: 'RightRail' }).props('closedBookReady')).toBe(true);
  app.getComponent({ name: 'RightRail' }).vm.$emit('closed-book-answer', deliveredAnswer.turn_id);
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
  expect(mocks.api.agentRunCreate.mock.calls[0][0]).toBe('');
  expect(mocks.api.agentRunCreate.mock.calls[0][1]).toMatchObject({ question_anchor_lid: undefined, entry_action: 'apply_lesson', learning_purpose: 'practice',
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

it('moves cloud free-reading book home and course entries into TopBar without rendering the second navigation bar', async () => {
  installReaderHost({
    request: vi.fn(), fetch: vi.fn(), ownerHeaders: () => ({}), publishedUrl: leaf => leaf,
    workspaceAction: vi.fn(), recoverSubmissions: async () => [], recoverWorkspaceBinding: async () => {},
    requestWorkspaceRecovery: vi.fn(), pendingSubmissions: () => [], retrySubmission: vi.fn(),
    readCourseInstance: async () => ({ instance: null }), readCourseRelease: vi.fn(),
    openLinkedPresentation: vi.fn(), runNotice: () => undefined, runFailureMessage: () => undefined,
  });
  const history = historyFor();
  mocks.api.agentHistory.mockResolvedValue({ ...history, current: { ...history.current, learning_workspace_id: 'lw-1' } });
  const cloudPublication = { book_id: 'startup', publication_id: 'pub-1' };
  const learningWorkspace = {
    learning_workspace_id: 'lw-1',
    goal: { text: '理解原理', constraints: [], revision: 1, created_at: 0 },
    goal_history: [], members: [{ published_book_ref: cloudPublication, readable: true, added_at: 0 }],
    archived: false, created_at: 0, updated_at: 0,
  };
  network.value = {
    ...network.value, enabled: true, linked: true,
    identity: { user_id: 'u1', csrf_token: 'csrf' },
    workspace: {
      workspace_id: 'ws-1', generation: 1, revision: 1, learning_workspace_id: 'lw-1',
      published_book_ref: cloudPublication, selected_chat: 'startup-chat',
      reader: { book_id: 'startup', revision: 0, viewport: { anchor_lid: '1.1', top_lid: '1.1', bottom_lid: '1.1', width: 1, visible_lids: ['1.1'] },
        profile: { profile_id: 'technical_learning', profile_version: '1' }, layout: { rev: 0, open_slots: [] } },
    },
  };
  wrapper = mount(App, { props: { learningWorkspace }, shallow: true, global: { renderStubDefaultSlot: true, stubs: {
    InquiryCoursePlayer: coursePlayer,
    ReaderWorkspace: defineComponent({ name: 'ReaderWorkspace', template: '<div><slot name="reader-navigation" /><slot /></div>',
      setup(_, { expose }) { expose({ showReader: mocks.showReader, showAssistant: mocks.showAssistant }); } }),
    ReaderPane: defineComponent({ template: '<div />', setup(_, { expose }) { expose({ captureScrollAnchor: () => null }); } }),
  } } });
  await flushPromises();
  const topBar = wrapper.getComponent({ name: 'TopBar' });
  expect(wrapper.find('.book-course-navigation').exists()).toBe(false);
  expect(topBar.props('bookHomeAvailable')).toBe(true);
  expect(topBar.props('bookCourseAvailable')).toBe(false);
  topBar.vm.$emit('open-book-home');
  await flushPromises();
  expect(wrapper.emitted('learning-collection')).toEqual([[{ kind: 'book', publication: cloudPublication, learningWorkspaceId: 'lw-1' }]]);

  wrapper.getComponent(coursePlayer).vm.$emit('available', true);
  await flushPromises();
  expect(wrapper.find('.book-course-navigation').exists()).toBe(false);
  expect(topBar.props('bookHomeAvailable')).toBe(true);
  expect(topBar.props('bookCourseAvailable')).toBe(true);
  topBar.vm.$emit('open-book-course');
  await flushPromises();
  expect(wrapper.find('.book-course-navigation').exists()).toBe(true);
  expect(topBar.props('bookHomeAvailable')).toBe(false);
  expect(topBar.props('bookCourseAvailable')).toBe(false);
});
