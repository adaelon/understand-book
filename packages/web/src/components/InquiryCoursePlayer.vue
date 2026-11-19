<script setup lang="ts">
import { t as uiText } from '../i18n';
import { computed, nextTick, onBeforeUnmount, ref, watch } from 'vue';
import { inquiryCourse, type CourseCatalog } from '../inquiry-course';
import { network, type LearningWorkspace, type PublishedBookRef } from '../reader-context';
import { ApiError } from '../api';
import { renderMarkdown } from '../md';
import type { CourseResourceState } from '../generated/CourseResourceState';
import type { CourseAction } from '../generated/CourseAction';
import type { CourseLessonView } from '../generated/CourseLessonView';
import type { CourseMutation } from '../generated/CourseMutation';
import type { CourseProgress } from '../generated/CourseProgress';
import type { CourseStage } from '../generated/CourseStage';
import BoardSourcePreview from './BoardSourcePreview.vue';
import type { SourcePopupView, SourceWindowLoader } from '../api';
import 'katex/dist/katex.min.css';
import ResourceBoundsExperiment from './ResourceBoundsExperiment.vue';
import CourseInquiryActivity from './CourseInquiryActivity.vue';
import CourseMathExperiment from './CourseMathExperiment.vue';
import ResourceDecisionLab from './ResourceDecisionLab.vue';
import CourseReviews from './CourseReviews.vue';
import CourseDecisionTutor from './CourseDecisionTutor.vue';
import CourseSupportBranch from './CourseSupportBranch.vue';
import CourseQuestions from './CourseQuestions.vue';
import CourseActivityTutor from './CourseActivityTutor.vue';
import CourseResourceObjects from './CourseResourceObjects.vue';
import type { CourseReplyIntent } from '../generated/CourseReplyIntent';
import type { CourseOverview } from '../generated/CourseOverview';

const props = defineProps<{ workspace: LearningWorkspace; releaseVersion?: number; publication?: PublishedBookRef; embedded?: boolean; catalogOnly?: boolean;
  collection?: { kind:'book'|'course'; title:string; external:boolean; overview?:CourseOverview; releaseVersion?:number };
  collectionLoading?:boolean; collectionError?:string;
}>();
const emit = defineEmits<{
  understanding: [publication: PublishedBookRef];
  launch: [target: { publication: PublishedBookRef; lessonId: string; runId?: string }];
  change: [lesson: CourseLessonView | null];
  engaged: [lesson: CourseLessonView];
  available: [value: boolean];
  blocked: [value: boolean];
  error: [value: string];
  source: [source: CourseLessonView['summary']['source'], lid?: string];
  exploration: [state: CourseResourceState];
  discussion: [question?:string];
  closedBook: [];
  practice: [];
  returnCollection: [];
  retryCollection: [];
}>();
const questions = ref<InstanceType<typeof CourseQuestions>>();
const reflection = ref('');
const inquiryDirty = ref(false);
const replyDraft = ref('');
const activityReply = ref('');
const activityTutor = ref<InstanceType<typeof CourseActivityTutor>>();
const experimentStage = ref<HTMLElement>();
const reviewingJob = ref<string | null>(null);
const decisionTutor = ref<InstanceType<typeof CourseDecisionTutor>>();
const experimentLab = ref<InstanceType<typeof ResourceDecisionLab>>();
const tutoring = computed(() => !reviewingQuestion.value && !!lesson.value?.decision && ['task','feedback'].includes(lesson.value.progress.stage) && !labOpen.value);
const labOpen = ref(false);
const roundHistoryOpen = ref(false);
const exploration = ref<CourseResourceState | null>(null);
const explaining = computed(() => !reviewingQuestion.value && (lesson.value?.progress.stage === 'explanation' || reviewingEarlier.value) && !labOpen.value);
function updateExploration(state: CourseResourceState) { exploration.value = state; emit('exploration', state); }
const sourceOpen = ref(false);
const sourceTrigger = ref<HTMLElement>();
const sourceRun = computed(() => lesson.value?.progress.run_id);
const readCourseSource = computed<SourceWindowLoader | undefined>(() => {
  const run = sourceRun.value, scope = props.workspace.learning_workspace_id;
  return run ? options => inquiryCourse.readSource(scope, run, options) : undefined;
});
const sourcePreview = computed<SourcePopupView | null>(() => {
  const current = lesson.value; if (!current?.source_excerpt) return null;
  const source = current.summary.source;
  return { source_ref_id: current.progress.run_id + ':source', label: source.label,
    source_url: `?book_id=${encodeURIComponent(source.publication.book_id)}&publication_id=${encodeURIComponent(source.publication.publication_id)}`,
    highlighted_quote: '', context_before: current.source_excerpt, context_after: '', stale: false, can_open_in_reader: !!props.embedded };
});
const catalog = ref<CourseCatalog | null>(null), lesson = ref<CourseLessonView | null>(null);
const knownPublicationRuns = ref(new Set<string>());
const loading = ref(false), busy = ref(false), error = ref(''), route = ref('');
watch(error, value => emit('error', value), { immediate: true });
const choice = ref(''), reason = ref(''), heading = ref<HTMLElement | null>(null);
function answerDraftKey(value: CourseLessonView) {
  return `course-answer:${network.value.identity?.user_id ?? 'local'}:${props.workspace.learning_workspace_id}:${value.progress.run_id}:${value.round_index}:${value.progress.stage}`;
}
watch([choice,reason,reflection], () => {
  if(!lesson.value || !['prediction','explanation','task','feedback'].includes(lesson.value.progress.stage))return;
  try { localStorage.setItem(answerDraftKey(lesson.value),JSON.stringify({choice:choice.value,reason:reason.value,reflection:reflection.value})); } catch { /* Submitted answers remain saved by the course. */ }
});
let generation = 0;
const pending = ref<CourseMutation | null>(null);
const allStages: { id: CourseStage; label: string }[] = [
  { id: 'introduction', label: '认识问题' }, { id: 'prediction', label: '先猜一猜' }, { id: 'experiment', label: '观察或取证' },
  { id: 'explanation', label: '我来解释' }, { id: 'task', label: '应用一下' }, { id: 'feedback', label: '反馈与下一步' },
];
const multiRound = computed(() => (lesson.value?.round_titles?.length ?? 1) > 1);
const finalTask = computed(() => ['task','feedback','complete'].includes(lesson.value?.progress.stage ?? ''));
const stages = computed(() => !multiRound.value ? allStages : lesson.value?.progress.stage === 'introduction' ? allStages.slice(0,1) : finalTask.value ? allStages.slice(4) : allStages.slice(1,4));
const stageIndex = computed(() => lesson.value?.progress.stage === 'complete' ? stages.value.length : stages.value.findIndex(s => s.id === lesson.value?.progress.stage));
const roundIndex = computed(() => lesson.value?.round_index ?? 0);
const activityExpression = computed(() => {
  const current=lesson.value;if(!current)return undefined;
  if(reviewingJob.value)return current.expressions.find(e=>e.job_ref===reviewingJob.value);
  if(current.decision)return undefined;
  const activities=current.progress.stage==='experiment'?['prediction','investigation']:current.progress.stage==='explanation'?['reflection']:current.progress.stage==='feedback'?['task','revision']:[];
  return current.expressions.filter(e=>(e.round_index ?? 0)===roundIndex.value && activities.includes(e.activity)).at(-1);
});
function replyStorageKey(kind: 'activity' | 'decision', question: string) {
  return `understand-book:learning:${network.value.identity?.user_id ?? 'local'}:course-reply:${JSON.stringify([props.workspace.learning_workspace_id,lesson.value?.progress.run_id,kind,question])}`;
}
const activityDraftKey = computed(() => activityExpression.value ? replyStorageKey('activity',activityExpression.value.job_ref) : null);
const decisionDraftKey = computed(() => lesson.value?.decision ? replyStorageKey('decision',lesson.value.decision.latest_job ?? `${lesson.value.round_index}:${lesson.value.decision.phase}:${lesson.value.decision.question}`) : null);
for (const [key,draft] of [[activityDraftKey,activityReply],[decisionDraftKey,replyDraft]] as const) {
  watch(key, value => { try { draft.value=value ? localStorage.getItem(value) ?? '' : ''; } catch { draft.value=''; } }, { flush:'sync' });
  watch(draft, value => { if(key.value) { try { if(value)localStorage.setItem(key.value,value);else localStorage.removeItem(key.value); } catch { /* Keep the current reply available in memory. */ } } }, { flush:'sync' });
}
const reviewingQuestion=computed(()=>activityExpression.value?.activity==='question');
const reviewingEarlier = computed(() => !!reviewingJob.value && !!lesson.value?.round_history[activityExpression.value?.round_index ?? 0]);
const earlierExperiment = computed(() => reviewingEarlier.value ? lesson.value?.round_history[activityExpression.value!.round_index ?? 0]?.exploration : null);
const earlierMathExperiment = computed(() => reviewingEarlier.value ? lesson.value?.round_history[activityExpression.value!.round_index ?? 0]?.math_exploration : null);
const activityReview = computed(() => lesson.value?.reviews.find(r=>r.expression.job_ref===activityExpression.value?.job_ref));
const feedbackHistory = computed(() => lesson.value?.reviews.filter(r=>r.expression.job_ref!==activityExpression.value?.job_ref && (!lesson.value?.decision || !['task','decision','transfer'].includes(r.expression.activity))) ?? []);
const moreRounds = computed(() => roundIndex.value + 1 < (lesson.value?.round_titles?.length ?? 1));
const currentReflection = computed(()=>lesson.value?.expressions.filter(e=>e.activity==='reflection' && (e.round_index??0)===roundIndex.value && (!e.intent || e.intent==='answer')).at(-1));
const reflectionSaved = computed(() => lesson.value?.expressions.some(expression => {
  if (expression.activity !== 'reflection' || (expression.round_index ?? 0) !== roundIndex.value) return false;
  if (!expression.closed_book_result_ref) return true;
  return lesson.value?.reviews.some(review => review.expression.job_ref === expression.job_ref && review.displayed && review.feedback?.course_reuse?.accepted);
}));
const resourceActivity = computed(() => !lesson.value?.inquiry_kind);
const batchMode = computed(() => lesson.value?.experiment?.kind === 'resource_bounds' && lesson.value.experiment.batch_size !== undefined);
const experimentReady = computed(() => !!lesson.value && (lesson.value.inquiry_kind ? !!lesson.value.activity_response && !inquiryDirty.value : lesson.value.observed.length >= 3 && (!lesson.value.lab || !!lesson.value.exploration)));
const trialLesson = computed(() => lesson.value?.progress.release_id.startsWith('local-course-trial:') ?? false);
const activityContinuation = computed(() => {
  const current=lesson.value;if(!current || !active.value)return undefined;
  const waitingForFeedback = !trialLesson.value && !activityReview.value?.displayed;
  if(reviewingJob.value)return {label:uiText('回到当前活动 →'),detail:uiText('这一轮的讨论已保留，可以接着当前课程往下学。')};
  if(current.progress.stage==='experiment' && current.inquiry_kind)return {label:experimentReady.value?uiText('对照依据，继续解释 →'):uiText('回到探究活动 →'),detail:uiText('保存自己的取证或推理结果，再与讲解对照。')};
  if(current.progress.stage==='experiment')return experimentReady.value
    ? {label:uiText('对照预测，看看原因 →'),detail:uiText('三种配置已观察完，接下来把实验现象和预测连起来。')}
    : {label:uiText('去图上验证 →'),detail:uiText(batchMode.value ? '接下来比较 1、4、8 条请求，已观察 {0} / 3 种配置。' : '接下来比较原配置、算力翻倍和带宽翻倍，已观察 {0} / 3 种配置。',[current.observed.length])};
  if(current.progress.stage==='explanation')return {
    label:moreRounds.value ? uiText('继续下一轮探究 →') : uiText('进入综合应用 →'),
    detail:moreRounds.value ? uiText('下一轮：{0}',[current.round_titles[roundIndex.value+1]]) : uiText('接下来换一个情境，应用刚才建立的理解。'),
    disabled:(multiRound.value && !reflectionSaved.value) || waitingForFeedback,
  };
  if(current.progress.stage==='feedback')return {
    label:trialLesson.value ? uiText('完成试课') : uiText('完成这一课 ✓'),
    detail:trialLesson.value ? uiText('回答已保存。可以结束试课，再和制课 Agent 讨论这一节。') : uiText('回答和反馈已保留，接下来可以继续学习或暂停。'),
    disabled:waitingForFeedback,
  };
  return undefined;
});
const course = computed(() => {
  const value = catalog.value?.course;
  if (!value || !props.publication || props.collection?.kind === 'course') return value;
  const lessons = value.lessons.filter(l => l.source.publication.book_id === props.publication!.book_id && l.source.publication.publication_id === props.publication!.publication_id);
  const ids = new Set(lessons.map(l => l.id));
  return { ...value, lessons, routes: value.routes.map(r => ({ ...r, lesson_ids: r.lesson_ids.filter(id => ids.has(id)) })).filter(r => r.lesson_ids.length) };
});
const selectedRoute = computed(() => course.value?.routes.find(r => r.id === route.value));
const lessons = computed(() => selectedRoute.value
  ? selectedRoute.value.lesson_ids.map(id => course.value?.lessons.find(l => l.id === id)).filter(l => l !== undefined)
  : course.value?.lessons ?? []);
