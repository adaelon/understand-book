// @vitest-environment happy-dom
import { mount, flushPromises } from '@vue/test-utils';
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import InquiryCoursePlayer from '@understand-book/web/reader/components/InquiryCoursePlayer.vue';
import ResourceDecisionLab from '@understand-book/web/reader/components/ResourceDecisionLab.vue';
import BoardSourcePreview from '@understand-book/web/reader/components/BoardSourcePreview.vue';
import BoardSourceReader from '@understand-book/web/reader/components/BoardSourceReader.vue';
import CourseActivityTutor from '@understand-book/web/reader/components/CourseActivityTutor.vue';
import CourseDecisionTutor from '@understand-book/web/reader/components/CourseDecisionTutor.vue';
import CourseInquiryActivity from '@understand-book/web/reader/components/CourseInquiryActivity.vue';
import CourseMathExperiment from '@understand-book/web/reader/components/CourseMathExperiment.vue';
import { inquiryCourse } from '@understand-book/web/reader/inquiry-course';
import { installIdentity, type LearningWorkspace } from '../network-context';
import type { CourseLessonView } from '@understand-book/web/reader/generated/CourseLessonView';
import sample from '../../../../docs/examples/inquiry-course/ai-infra-resource-bounds.v2.course.json';
import extended from '../../../../docs/examples/inquiry-course/ai-infra-resource-bounds.course.json';

vi.mock('@understand-book/web/reader/inquiry-course', () => ({ inquiryCourse: { catalog:vi.fn(), read:vi.fn(), mutate:vi.fn(), readSource:vi.fn() } }));
const authored = sample.lessons[0]!;
const publication = authored.summary.source.publication;
const workspace: LearningWorkspace = { learning_workspace_id:'course-space',goal:{text:'学习资源下界',constraints:[],revision:1,created_at:1},goal_history:[],members:[],archived:false,created_at:1,updated_at:1 };
let view: CourseLessonView;
const mounted: ReturnType<typeof mount>[] = [];
beforeEach(() => {
  localStorage.clear();
  vi.clearAllMocks(); installIdentity({user_id:'reader',csrf_token:'test'});
  view = { ...authored, resolved_questions:[], math_exploration:null,activity_response:null,inquiry_kind:null, progress:{ run_id:'run',release_id:'release',release_version:1,lesson_id:authored.summary.id,revision:4,goal_revision:1,stage:'task',status:'active' },
    prediction_response:{choice:authored.prediction.options[0]!.id,reason:'先猜一猜'}, preset:'baseline',observed:['baseline','double_compute','double_bandwidth'],
    answer:null,hint:null,solution:null,assessment_status:null,support_delivered:[],source_excerpt:null,exploration:null,expressions:[],reviews:[],round_index:0,round_titles:[authored.summary.question],round_history:[],decision:null } as CourseLessonView;
  vi.mocked(inquiryCourse.catalog).mockResolvedValue({release_id:'release',release_version:1,course:{...sample,lessons:[authored.summary]},progress:[view.progress]});
  vi.mocked(inquiryCourse.read).mockImplementation(async () => structuredClone(view));
  vi.mocked(inquiryCourse.readSource).mockResolvedValue({source_ref_id:'run:source',source_lid:authored.summary.source.start_lid,focus_lid:authored.summary.source.start_lid,
    segments:[{lid:authored.summary.source.start_lid,kind:'paragraph',text:'真实原文：计算和读取充分重叠。',highlights:[]}],previous_lid:null,next_lid:null});
  vi.mocked(inquiryCourse.mutate).mockImplementation(async (_scope, request) => {
    if (request.action.kind === 'source') view.source_excerpt = '真实原文';
    if (request.action.kind === 'pause') view.progress.status = 'paused';
    if (request.action.kind === 'resume') view.progress.status = 'active';
    if (request.action.kind === 'advance') view.progress.stage = 'explanation';
    if (request.action.kind === 'explore') view.exploration = {...request.action.state};
    view.progress.revision++; return structuredClone(view);
  });
});
afterEach(() => { mounted.splice(0).forEach(w=>w.unmount()); document.body.innerHTML=''; sessionStorage.clear(); vi.useRealTimers(); vi.restoreAllMocks(); });
async function player(catalogOnly = false, props:Partial<InstanceType<typeof InquiryCoursePlayer>['$props']> = {}) {
  const w = mount(InquiryCoursePlayer,{props:{workspace,publication,embedded:!catalogOnly,catalogOnly,...props},attachTo:document.body}); mounted.push(w);
  await flushPromises(); if (!catalogOnly) { await w.vm.openLesson(authored.summary.id); await flushPromises(); } return w;
}
async function click(w: Pick<ReturnType<typeof mount>, 'findAll'>, label: string) {
  const button=w.findAll('button').find(b=>b.text()===label); expect(button,label).toBeTruthy(); await button!.trigger('click'); await flushPromises();
}
it('pauses before returning to the catalog and preserves the draft when resuming in place',async()=>{
  view.progress.stage='prediction';
  vi.mocked(inquiryCourse.mutate).mockImplementation(async(_scope,request)=>{
    if(request.action.kind==='pause')view.progress.status='paused';
    if(request.action.kind==='resume')view.progress.status='active';
    view.progress.revision++;return structuredClone(view);
  });
  const w=await player(); await w.get('#course-reason').setValue('我想先比较两项时间。');
  await w.vm.act({kind:'pause'}); await flushPromises();
  expect(w.text()).toContain('从这里继续');
  await w.vm.act({kind:'resume'}); await flushPromises();
  expect((w.get('#course-reason').element as HTMLTextAreaElement).value).toBe('我想先比较两项时间。');
  const run=view.progress.run_id;
  await w.vm.back(); await flushPromises();
  expect(view.progress.status).toBe('paused'); expect(view.progress.run_id).toBe(run);
  expect(view.progress.stage).toBe('prediction'); expect(w.emitted('change')?.at(-1)).toEqual([null]);
  expect(Object.keys(sessionStorage).filter(key=>key.startsWith('understand-book:course:'))).toEqual([]);
});
it('stays on the course when saving a pause fails, then retries the same operation',async()=>{
  const w=await player();
  vi.mocked(inquiryCourse.mutate).mockRejectedValueOnce(new Error('暂停尚未保存'));
  await w.vm.back(); await flushPromises();
  expect(w.find('.lesson-player').exists()).toBe(true); expect(w.emitted('error')?.at(-1)).toEqual(['暂停尚未保存']);
  const pending=vi.mocked(inquiryCourse.mutate).mock.calls.at(-1)![1];
  vi.mocked(inquiryCourse.mutate).mockImplementation(async()=>{view.progress.status='paused';view.progress.revision++;return structuredClone(view);});
  await w.vm.retry(); await flushPromises();
  expect(vi.mocked(inquiryCourse.mutate).mock.calls.at(-1)![1]).toEqual(pending);
  expect(view.progress.status).toBe('paused');
});
it('returns from short support to the original response without losing the reply draft',async()=>{
  const expression={fact_ref:'f',job_ref:'j',activity:'reflection',response:'重叠以后读取就能忽略吗？',round_index:0};
  view.progress.stage='explanation';view.expressions=[expression];
  view.reviews=[{expression,status:'ready',displayed:true,understanding:[],error:null,feedback:{response_quote:expression.response,acknowledgement:'先区分开始和结束。',guidance:'可以补看同时进行的含义。',follow_up:'哪一项更晚完成？',support_branch:{focus:'同时开始与全部完成',explanation:'同时开始，仍要等全部完成。',example:'5 分钟与 8 分钟，全部完成要 8 分钟。',return_prompt:'回到原问题，哪一项更晚结束？'}}}];
  vi.mocked(inquiryCourse.mutate).mockImplementation(async(_scope,request)=>{
    if(request.action.kind==='open_support')view.support_visit={job_ref:'j',original_response:expression.response,branch:view.reviews[0].feedback!.support_branch!};
    if(request.action.kind==='return_from_support')view.support_visit=undefined;
    view.progress.revision++;return structuredClone(view);
  });
  const w=await player();await w.get('#activity-reply').setValue('我想比较两项时间');
  await w.get('.support-link').trigger('click');await flushPromises();
  expect(w.get('[aria-label="短补学"]').text()).toContain('全部完成要 8 分钟');
  expect(w.get('#activity-reply').isVisible()).toBe(false);
  await click(w,'回到原问题，接着想 →');
  expect((w.get('#activity-reply').element as HTMLTextAreaElement).value).toBe('我想比较两项时间');
  expect(w.getComponent(CourseActivityTutor).props('expression').job_ref).toBe('j');
  expect(view.progress.stage).toBe('explanation');
});

