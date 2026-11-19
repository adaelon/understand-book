// @vitest-environment happy-dom
import { flushPromises, mount } from '@vue/test-utils';
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import { setLocale } from '../i18n';
import { inquiryCourse, type CourseCatalog } from '../inquiry-course';
import type { CourseLessonView } from '../generated/CourseLessonView';
import type { ClosedBookPractice as Practice } from '../generated/ClosedBookPractice';
import ClosedBookPractice from './ClosedBookPractice.vue';
import CourseQuestions from './CourseQuestions.vue';
import UnderstandingEntry from './UnderstandingEntry.vue';
import CourseFeedbackOptions from './CourseFeedbackOptions.vue';
import LearningHomeTask from './LearningHomeTask.vue';
import { learningTaskAction, learningTaskStage, type LearningTaskView } from '../learning-tasks';

vi.mock('../inquiry-course', () => ({ inquiryCourse: { catalog: vi.fn(), read: vi.fn() } }));
const mounted: ReturnType<typeof mount>[] = [];
const practice: Practice = { id:'practice',owner_id:'reader',course:true,chat_session_id:'chat',source_id:'source',
  source_revision:'1',goal_revision:1,revision:1,status:'active',can_resume:true,target:'Compare the two durations',
  criteria:'Explain the bound',attempt:1,assisted:false,needs_retry:false,visible_turn_ids:[],draft:'',completion_reason:'',learner_quote:'' };
beforeEach(() => {
  setLocale('en'); vi.clearAllMocks();
  vi.spyOn(HTMLDialogElement.prototype, 'showModal').mockImplementation(() => {});
  vi.spyOn(HTMLDialogElement.prototype, 'close').mockImplementation(() => {});
});
afterEach(() => { mounted.splice(0).forEach(w => w.unmount()); vi.restoreAllMocks(); setLocale('zh-CN'); localStorage.clear(); });
function renderPractice(state: Practice | null = practice) {
  const w = mount(ClosedBookPractice, { props:{state,restoring:false,busy:false,error:'',messages:[],draft:''}, global:{stubs:{teleport:true}} });
  mounted.push(w); return w;
}