const lessonGroups = computed(() => {
  const groups = (course.value?.units ?? []).map(unit => ({ ...unit, lessons:lessons.value.filter(item => item.unit_id === unit.id) })).filter(unit => unit.lessons.length);
  return selectedRoute.value ? groups.sort((a,b) => lessons.value.indexOf(a.lessons[0]!) - lessons.value.indexOf(b.lessons[0]!)) : groups;
});
const collectionTitle = computed(() => props.collection?.title || uiText('本书课程'));
const collectionReturnLabel = computed(() => uiText(props.collection?.kind === 'course' ? '返回整套课程总目录' : '返回书籍学习首页'));
const currentUnit = computed(() => {
  const version = lesson.value?.progress.release_version;
  const overview = props.collection?.releaseVersion === version ? props.collection?.overview : catalog.value?.release_version === version ? catalog.value?.course : undefined;
  return overview?.units.find(unit => unit.id === lesson.value?.summary.unit_id)?.title || uiText('原课单元');
});
const nextLesson = computed(() => {
  if (lesson.value?.progress.release_version !== catalog.value?.release_version || goalChanged.value) return undefined;
  const index=lessons.value.findIndex(item=>item.id===lesson.value?.summary.id);
  return index>=0 ? lessons.value[index+1] : undefined;
});
function latest(id: string) { return catalog.value?.progress.find(p => p.lesson_id === id && p.release_version === catalog.value?.release_version && p.goal_revision === props.workspace.goal.revision); }
const completed = computed(() => course.value?.lessons.filter(l => latest(l.id)?.status === 'completed').length ?? 0);
const skipped = computed(() => course.value?.lessons.filter(l => latest(l.id)?.status === 'skipped').length ?? 0);
const oldRuns = computed(() => catalog.value?.progress.filter(p => p.run_id !== latest(p.lesson_id)?.run_id
  && (!props.publication || course.value?.lessons.some(l => l.id === p.lesson_id) || knownPublicationRuns.value.has(p.run_id))) ?? []);