function replyView(kind:'activity'|'decision') {
  const expression={fact_ref:'reply-fact',job_ref:'reply-job',activity:kind==='activity'?'reflection':'task',response:'我还需要比较结束时间。',round_index:0};
  const branch={focus:'全部完成',explanation:'同时开始仍需等待全部结束。',example:'5 和 8 分钟需要等 8 分钟。',return_prompt:'哪一项更晚完成？'};
  view.progress.stage=kind==='activity'?'explanation':'feedback';view.expressions=[expression];view.exploration={...authored.lab};
  view.reviews=[{expression,status:'ready',displayed:true,understanding:[],error:null,feedback:{response_quote:expression.response,acknowledgement:'先比较两项。',guidance:'找到较晚结束的一项。',follow_up:'哪一项更晚完成？',support_branch:branch}}];
  if(kind==='decision')view.decision={phase:'decision',latest_job:expression.job_ref,question:'需要等待多久？',can_continue:false,focus_title:null,support:null,attention:null};
  return {expression,branch,selector:kind==='activity'?'#activity-reply':'#decision-reply'};
}

it.each(['activity','decision'] as const)('restores the %s reply after refreshing inside short support',async kind=>{
  const {expression,branch,selector}=replyView(kind);
  vi.mocked(inquiryCourse.mutate).mockImplementation(async(_scope,request)=>{
    if(request.action.kind==='open_support')view.support_visit={job_ref:expression.job_ref,original_response:expression.response,branch};
    if(request.action.kind==='return_from_support')view.support_visit=undefined;
    view.progress.revision++;return structuredClone(view);
  });
  const first=await player();await first.get(selector).setValue('两项都要做完，我准备比较它们的结束时刻。');
  await first.vm.act({kind:'open_support',job_ref:expression.job_ref});await flushPromises();first.unmount();
  const restored=await player();expect(restored.get('[aria-label="短补学"]').isVisible()).toBe(true);
  await click(restored,'回到原问题，接着想 →');
  expect((restored.get(selector).element as HTMLTextAreaElement).value).toBe('两项都要做完，我准备比较它们的结束时刻。');
});

it.each(['activity','decision'] as const)('isolates %s reply drafts by question and course run',async kind=>{
  const {expression,selector}=replyView(kind),w=await player();
  await w.get(selector).setValue('第一道反馈的续答');
  const next={...expression,job_ref:'another-job',fact_ref:'another-fact'};
  view.expressions.push(next);view.reviews.push({...view.reviews[0]!,expression:next});
  if(view.decision)view.decision.latest_job=next.job_ref;
  await w.vm.retry();await flushPromises();
  expect((w.get(selector).element as HTMLTextAreaElement).value).toBe('');
  await w.get(selector).setValue('第二道反馈的续答');
  view.expressions.pop();view.reviews.pop();if(view.decision)view.decision.latest_job=expression.job_ref;
  await w.vm.retry();await flushPromises();
  expect((w.get(selector).element as HTMLTextAreaElement).value).toBe('第一道反馈的续答');
  view.progress.run_id='another-run';await w.vm.retry();await flushPromises();
  expect((w.get(selector).element as HTMLTextAreaElement).value).toBe('');
});

it.each(['activity','decision'] as const)('clears a saved %s reply only after the failed submission succeeds on retry',async kind=>{
  const {selector}=replyView(kind),w=await player(),text='两项都要完成，因此比较结束时刻。';
  await w.get(selector).setValue(text);
  const draftKey=Object.keys(localStorage).find(key=>key.includes(':course-reply:'))!;
  vi.mocked(inquiryCourse.mutate).mockRejectedValueOnce(new Error('保存回应的连接已断开'));
  const tutor=kind==='activity'?w.getComponent(CourseActivityTutor):w.getComponent(CourseDecisionTutor);
  tutor.vm.$emit('reply','answer',text);await flushPromises();
  expect(localStorage.getItem(draftKey)).toBe(text);
  const request=vi.mocked(inquiryCourse.mutate).mock.calls.at(-1)![1];
  expect(request.action.kind).toBe(kind==='activity'?'feedback_reply':'decision_reply');
  await w.vm.retry();await flushPromises();
  expect(vi.mocked(inquiryCourse.mutate).mock.calls.at(-1)![1]).toEqual(request);
  expect(localStorage.getItem(draftKey)).toBeNull();
  w.unmount();const restored=await player();
  expect((restored.get(selector).element as HTMLTextAreaElement).value).toBe('');
});