it('localizes practice controls, help, and canned retry while preserving the learner answer', async () => {
  const w=renderPractice({...practice,assisted:true});
  await w.setProps({draft:'我自己的回答',messages:[{id:'first',user:'',answer:'Compare the two durations.'},{id:'reply',user:'我的原文',answer:'Explain why.'}]});
  expect(w.get('h1').text()).toBe('Practice to mastery');
  expect(w.get('textarea').attributes('aria-label')).toBe('Your answer');
  expect(w.get('textarea').attributes('placeholder')).toBe('Write your understanding, approach, or where you are stuck…');
  expect(w.text()).toContain('Give me a hint'); expect(w.text()).toContain('Explain this step');
  expect(w.text()).toContain('我的原文');
  await w.get('form').trigger('submit'); expect(w.emitted('send')?.[0]).toEqual(['我自己的回答']);
  await w.findAll('button').find(b=>b.text()==='Try independently →')!.trigger('click');
  expect(w.emitted('send')?.[1]).toEqual(['Give me a new question for the same goal so I can try independently.']);
});
it('localizes practice recovery, pause, changed-goal and completion states', async () => {
  const w=renderPractice();
  await w.setProps({restoring:true}); expect(w.text()).toContain('Restoring practice…');
  await w.setProps({error:'Offline'}); expect(w.text()).toContain('Practice could not be restored. Your materials remain hidden.');
  await w.setProps({restoring:false,error:'',state:{...practice,status:'paused'}});
  expect(w.text()).toContain('Your question and draft are saved. Continue when you are ready.');
  expect(w.text()).toContain('Continue practice');
  await w.setProps({state:{...practice,status:'paused',can_resume:false}}); expect(w.text()).toContain('Start the current goal');
  await w.setProps({state:{...practice,status:'complete'}}); expect(w.text()).toContain('You can now do this independently');
  expect(w.text()).toContain('Return to lesson');
  expect(w.text()).toContain('Return this answer to the lesson');
  expect(w.text()).not.toMatch(/[\u3400-\u9fff]/);
  await w.setProps({state:{...practice,course:false,status:'complete'}});
  expect(w.text()).toContain('Return to learning'); expect(w.text()).not.toContain('Return to lesson');
});
it('shows neutral loading and retry until a saved practice is known', async () => {
  const w=renderPractice(null);
  await w.setProps({restoring:true});
  expect(w.find('h1').exists()).toBe(false);
  expect(w.get('[role=status]').attributes('aria-label')).toBe('Loading');
  expect(w.findAll('button')).toHaveLength(0);
  await w.get('dialog').trigger('cancel');expect(w.emitted('leave')).toBeUndefined();
  await w.setProps({error:'Offline'});
  expect(w.get('[role=alert]').text()).toContain('Your learning could not be restored. Please retry.');
  expect(w.text()).not.toContain('practice');
  await w.get('button').trigger('click');expect(w.emitted('retry')).toHaveLength(1);
  await w.setProps({restoring:false,error:'',state:practice});
  expect(w.get('h1').text()).toBe('Practice to mastery');
  expect(w.find('textarea').exists()).toBe(true);
});
it('localizes question scaffolding and leaves inserted learner text unchanged', async () => {
  const w=mount(CourseQuestions,{props:{expressions:[],resolved:[]}});mounted.push(w);
  expect(w.attributes('aria-label')).toBe('My questions');
  await w.findAll('button').find(b=>b.text()==='+ Add a question')!.trigger('click');
  expect(w.get('textarea').attributes('placeholder')).toBe('A single sentence is enough. Start with what is unclear.');
  await w.findAll('button').find(b=>b.text()==='Not sure how to ask? Try these three prompts')!.trigger('click');
  const inputs=w.findAll('.scaffold input');
  await inputs[0]!.setValue('原文 A');await inputs[1]!.setValue('为什么更慢');await inputs[2]!.setValue('改变带宽');
  await w.findAll('button').find(b=>b.text()==='Add to the question and refine it')!.trigger('click');
  await w.get('form').trigger('submit');
  expect(w.emitted('ask')?.[0]).toEqual(['I am looking at 原文 A.\nI do not understand: 为什么更慢\nI want to understand: 改变带宽']);
  await w.setProps({expressions:[{fact_ref:'fact',job_ref:'job',activity:'reflection',intent:'question',response:'原始问题'}]});
  expect(w.text()).toContain('1 still being explored');expect(w.text()).toContain('原始问题');
  await w.setProps({resolved:['fact']});expect(w.text()).toContain('I understand it now');
});
it('localizes feedback actions without translating authored guidance', async () => {
  const w=mount(CourseFeedbackOptions,{props:{feedback:{response_quote:'原文',acknowledgement:'ack',guidance:'guidance',follow_up:null,
    support_branch:{focus:'用户关注点',explanation:'explanation',example:'example',return_prompt:'return'},
    question_guide:{suggested_question:'保留我的提问',why:'给出的依据',try_next:'实际的操作'}}}});mounted.push(w);
  expect(w.text()).toContain('Work on this first');expect(w.text()).toContain('How to find an answer');
  expect(w.text()).toContain('用户关注点');expect(w.text()).toContain('给出的依据');
  await w.get('.question-guide button').trigger('click');expect(w.emitted('question')?.[0]).toEqual(['保留我的提问']);
});
it('localizes the understanding summary, progress and error recovery', async () => {
  vi.mocked(inquiryCourse.catalog).mockResolvedValue({release_id:'release',release_version:1,course:{lessons:[{id:'lesson'}]},progress:[{run_id:'run',lesson_id:'lesson',release_version:1,status:'completed'}]} as CourseCatalog);
  vi.mocked(inquiryCourse.read).mockResolvedValue({summary:{source:{publication:{}}},reviews:[{displayed:true,understanding:[{learner_summary:{headline:'保留我的理解'}}]}]} as CourseLessonView);
  const w=mount(UnderstandingEntry,{props:{workspaceId:'workspace'}});mounted.push(w);await flushPromises();
  expect(w.attributes('aria-label')).toBe('My understanding and progress');
  expect(w.text()).toContain('保留我的理解');expect(w.text()).toContain('1 / 1 lessons');
  expect(w.get('progress').attributes('aria-label')).toBe('Course completion progress');
  vi.mocked(inquiryCourse.catalog).mockRejectedValueOnce(new Error('offline'));
  await w.setProps({workspaceId:'offline'});await flushPromises();
  expect(w.get('[role=status]').text()).toContain('Progress is temporarily unavailable');
  vi.mocked(inquiryCourse.catalog).mockResolvedValue({release_id:'release',release_version:1,course:null,progress:[]});
  await w.get('[role=status] button').trigger('click');await flushPromises();
  expect(w.find('[role=status]').exists()).toBe(false);expect(w.text()).toContain('Connect the things you understand.');
});
it('retains the Chinese UI when Chinese is selected', () => {
  setLocale('zh-CN');const w=renderPractice();expect(w.get('h1').text()).toBe('练到会');expect(w.text()).toContain('给一点提示');
});