const blocked = computed(() => busy.value || loading.value || props.workspace.archived || !!pending.value);
watch(blocked, value => emit('blocked', value), { immediate: true });
const active = computed(() => lesson.value?.progress.status === 'active');
const canRestart = computed(() => course.value?.lessons.some(item => item.id === lesson.value?.summary.id));
const goalChanged = computed(() => !!lesson.value && lesson.value.progress.goal_revision !== props.workspace.goal.revision);
const prompt = computed(() => lesson.value?.progress.stage === 'prediction' ? lesson.value.prediction : lesson.value?.task);
const selectedAnswer = computed(() => lesson.value?.task?.options.find(o => o.id === lesson.value?.answer?.choice)?.text);
const readingPrompt = computed(() => labOpen.value || reviewingEarlier.value ? undefined : ['introduction','prediction'].includes(lesson.value?.progress.stage ?? '') ? lesson.value?.reading?.before_experiment : lesson.value?.progress.stage === 'explanation' ? lesson.value.reading?.after_experiment : undefined);
const currentTask = computed(() => {
  const current = lesson.value;
  if (!current || current.support_visit || current.progress.status === 'completed' || current.progress.status === 'skipped') return null;
  if (reviewingJob.value) return { title:uiText('接着这次表达'), detail:uiText('先回应这里的反馈，再回到当前课程任务。') };
  if (labOpen.value) return { title:uiText('带着当前问题查看实验'), detail:uiText('观察后回到课程，继续原来的问题。') };
  switch (current.progress.stage) {
    case 'introduction': return { title:uiText('先认识问题和必要概念'), detail:uiText('读完下面的讲解，再留下自己的初步判断。') };
    case 'prediction': return { title:uiText('先作出你的判断'), detail:uiText('写下判断和理由，提交后再用观察或材料检验。') };
    case 'experiment': return { title:uiText('找到能支持判断的依据'), detail:uiText(current.inquiry_kind ? '完成下面的取证或推理，保存后再对照讲解。' : '比较下面的配置，保留能支持你解释的观察。') };
    case 'explanation': return { title:uiText('把依据组织成自己的解释'), detail:uiText(activityExpression.value ? '对照这次反馈，补清尚未说清的部分，或继续下一段。' : '把观察或原文依据与你的判断连起来，写在下面。') };
    case 'task': return { title:uiText('换个情境，独立作出判断'), detail:uiText('用本题条件说明你的计算或推理；需要时可以求助。') };
    case 'feedback': return { title:uiText('对照反馈，决定下一步'), detail:uiText(current.decision?.can_continue ? '本题已有足够依据，可以按下方提示继续。' : '已提交的回答保留在这里；根据反馈补充、求助或继续课程。') };
    default: return null;
  }
});
const contextualPractice = computed(() => {
  const current = lesson.value;
  if (!current || !active.value || goalChanged.value || current.support_visit || reviewingJob.value || labOpen.value) return false;
  if (current.progress.stage === 'explanation') return !!current.explanation && (!activityExpression.value || (!!activityReview.value?.displayed && activityReview.value.status === 'ready'));
  if (current.progress.stage !== 'feedback') return false;
  const job = current.decision?.latest_job ?? activityExpression.value?.job_ref;
  if (!job) return !!current.solution;
  return current.reviews.some(review => review.expression.job_ref === job && review.displayed && review.status === 'ready');
});
const completionEvidence = computed(() => {
  const current = lesson.value;
  if (current?.progress.status !== 'completed') return [];
  const expression = [...current.expressions].reverse().find(item => ['task','decision','transfer','revision'].includes(item.activity) && (!item.intent || item.intent === 'answer'));
  const review = current.reviews.find(item => item.expression.job_ref === expression?.job_ref);
  return review?.displayed && review.status === 'ready' ? review.understanding : [];
});
const statusLabel = (status?: string) => uiText(({ active: '学习中', paused: '已暂停', completed: '已完成', skipped: '已跳过' }[status ?? ''] ?? '未开始'));
function failure(e: unknown, token: number) { if (token === generation) error.value = e instanceof Error ? e.message : uiText('课程暂时无法读取，请重试。'); }
async function focusHeading() { await nextTick(); heading.value?.focus({ preventScroll: true }); heading.value?.scrollIntoView({ block: 'nearest', behavior: 'instant' }); }
async function load() {
  const token = ++generation; loading.value = true; error.value = '';
  try { const result = await inquiryCourse.catalog(props.workspace.learning_workspace_id); if (token === generation) {
    catalog.value = result;
    const saved = sessionStorage.getItem(routeStorageKey(result));
    route.value = result.course?.routes.some(item => item.id === saved) ? saved! : '';
  } }
  catch (e) { if (!(props.embedded && e instanceof ApiError && e.status === 404)) failure(e, token); }
  finally { if (token === generation) loading.value = false; }
}
async function open(run: CourseProgress) {
  if (busy.value) return;
  const token = ++generation; loading.value = true; error.value = '';
  try { const result = await inquiryCourse.read(props.workspace.learning_workspace_id, run.run_id); if (token === generation) {
    if (props.catalogOnly || props.publication && (result.summary.source.publication.book_id !== props.publication.book_id || result.summary.source.publication.publication_id !== props.publication.publication_id)) emit('launch', { publication: result.summary.source.publication, lessonId: result.summary.id, runId: run.run_id });
    else { install(result); if(reviewingEarlier.value){await nextTick();await activityTutor.value?.focus();}else await focusHeading(); }
  } }
  catch (e) { failure(e, token); }
  finally { if (token === generation) loading.value = false; }
}
function install(value: CourseLessonView) {
  if (props.publication && (value.summary.source.publication.book_id !== props.publication.book_id || value.summary.source.publication.publication_id !== props.publication.publication_id)) throw new Error(uiText('这次课程记录属于另一份资料，请从工作区打开。'));
  const runChanged=lesson.value?.progress.run_id!==value.progress.run_id;
  if(runChanged){
    const latest=value.expressions.at(-1),review=value.reviews?.find(r=>r.expression.job_ref===latest?.job_ref);
    reviewingJob.value=latest && !!value.round_history[latest.round_index ?? 0] && ['prediction','reflection'].includes(latest.activity) && !review?.displayed ? latest.job_ref : null;
  }
  const experimentChanged = runChanged || lesson.value?.round_index !== value.round_index || lesson.value?.decision?.phase !== value.decision?.phase;
  const changed = lesson.value?.progress.stage !== value.progress.stage || experimentChanged;
  if(!runChanged && changed)reviewingJob.value=null;
  if (experimentChanged) exploration.value = value.exploration;
  lesson.value = value;
  knownPublicationRuns.value.add(value.progress.run_id);
  if (changed) { sourceOpen.value = false; labOpen.value = false; roundHistoryOpen.value = false; }
  if (changed) { choice.value = ''; reason.value = ''; reflection.value = ''; }
  if (changed) { try {
    const draft=JSON.parse(localStorage.getItem(answerDraftKey(value)) ?? 'null');
    if(draft){choice.value=draft.choice ?? '';reason.value=draft.reason ?? '';reflection.value=draft.reflection ?? '';}
  } catch { /* A missing local draft does not affect the saved course. */ } }
  if (catalog.value) {
    const exists = catalog.value.progress.some(p => p.run_id === value.progress.run_id);
    catalog.value.progress = exists
      ? catalog.value.progress.map(p => p.run_id === value.progress.run_id ? value.progress : p)
      : [value.progress, ...catalog.value.progress];
  }
}
async function act(action: CourseAction, fresh = false) {
  if (blocked.value) return;
  pending.value = { operation_id: crypto.randomUUID(), run_id: fresh ? null : lesson.value?.progress.run_id ?? null,
    expected_revision: fresh ? 0 : lesson.value?.progress.revision ?? 0, release_version: fresh ? catalog.value?.release_version ?? props.releaseVersion ?? 1 : lesson.value?.progress.release_version ?? props.releaseVersion ?? 1, action };
  await retry();
}
async function showSource(event: MouseEvent) {
  if (!lesson.value) return;
  if (event.currentTarget instanceof HTMLElement) sourceTrigger.value = event.currentTarget;
  sourceOpen.value = true;
  if (!lesson.value.source_excerpt) await act({ kind: 'source' });
}
function openSourceInReader(lid?: string) {
  if (!lesson.value) return;
  sourceOpen.value = false;
  emit('source', lesson.value.summary.source, lid);
}
async function retry() {
  if (!pending.value || busy.value) return;
  const token = generation, request = pending.value, before = lesson.value?.progress.stage;
  const reply = request.action.kind==='feedback_reply' ? { key:activityDraftKey.value,draft:activityReply,text:activityReply.value,action:request.action }
    : request.action.kind==='decision_reply' ? { key:decisionDraftKey.value,draft:replyDraft,text:replyDraft.value,action:request.action } : null;
  busy.value = true; error.value = '';
  try {
    let result = await inquiryCourse.mutate(props.workspace.learning_workspace_id, request);
    if (result.expressions?.length) result = await inquiryCourse.read(props.workspace.learning_workspace_id, result.progress.run_id);
    if (token === generation) {
      if(reply?.key && reply.action.intent!=='explain') {
        try { if(localStorage.getItem(reply.key)?.trim()===reply.action.response.trim())localStorage.removeItem(reply.key); } catch { /* The response is already saved by the course. */ }
        if(reply.draft.value.trim()===reply.action.response.trim())reply.draft.value='';
      }
      install(result); pending.value = null;
      if (['start','resume','advance','predict','observe','explore','investigate','reflect','feedback_reply','decision_reply','answer','ask_question','open_support','return_from_support','hint','use_closed_book'].includes(request.action.kind)) emit('engaged', result);
      if(request.action.kind==='feedback_reply' && reviewingJob.value)reviewingJob.value=result.expressions.at(-1)?.job_ref ?? null;
      if(reply?.action.intent==='explain' && !reply.draft.value)reply.draft.value=reply.text;
      if (result.decision && ['answer','decision_reply','advance','use_closed_book'].includes(request.action.kind)) { await nextTick(); await decisionTutor.value?.focus(); } else if (['predict','reflect','feedback_reply','use_closed_book'].includes(request.action.kind) && activityExpression.value) { await nextTick(); await activityTutor.value?.focus(); } else if (before !== result.progress.stage) await focusHeading();
    }
  } catch (e) { failure(e, token); }
  finally { if (token === generation) busy.value = false; }
}
async function reconcile() {
  ++generation; const run = lesson.value?.progress;
  pending.value = null; busy.value = false;
  if (run) await open(run); else await load();
}
async function pauseBeforeNavigation() {
  if (blocked.value) return false;
  if (active.value) {
    await act({ kind: 'pause' });
    if (pending.value || active.value) return false;
  }
  return true;
}
async function back() {
  if (!await pauseBeforeNavigation()) return;
  sessionStorage.removeItem(savedRunKey());
  lesson.value = null; choice.value = ''; reason.value = ''; error.value = '';
}
async function returnToCollection() {
  if (!props.collection?.external) { await back(); return; }
  if (props.collectionLoading || props.collectionError || !await pauseBeforeNavigation()) return;
  emit('returnCollection');
}
async function openLesson(id?: string, runId?: string): Promise<CourseLessonView> {
  if (!catalog.value) {
    await load();
    if (!catalog.value) throw new Error(error.value || uiText('课程暂时无法读取，请重试。'));
  }
  const run = runId ? catalog.value?.progress.find(p => p.run_id === runId) : id ? latest(id) : undefined;
  if (runId && !run) throw new Error(uiText('这次课程记录暂时不可用，请从课程目录选择。'));
  if (pending.value) {
    const sameTarget = pending.value.action.kind === 'start'
      ? !runId && pending.value.action.lesson_id === id
      : !!run && pending.value.run_id === run.run_id;
    if (!sameTarget) throw new Error(error.value || uiText('课程操作尚未完成，请重试。'));
    await retry();
  } else if (run) await open(run);
  else if (id) await act({ kind: 'start', lesson_id: id }, true);
  if (error.value || pending.value) throw new Error(error.value || uiText('课程操作尚未完成，请重试。'));
  const current = lesson.value;
  if (!current || (id && current.summary.id !== id) || (runId && current.progress.run_id !== runId) || (!runId && current.progress.goal_revision !== props.workspace.goal.revision)) throw new Error(uiText('课程尚未打开，请重试。'));
  return current;
}
async function launch(item: NonNullable<CourseCatalog['course']>['lessons'][number]) {
  if (props.catalogOnly) emit('launch', { publication: item.source.publication, lessonId: item.id, runId: latest(item.id)?.run_id });
  else if (props.publication && (item.source.publication.book_id !== props.publication.book_id || item.source.publication.publication_id !== props.publication.publication_id)) {
    if (await pauseBeforeNavigation()) emit('launch', { publication:item.source.publication, lessonId:item.id, runId:latest(item.id)?.run_id });
  }
  else void openLesson(item.id).catch(failure => { error.value = failure instanceof Error ? failure.message : uiText('课程尚未打开，请重试。'); });
}
const routeStorageKey = (value:CourseCatalog) => `understand-book:course-route:${network.value.identity?.user_id ?? 'local'}:${props.workspace.learning_workspace_id}:${value.release_id}:${value.release_version}`;
watch(route, value => { if (catalog.value) sessionStorage.setItem(routeStorageKey(catalog.value),value); });
const savedRunKey = () => `understand-book:course:${network.value.identity?.user_id}:${props.workspace.learning_workspace_id}:${props.publication?.publication_id ?? ''}`;
watch(lesson, value => { emit('change', value); if (props.embedded && value) sessionStorage.setItem(savedRunKey(), value.progress.run_id); });
watch([course, lesson, oldRuns], ([catalog, current, history]) => emit('available', !!catalog?.lessons.length || !!current || !!history.length));
watch(() => `${network.value.epoch}:${network.value.identity?.user_id}:${props.workspace.learning_workspace_id}:${props.releaseVersion}:${props.publication?.publication_id}`, async () => {
  lesson.value = null; catalog.value = null; knownPublicationRuns.value.clear(); pending.value = null; busy.value = false; route.value = ''; void load();
}, { immediate: true });
async function resumeSaved(): Promise<CourseLessonView | null> {
  if (!catalog.value) {
    await load();
    if (!catalog.value) throw new Error(error.value || uiText('课程暂时无法读取，请重试。'));
  }
  const id = sessionStorage.getItem(savedRunKey());
  return id ? openLesson(undefined, id) : null;
}
async function applyClosedBookResult(resultRef: string): Promise<CourseLessonView> {
  const runId = lesson.value?.progress.run_id;
  if (!runId) throw new Error(uiText('课程尚未打开，请重试。'));
  if (busy.value || loading.value) throw new Error(uiText('课程操作尚未完成，请重试。'));
  if (props.workspace.archived) throw new Error(uiText('工作区已归档，可以查看记录；恢复工作区后可继续学习。'));
  if (goalChanged.value) throw new Error(uiText('学习目标已经改变。这次记录会保留；请从课程目录重新开始适合当前目标的课程。'));
  if (pending.value) {
    if (pending.value.run_id !== runId || pending.value.action.kind !== 'use_closed_book' || pending.value.action.result_ref !== resultRef) throw new Error(error.value || uiText('课程操作尚未完成，请重试。'));
    await retry();
  } else await act({ kind:'use_closed_book', result_ref:resultRef });
  if (busy.value || pending.value || error.value) throw new Error(error.value || uiText('课程操作尚未完成，请重试。'));
  const current = lesson.value;
  if (!current || current.progress.run_id !== runId) throw new Error(uiText('课程操作尚未完成，请重试。'));
  return current;
}
async function showTask() {
  labOpen.value = false;
  await nextTick();
  if (lesson.value?.decision && ['feedback','complete'].includes(lesson.value.progress.stage)) await decisionTutor.value?.focus();
  else if (activityExpression.value) await activityTutor.value?.focus();
  else await focusHeading();
}
defineExpose({ act, retry: () => pending.value ? retry() : lesson.value ? open(lesson.value.progress) : load(), openLesson, resumeSaved, applyClosedBookResult, back, returnToCollection, showExperiment: () => { labOpen.value = true; }, showTask });
let pollTimer: ReturnType<typeof setTimeout> | undefined;
let polling = false;
let disposed = false;
async function refreshReviews() {
  if (disposed || polling || blocked.value || !lesson.value || document.visibilityState === 'hidden') return;
  const current = lesson.value, token = generation;
  if (!current.expressions?.length) return;
  const needsRefresh = current.expressions.some(e=>!current.reviews.some(r=>r.expression.job_ref===e.job_ref)) || current.reviews.some(r => ['pending','running'].includes(r.status));
  if (!needsRefresh) return;
  polling = true;
  try {
    const value = await inquiryCourse.read(props.workspace.learning_workspace_id, current.progress.run_id);
    if (token !== generation || busy.value || pending.value || lesson.value?.progress.run_id !== current.progress.run_id || lesson.value.progress.revision > value.progress.revision) return;
    install(value);
  } catch (e) { failure(e, token); }
  finally { polling = false; }
}
function scheduleReviews() { if(!disposed)pollTimer = setTimeout(async () => { await refreshReviews(); scheduleReviews(); }, 2200); }
scheduleReviews();
async function revisitFeedback(job:string) {
  const expression=lesson.value?.expressions.find(e=>e.job_ref===job);if(!expression)return;
  if(expression.activity==='question'){reviewingJob.value=job;}else reviewingJob.value=lesson.value?.expressions.filter(e=>e.activity===expression.activity && (e.round_index ?? 0)===(expression.round_index ?? 0)).at(-1)?.job_ref ?? job;
  labOpen.value=false;await nextTick();await activityTutor.value?.focus();
}
async function askQuestion(response:string) {
  const before=lesson.value?.expressions.length;
  await act({kind:'ask_question',response});
  if(!pending.value && lesson.value && lesson.value.expressions.length!==(before??0)){
    questions.value?.clear();await revisitFeedback(lesson.value.expressions.at(-1)!.job_ref);
  }
}
async function openSupport(job_ref:string){await act({kind:'open_support',job_ref});await focusHeading();}
async function returnFromSupport(){
  const job=lesson.value?.support_visit?.job_ref;
  await act({kind:'return_from_support'});
  if(!pending.value && job){
    const expression=lesson.value?.expressions.find(e=>e.job_ref===job);
    if(expression && ['task','decision','transfer'].includes(expression.activity) && lesson.value?.decision){await nextTick();await decisionTutor.value?.focus();}
    else {reviewingJob.value=job;labOpen.value=false;await nextTick();await activityTutor.value?.focus();}
  }
}
async function continueActivity() {
  if(blocked.value || goalChanged.value || !active.value || !activityContinuation.value || activityContinuation.value.disabled)return;
  if(reviewingJob.value){reviewingJob.value=null;await focusHeading();return;}
  if(lesson.value?.progress.stage==='experiment' && !experimentReady.value){
    labOpen.value=false;await nextTick();
    experimentStage.value?.focus({preventScroll:true});
    experimentStage.value?.scrollIntoView({block:'start',behavior:'instant'});
    return;
  }
  await act({kind:'advance'});
}
async function replyActivity(intent: CourseReplyIntent, response: string) {
  const expression=activityExpression.value;
  if(!expression || !response.trim() || blocked.value)return;
  const state=exploration.value;
  if(!reviewingEarlier.value && state && Object.keys(state).some(key=>state[key as keyof CourseResourceState]!==lesson.value?.exploration?.[key as keyof CourseResourceState])) {
    await act({kind:'explore',state:{...state}});if(pending.value)return;
  }
  await act({kind:'feedback_reply',job_ref:expression.job_ref,response,intent});
  if(!pending.value && reviewingJob.value)reviewingJob.value=lesson.value?.expressions.at(-1)?.job_ref??null;
}
async function replyDecision(intent: CourseReplyIntent, response: string) {
  if (!response.trim() || blocked.value || !lesson.value?.decision) return;
  const state=exploration.value;
  if (state && Object.keys(state).some(key=>state[key as keyof CourseResourceState]!==lesson.value?.exploration?.[key as keyof CourseResourceState])) {
    await act({kind:'explore',state:{...state}}); if(pending.value) return;
  }
  await act({kind:'decision_reply',job_ref:lesson.value.decision.latest_job,response,intent});
}
async function submitChoice() {
  if (!lesson.value) return;
  if (lesson.value.decision && exploration.value) {
    await act({kind:'explore',state:{...exploration.value}});
    if (pending.value) return;
  }
  await act({kind:lesson.value.progress.stage === 'prediction' ? 'predict' : 'answer',response:{choice:choice.value,reason:reason.value}});
}
async function submitReflection() {
  if (!reflection.value.trim() || blocked.value) return;
  const state = exploration.value;
  if (lesson.value?.progress.stage === 'explanation' && state && Object.keys(state).some(key => state[key as keyof CourseResourceState] !== lesson.value?.exploration?.[key as keyof CourseResourceState])) {
    await act({ kind:'explore', state: { ...state } });
    if (pending.value) return;
  }
  await act({ kind:'reflect', response:reflection.value });
  if (!pending.value) reflection.value = '';
}
onBeforeUnmount(() => { disposed=true; generation++; clearTimeout(pollTimer); });
</script>