it('turns the learner question into a real course exchange while keeping its original text',async()=>{
  view.progress.stage='prediction';
  vi.mocked(inquiryCourse.mutate).mockImplementation(async(_scope,request)=>{
    if(request.action.kind==='ask_question')view.expressions.push({fact_ref:'question-f',job_ref:'question-j',activity:'question',intent:'question',response:request.action.response,round_index:0});
    if(request.action.kind==='resolve_question')view.resolved_questions=request.action.resolved?[request.action.fact_ref]:[];
    view.progress.revision++;return structuredClone(view);
  });
  const w=await player();await click(w,'＋ 记下一个疑问');await click(w,'不知道怎么问？试着补三句话');
  const fields=w.findAll('.scaffold input');await fields[0].setValue('两项时间');await fields[1].setValue('为什么不能相加');await fields[2].setValue('什么时候取较大值');
  await click(w,'放进问题框，再改一改');const original=(w.get('#course-question').element as HTMLTextAreaElement).value;
  await click(w,'带着这个问题探索 →');
  expect(view.progress.stage).toBe('prediction');expect(view.expressions[0].response).toBe(original);
  expect(w.getComponent(CourseActivityTutor).props('expression').activity).toBe('question');
  await click(w,'这点我想明白了');expect(view.resolved_questions).toEqual(['question-f']);
  expect(w.get('.course-questions li p').text()).toBe(original);
});
it.each(['reading','reasoning','practice'] as const)('runs an open %s activity without a resource model and retains saved work',async(activity)=>{
  view.inquiry_kind=activity;view.lab=null;view.exploration=null;view.decision=null;view.observed=[];
  view.prediction={question:'哪些条件支持这个结论？',options:[]};view.task={question:'构造并解释一个反例。',options:[]};
  view.experiment={kind:'inquiry',activity,instruction:'比较成立条件',material:'**原文条件**：分段时间相等。',response_prompt:'写出依据与推理。'};
  view.progress.stage='prediction';
  vi.mocked(inquiryCourse.mutate).mockImplementation(async(_scope,request)=>{
    if(request.action.kind==='predict'){view.prediction_response={...request.action.response};view.progress.stage='experiment';}
    if(request.action.kind==='investigate')view.activity_response=request.action.response;
    if(request.action.kind==='advance')view.progress.stage='explanation';
    view.progress.revision++;return structuredClone(view);
  });
  const w=await player();expect(w.findComponent(ResourceDecisionLab).exists()).toBe(false);expect(w.find('input[type=radio]').exists()).toBe(false);
  await w.get('#course-reason').setValue('需要核对分段用时。');await w.get('form.stage-content').trigger('submit');await flushPromises();
  expect(inquiryCourse.mutate).toHaveBeenCalledWith('course-space',expect.objectContaining({action:{kind:'predict',response:{choice:'',reason:'需要核对分段用时。'}}}));
  const activityView=w.getComponent(CourseInquiryActivity);
  expect(activityView.get('.activity-material strong').text()).toBe('原文条件');
  const proceed=()=>w.findAll('button').find(b=>b.text()==='对照依据，继续解释 →')!;
  expect(w.findAll('button').map(b=>b.text())).toContain('对照依据，继续解释 →');
  expect(proceed().attributes('disabled')).toBeDefined();
  await activityView.get('textarea').setValue('等时间时各段路程分别为速率乘以相同时间。');
  await activityView.get('form').trigger('submit');await flushPromises();
  expect(proceed().attributes('disabled')).toBeUndefined();expect(activityView.text()).toContain('探究结果已保存');
  await activityView.get('textarea').setValue('补充：不等时间需要加权。');
  expect(proceed().attributes('disabled')).toBeDefined();
  await activityView.get('form').trigger('submit');await flushPromises();await proceed().trigger('click');await flushPromises();
  expect(w.getComponent(CourseInquiryActivity).props('readonly')).toBe(true);
  expect((w.getComponent(CourseInquiryActivity).get('textarea').element as HTMLTextAreaElement).value).toBe('补充：不等时间需要加权。');
});
it('saves the mathematics configuration with the learner text and hides it in the independent task',async()=>{
  view.inquiry_kind='reasoning';view.lab=null;view.progress.stage='experiment';
  view.experiment={kind:'inquiry',activity:'reasoning',instruction:'比较弦高',material:'观察同一横坐标上的高差。',response_prompt:'解释你的观察',visualization:{kind:'convex_chord',function:'square',a:-1,b:1,t:.5}};
  vi.mocked(inquiryCourse.mutate).mockImplementation(async(_scope,request)=>{
    if(request.action.kind==='investigate'){view.activity_response=request.action.response;view.math_exploration=request.action.visualization ? {...request.action.visualization} : null;}
    view.progress.revision++;return structuredClone(view);
  });
  const w=await player(),lab=w.getComponent(CourseMathExperiment);
  await lab.findAll('input[type=range]')[2].setValue('0.25');
  await w.get('#course-investigation').setValue('同一横坐标下，弦高为 1，曲线高为 0.25。');
  await w.getComponent(CourseInquiryActivity).get('form').trigger('submit');await flushPromises();
  expect(view.math_exploration).toMatchObject({kind:'convex_chord',t:.25});
  expect(w.text()).toContain('探究结果已保存');
  view.progress.stage='task';await w.vm.retry();await flushPromises();
  expect(w.findComponent(CourseMathExperiment).exists()).toBe(false);
  expect(w.find('#course-reason').exists()).toBe(true);
});
it('restores a long independent-answer draft after the player is remounted',async()=>{
  const first=await player();
  await first.get('#course-reason').setValue('任取两个向量，先展开二次型差；下面继续补写证明。');
  first.unmount();
  const restored=await player();
  expect((restored.get('#course-reason').element as HTMLTextAreaElement).value).toBe('任取两个向量，先展开二次型差；下面继续补写证明。');
  expect(inquiryCourse.mutate).not.toHaveBeenCalled();
});
it('opens the book from the catalog without starting a second inline player',async()=>{
  const w=await player(true); await click(w,'继续学习');
  expect(w.emitted('launch')?.[0]).toEqual([{publication,lessonId:authored.summary.id,runId:'run'}]);
  expect(inquiryCourse.mutate).not.toHaveBeenCalled(); expect(w.find('.lesson-player').exists()).toBe(false);
});
it('opens the next lesson in the unit without overwriting the completed lesson',async()=>{
  view.progress.stage='complete';view.progress.status='completed';
  const next={...authored.summary,id:'batch-reuse-and-wait',question:'一批生成八个 token，为什么单个请求没有快八倍？'};
  vi.mocked(inquiryCourse.catalog).mockResolvedValue({release_id:'release',release_version:1,course:{...sample,routes:[],lessons:[authored.summary,next]},progress:[view.progress]});
  vi.mocked(inquiryCourse.mutate).mockResolvedValue({...structuredClone(view),summary:next,progress:{...view.progress,run_id:'next-run',lesson_id:next.id,revision:0,stage:'introduction',status:'active'}});
  const w=await player();expect(w.get('.next-lesson').text()).toContain(next.question);
  await click(w,'进入下一课 →');
  expect(inquiryCourse.mutate).toHaveBeenCalledWith('course-space',expect.objectContaining({run_id:null,action:{kind:'start',lesson_id:next.id}}));
  expect(w.get('h3').text()).toBe(next.question);expect(view.progress.status).toBe('completed');
});
it('starts an active lesson from the beginning and keeps same-version history without replacing the latest run',async()=>{
  const original=structuredClone(view);
  const fresh:CourseLessonView={...structuredClone(view),progress:{...view.progress,run_id:'fresh-run',revision:0,stage:'introduction'},prediction_response:null,observed:[],experiment:null,lab:null,exploration:null,task:null};
  vi.mocked(inquiryCourse.read).mockImplementation(async(_workspace,run)=>structuredClone(run==='fresh-run'?fresh:original));
  vi.mocked(inquiryCourse.mutate).mockImplementation(async(_scope,request)=>{
    const run=request.action.kind==='start' || request.run_id==='fresh-run'?fresh:original;
    if(request.action.kind==='pause')run.progress.status='paused';
    if(request.action.kind==='resume')run.progress.status='active';
    run.progress.revision++;return structuredClone(run);
  });
  const w=await player();
  await click(w,'从头学习');
  expect(inquiryCourse.mutate).toHaveBeenCalledWith('course-space',expect.objectContaining({run_id:null,expected_revision:0,release_version:1,action:{kind:'start',lesson_id:authored.summary.id}}));
  expect(w.text()).toContain('先认识这个问题');
  expect(w.findAll('.stage-track .passed')).toHaveLength(0);
  await click(w,'← 课程目录');
  expect(w.get('.history').text()).toContain('以前的学习记录（1）');
  await w.get('.history button').trigger('click');await flushPromises();
  expect(w.text()).toContain('这次由你来判断');
  await click(w,'← 课程目录');
  expect(w.get('.history').text()).toContain('以前的学习记录（1）');
  await click(w,'继续学习');
  await click(w,'从这里继续');
  expect(w.text()).toContain('先认识这个问题');
  expect(inquiryCourse.read).toHaveBeenLastCalledWith('course-space','fresh-run');
});
it('keeps the decision model visible while composing the first answer and saves those exact parameters',async()=>{
  view.decision={phase:'decision',latest_job:null,question:authored.task.question,can_continue:false,focus_title:null,support:null,attention:null};
  view.exploration={...authored.lab};
  const w=await player();const lab=w.getComponent(ResourceDecisionLab);
  expect(lab.isVisible()).toBe(true);expect(lab.props('review')).toBe(true);
  await click(lab,'调整实验参数');await click(lab,'当前算力 ×2');
  await w.get(`input[value="${authored.task.options[0]!.id}"]`).setValue(true);
  await w.get('#course-reason').setValue('我的理由参考了当前两项时间。');
  await w.get('form.stage-content').trigger('submit');await flushPromises();
  expect(vi.mocked(inquiryCourse.mutate).mock.calls.map(call=>call[1].action)).toEqual([
    {kind:'explore',state:{...authored.lab,compute_tflops:authored.lab.compute_tflops*2}},
    {kind:'answer',response:{choice:authored.task.options[0]!.id,reason:'我的理由参考了当前两项时间。'}},
  ]);
});
it('previews the course source without leaving the task and only opens the reader on explicit request',async()=>{
  const w=await player(); await w.get('#course-reason').setValue('我算出 12 和 8，需要升级算力。');
  const lab=w.getComponent(ResourceDecisionLab);
  await click(lab,'恢复本轮实验起点'); await click(lab,'当前算力 ×2');
  await lab.get('.surface').trigger('keydown',{key:'ArrowRight'});
  const shape=lab.get('.surface polygon').attributes('points');
  w.vm.showExperiment(); await flushPromises(); w.vm.showTask(); await flushPromises();
  await click(w,'查看本课来源');
  expect(w.emitted('source')).toBeUndefined();
  expect(inquiryCourse.readSource).toHaveBeenCalledWith('course-space','run',{direction:'around',include_outline:true});
  expect(document.querySelector('[role="dialog"]')?.getAttribute('aria-label')).toBe('本课来源');
  expect(document.querySelector('[role="dialog"]')?.textContent).toContain('计算和读取充分重叠');
  (document.querySelector('.board-source-close') as HTMLButtonElement).click();await flushPromises();
  expect(w.findComponent(BoardSourcePreview).exists()).toBe(false);
  expect((w.get('#course-reason').element as HTMLTextAreaElement).value).toContain('12 和 8');
  expect(w.getComponent(ResourceDecisionLab).get('.surface polygon').attributes('points')).toBe(shape);
  expect(w.emitted('exploration')?.at(-1)?.[0]).toMatchObject({compute_tflops:120,work_gflops:600});
  await click(w,'查看本课来源');
  expect(inquiryCourse.mutate).toHaveBeenCalledTimes(1);
  w.getComponent(BoardSourceReader).vm.$emit('position','3.6.3');await flushPromises();
  (document.querySelector('.board-source-open-reader') as HTMLButtonElement).click();await flushPromises();
  expect(w.emitted('source')?.[0]).toEqual([authored.summary.source,'3.6.3']);
  expect(w.findComponent(BoardSourcePreview).exists()).toBe(false);
  expect((w.get('#course-reason').element as HTMLTextAreaElement).value).toContain('12 和 8');
});
it('opens the source immediately, retries a failed delivery in place, and restores focus to the reading task',async()=>{
  view.progress.stage='prediction';
  let fail!: (error:Error)=>void;
  vi.mocked(inquiryCourse.mutate).mockImplementationOnce(()=>new Promise((_resolve,reject)=>{fail=reject;}));
  const w=await player(),trigger=w.get('.source-activity button');
  await trigger.trigger('click');await flushPromises();
  expect(w.getComponent(BoardSourcePreview).props('loading')).toBe(true);
  expect(document.querySelector('[role="dialog"]')?.textContent).toContain(authored.reading.before_experiment);
  expect(inquiryCourse.readSource).not.toHaveBeenCalled();
  fail(new Error('来源暂时无法读取'));await flushPromises();
  expect(w.getComponent(BoardSourcePreview).props('loading')).toBe(false);
  expect(document.querySelector('.board-source-error')?.textContent).toContain('来源暂时无法读取');
  (document.querySelector('.board-source-error button') as HTMLButtonElement).click();await flushPromises();
  expect(vi.mocked(inquiryCourse.mutate).mock.calls[1]![1]).toEqual(vi.mocked(inquiryCourse.mutate).mock.calls[0]![1]);
  expect(w.getComponent(BoardSourcePreview).props('error')).toBe('');
  expect(document.querySelector('[role="dialog"]')?.textContent).toContain('计算和读取充分重叠');
  (document.querySelector('.board-source-close') as HTMLButtonElement).click();await flushPromises();
  expect(document.activeElement).toBe(trigger.element);
  expect(view.progress.stage).toBe('prediction');
});
it('does not reopen the source when a closed loading preview finishes',async()=>{
  let finish!: (value:CourseLessonView)=>void;
  vi.mocked(inquiryCourse.mutate).mockImplementationOnce(()=>new Promise(resolve=>{finish=resolve;}));
  const w=await player();await click(w,'查看本课来源');
  (document.querySelector('.board-source-close') as HTMLButtonElement).click();await flushPromises();
  view.source_excerpt='真实原文';view.progress.revision++;
  finish(structuredClone(view));await flushPromises();
  expect(w.findComponent(BoardSourcePreview).exists()).toBe(false);
  expect(w.emitted('source')).toBeUndefined();
  expect(inquiryCourse.readSource).not.toHaveBeenCalled();
});
it('keeps the 3D view and unsaved parameters when progressing out of the experiment',async()=>{
  view.progress.stage='experiment'; view.exploration={...authored.lab};
  const w=await player(); const lab=w.getComponent(ResourceDecisionLab);
  await click(lab,'当前算力 ×2'); await lab.get('.surface').trigger('keydown',{key:'ArrowRight'});
  const shape=lab.get('.surface polygon').attributes('points');
  await click(w,'对照预测，看看原因 →');
  expect(w.text()).toContain(authored.reading.after_experiment);
  expect(lab.isVisible()).toBe(true);
  expect(lab.props('review')).toBe(true);
  await w.get('#course-reflection').setValue('算力翻倍后读取仍需 8 ms。');
  await click(lab,'3D 下界模型');
  expect(lab.get('.surface').isVisible()).toBe(true);
  expect(w.get('#course-reflection').isVisible()).toBe(true);
  w.vm.showExperiment(); await flushPromises(); w.vm.showTask(); await flushPromises();
  expect(w.getComponent(ResourceDecisionLab).element).toBe(lab.element);
  expect(lab.get('.surface polygon').attributes('points')).toBe(shape);
  expect(lab.get('.readouts').text()).toContain('8.00');
  expect((w.get('#course-reflection').element as HTMLTextAreaElement).value).toContain('8 ms');
});
it('saves the configuration being explained before submitting the explanation for feedback',async()=>{
  view.progress.stage='explanation'; view.exploration={...authored.lab};
  const w=await player(); const lab=w.getComponent(ResourceDecisionLab);
  await click(lab,'调整实验参数'); await click(lab,'当前算力 ×2');
  await w.get('#course-reflection').setValue('图上计算为 5 ms，读取为 8 ms，因此下界是 8 ms。');
  await w.get('.reflection-form').trigger('submit'); await flushPromises();
  expect(vi.mocked(inquiryCourse.mutate).mock.calls.map(call=>call[1].action)).toEqual([
    {kind:'explore',state:{...authored.lab,compute_tflops:authored.lab.compute_tflops*2}},
    {kind:'reflect',response:'图上计算为 5 ms，读取为 8 ms，因此下界是 8 ms。'},
  ]);
});
it('keeps the explanation draft if its experiment configuration could not be saved',async()=>{
  view.progress.stage='explanation'; view.exploration={...authored.lab};
  const w=await player(); const lab=w.getComponent(ResourceDecisionLab);
  await click(lab,'调整实验参数'); await click(lab,'当前算力 ×2');
  vi.mocked(inquiryCourse.mutate).mockRejectedValueOnce(new Error('暂时无法保存'));
  await w.get('#course-reflection').setValue('我需要解释眼前这组配置。');
  await w.get('.reflection-form').trigger('submit'); await flushPromises();
  expect(inquiryCourse.mutate).toHaveBeenCalledTimes(1);
  expect((w.get('#course-reflection').element as HTMLTextAreaElement).value).toBe('我需要解释眼前这组配置。');
  expect(w.get('.course-error').text()).toContain('暂时无法保存');
  expect(lab.get('.readouts').text()).toContain('8.00');
});
it('brings the last unread earlier-round response into the course without losing the current prediction',async()=>{
  const expression={fact_ref:'earlier',job_ref:'earlier-review',activity:'reflection',response:'这个部分没懂',round_index:0,round_title:'找到升级收益的转折点'};
  view.progress.stage='prediction';view.round_index=1;view.round_titles=['找到升级收益的转折点','工作量变化后重新设计'];
  view.round_history=[{title:view.round_titles[0]!,prediction:view.prediction,response:view.prediction_response!,exploration:{...authored.lab},math_exploration:null,activity_response:null,reflection:expression.response,explanation:authored.explanation}];
  view.expressions=[expression];view.reviews=[{expression,status:'ready',displayed:false,feedback:null,understanding:[],error:null}];
  const w=await player();
  expect(w.get('.activity-task .activity-tutor').text()).toContain('这个部分没懂');
  expect(w.get('#course-reason').isVisible()).toBe(false);
  await w.get('#course-reason').setValue('第 2 轮的预测草稿');
  w.getComponent(CourseActivityTutor).vm.$emit('reply','question','为什么读取还是 8？');await flushPromises();
  expect(inquiryCourse.mutate).toHaveBeenCalledWith('course-space',expect.objectContaining({action:{kind:'feedback_reply',job_ref:'earlier-review',intent:'question',response:'为什么读取还是 8？'}}));
  expect(vi.mocked(inquiryCourse.mutate).mock.calls.some(call=>call[1].action.kind==='explore')).toBe(false);
  await click(w,'回到当前活动 →');
  expect(w.get('#course-reason').isVisible()).toBe(true);expect((w.get('#course-reason').element as HTMLTextAreaElement).value).toBe('第 2 轮的预测草稿');
  expect(view.progress.stage).toBe('prediction');expect(view.round_index).toBe(1);
});
it('does not mark historical feedback as displayed merely by polling',async()=>{
  vi.useFakeTimers(); vi.spyOn(document,'visibilityState','get').mockReturnValue('visible');
  const expression={fact_ref:'reason',job_ref:'review',activity:'prediction',response:'我还需要比较读取时间。'};
  view.expressions=[expression]; view.reviews=[{expression,status:'pending',displayed:false,feedback:null,understanding:[],error:null}];
  await player(); const reads=vi.mocked(inquiryCourse.read).mock.calls.length;
  view.reviews[0]!.status='ready';
  await vi.advanceTimersByTimeAsync(2300); await flushPromises();
  expect(vi.mocked(inquiryCourse.read).mock.calls.length).toBeGreaterThan(reads);
  expect(inquiryCourse.mutate).not.toHaveBeenCalled();
  const settled=vi.mocked(inquiryCourse.read).mock.calls.length;
  await vi.advanceTimersByTimeAsync(4600); await flushPromises();
  expect(vi.mocked(inquiryCourse.read).mock.calls.length).toBe(settled);
});
it('does not schedule another review poll when an in-flight read finishes after unmount',async()=>{
  vi.useFakeTimers();vi.spyOn(document,'visibilityState','get').mockReturnValue('visible');
  const expression={fact_ref:'pending-fact',job_ref:'pending-job',activity:'prediction',response:'等回复。'};
  view.expressions=[expression];view.reviews=[{expression,status:'pending',displayed:false,feedback:null,understanding:[],error:null}];
  const w=await player();
  let finish!: (value:CourseLessonView)=>void;
  vi.mocked(inquiryCourse.read).mockImplementationOnce(()=>new Promise(resolve=>{finish=resolve;}));
  await vi.advanceTimersByTimeAsync(2200);expect(finish).toBeTypeOf('function');
  w.unmount();finish(structuredClone(view));await flushPromises();
  const reads=vi.mocked(inquiryCourse.read).mock.calls.length;
  await vi.advanceTimersByTimeAsync(6600);await flushPromises();
  expect(vi.mocked(inquiryCourse.read).mock.calls.length).toBe(reads);
  expect(vi.getTimerCount()).toBe(0);
});
it('takes a settled prediction to the experiment, then to explanation only after the configurations were observed',async()=>{
  const expression={fact_ref:'answer',job_ref:'accepted-answer',activity:'prediction',response:'读取时间约 10.45 ms，下界也是 10.45 ms',round_index:0};
  view.progress.stage='experiment';view.observed=[];view.exploration={...authored.lab};
  view.expressions=[expression];view.reviews=[{expression,status:'ready',displayed:true,feedback:{response_quote:expression.response,acknowledgement:'对，读取决定下界。',guidance:'接着对照实验。',follow_up:null},understanding:[],error:null}];
  const w=await player(),tutor=w.getComponent(CourseActivityTutor),lab=w.getComponent(ResourceDecisionLab);
  expect(tutor.find('form').exists()).toBe(false);
  await click(tutor,'去图上验证 →');
  expect(inquiryCourse.mutate).not.toHaveBeenCalled();expect(view.progress.stage).toBe('experiment');
  expect(document.activeElement).toBe(w.get('.experiment-stage').element);
  expect(w.findAll('button').find(b=>b.text()==='对照预测，看看原因 →')!.attributes('disabled')).toBeDefined();
  vi.mocked(inquiryCourse.mutate).mockImplementation(async(_scope,request)=>{
    if(request.action.kind==='observe')view.observed.push(request.action.preset);
    if(request.action.kind==='advance')view.progress.stage='explanation';
    view.progress.revision++;return structuredClone(view);
  });
  await click(lab,'原配置');await click(lab,'算力翻倍');await click(lab,'带宽翻倍');
  expect(tutor.text()).toContain('三种配置已观察完');
  await click(tutor,'对照预测，看看原因 →');
  expect(vi.mocked(inquiryCourse.mutate).mock.calls.map(call=>call[1].action.kind)).toEqual(['observe','observe','observe','advance']);
  expect(view.progress.stage).toBe('explanation');expect(view.progress.status).toBe('active');
  expect(w.get('#course-reflection').isVisible()).toBe(true);
});
it('retains a book history entry after a newer release removes its course',async()=>{
  vi.mocked(inquiryCourse.catalog).mockResolvedValue({release_id:'release',release_version:2,course:null,progress:[view.progress]});
  const w=mount(InquiryCoursePlayer,{props:{workspace,publication,embedded:true}}); mounted.push(w); await flushPromises();
  await w.vm.openLesson(undefined,'run'); await flushPromises(); await w.vm.back(); await flushPromises();
  expect(w.text()).toContain('以前的学习记录（1）');
  expect(w.emitted('available')?.at(-1)).toEqual([true]);
});

