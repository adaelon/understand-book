// @vitest-environment happy-dom
import { mount, flushPromises } from '@vue/test-utils';
import { afterEach, expect, it, vi } from 'vitest';
import { installIdentity, network, type LearningWorkspace } from '../network-context';
import { daysUntilExam, type ExamSprintInput } from '../exam-sprints';
import ExamSprintPanel from './ExamSprintPanel.vue';

const settings: ExamSprintInput = { exam_date:'2026-10-23', syllabus:'前三章', daily_minutes:60, utc_offset_minutes:480, entry_mode:'diagnostic' };
const workspace: LearningWorkspace = { learning_workspace_id:'my-course',goal:{text:'通过期末',constraints:[],revision:2,created_at:1},goal_history:[],members:[{published_book_ref:{book_id:'book',publication_id:'v1'},readable:true,added_at:1}],archived:false,created_at:1,updated_at:1 };
afterEach(() => { vi.useRealTimers(); vi.unstubAllGlobals(); network.value = { ...network.value, enabled:false, identity:null, workspace:null }; });

it('recalculates remaining calendar days at local midnight, including past exams', () => {
  expect(daysUntilExam(settings, Date.parse('2026-10-20T15:59:00Z'))).toBe(3);
  expect(daysUntilExam(settings, Date.parse('2026-10-20T16:01:00Z'))).toBe(2);
  expect(daysUntilExam(settings, Date.parse('2026-10-23T16:01:00Z'))).toBe(-1);
});

it('lets the student bypass diagnosis and open the exact question in the existing learning workspace', async () => {
  installIdentity({ user_id:'student',csrf_token:'csrf' });
  const fetcher = vi.fn(async () => Response.json({sprint:{settings,goal_revision:2,current_goal_revision:2,remaining_days:3,updated_at:1,as_of:1}}));
  vi.stubGlobal('fetch',fetcher);
  const wrapper = mount(ExamSprintPanel,{props:{workspace}});
  try {
    await flushPromises();
    await wrapper.get('input[value=question]').setValue(true);
    await wrapper.get('textarea').setValue('这一步为什么可以换元？');
    const button = wrapper.findAll('button').find(button => button.text()==='打开这道题')!;
    await button.trigger('click');
    expect(wrapper.emitted('begin')).toEqual([['这一步为什么可以换元？']]);
    expect(fetcher).toHaveBeenCalledTimes(1);
  } finally { wrapper.unmount(); }
});

it('saves date and syllabus against the current goal revision and receives the new goal', async () => {
  installIdentity({user_id:'student',csrf_token:'csrf'});
  let submitted: any;
  vi.stubGlobal('fetch',vi.fn(async (_url:string,init:RequestInit) => {
    if(init.method==='POST') {
      submitted=JSON.parse(String(init.body));
      return Response.json({workspace:{...workspace,goal:{...workspace.goal,revision:3,text:submitted.goal_text}},sprint:{settings:submitted.sprint,goal_revision:3,current_goal_revision:3,remaining_days:4,updated_at:1,as_of:1}});
    }
    return Response.json({sprint:{settings,goal_revision:2,current_goal_revision:2,remaining_days:3,updated_at:1,as_of:1}});
  }));
  const wrapper = mount(ExamSprintPanel,{props:{workspace}});
  try {
    await flushPromises(); await wrapper.findAll('button').find(button=>button.text()==='调整考试安排')!.trigger('click');
    await wrapper.get('input[type=date]').setValue('2026-10-24');
    await wrapper.findAll('textarea')[0].setValue('只复习第四章');
    await wrapper.get('form').trigger('submit'); await flushPromises();
    expect(submitted).toMatchObject({expected_goal_revision:2,goal_text:'通过期末',sprint:{exam_date:'2026-10-24',syllabus:'只复习第四章'}});
    expect((wrapper.emitted('updated')?.[0][0] as LearningWorkspace).goal.revision).toBe(3);
  } finally { wrapper.unmount(); }
});