<template>
  <section v-if="loading || error || course || lesson || oldRuns.length" class="inquiry-course" :class="{ embedded }" :aria-label="uiText('探究课程')" :aria-busy="loading || busy">
    <p v-if="collectionLoading" class="muted" role="status">{{ uiText('正在读取课程归属…') }}</p>
    <div v-if="collectionError" class="collection-error" role="alert"><span>{{ collectionError }}</span><button class="text-button" @click="emit('retryCollection')">{{ uiText('重试读取课程归属') }}</button></div>
    <p v-if="loading" class="muted" role="status">{{ uiText('正在读取课程…') }}</p>
    <div v-if="error" class="course-error" role="alert"><p>{{ error }}</p><div class="actions"><button v-if="pending" :disabled="busy" @click="retry">{{ uiText('重试刚才的操作') }}</button><button :disabled="busy" @click="reconcile">{{ uiText('重新读取进度') }}</button></div><small v-if="pending">{{ uiText('重试会沿用同一次操作，避免重复记录。你填写的内容仍保留在这里。') }}</small></div>
    <p v-if="workspace.archived" class="notice">{{ uiText('工作区已归档，可以查看记录；恢复工作区后可继续学习。') }}</p>
    <template v-if="course && !lesson">
      <button v-if="collection?.external" class="text-button collection-return" :disabled="blocked || collectionLoading || !!collectionError" @click="returnToCollection">{{ collectionReturnLabel }}</button>
      <div class="catalog-heading"><div><span class="eyebrow">{{ uiText(collection?.kind === 'course' ? '整套课程' : '本书课程目录') }}</span><h3>{{ collection?.title || uiText('从一个问题开始') }}</h3><p class="provided-lessons">{{ uiText('已提供 {0} 课', [course.lessons.length]) }}</p></div><div class="course-count"><strong>{{ completed }}<span> / {{ course.lessons.length }}</span></strong><small>{{ uiText('课已完成') }}<span v-if="skipped"> · {{ skipped }} {{ uiText('课已跳过') }}</span></small></div></div>
      <div v-if="course.routes.length" class="route-select"><label for="course-route">{{ uiText('学习路线') }}</label><select id="course-route" v-model="route"><option value="">{{ uiText('按章节学习') }}</option><option v-for="item in course.routes" :key="item.id" :value="item.id">{{ item.title }}</option></select></div>
      <p v-if="selectedRoute" class="route-question">{{ selectedRoute.question }}</p>
      <section v-for="unit in lessonGroups" :key="unit.id" class="course-unit" :aria-label="unit.title"><h4>{{ unit.title }}</h4>
        <ol class="lesson-list"><li v-for="item in unit.lessons" :key="item.id"><span class="lesson-number" :class="{ done: latest(item.id)?.status === 'completed' }">{{ latest(item.id)?.status === 'completed' ? '✓' : String(lessons.indexOf(item) + 1).padStart(2, '0') }}</span><div class="lesson-info"><h5>{{ item.question }}</h5><p>{{ item.objective }}</p><div class="lesson-meta"><span>{{ uiText('约 {0} 分钟', [item.estimated_minutes]) }}</span><span>{{ statusLabel(latest(item.id)?.status) }}</span></div></div><button class="primary" :disabled="blocked" @click="launch(item)">{{ latest(item.id) ? (latest(item.id)?.status === 'completed' || latest(item.id)?.status === 'skipped' ? uiText('查看记录') : uiText('继续学习')) : uiText('开始这一课') }}</button></li></ol>
      </section>

      <p class="catalog-note">{{ uiText(collection?.kind === 'course' ? '按整套课程的单元与路线继续学习。' : '这里列出已提供的课程；未列出的章节仍可阅读原文。') }} {{ uiText('每轮都可以暂停，回来接着学；具体用时见各课说明。') }}</p>
    </template>
    <article v-else-if="lesson" class="lesson-player">
      <nav class="collection-breadcrumb" :aria-label="uiText('课程归属')"><span>{{ collectionTitle }}</span><span aria-hidden="true">›</span><span>{{ currentUnit }}</span><span aria-hidden="true">›</span><span aria-current="page">{{ lesson.summary.question }}</span></nav>
      <div class="lesson-top"><div class="lesson-navigation"><button v-if="collection?.external" class="text-button collection-return" :disabled="blocked || collectionLoading || !!collectionError" @click="returnToCollection">{{ collectionReturnLabel }}</button><button class="text-button" :disabled="busy || !!pending" @click="back">{{ uiText(collection ? '课程总目录' : '← 课程目录') }}</button></div><div class="lesson-actions"><span>{{ uiText('约 {0} 分钟', [lesson.summary.estimated_minutes]) }} · {{ statusLabel(lesson.progress.status) }}</span><button v-if="canRestart && (lesson.progress.stage !== 'introduction' || !active || goalChanged || lesson.progress.release_version !== catalog?.release_version)" class="restart-button" :disabled="blocked" :title="uiText('从第一步开始，已有学习记录会保留')" @click="act({ kind: 'start', lesson_id: lesson.summary.id }, true)">{{ uiText('从头学习') }}</button></div></div>
      <h3 ref="heading" tabindex="-1">{{ lesson.summary.question }}</h3>
      <p class="course-source">{{ lesson.summary.source.label }}</p>
      <details v-if="multiRound" class="course-route-overview"><summary>{{ uiText('查看本课路线') }}</summary><ol class="round-map" :aria-label="uiText('本课探究路线')"><li v-for="(title,index) in [...lesson.round_titles, uiText('综合应用')]" :key="index" :class="{ current: finalTask ? index === lesson.round_titles.length : index === roundIndex, done: index < (finalTask ? lesson.round_titles.length : roundIndex) || lesson.progress.stage === 'complete' }" :aria-current="(finalTask ? index === lesson.round_titles.length : index === roundIndex) ? 'step' : undefined"><span>{{ index < (finalTask ? lesson.round_titles.length : roundIndex) || lesson.progress.stage === 'complete' ? '✓' : String(index + 1).padStart(2,'0') }}</span><strong>{{ title }}</strong></li></ol></details>
      <p v-if="multiRound && !finalTask" class="round-caption">{{ uiText('第 {0} / {1} 轮', [roundIndex + 1, lesson.round_titles.length]) }} · {{ lesson.round_titles[roundIndex] }}</p>
      <ol class="stage-track" :aria-label="uiText('学习进度')"><li v-for="(stage,index) in stages" :key="stage.id" :class="{ current: stageIndex === index, passed: stageIndex > index }" :aria-current="stageIndex === index ? 'step' : undefined"><span>{{ stageIndex > index ? '✓' : index + 1 }}</span>{{ uiText(stage.label) }}</li></ol>
      <section v-if="currentTask" class="current-task" :aria-label="uiText('当前学习任务')">
        <span class="eyebrow">{{ uiText('当前任务') }}</span><h4>{{ currentTask.title }}</h4>
        <p>{{ currentTask.detail }}</p><p class="task-goal"><strong>{{ uiText('本课目标') }}</strong>{{ lesson.summary.objective }}</p>
      </section>
      <button class="text-button" @click="emit('understanding',lesson.summary.source.publication)">{{ uiText('我的理解与进展 →') }}</button>
      <aside v-if="readingPrompt" class="source-activity"><span class="eyebrow">{{ uiText('带着问题读原文') }}</span><p>{{ readingPrompt }}</p><button :disabled="blocked" @click="showSource">{{ lesson.progress.stage === 'explanation' ? uiText('核对本课来源 →') : uiText('查看本课来源 →') }}</button><small>{{ uiText('返回后接着当前任务，回答与活动记录会保留。') }}</small></aside>
      <p v-if="goalChanged" class="notice">{{ uiText('学习目标已经改变。这次记录会保留；请从课程目录重新开始适合当前目标的课程。') }}</p>
      <div v-if="lesson.progress.status === 'paused'" class="pause-card"><h4>{{ uiText('停在这里也没关系') }}</h4><p>{{ uiText('你的初步判断、活动记录和已提交回答都已保存。') }}</p><button class="primary" :disabled="blocked || goalChanged" @click="act({ kind: 'resume' })">{{ uiText('从这里继续') }}</button></div>
      <template v-else-if="lesson.progress.status === 'skipped'"><div class="finish-card"><span class="eyebrow">{{ uiText('已保留跳过记录') }}</span><h4>{{ uiText('这一课先放一放') }}</h4><p>{{ uiText('跳过不会记为答对或学会。准备好后，可以重新开始。') }}</p></div></template>
      <template v-else>
        <CourseSupportBranch v-if="lesson.support_visit" :visit="lesson.support_visit" :disabled="blocked || goalChanged" @return="returnFromSupport" />
        <div v-show="!lesson.support_visit" class="activity-workspace" :class="{ explaining: (explaining || tutoring) && (lesson.experiment || earlierExperiment), tutoring: tutoring || (!!activityExpression && explaining) }">
        <div v-if="lesson.experiment?.kind === 'resource_bounds'" ref="experimentStage" tabindex="-1" v-show="!reviewingQuestion && !reviewingEarlier && (lesson.progress.stage === 'experiment' || labOpen || explaining || tutoring)" :key="lesson.progress.run_id + ':' + roundIndex + ':' + lesson.decision?.phase" class="stage-content experiment-stage"><div v-show="!explaining && !tutoring"><span class="eyebrow">{{ uiText('动手观察') }}</span><h4>{{ uiText('只改一个条件，会发生什么？') }}</h4><p class="prediction-note">{{ uiText('你的预测：') }}{{ lesson.prediction.options.find(o => o.id === lesson?.prediction_response?.choice)?.text }}</p></div><ResourceDecisionLab ref="experimentLab" v-if="lesson.lab" :review="explaining || tutoring" :scene-key="lesson.progress.run_id + ':' + roundIndex + ':' + lesson.decision?.phase" :presented="!sourceOpen" @question="question=>emit('discussion',question)" :experiment="lesson.experiment" :lab="lesson.lab" :saved="lesson.exploration" @change="updateExploration" :observed="lesson.observed" :disabled="blocked || goalChanged || !active" @observe="preset => act({ kind:'observe', preset })" @save="state => act({ kind:'explore', state })" /><ResourceBoundsExperiment v-else :experiment="lesson.experiment" :preset="lesson.preset" :observed="lesson.observed" :disabled="blocked || goalChanged || !active" @observe="preset => act({ kind: 'observe', preset })" /><p v-show="!explaining && !tutoring" class="muted">{{ uiText('已观察 {0} / 3 种配置', [lesson.observed.length]) }}</p><button v-if="lesson.progress.stage === 'experiment'" class="primary" :disabled="blocked || !experimentReady || goalChanged" @click="act({ kind: 'advance' })">{{ uiText('对照预测，看看原因 →') }}</button></div>
        <div v-if="lesson.experiment?.kind === 'inquiry' && !reviewingEarlier && (!lesson.experiment.visualization || ['experiment','explanation'].includes(lesson.progress.stage))" ref="experimentStage" tabindex="-1" class="stage-content inquiry-stage"><CourseInquiryActivity :key="lesson.progress.run_id + ':' + roundIndex" :activity="lesson.experiment" :response="lesson.activity_response" :visualization="lesson.math_exploration" :draft-key="`${lesson.progress.run_id}:${lesson.round_index}`" @dirty="inquiryDirty=$event" :disabled="blocked || goalChanged || !active" :readonly="lesson.progress.stage !== 'experiment'" @save="(response,visualization) => act({kind:'investigate',response,visualization})" /><button v-if="lesson.progress.stage === 'experiment'" class="primary" :disabled="blocked || !experimentReady || goalChanged" @click="act({kind:'advance'})">{{ uiText('对照依据，继续解释 →') }}</button></div>
        <div v-if="earlierExperiment && !labOpen" class="stage-content experiment-stage"><span class="eyebrow">{{ uiText('对照这一轮保存的实验') }}</span><ResourceDecisionLab input-prefix="course-history-value" :scene-key="lesson.progress.run_id + ':history:' + activityExpression?.round_index" @question="question=>emit('discussion',question)" :experiment="{kind:'resource_bounds',instruction:'',...earlierExperiment}" :lab="earlierExperiment" :saved="earlierExperiment" :observed="[]" review disabled /></div>
        <CourseMathExperiment v-if="earlierMathExperiment && !labOpen" :model-value="earlierMathExperiment" disabled />
        <div v-show="!labOpen" class="activity-task">
        <div v-show="!reviewingEarlier && !reviewingQuestion">
        <div v-if="lesson.progress.stage === 'introduction'" class="stage-content"><span class="eyebrow">{{ uiText('01 / 准备') }}</span><h4>{{ uiText('先认识这个问题') }}</h4><div class="course-prose" v-html="renderMarkdown(lesson.introduction ?? '')" /><CourseResourceObjects v-if="resourceActivity" :scene-key="lesson.progress.run_id + ':intro'" :presented="!sourceOpen" @question="question=>emit('discussion',question)" /><div v-if="lesson.concepts?.length" class="concepts"><article v-for="(concept,i) in lesson.concepts" :key="concept.title"><span class="concept-index">0{{ i + 1 }}</span><div><h5>{{ concept.title }}</h5><div class="course-prose" v-html="renderMarkdown(concept.explanation)" /><div class="relation course-prose" v-html="renderMarkdown(concept.relation)" /></div></article></div><div class="prerequisite"><strong>{{ uiText('开始前') }}</strong><p>{{ lesson.summary.prerequisites }}</p></div><button class="primary" :disabled="blocked || goalChanged" @click="act({ kind: 'advance' })">{{ uiText('做出我的预测 →') }}</button></div>
        <form v-else-if="lesson.progress.stage === 'prediction' || lesson.progress.stage === 'task'" class="stage-content" @submit.prevent="submitChoice">
          <span class="eyebrow">{{ lesson.progress.stage === 'prediction' ? uiText('先猜后验') : uiText('综合应用 / 换个情境') }}</span><h4>{{ lesson.progress.stage === 'prediction' ? uiText(prompt?.options.length ? '现在，你会怎么选？' : '现在，你会怎样判断？') : uiText('这次由你来判断') }}</h4>
          <div class="course-prose question" v-html="renderMarkdown(prompt?.question ?? '')" /><fieldset :disabled="blocked || goalChanged"><legend class="sr-only">{{ uiText('选择一个回答') }}</legend><label v-for="option in prompt?.options" :key="option.id" class="option" :class="{ selected: choice === option.id }"><input v-model="choice" type="radio" name="course-answer" :value="option.id" required /><span>{{ option.text }}</span></label><label class="reason-label" for="course-reason">{{ lesson.progress.stage === 'prediction' ? uiText('为什么这样猜？') : uiText(lesson.inquiry_kind === 'reading' ? '写下你的判断和依据' : '写下你的计算或理由') }}<span v-if="!trialLesson">{{ uiText('（会收到针对这段思路的反馈）') }}</span></label><textarea id="course-reason" v-model="reason" :rows="lesson.progress.stage === 'task' ? 8 : 3" :placeholder="lesson.progress.stage === 'prediction' ? uiText('可以从直觉说起，不确定也没关系。') : uiText(lesson.inquiry_kind === 'reading' ? '哪些条件支持你的判断？' : '用题目中的条件说明你的计算和判断。')" /></fieldset>
          <p v-if="lesson.progress.stage === 'prediction'" class="muted">{{ uiText('预测用来和观察对照，猜错也可以继续。') }}</p><p v-else class="muted">{{ prompt?.options.length ? uiText('选择只核对方案；你的文字会单独分析，并保存到“我的理解”。不写理由时不会据选择推断理解。') : uiText('请写出推理和依据。回应会保留到“我的理解”，提交本身不代表已通过判断。') }}</p>
          <div v-if="lesson.hint" class="hint-card"><strong>{{ uiText('提示') }}</strong><div class="course-prose" v-html="renderMarkdown(lesson.hint ?? '')" /><small>{{ uiText('这次作答会保留“看过提示”的记录。') }}</small></div>
          <div class="actions"><button class="primary" type="submit" :disabled="blocked || (prompt?.options.length ? !choice : !reason.trim()) || goalChanged">{{ lesson.progress.stage === 'prediction' ? uiText('保存初步判断，开始探究 →') : uiText('提交并查看反馈 →') }}</button><button v-if="lesson.progress.stage === 'task' && !lesson.hint" type="button" :disabled="blocked || goalChanged" @click="act({ kind: 'hint' })">{{ uiText('给我一点提示') }}</button></div>
        </form>

        <div v-else-if="lesson.progress.stage === 'explanation'" class="stage-content"><span class="eyebrow">{{ uiText('把观察连起来') }}</span><h4>{{ multiRound ? lesson.round_titles[roundIndex] : resourceActivity ? uiText('等得最久的那一项，决定了下界') : lesson.summary.question }}</h4><details v-if="activityExpression" class="prepared-explanation"><summary>{{ uiText('回看本轮讲解') }}</summary><div class="course-prose" v-html="renderMarkdown(lesson.explanation ?? '')" /></details><div v-else class="course-prose" v-html="renderMarkdown(lesson.explanation ?? '')" /><form v-if="lesson.reflection_prompt && !activityExpression" class="reflection-form" @submit.prevent="submitReflection"><label for="course-reflection"><strong>{{ uiText('把观察变成自己的解释') }}</strong><div class="course-prose" v-html="renderMarkdown(lesson.reflection_prompt)" /></label><textarea id="course-reflection" v-model="reflection" rows="3" :disabled="blocked || goalChanged" :placeholder="uiText('把观察或材料中的依据与你的判断连起来。')" /><button :disabled="blocked || !reflection.trim() || goalChanged">{{ uiText('提交我的解释') }}</button><small v-if="lesson.lab">{{ uiText('提交解释时，会一并保存图中使用的实验配置。') }}</small><small v-if="reflectionSaved">{{ uiText('回应会直接出现在当前活动中。') }}</small></form><p v-if="multiRound && !reflectionSaved" class="muted">{{ uiText('写下本轮的解释后继续；不确定的地方也可以如实记录。') }}</p></div>
        <CourseDecisionTutor v-else-if="lesson.decision && ['feedback','complete'].includes(lesson.progress.stage)" :finished="lesson.progress.stage === 'complete'" ref="decisionTutor" :decision="lesson.decision" :reviews="lesson.reviews" v-model:draft="replyDraft" :disabled="blocked || goalChanged" @support="openSupport" @understanding="emit('understanding',lesson.summary.source.publication)" @reply="replyDecision" @read="job_ref => act({kind:'read_feedback',job_ref})" @retry="job_ref => act({kind:'retry_feedback',job_ref})" @advance="act({kind:'advance'})" @pause="act({kind:'pause'})" @inspect="lesson.lab && experimentLab?.useState(lesson.lab)" />
        <div v-else-if="lesson.progress.stage === 'feedback' || lesson.progress.stage === 'complete'" class="stage-content"><span class="eyebrow">{{ lesson.progress.status === 'completed' ? uiText('本课已完成') : uiText('06 / 回看你的判断') }}</span><h4>{{ lesson.assessment_status === 'unassessed' ? uiText('对照参考推理，回看自己的回答') : lesson.assessment_status === 'correct' ? uiText('方案选择正确，再看理由是否成立') : uiText('一起看看，差别出在哪里') }}</h4><div class="answer-review"><strong>{{ uiText('你的回答') }}</strong><p v-if="selectedAnswer">{{ selectedAnswer }}</p><p v-if="lesson.answer?.reason" class="course-prose">{{ uiText('你的思路：') }}{{ lesson.answer.reason }}</p><small>{{ lesson.hint ? uiText('看过提示后作答') : uiText('完成讲解后作答') }} {{ uiText('· 记录属于这一题的表现') }}</small></div><div class="solution"><h5>{{ uiText('完整推理与参考方案') }}</h5><div class="course-prose" v-html="renderMarkdown(lesson.solution ?? '')" /></div><form v-if="active && !activityExpression" class="reflection-form" @submit.prevent="submitReflection"><label for="course-revision"><strong>{{ uiText('现在，你会怎样修正或补充？') }}</strong></label><textarea id="course-revision" v-model="reflection" rows="3" :disabled="blocked || goalChanged" :placeholder="uiText('回应反馈中的追问，或补充你的推理依据。')" /><button :disabled="blocked || !reflection.trim() || goalChanged">{{ uiText('保存这次理解变化') }}</button></form><button v-if="active && !activityExpression" class="primary" :disabled="blocked || goalChanged" @click="act({ kind: 'advance' })">{{ uiText('完成这一课 ✓') }}</button><p v-else class="course-prose">{{ lesson.summary.objective }}</p></div>
        </div>
        <p v-if="reviewingEarlier" class="earlier-feedback-note">{{ uiText('先接上这一轮的回应，当前课程进度会保留。') }}</p>
        <CourseActivityTutor v-if="activityExpression" ref="activityTutor" :expression="activityExpression" :review="activityReview" v-model:draft="activityReply" :disabled="blocked || goalChanged" :readonly="!active" :presented="!sourceOpen && !labOpen && !lesson.support_visit" :continuation="activityContinuation" @continue="continueActivity" @read="job_ref => act({kind:'read_feedback',job_ref})" @retry="job_ref => act({kind:'retry_feedback',job_ref})" @support="openSupport" @reply="replyActivity" @understanding="emit('understanding',lesson.summary.source.publication)" />
        <button v-if="lesson.progress.stage === 'explanation' && !reviewingEarlier && !activityExpression" class="primary" :disabled="blocked || goalChanged || (multiRound && !reflectionSaved) || (!!activityExpression && !activityReview?.displayed)" @click="act({kind:'advance'})">{{ moreRounds ? uiText('继续下一轮探究 →') : uiText('进入综合应用 →') }}</button>
        <button v-if="reviewingJob && !active" class="text-button" @click="reviewingJob=null; activityReply=''; focusHeading()">{{ uiText('回到当前活动 →') }}</button>
        </div>
      </div>
      </template>
      <aside v-if="contextualPractice" class="contextual-practice" :aria-label="uiText('试着自己解释或应用')">
        <div><strong>{{ uiText('想检验刚才的理解？') }}</strong><p>{{ uiText('闭卷表达会收起资料，按回答追问或补讲，可随时暂停。') }}</p></div>
        <div class="actions"><button type="button" :disabled="blocked" @click="emit('closedBook')">{{ lesson.progress.stage === 'explanation' ? uiText('我来解释一下') : uiText('练到会') }}</button><button type="button" :disabled="blocked" @click="emit('practice')">{{ uiText('给我一道应用题') }}</button></div>
      </aside>
      <section v-if="currentReflection && lesson.prediction_response?.reason && !lesson.support_visit" class="thought-comparison" aria-label="对照最初猜想与现在解释"><div><span>一开始的猜想</span><blockquote>{{lesson.prediction_response.reason}}</blockquote></div><div><span>观察后的解释</span><blockquote>{{currentReflection.response}}</blockquote><small>保留当时表达，具体变化见本课理解。</small></div></section>
      <CourseQuestions v-show="!lesson.support_visit" :key="lesson.progress.run_id" ref="questions" :expressions="lesson.expressions" :resolved="lesson.resolved_questions ?? []" :disabled="blocked || goalChanged || !active" @ask="askQuestion" @revisit="revisitFeedback" @resolve="(fact_ref,resolved)=>act({kind:'resolve_question',fact_ref,resolved})" />
      <section v-if="lesson.progress.status==='completed'" class="finish-card next-lesson" :aria-label="uiText('本课学习结果')">
        <span class="eyebrow">{{ uiText('学习进度') }}</span><h4>{{ uiText('本课已完成') }}</h4>
        <div v-if="completionEvidence.length" class="completion-evidence"><strong>{{ uiText('本次作答体现的理解') }}</strong>
          <article v-for="evidence in completionEvidence" :key="evidence.evidence_ref"><p>{{ evidence.interpretation }}</p><ul v-if="evidence.demonstrated.length"><li v-for="item in evidence.demonstrated" :key="item">{{ item }}</li></ul><p v-if="evidence.difficulties.length || evidence.unconfirmed.length" class="muted">{{ uiText('还可以继续验证：') }}{{ [...evidence.difficulties,...evidence.unconfirmed].join('；') }}</p><small v-if="evidence.assistance">{{ evidence.assistance }}</small></article>
        </div>
        <p v-else>{{ uiText('作答、反馈和获得的帮助已经保留，可以回看本次表现。') }}</p>
        <div v-if="nextLesson" class="next-lesson-target"><span class="eyebrow">{{ uiText('下一课') }}</span><h5>{{ nextLesson.question }}</h5><p>{{ nextLesson.objective }}</p><button class="primary" :disabled="blocked" @click="launch(nextLesson)">{{ uiText('进入下一课 →') }}</button></div>
        <div class="actions"><button class="text-button" :disabled="blocked" @click="back">{{ uiText('今天先到这里') }}</button><button class="text-button" :disabled="blocked" @click="emit('understanding',lesson.summary.source.publication)">{{ uiText('查看本次表现与依据') }}</button></div>
      </section>
      <details v-if="lesson.round_history?.length" class="round-history" :open="roundHistoryOpen" @toggle="roundHistoryOpen = ($event.target as HTMLDetailsElement).open"><summary>{{ uiText('回看前面的探究（{0}）', [lesson.round_history.length]) }}</summary><article v-for="(record,index) in lesson.round_history" :key="index"><h5>{{ index + 1 }} · {{ record.title }}</h5><div class="course-prose" v-html="renderMarkdown(record.prediction.question)" /><strong>{{ uiText('当时的预测') }}</strong><p>{{ record.prediction.options.find(o => o.id === record.response.choice)?.text }}</p><p v-if="record.response.reason" class="course-prose">{{ record.response.reason }}</p><p v-if="record.exploration" class="history-config">F = {{ record.exploration.work_gflops }} GFLOPs · R = {{ record.exploration.read_gb }} GB · C = {{ record.exploration.compute_tflops }} TFLOP/s · W = {{ record.exploration.bandwidth_tb_s }} TB/s · T = {{ record.exploration.target_ms }} ms</p><CourseMathExperiment v-if="roundHistoryOpen && record.math_exploration" :model-value="record.math_exploration" disabled /><template v-if="record.activity_response"><strong>{{ uiText('当时的探究记录') }}</strong><div class="course-prose" v-html="renderMarkdown(record.activity_response)" /></template><strong v-if="record.reflection">{{ uiText('实验后的解释') }}</strong><p v-if="record.reflection" class="course-prose">{{ record.reflection }}</p><details><summary>{{ uiText('回看本轮讲解') }}</summary><div class="course-prose" v-html="renderMarkdown(record.explanation)" /></details></article></details>
      <details v-if="feedbackHistory.length" class="feedback-history"><summary>{{ uiText('回看前面的反馈（{0}）', [feedbackHistory.length]) }}</summary><CourseReviews @continue="revisitFeedback" :reviews="feedbackHistory" :expressions="feedbackHistory.map(r=>r.expression)" :disabled="blocked" @retry="job_ref => act({ kind:'retry_feedback', job_ref })" @read="job_ref => act({ kind:'read_feedback', job_ref })" @understanding="emit('understanding', lesson.summary.source.publication)" /></details>
      <footer class="lesson-footer">
        <div><span>{{ lesson.summary.source.label }}</span><small>{{ uiText('原文 {0}–{1} · 课程第 {2} 版', [lesson.summary.source.start_lid, lesson.summary.source.end_lid, lesson.progress.release_version]) }}</small><button v-if="active || lesson.source_excerpt" class="text-button" :disabled="busy || !!pending || (!lesson.source_excerpt && (workspace.archived || goalChanged))" @click="showSource">{{ uiText('查看本课来源') }}</button></div>
        <div v-if="active" class="actions"><button class="text-button" :disabled="blocked" @click="act({ kind: 'pause' })">{{ uiText('暂停，稍后继续') }}</button><button class="text-button" :disabled="blocked" @click="act({ kind: 'skip' })">{{ uiText('跳过这一课') }}</button></div>
      </footer>
      <BoardSourcePreview v-if="sourceOpen" context="course" :source="sourcePreview" :trigger="sourceTrigger"
        :loading="busy && pending?.action.kind === 'source'" :error="pending?.action.kind === 'source' ? error : ''"
        :read-window="readCourseSource" @close="sourceOpen = false" @retry="retry" @open-reader="openSourceInReader">
        <template #sources><div v-if="readingPrompt" class="course-source-task"><strong>{{ uiText('这次要找的依据') }}</strong><p>{{ readingPrompt }}</p></div></template>
      </BoardSourcePreview>
      <details class="duration"><summary>{{ uiText('关于学习用时') }}</summary><p>{{ lesson.summary.duration_basis }}</p></details>
    </article>
      <details v-if="!lesson && oldRuns.length" class="history"><summary>{{ uiText('以前的学习记录（{0}）', [oldRuns.length]) }}</summary><button v-for="run in oldRuns" :key="run.run_id" :disabled="blocked" @click="open(run)">{{ uiText('第 {0} 版', [run.release_version]) }} · {{ course?.lessons.find(l => l.id === run.lesson_id)?.question ?? run.lesson_id }} · {{ statusLabel(run.status) }}</button></details>
  </section>