it('keeps each investigation separate and carries its learner explanation into the next round',async()=>{
  const authoredRound = extended.lessons[0]!.follow_up_rounds[0]!;
  view.round_index=0; view.round_titles=[extended.lessons[0]!.round_title,authoredRound.title]; view.round_history=[];
  view.progress.stage='explanation'; view.exploration={...authored.lab};
  vi.mocked(inquiryCourse.mutate).mockImplementation(async (_scope, request) => {
    if (request.action.kind==='reflect') {
      view.expressions.push({fact_ref:'reflection',job_ref:'round-one',activity:'reflection',response:request.action.response,round_index:0,round_title:view.round_titles[0]!});
      view.reviews=[{expression:view.expressions[0]!,status:'ready',displayed:false,feedback:null,understanding:[],error:null}];
    }
    if (request.action.kind==='read_feedback') view.reviews[0]!.displayed=true;
    if (request.action.kind==='advance') {
      view.round_history=[{title:view.round_titles[0]!,prediction:view.prediction,response:view.prediction_response!,exploration:view.exploration,math_exploration:null,activity_response:null,reflection:view.expressions[0]!.response,explanation:view.explanation!}];
      view.round_index=1; view.progress.stage='prediction'; view.prediction=authoredRound.prediction;
      view.prediction_response=null; view.experiment=null; view.exploration=null; view.lab=null; view.observed=[];view.explanation=null;
    }
    if (request.action.kind==='predict') {
      view.progress.stage='experiment'; view.prediction_response={...request.action.response};
      view.experiment={...authoredRound.experiment,kind:'resource_bounds'}; view.lab=authoredRound.lab;
    }
    view.progress.revision++; return structuredClone(view);
  });
  const w=await player();
  expect(w.get('.round-map').text()).toContain(authoredRound.title);
  expect(w.findAll('button').find(b=>b.text()==='继续下一轮探究 →')!.attributes('disabled')).toBeDefined();
  await w.get('#course-reflection').setValue('读取仍需 20.9 ms，所以缩短计算不能降低当前下界。');
  await w.get('.reflection-form').trigger('submit'); await flushPromises();
  expect(inquiryCourse.mutate).toHaveBeenCalledWith('course-space',expect.objectContaining({action:{kind:'reflect',response:'读取仍需 20.9 ms，所以缩短计算不能降低当前下界。'}}));
  expect(w.find('.activity-task .activity-tutor').exists()).toBe(true);
  expect(w.findAll('button').find(b=>b.text()==='继续下一轮探究 →')!.attributes('disabled')).toBeDefined();
  await w.vm.act({kind:'read_feedback',job_ref:'round-one'});await flushPromises();
  expect(w.findAll('button').find(b=>b.text()==='继续下一轮探究 →')!.attributes('disabled')).toBeUndefined();
  await click(w,'继续下一轮探究 →');
  expect(w.text()).toContain(authoredRound.prediction.question);
  expect(w.findComponent(ResourceDecisionLab).exists()).toBe(false);
  expect(w.get('.round-history').text()).toContain('读取仍需 20.9 ms');
  expect((w.get('#course-reason').element as HTMLTextAreaElement).value).toBe('');
  await w.vm.act({kind:'predict',response:{choice:'switch',reason:'读取会成为下一项限制'}}); await flushPromises();
  expect(w.getComponent(ResourceDecisionLab).get('.bound').text()).toContain('10.00');
  expect(w.get('.prediction-note').text()).toContain(authoredRound.prediction.options[1]!.text);
  expect(w.getComponent(ResourceDecisionLab).props('observed')).toEqual([]);
});