const learningTask: LearningTaskView = {
  learning_workspace_id:'workspace',published_book_ref:{publication_id:'publication',book_id:'book'},
  course_run_id:'run',chat_session_id:'chat',selected_at:1,kind:'course',title:'我的学习问题',status:'active',
  lesson_id:'lesson',stage:'prediction',closed_book_id:null,question:'我的原始回答',message:null,
};
it('localizes every task stage and action, including dynamic labels', () => {
  const stages: Record<string,string> = {
    introduction:'Start with the question and key concepts',prediction:'Leave your prediction and reasoning',
    experiment:'Continue observing, gathering evidence or experimenting',explanation:'Use the evidence to build your own explanation',
    task:'Test your understanding in a new situation',feedback:'Use the feedback to continue the current task',
    unknown:'Continue your previous learning task',
  };
  for (const [stage,label] of Object.entries(stages)) expect(learningTaskStage({...learningTask,stage})).toBe(label);
  expect(learningTaskAction(learningTask)).toBe('Continue answering');
  expect(learningTaskAction({...learningTask,stage:'experiment'})).toBe('Continue this lesson');
  expect(learningTaskAction({...learningTask,kind:'closed_book',status:'paused'})).toBe('Continue practice');
  expect(learningTaskStage({...learningTask,kind:'closed_book'})).toBe('Closed-book practice · Your question and draft are saved');
  expect(learningTaskAction({...learningTask,kind:'conversation'})).toBe('Continue the discussion');
  expect(learningTaskStage({...learningTask,kind:'conversation'})).toBe('Return to your original question and conversation');
});
it('localizes learning-home states and system messages while retaining authored content', async () => {
  const w=mount(LearningHomeTask,{props:{tasks:[],loading:false,error:'',busy:false}});mounted.push(w);
  expect(w.text()).toContain('Choose a book or a complete course');
  expect(w.text()).toContain('Follow a learning path, one question at a time.');
  expect(w.text()).toContain('Learn with your own materials');
  expect(w.text()).not.toMatch(/[\u3400-\u9fff]/);
  await w.get('.task-primary').trigger('click');expect(w.emitted('courses')).toHaveLength(1);
  const unavailable: LearningTaskView = {...learningTask,course_run_id:'missing',status:'unavailable',message:'原学习材料当前不可用'};
  const withContext: LearningTaskView = {...learningTask,course_context:{release_id:'original-release',release_version:2,
    collection_kind:'course',collection_title:'保留原课程名称',unit_title:'保留原单元名称',lesson_number:3,lesson_count:8}};
  await w.setProps({tasks:[withContext,unavailable]});
  expect(w.text()).toContain('Pick up where you left off');
  expect(w.text()).toContain('我的学习问题');expect(w.text()).toContain('我的原始回答');
  expect(w.get('.task-belonging').attributes('aria-label')).toBe('Your learning path');
  expect(w.get('.task-collection').text()).toBe('保留原课程名称');
  expect(w.get('.task-course-position').findAll('span').map(span=>span.text())).toEqual(['保留原单元名称','Lesson 3 of 8']);
  await w.get('.task-collection').trigger('click');
  expect(w.emitted('collection')?.[0]).toEqual([{kind:'course',releaseId:'original-release',version:2,learningWorkspaceId:'workspace'}]);
  expect(w.emitted('resume')).toBeUndefined();
  expect(w.get('.other-tasks small').text()).toBe('The original learning materials are currently unavailable.');
  await w.get('.task-primary').trigger('click');expect(w.emitted('resume')?.[0]).toEqual([withContext]);
  await w.setProps({tasks:[unavailable]});
  expect(w.get('.task-message').text()).toBe('The original learning materials are currently unavailable.');
  expect(w.get('.task-primary').attributes('disabled')).toBeDefined();
  await w.setProps({tasks:[],loading:true});expect(w.text()).toContain('Restoring your learning…');
  expect(w.get('[role=status]').text()).toBe('Restoring your learning task…');
  await w.setProps({loading:false,error:'Your learning tasks could not be loaded. Please retry.'});
  expect(w.get('h2').text()).toBe('Your saved learning is temporarily unavailable');
  await w.get('[role=alert] button').trigger('click');expect(w.emitted('retry')).toHaveLength(1);
});