</template>

<style scoped>
.collection-breadcrumb{display:flex;align-items:baseline;flex-wrap:wrap;gap:6px 9px;margin:0 0 12px;font-size:11px;line-height:1.8;color:var(--muted)}.collection-breadcrumb>[aria-current]{color:var(--ink)}.collection-breadcrumb>[aria-hidden]{color:var(--brand-accent)}.lesson-navigation{display:flex;flex-wrap:wrap;gap:6px 18px}.collection-error{display:flex;flex-wrap:wrap;gap:6px 18px;align-items:center;border-left:2px solid var(--brand-accent);padding:8px 14px;margin-bottom:18px;font-size:12px;background:var(--surface-soft)}.provided-lessons{font-size:12px;color:var(--muted);margin:-14px 0 22px}.course-unit{margin:22px 0}.course-unit>h4{font-size:16px;font-weight:600;margin:0 0 4px;color:var(--brand-accent)}.lesson-info h5{font-size:18px;line-height:1.5;margin:4px 0 7px;font-weight:600}@media(max-width:600px){.collection-breadcrumb{font-size:11px}.lesson-top{align-items:flex-start;flex-wrap:wrap}.lesson-navigation{gap:4px 16px}.course-unit>h4{font-size:15px}}
.current-task{margin:18px 0 12px;padding:18px 20px;border-left:3px solid var(--brand-accent);border-radius:0 10px 10px 0;background:var(--surface-soft)}.current-task h4{margin:6px 0;font-size:18px}.current-task>p{margin:6px 0;font-size:13px;line-height:1.8}.current-task .task-goal{margin-top:12px;font-size:12px;color:var(--muted)}.task-goal strong{margin-right:9px;font-weight:500;color:var(--ink)}.course-route-overview{margin:18px 0;color:var(--muted);font-size:12px}.course-route-overview .round-map{margin:12px 0}.contextual-practice{margin:24px 0;padding:18px 0;border-block:1px solid var(--line);display:flex;flex-wrap:wrap;align-items:center;justify-content:space-between;gap:14px 22px}.contextual-practice>div:first-child{flex:1 1 240px}.contextual-practice strong{font-size:13px}.contextual-practice p{margin:5px 0 0;font-size:12px;line-height:1.8;color:var(--muted)}.contextual-practice button{font-size:12px}.completion-evidence{padding:18px 0;border-block:1px solid var(--line);margin:18px 0}.completion-evidence>strong{font-size:13px}.completion-evidence p,.completion-evidence li{font-size:13px;line-height:1.8}.completion-evidence small{color:var(--muted)}.next-lesson-target{margin:22px 0}.next-lesson-target h5{font-size:18px;margin:6px 0 10px}.next-lesson-target p{font-size:13px}.next-lesson>.actions{margin-top:12px;gap:18px}@media(max-width:600px){.current-task{padding:15px}.current-task h4{font-size:17px}.contextual-practice>.actions{width:100%}.contextual-practice button{flex:1 1 130px}.course-route-overview{margin-block:14px}}
div.course-prose{white-space:normal}div.course-prose :deep(p){margin:0 0 1em}div.course-prose :deep(li){margin:.4em 0}div.course-prose :deep(.katex-display){overflow-x:auto;overflow-y:hidden;padding:4px 0}
.thought-comparison{display:grid;grid-template-columns:1fr 1fr;gap:20px;border-block:1px solid var(--line);margin:24px 0;padding:22px 0}.thought-comparison span{font:500 11px/1.6 var(--sans);color:var(--brand-accent)}.thought-comparison blockquote{font:14px/1.9 var(--sans);margin:10px 0 0;white-space:pre-line;overflow-wrap:anywhere}.thought-comparison small{display:block;margin-top:8px;font-size:11px;color:var(--muted)}@media(max-width:600px){.thought-comparison{grid-template-columns:1fr;gap:18px}}