it('shows the current task and keeps explanations and optional practice out of the prediction',async()=>{
  view.progress.stage='prediction';view.explanation='尚未揭示的解释内容';view.solution='尚未揭示的最终答案';
  const w=await player();
  expect(w.get('[aria-label="当前学习任务"]').text()).toContain('先作出你的判断');
  expect(w.get('[aria-label="当前学习任务"]').text()).toContain(authored.summary.objective);
  expect(w.text()).not.toContain('尚未揭示的解释内容');expect(w.text()).not.toContain('尚未揭示的最终答案');
  expect(w.find('.contextual-practice').exists()).toBe(false);
  expect(w.findAll('#course-reason')).toHaveLength(1);
});

it('offers explained closed-book practice and an ordinary application at the delivered explanation',async()=>{
  view.progress.stage='explanation';view.explanation='已经展示的本轮讲解';
  const w=await player();
  const practice=w.get('.contextual-practice');
  expect(practice.text()).toContain('闭卷表达会收起资料，按回答追问或补讲，可随时暂停。');
  await click(practice,'我来解释一下');await click(practice,'给我一道应用题');
  expect(w.emitted('closedBook')).toEqual([[]]);expect(w.emitted('practice')).toEqual([[]]);
  expect(inquiryCourse.mutate).not.toHaveBeenCalled();
});

it('keeps one response input and one completion action when final activity feedback is available',async()=>{
  const expression={fact_ref:'task-fact',job_ref:'task-job',activity:'task',response:'我选择增加带宽。',round_index:0};
  view.progress.stage='feedback';view.expressions=[expression];
  view.reviews=[{expression,status:'ready',displayed:true,understanding:[],error:null,feedback:{response_quote:expression.response,acknowledgement:'你比较了两个时间。',guidance:'还要说明较慢的一项。',follow_up:'哪一项决定下界？'}}];
  const w=await player();
  expect(w.find('#course-revision').exists()).toBe(false);expect(w.findAll('#activity-reply')).toHaveLength(1);
  expect(w.findAll('button').filter(button=>button.text()==='完成这一课 ✓')).toHaveLength(1);
  const tutor=w.getComponent(CourseActivityTutor);await tutor.get('#activity-reply').setValue('读取时间更长，所以它决定下界。');
  await tutor.get('form').trigger('submit');await flushPromises();
  expect(inquiryCourse.mutate).toHaveBeenCalledWith('course-space',expect.objectContaining({action:{kind:'feedback_reply',job_ref:'task-job',response:'读取时间更长，所以它决定下界。',intent:'answer'}}));
});

it('retains the ordinary revision form when there is no activity feedback to answer',async()=>{
  view.progress.stage='feedback';
  const w=await player();expect(w.find('#course-revision').exists()).toBe(true);expect(w.find('#activity-reply').exists()).toBe(false);
  await w.get('#course-revision').setValue('还需要说明读取耗时。');await w.get('.reflection-form').trigger('submit');await flushPromises();
  expect(inquiryCourse.mutate).toHaveBeenCalledWith('course-space',expect.objectContaining({action:{kind:'reflect',response:'还需要说明读取耗时。'}}));
});

it('emits engagement for committed learning actions but not reads or automatic feedback delivery',async()=>{
  const w=await player();expect(w.emitted('engaged')).toBeUndefined();
  await w.vm.act({kind:'read_feedback',job_ref:'ready'});await flushPromises();
  expect(w.emitted('engaged')).toBeUndefined();
  await w.vm.act({kind:'resume'});await flushPromises();
  expect(w.emitted('engaged')).toHaveLength(1);
  expect(w.emitted('engaged')![0]![0]).toMatchObject({progress:{run_id:'run',status:'active'}});
  await w.vm.retry();await flushPromises();expect(w.emitted('engaged')).toHaveLength(1);
  vi.mocked(inquiryCourse.mutate).mockRejectedValueOnce(new Error('保存失败'));
  await w.vm.act({kind:'advance'});await flushPromises();expect(w.emitted('engaged')).toHaveLength(1);
});

it('returns the opened lesson and propagates read failures so the entry can retry the same target',async()=>{
  const w=await player();
  vi.mocked(inquiryCourse.read).mockRejectedValueOnce(new Error('读取原课程失败'));
  await expect(w.vm.openLesson(authored.summary.id,'run')).rejects.toThrow('读取原课程失败');
  expect(await w.vm.openLesson(authored.summary.id,'run')).toMatchObject({progress:{run_id:'run'}});
  expect(inquiryCourse.mutate).not.toHaveBeenCalled();
});

it('retries a failed start with the same operation before reporting the opened lesson',async()=>{
  vi.mocked(inquiryCourse.catalog).mockResolvedValue({release_id:'release',release_version:1,course:{...sample,lessons:[authored.summary]},progress:[]});
  const w=mount(InquiryCoursePlayer,{props:{workspace,publication,embedded:true}});mounted.push(w);await flushPromises();
  vi.mocked(inquiryCourse.mutate).mockRejectedValueOnce(new Error('开课响应丢失'));
  await expect(w.vm.openLesson(authored.summary.id)).rejects.toThrow('开课响应丢失');
  const request=vi.mocked(inquiryCourse.mutate).mock.calls.at(-1)![1];
  expect(await w.vm.openLesson(authored.summary.id)).toMatchObject({progress:{run_id:'run'}});
  expect(vi.mocked(inquiryCourse.mutate).mock.calls.at(-1)![1]).toEqual(request);
});

it('starts a course for the current goal instead of silently reopening a previous goal run',async()=>{
  const revised={...workspace,goal:{...workspace.goal,revision:2,text:'判断带宽升级收益'}};
  const fresh={...structuredClone(view),progress:{...view.progress,run_id:'new-goal-run',goal_revision:2}};
  vi.mocked(inquiryCourse.mutate).mockResolvedValue(fresh);
  const w=mount(InquiryCoursePlayer,{props:{workspace:revised,publication,embedded:true}});mounted.push(w);await flushPromises();
  expect(await w.vm.openLesson(authored.summary.id)).toMatchObject({progress:{run_id:'new-goal-run',goal_revision:2}});
  expect(inquiryCourse.mutate).toHaveBeenCalledWith('course-space',expect.objectContaining({run_id:null,action:{kind:'start',lesson_id:authored.summary.id}}));
  expect(inquiryCourse.read).not.toHaveBeenCalled();
});