.experiment-stage{scroll-margin-top:16px}
.inquiry-stage>.primary{margin-top:16px}
.feedback-history,.prepared-explanation{margin:20px 0;color:var(--muted);font-size:12px}.prepared-explanation .course-prose{color:var(--ink)}
.course-source-task{flex-shrink:0;padding:12px 26px;border-bottom:1px solid var(--hairline);background:var(--surface-soft);color:var(--ink);font:13px/1.7 var(--sans);max-height:25dvh;overflow:auto}.course-source-task strong{font-size:11px;color:var(--brand-accent)}.course-source-task p{margin:4px 0 0}@media(max-width:600px){.course-source-task{padding:10px 17px;font-size:12px}}
.lesson-actions{display:flex;flex-wrap:wrap;align-items:center;justify-content:flex-end;gap:8px 14px}.lesson-actions .restart-button{min-height:36px;padding:6px 12px;font-size:12px;white-space:nowrap}
.tutoring{--course-teaching-offset:calc(min(330px,44dvh) + 16px)}.activity-workspace.tutoring>.experiment-stage{max-height:min(330px,44dvh)}
@container (max-width:779px){
  .tutoring :deep(.decision-lab.review){display:grid;grid-template-columns:minmax(0,1fr) 105px;gap:8px}
  .tutoring :deep(.review .plot-switch){grid-column:1/-1;grid-row:1;margin:0}
  .tutoring :deep(.review .plots){grid-column:1;grid-row:2}
  .tutoring :deep(.review .plots svg){height:190px}
  .tutoring :deep(.review .readouts){grid-column:2;grid-row:2;grid-template-columns:1fr;margin:0}
  .tutoring :deep(.review .readouts>div){padding:7px}
  .tutoring :deep(.review .readouts>div+div){border-left:0;border-top:1px solid var(--line)}
  .tutoring :deep(.review .target-result){grid-column:1/-1;grid-row:3;margin:0}
  .tutoring :deep(.review .batch-readouts){grid-column:1/-1;grid-row:4}
  .tutoring :deep(.review .plots.objects){grid-column:1/-1}
  .tutoring :deep(.review:has(.plots.objects) .readouts){grid-column:1/-1;grid-row:2;grid-template-columns:repeat(3,minmax(0,1fr))}
  .tutoring :deep(.review .plots.objects){grid-row:3}
  .tutoring :deep(.review:has(.plots.objects) .target-result){grid-row:4}
  .tutoring :deep(.review:has(.plots.objects) .batch-readouts){grid-row:5}
  .tutoring :deep(.review .presets){grid-row:6!important}
  .tutoring :deep(.review .controls){grid-row:7!important}
  .tutoring :deep(.review .lab-actions){grid-row:8!important}
  .tutoring :deep(.review .presets){grid-column:1/-1;grid-row:4}
  .tutoring :deep(.review .controls){grid-column:1/-1;grid-row:5}
  .tutoring :deep(.review .lab-actions){grid-column:1/-1;grid-row:6}
}
@container (min-width:780px){.tutoring{--course-teaching-offset:16px}.activity-workspace.tutoring>.experiment-stage{max-height:calc(100dvh - 140px)}.tutoring :deep(.review .plots svg){height:260px}}
.lesson-player{container-type:inline-size}.activity-workspace.explaining{display:grid;gap:24px;align-items:start}.explaining>.experiment-stage{position:sticky;top:0;z-index:2;background:var(--canvas);padding:10px;border:1px solid var(--line);border-radius:12px;max-height:52dvh;overflow:auto;box-shadow:0 5px 12px #00000008}.explaining>.activity-task{min-width:0}.explaining .course-prose{font-size:14px;line-height:1.9}.explaining .reflection-form{scroll-margin-top:calc(52dvh + 16px)}
@container (min-width:780px){.activity-workspace.explaining{grid-template-columns:minmax(0,1fr) minmax(0,1fr);gap:24px}.explaining>.experiment-stage{max-height:calc(100dvh - 140px)}.explaining .reflection-form{scroll-margin-top:16px}}

.course-source{margin:-12px 0 24px;font-size:12px;color:var(--muted);line-height:1.8}.round-map{list-style:none;display:grid;grid-template-columns:repeat(4,minmax(0,1fr));padding:0;margin:20px 0 24px;gap:8px}.round-map li{display:grid;align-content:start;gap:10px;padding:14px 12px;border:1px solid var(--line);border-radius:10px;color:var(--muted);background:var(--canvas)}.round-map li>span{font-size:11px;letter-spacing:.05em}.round-map strong{font-size:12px;font-weight:500;line-height:1.7}.round-map .current{border-color:var(--brand-accent);color:var(--brand-accent);background:var(--surface-soft)}.round-map .done{color:var(--ink)}.round-caption{font-size:12px;color:var(--brand-accent);margin:20px 0 16px}.round-history{border:1px solid var(--line);border-radius:12px;padding:16px 20px;margin:24px 0;font-size:13px}.round-history>article{border-top:1px solid var(--line);margin-top:16px;padding-top:16px}.round-history h5{font-size:16px;margin:0 0 14px}.round-history .course-prose{font-size:13px;margin:8px 0 18px}.round-history strong{font-size:12px}.history-config{font-size:11px;color:var(--muted);padding:12px;background:var(--surface-soft);border-radius:8px}@media(max-width:650px){.round-map{grid-template-columns:repeat(2,minmax(0,1fr))}.round-map li{padding:12px}.round-history{padding:14px}}

.inquiry-course.embedded{margin:0;border:0;border-radius:0;padding:24px clamp(18px,3vw,40px)}.embedded .lesson-player{max-width:940px;margin:auto}.source-activity{margin:22px 0;padding:18px 20px;border-left:3px solid var(--brand-accent);background:var(--surface-soft);border-radius:0 10px 10px 0}.source-activity p{margin:6px 0 12px}.source-activity small{display:block;color:var(--muted);margin-top:8px;font-size:11px}

.concepts{display:grid;gap:14px;margin:26px 0}.concepts article{display:flex;align-items:flex-start;gap:16px;padding:20px;border:1px solid var(--line);border-radius:12px}.concept-index{font-size:12px;color:var(--brand-accent);padding-top:3px}.concepts h5{font-size:16px;margin:0 0 7px}.concepts p{font-size:13px;line-height:1.9;margin:0 0 12px;color:var(--muted)}.relation{font-size:14px;padding:10px 13px;background:var(--surface-soft);border-radius:8px;font-variant-numeric:tabular-nums}.reflection-form{padding:20px;border:1px solid var(--line);border-radius:12px;margin:24px 0}.reflection-form label{font-size:13px}.reflection-form label p{color:var(--muted)}.reflection-form textarea{margin:12px 0}.reflection-form small{display:block;color:var(--muted);font-size:11px;margin-top:10px}.solution{padding:16px 0}.solution summary{font-size:13px;color:var(--brand-accent)}

.inquiry-course{color:var(--ink);background:var(--canvas);border:1px solid var(--line);border-radius:16px;margin-top:20px;padding:clamp(18px,3vw,32px);font-size:14px;line-height:1.75;overflow-wrap:anywhere}.catalog-heading{display:flex;align-items:center;justify-content:space-between;gap:20px}.eyebrow{font-size:11px;font-weight:600;letter-spacing:.07em;color:var(--muted)}h3{font-size:clamp(22px,2.4vw,29px);font-weight:600;line-height:1.45;letter-spacing:-.025em;margin:8px 0 24px}h3:focus{outline:none}h4{font-size:20px;line-height:1.5;margin:9px 0 20px;font-weight:600}.course-count{display:grid;text-align:right;flex-shrink:0}.course-count strong{font-size:28px;line-height:1.3}.course-count strong span{font-size:16px;color:var(--muted);font-weight:400}.course-count small{font-size:11px;color:var(--muted)}button,select,textarea{font:inherit;color:var(--ink)}button{min-height:44px;padding:10px 16px;border:1px solid var(--line);border-radius:9px;background:var(--canvas);cursor:pointer;line-height:1.5}button:disabled{opacity:.5;cursor:default}.primary{background:var(--brand-accent);color:var(--brand-on-accent);border-color:var(--brand-accent);font-weight:500}.text-button{padding:6px 0;min-height:40px;border:0;background:transparent;font-size:12px;color:var(--muted)}button:focus-visible,select:focus-visible,textarea:focus-visible{outline:2px solid var(--brand-accent);outline-offset:3px}.route-select{display:flex;gap:12px;align-items:center;margin-bottom:16px;font-size:12px}.route-select select{min-width:0;max-width:100%;border:1px solid var(--line);padding:8px;border-radius:8px;background:var(--canvas)}.route-question{color:var(--muted)}.lesson-list{list-style:none;padding:0;margin:0}.lesson-list li{display:flex;align-items:center;gap:18px;padding:24px 0;border-top:1px solid var(--line)}.lesson-number{display:grid;place-items:center;align-self:flex-start;flex-shrink:0;width:38px;height:38px;border:1px solid var(--line);border-radius:50%;font-size:12px;color:var(--muted);margin-top:5px}.lesson-number.done{background:var(--brand-accent);border-color:var(--brand-accent);color:var(--brand-on-accent)}.lesson-info{flex:1;min-width:0}.lesson-info h4{font-size:18px;margin:4px 0 7px}.lesson-info p{margin:0;font-size:13px;color:var(--muted)}.lesson-info p.unit-name{font-size:11px}.lesson-meta{display:flex;gap:14px;margin-top:12px;font-size:11px;color:var(--muted)}.lesson-list .primary{flex-shrink:0;font-size:13px}.catalog-note{font-size:12px;color:var(--muted);padding-top:14px;border-top:1px solid var(--line)}.lesson-player{max-width:940px;margin:0 auto}.lesson-top{display:flex;align-items:center;justify-content:space-between;gap:12px;font-size:12px;color:var(--muted);margin-bottom:16px}.stage-track{display:flex;list-style:none;padding:0;margin:0 0 34px;gap:6px}.stage-track li{display:flex;align-items:center;flex:1;gap:6px;font-size:11px;color:var(--muted);border-bottom:2px solid var(--line);padding-bottom:12px}.stage-track li span{width:20px;height:20px;border-radius:50%;border:1px solid var(--line);display:grid;place-items:center;font-size:10px;flex-shrink:0}.stage-track .current{color:var(--brand-accent);border-color:var(--brand-accent);font-weight:600}.stage-track .current span{background:var(--brand-accent);color:var(--brand-on-accent);border-color:var(--brand-accent)}.stage-track .passed{color:var(--ink)}.course-prose{white-space:pre-line;line-height:1.95;font-size:15px;margin:18px 0 25px}.prerequisite,.answer-review,.hint-card,.finish-card,.pause-card{padding:18px 22px;border:1px solid var(--line);background:var(--surface-soft);border-radius:12px;margin:22px 0}.prerequisite p{margin:5px 0;color:var(--muted);font-size:13px}.prerequisite strong,.answer-review strong,.hint-card strong{font-size:12px}.answer-review p,.hint-card p{margin:8px 0 12px}.answer-review small,.hint-card small{color:var(--muted);font-size:12px}.finish-card h4,.pause-card h4{margin:4px 0 10px}.actions{display:flex;flex-wrap:wrap;gap:10px;align-items:center}.question{font-size:15px}fieldset{padding:0;border:0;margin:0;min-width:0}.option{display:flex;align-items:baseline;gap:12px;padding:14px 16px;border:1px solid var(--line);border-radius:10px;margin:10px 0;cursor:pointer;font-size:14px}.option.selected{border-color:var(--brand-accent);background:var(--surface-soft);box-shadow:inset 0 0 0 1px var(--brand-accent)}.option input{accent-color:var(--brand-accent);flex-shrink:0}.reason-label{display:block;margin-top:24px;margin-bottom:8px;font-size:13px}.reason-label span{font-size:11px;color:var(--muted)}textarea{width:100%;box-sizing:border-box;border:1px solid var(--line);border-radius:10px;resize:vertical;padding:12px;background:var(--canvas);line-height:1.8;min-height:96px}.muted,.prediction-note{font-size:12px;color:var(--muted)}.prediction-note{padding-left:12px;border-left:2px solid var(--brand-accent)}.lesson-footer{display:flex;flex-wrap:wrap;justify-content:space-between;gap:12px;margin-top:36px;padding-top:20px;border-top:1px solid var(--line);font-size:12px;color:var(--muted)}.lesson-footer small{display:block;font-size:11px;margin-top:3px}.lesson-footer .actions{gap:18px;align-items:flex-start}.duration{font-size:11px;color:var(--muted);margin-top:12px}summary{cursor:pointer}.history button{display:block;margin:8px 0;font-size:12px}.course-error,.notice{padding:14px 18px;border-left:3px solid var(--brand-accent);background:var(--surface-soft);margin:10px 0 20px}.course-error small{display:block;margin-top:10px}.restart{margin:20px 0}.sr-only{position:absolute;width:1px;height:1px;padding:0;margin:-1px;overflow:hidden;clip:rect(0,0,0,0);white-space:nowrap;border:0}@media(max-width:600px){.inquiry-course{padding:18px 14px}.catalog-heading{gap:12px}.lesson-list li{flex-wrap:wrap;gap:12px}.lesson-info{flex-basis:calc(100% - 54px)}.lesson-list .primary{margin-left:50px}.stage-track{gap:3px;margin-bottom:25px}.stage-track li{flex-direction:column;gap:7px;font-size:10px}.lesson-footer{gap:8px}.course-prose{font-size:14px}.option{padding:13px 11px}.prerequisite,.answer-review,.hint-card,.finish-card,.pause-card{padding:15px}.route-select{align-items:flex-start;flex-direction:column;gap:5px}.course-count strong{font-size:24px}h3{font-size:23px}.actions .primary{width:100%}}
</style>