it('distinguishes no saved task from a failed saved-task read',async()=>{
  const w=mount(InquiryCoursePlayer,{props:{workspace,publication,embedded:true}});mounted.push(w);await flushPromises();
  expect(await w.vm.resumeSaved()).toBeNull();
  await w.vm.openLesson(authored.summary.id,'run');
  vi.mocked(inquiryCourse.read).mockRejectedValueOnce(new Error('保存的任务暂时无法读取'));
  await expect(w.vm.resumeSaved()).rejects.toThrow('保存的任务暂时无法读取');
  expect(await w.vm.resumeSaved()).toMatchObject({progress:{run_id:'run'}});
});

it('shows course completion separately from the actual final-answer evidence and assistance',async()=>{
  const expression={fact_ref:'last-fact',job_ref:'last-job',activity:'task',response:'应该比较两项结束时间。',round_index:0};
  view.progress.stage='complete';view.progress.status='completed';view.expressions=[expression];
  view.reviews=[{expression,status:'ready',displayed:true,feedback:null,error:null,understanding:[{evidence_ref:'evidence',interpretation:'你在提示后说清了比较方法。',demonstrated:['能比较计算与读取时间。'],difficulties:[],unconfirmed:['还需要在新情境独立应用。'],assistance:'本次使用过提示。'}]}];
  const w=await player(),result=w.get('[aria-label="本课学习结果"]');
  expect(result.text()).toContain('本课已完成');expect(result.text()).toContain('你在提示后说清了比较方法。');
  expect(result.text()).toContain('还需要在新情境独立应用。');expect(result.text()).toContain('本次使用过提示。');
  expect(result.text()).not.toContain('已独立掌握');
});

it('waits for the original course to accept reused closed-book evidence before advancing',async()=>{
  const expression={fact_ref:'reused-fact',job_ref:'reused-job',activity:'reflection',response:'两项充分重叠时，较晚完成的一项决定下界。',round_index:0,closed_book_result_ref:'closed-result'};
  view.progress.stage='explanation';view.round_titles=['找出瓶颈','判断升级收益'];view.expressions=[expression];
  view.reviews=[{expression,status:'ready',displayed:false,feedback:null,understanding:[],error:null}];
  const w=await player(),tutor=w.getComponent(CourseActivityTutor);
  const next=()=>w.findAll('button').find(button=>button.text()==='继续下一轮探究 →')!;
  expect(next().attributes('disabled')).toBeDefined();
  expect(tutor.get('.reuse-result').text()).toContain('按本课要求核对后再继续');
  view.reviews[0]={...view.reviews[0]!,displayed:true,feedback:{response_quote:expression.response,acknowledgement:'你已经说明取较大值的条件。',guidance:'还需用本轮数据解释预测。',follow_up:null,course_reuse:{accepted:false,reason:'本轮还需要说明观察如何改变你的预测。'}}};
  await w.vm.retry();await flushPromises();
  expect(next().attributes('disabled')).toBeDefined();expect(tutor.find('form').exists()).toBe(true);
  expect(tutor.get('.reuse-result').text()).toContain('本轮还需要说明观察如何改变你的预测。');
  view.reviews[0]!.feedback!.course_reuse={accepted:true,reason:'已经独立说明了本轮所需的条件与观察依据。'};
  await w.vm.retry();await flushPromises();
  expect(next().attributes('disabled')).toBeUndefined();expect(tutor.get('.reuse-result').text()).toContain('这次练习已满足本段要求。');
});

it('offers additional practice only after the current feedback has arrived',async()=>{
  const earlier={fact_ref:'old-fact',job_ref:'old-job',activity:'task',response:'先比较时间。',round_index:0};
  const current={...earlier,fact_ref:'new-fact',job_ref:'new-job',response:'读取较慢，所以先改带宽。'};
  view.progress.stage='feedback';view.expressions=[earlier,current];
  view.reviews=[{expression:earlier,status:'ready',displayed:true,feedback:null,understanding:[],error:null},{expression:current,status:'running',displayed:false,feedback:null,understanding:[],error:null}];
  const w=await player();expect(w.find('.contextual-practice').exists()).toBe(false);
  view.reviews[1]={...view.reviews[1]!,status:'ready',displayed:true,feedback:{response_quote:current.response,acknowledgement:'你的理由已经清楚。',guidance:'可以在新情境应用。',follow_up:null}};
  await w.vm.retry();await flushPromises();expect(w.find('.contextual-practice').exists()).toBe(true);
});

it('retries the same closed-book result operation and returns its course feedback without advancing',async()=>{
  view.progress.stage='explanation';
  const w=await player();
  vi.mocked(inquiryCourse.mutate).mockRejectedValueOnce(new Error('练习回课响应丢失'));
  await expect(w.vm.applyClosedBookResult('closed-result')).rejects.toThrow('练习回课响应丢失');
  const original=vi.mocked(inquiryCourse.mutate).mock.calls.at(-1)![1];
  expect(original.action).toEqual({kind:'use_closed_book',result_ref:'closed-result'});
  await expect(w.vm.applyClosedBookResult('different-result')).rejects.toThrow('练习回课响应丢失');
  expect(inquiryCourse.mutate).toHaveBeenCalledTimes(1);expect(w.emitted('engaged')).toBeUndefined();
  vi.mocked(inquiryCourse.mutate).mockImplementation(async(_scope,request)=>{
    if(request.action.kind==='use_closed_book')view.expressions.push({fact_ref:'reused-fact',job_ref:'reused-job',activity:'reflection',response:'读取更晚完成，因此它决定下界。',round_index:0,closed_book_result_ref:request.action.result_ref});
    view.progress.revision++;return structuredClone(view);
  });
  const scroll=vi.spyOn(HTMLElement.prototype,'scrollIntoView');
  const result=await w.vm.applyClosedBookResult('closed-result');await flushPromises();
  expect(result.expressions.at(-1)?.closed_book_result_ref).toBe('closed-result');
  expect(result.progress.stage).toBe('explanation');expect(result.progress.status).toBe('active');
  expect(vi.mocked(inquiryCourse.mutate).mock.calls.map(call=>call[1])).toEqual([original,original]);
  expect(w.emitted('engaged')).toHaveLength(1);
  expect(scroll.mock.contexts).toContain(w.get('.activity-tutor').element);
});

it('returns to an unchanged choice task when the closed-book result cannot replace its answer',async()=>{
  view.progress.stage='prediction';view.prediction_response=null;
  const w=await player();
  const result=await w.vm.applyClosedBookResult('choice-result');await flushPromises();
  expect(inquiryCourse.mutate).toHaveBeenCalledWith('course-space',expect.objectContaining({action:{kind:'use_closed_book',result_ref:'choice-result'}}));
  expect(result.progress.stage).toBe('prediction');expect(result.prediction_response).toBeFalsy();expect(result.expressions).toHaveLength(0);
  expect(w.emitted('engaged')).toHaveLength(1);
});

it('groups the provided book lessons by their authored units without implying complete book coverage',async()=>{
  const second={...authored.summary,id:'later-lesson',unit_id:'later-unit',question:'怎样检验实际耗时？'};
  const otherBook={...authored.summary,id:'other-book-lesson',question:'另一书的课程',source:{...authored.summary.source,publication:{book_id:'another-book',publication_id:'another-publication'}}};
  vi.mocked(inquiryCourse.catalog).mockResolvedValue({release_id:'release',release_version:1,course:{...sample,routes:[],units:[...sample.units,{id:'later-unit',title:'第二单元 · 实测校准'}],lessons:[authored.summary,second,otherBook]},progress:[view.progress]});
  const w=await player(true,{collection:{kind:'book',title:'深入理解 AI Infra',external:true}});
  const groups=w.findAll('.course-unit');
  expect(groups).toHaveLength(2);expect(groups[0]!.text()).toContain(authored.summary.question);expect(groups[1]!.text()).toContain(second.question);
  expect(w.get('.provided-lessons').text()).toBe('已提供 2 课');expect(w.text()).toContain('未列出的章节仍可阅读原文');
  expect(w.text()).not.toContain(otherBook.question);
  expect(w.findAll('.lesson-list li')).toHaveLength(2);expect(inquiryCourse.mutate).not.toHaveBeenCalled();
});

it('keeps the book, authored unit and current lesson visible while working on the task',async()=>{
  const w=await player(false,{collection:{kind:'book',title:'深入理解 AI Infra',external:true}});
  const breadcrumb=w.get('[aria-label="课程归属"]');
  expect(breadcrumb.text()).toContain('深入理解 AI Infra');expect(breadcrumb.text()).toContain(sample.units.find(unit=>unit.id===authored.summary.unit_id)!.title);
  expect(breadcrumb.get('[aria-current="page"]').text()).toBe(authored.summary.question);
  expect(w.get('.collection-return').text()).toBe('返回书籍学习首页');
  expect(w.findAll('button').some(button=>button.text()==='课程总目录')).toBe(true);
});

it('keeps the complete cross-book route and launches its next publication in the same workspace without a local start',async()=>{
  view.progress.stage='complete';view.progress.status='completed';
  const nextPublication={book_id:'systems-book',publication_id:'systems-publication-v3'};
  const next={...authored.summary,id:'systems-next',unit_id:'systems-unit',question:'怎样测量系统中的等待？',source:{...authored.summary.source,publication:nextPublication}};
  const route={id:'whole-course',title:'从资源到系统',question:'怎样解释整体性能？',structure_ref:'1',lesson_ids:[authored.summary.id,next.id]};
  vi.mocked(inquiryCourse.catalog).mockResolvedValue({release_id:'release',release_version:1,course:{...sample,units:[...sample.units,{id:'systems-unit',title:'系统测量'}],routes:[route],lessons:[next,authored.summary]},progress:[view.progress,
    {...view.progress,lesson_id:next.id,run_id:'old-version-next',release_version:0},
    {...view.progress,lesson_id:next.id,run_id:'old-goal-next',goal_revision:0},
    {...view.progress,lesson_id:next.id,run_id:'current-next',stage:'introduction',status:'active'}]});
  sessionStorage.setItem('understand-book:course-route:reader:course-space:release:1','whole-course');
  const w=await player(false,{collection:{kind:'course',title:'系统性能分析',external:true}});
  expect(w.get('.next-lesson-target').text()).toContain(next.question);
  await click(w,'进入下一课 →');
  expect(w.emitted('launch')?.at(-1)).toEqual([{publication:nextPublication,lessonId:next.id,runId:'current-next'}]);
  expect(inquiryCourse.mutate).not.toHaveBeenCalled();expect(w.get('[aria-label="课程归属"]').text()).toContain('系统性能分析');
  await click(w,'课程总目录');
  expect(w.get('#course-route').element).toHaveProperty('value','whole-course');expect(w.findAll('.lesson-list li')).toHaveLength(2);
  expect(w.findAll('.course-unit')[0]!.text()).toContain(authored.summary.question);
});

it.each(['version','goal'] as const)('does not offer the current collection next lesson from an older %s record',async kind=>{
  view.progress.stage='complete';view.progress.status='completed';
  if(kind==='version')view.progress.release_version=0;else view.progress.goal_revision=0;
  const next={...authored.summary,id:'next-current',question:'当前版本的下一课'};
  vi.mocked(inquiryCourse.catalog).mockResolvedValue({release_id:'release',release_version:1,course:{...sample,routes:[],lessons:[authored.summary,next]},progress:[view.progress]});
  const w=await player(true,{catalogOnly:false,embedded:true,collection:{kind:'book',title:'深入理解 AI Infra',external:true,releaseVersion:view.progress.release_version,overview:{...sample,lessons:sample.lessons.map(item=>item.summary),units:[{id:authored.summary.unit_id,title:'原记录所属单元'}]}}});
  await w.vm.openLesson(authored.summary.id,'run');await flushPromises();
  expect(w.find('.next-lesson-target').exists()).toBe(false);expect(w.get('[aria-label="课程归属"]').text()).toContain('原记录所属单元');expect(inquiryCourse.mutate).not.toHaveBeenCalled();
});

it('stays on the original task if saving its pause fails before returning to the parent collection',async()=>{
  view.progress.stage='prediction';
  const w=await player(false,{collection:{kind:'book',title:'深入理解 AI Infra',external:true}});
  await w.get('#course-reason').setValue('先比较读取和计算，尚未写完。');
  vi.mocked(inquiryCourse.mutate).mockRejectedValueOnce(new Error('暂停尚未保存'));
  await click(w,'返回书籍学习首页');
  expect(w.emitted('returnCollection')).toBeUndefined();expect((w.get('#course-reason').element as HTMLTextAreaElement).value).toBe('先比较读取和计算，尚未写完。');
  const operation=vi.mocked(inquiryCourse.mutate).mock.calls.at(-1)![1];
  await click(w,'重试刚才的操作');
  expect(vi.mocked(inquiryCourse.mutate).mock.calls.at(-1)![1]).toEqual(operation);
  expect(w.emitted('returnCollection')).toBeUndefined();
  await click(w,'返回书籍学习首页');
  expect(w.emitted('returnCollection')).toHaveLength(1);expect(view.progress.status).toBe('paused');
  expect(w.emitted('engaged')).toBeUndefined();expect(localStorage.getItem('course-answer:reader:course-space:run:0:prediction')).toContain('尚未写完');
});

it('allows the current answer and a metadata retry when the collection summary cannot be read',async()=>{
  view.progress.stage='prediction';
  const w=await player(false,{collection:{kind:'book',title:'深入理解 AI Infra',external:true},collectionError:'课程归属暂时无法读取'});
  expect(w.get('.collection-return').attributes('disabled')).toBeDefined();
  await click(w,'重试读取课程归属');expect(w.emitted('retryCollection')).toHaveLength(1);
  await w.get('input[type="radio"]').setValue();await w.get('#course-reason').setValue('比较两项耗时后再判断。');await w.get('form.stage-content').trigger('submit');await flushPromises();
  expect(inquiryCourse.mutate).toHaveBeenCalledWith('course-space',expect.objectContaining({action:{kind:'predict',response:{choice:authored.prediction.options[0]!.id,reason:'比较两项耗时后再判断。'}}}));
});

it('opens historical cross-book records at their original publication without installing them in the current reader',async()=>{
  const originalPublication={book_id:'other-book',publication_id:'historical-publication'};
  const original={...authored.summary,id:'historical-lesson',question:'原版的另一书课程',source:{...authored.summary.source,publication:originalPublication}};
  const progress={...view.progress,lesson_id:original.id,run_id:'historical-run',release_version:0};
  vi.mocked(inquiryCourse.catalog).mockResolvedValue({release_id:'release',release_version:1,course:{...sample,lessons:[authored.summary,original]},progress:[view.progress,progress]});
  vi.mocked(inquiryCourse.read).mockResolvedValue({...structuredClone(view),summary:original,progress});
  const w=await player(true,{catalogOnly:false,embedded:true,collection:{kind:'course',title:'跨书分析课程',external:true}});
  await w.get('.history button').trigger('click');await flushPromises();
  expect(w.emitted('launch')?.at(-1)).toEqual([{publication:originalPublication,lessonId:original.id,runId:'historical-run'}]);
  expect(w.find('.lesson-player').exists()).toBe(false);expect(inquiryCourse.mutate).not.toHaveBeenCalled();
});
