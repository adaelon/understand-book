import { mkdirSync, mkdtempSync, readFileSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { describe, it, expect } from 'vitest';
import { routeCourseBuild, closeCourseBuild, freezeCourseTask, type CourseGenerationTask } from '../src/course-build';
import { source } from './fixtures/teaching-source';
import type { AcceptedCourseRoute } from '../src/accepted-course-route';
import type { TeachingBuildInput } from '../src/teaching-build';
import { buildReproducibleProfileArtifactHeader } from '../src/profile-artifact';
import { renderAutomaticBuildTaskInput, runAutomaticBuildStageWriter, resolveAutomaticBuildPromptAsset, resolveAutomaticBuildExecutorPrompt } from '../../../skills/build/automatic-build';
import { automaticBuildExtractionPolicy } from '../src/semantic-artifact';
import { resolveContentProfile } from '../src/content-profile';
import { standardDeepStageClosure } from '../src/build-capability';
import { nextAutomaticBuildAction } from '../src/build-orchestrator';
import { buildAutomaticBuildPreflight } from '../src/automatic-build-budget';

function fixture():TeachingBuildInput {
  const workspace=mkdtempSync(path.join(tmpdir(),'course-build-'));
  return {source,units:[],target:{kind:'source_file',profile_id:'technical_learning',book_id:source.source_id,root_dir:workspace,workspace_dir:workspace,source_path:path.join(workspace,'source.txt'),
    target_ref:{version:'build_target_ref.v2',profile_id:'technical_learning',book_id:source.source_id,workspace_dir:workspace,input_fingerprint:source.source_revision}},
    structure:{header:buildReproducibleProfileArtifactHeader({book_id:source.source_id}),spine:['1'].map(lid=>({lid,role:'foundation',summary:{text:'平均速率',evidence_lids:source.passages.filter(p=>p.unit_lid===lid).map(p=>p.lid)},key_stop_ids:[],depends_on:[]})),throughlines:[],key_stops:[]}};
}
const scope={title:'平均速率',question:'为什么要比较总路程与总时间？',route:'chapters',unit_lids:['1']};
function pending(input:TeachingBuildInput, route?:AcceptedCourseRoute){const state=routeCourseBuild(input, undefined, route);if(state.course_blocked)throw new Error(state.course_blocked);return Object.values(state.generation_tasks??{}).flatMap(t=>t.kind==='course'&&state.pending_tasks.includes(t.task.descriptor.work_unit_id)?[t.task]:[]);}
function write(input:TeachingBuildInput,task:CourseGenerationTask,value:unknown){
  freezeCourseTask(input.target,task);
  expect(renderAutomaticBuildTaskInput(input.target,'course',task.descriptor.work_unit_id,{policy_generation_id:task.policy_generation_id}).stdout).toBe(task.rendered_input);
  const file=path.join(input.target.workspace_dir,'candidate.json');writeFileSync(file,JSON.stringify(value));
  return runAutomaticBuildStageWriter(input.target,'course',task.descriptor.work_unit_id,file,{policy_generation_id:task.policy_generation_id,executor:'fixture',attempt:1,generated_at:'2026-11-12T00:00:00.000Z'});
}
function candidate(task:CourseGenerationTask):unknown {
  const lids=task.source.passages.map(p=>p.lid);
  switch(task.task_kind){
    case 'scope':return scope;
    case 'plan':return {language:'zh-CN',unit_title:'速率的依据',lessons:[{id:'rate',question:'平均速率如何确定？',objective:'由路程和时间解释速率',prerequisites:'会进行除法',estimated_minutes:20,duration_basis:'阅读5分、推理与作答10分、反馈5分',source_lids:lids,blueprint:'先独立算总路程和总时间，再设计两种情境，最终解释速率不可随意平均。'}]};
    case 'lesson':return {introduction:'先说出你对平均速率的理解。',prediction:{question:'两个速率可以直接平均吗？',options:[]},experiment:{kind:'inquiry',activity:'reasoning',instruction:'核对路程与时间的关系',material:'阅读原文，比较平均速率与分段速率。',response_prompt:'写下成立条件及理由。'},explanation:'总路程除以总时间；不能忽略各段时间。',task:{question:'说明什么时候可以直接平均两个速率。',options:[]},hint:'从各段用时考虑。',assessment:{rule:{kind:'open',items:[{id:'time',criterion:'说明等时间条件及加权关系',source_lids:lids}]},source_lids:lids,source_sufficient:true,feedback:'after_submit'},solution:'等时间才可直接平均；一般情形按时间加权。',reflection_prompt:'哪些依据改变了你的初步判断？',reading:{before_experiment:'找到平均速率的定义',after_experiment:'用定义核对上述条件'}};
    case 'solve':return {solutions:JSON.parse(task.rendered_input).questions.map((q:{question:string})=>({question:q.question,reasoning:'以总路程除以总时间独立推导。',answer:'等时间才可直接平均。'})),issues:[]};
    case 'review':return {accepted:true,checked_lids:lids,reasoning:'来源提供了定义，独立解答与作者解答一致，活动需要学生写出推理。',issues:[]};
  }
}
describe('automatic course construction',()=>{
  it('holds the accepted chapter selection and requires independent verification of a matching trial',()=>{
    const input = fixture();
    for (let i=0; i<5; i++) {
      const task = pending(input)[0];
      write(input,task,candidate(task));
    }
    closeCourseBuild(input);
    const courseFile = path.join(input.target.workspace_dir,'inquiry_course.json');
    const draftDir = path.join(input.target.workspace_dir,'.build','course-trials','pilot-1');
    mkdirSync(draftDir,{recursive:true});
    writeFileSync(path.join(draftDir,'inquiry_course.json'),readFileSync(courseFile));

    input.structure.spine.push({...input.structure.spine[0],lid:'2'});
    const route:AcceptedCourseRoute = {status:'accepted',source_id:source.source_id,source_revision:source.source_revision,
      title:'速率与时间',question:'怎样确认速率比较的条件？',audience:'会除法的读者',final_task:'解释不同测量条件下的比较',
      unit_lids:['1'],example_lessons:['怎样确认平均速率？'],trial_revision:'pilot-1'};
    const tasks = pending(input,route);
    expect(tasks.map(t=>t.task_kind)).toEqual(['plan']);
    const planInput = JSON.parse(tasks[0].rendered_input);
    expect(planInput.accepted_route).toEqual(route);
    expect(planInput.trial_candidate.question).toBe('平均速率如何确定？');
    expect(planInput.source.passages.every((p:{unit_lid:string})=>p.unit_lid==='1')).toBe(true);
    const plan = candidate(tasks[0]);
    write(input,tasks[0],plan);
    const solve = pending(input,route)[0];
    expect(solve.task_kind).toBe('solve');
    const trialArtifact = JSON.parse(readFileSync(courseFile,'utf8'));
    expect(solve.rendered_input).not.toContain(trialArtifact.course.lessons[0].solution);
    expect(()=>closeCourseBuild(input,undefined,route)).toThrow();
    write(input,solve,candidate(solve));
    const review = pending(input,route)[0];
    expect(review.task_kind).toBe('review');
    write(input,review,candidate(review));
    closeCourseBuild(input,undefined,route);
    expect(JSON.parse(readFileSync(courseFile,'utf8')).course.routes[0]).toMatchObject({question:route.question,structure_ref:'1'});
    const report = JSON.parse(readFileSync(path.join(input.target.workspace_dir,'course_build_report.json'),'utf8'));
    expect(report.accepted_route).toEqual(route);
    expect(report.lessons[0].trial_revision).toBe('pilot-1');
    expect(report.task_ids).toHaveLength(3);

    const revised = {...route, final_task:'在新前提下独立评价测量方式'};
    const amended = pending(input,revised)[0];
    const otherPlan = candidate(amended) as {lessons:{objective:string}[]};
    otherPlan.lessons[0].objective = '评价测量方式';
    write(input,amended,otherPlan);
    expect(pending(input,revised)[0].task_kind).toBe('lesson');
    expect(routeCourseBuild(input,undefined,{...route,source_revision:'other'}).course_blocked).toMatch(/版本/);
    expect(routeCourseBuild(input,undefined,{...route,unit_lids:['missing']}).course_blocked).toContain('missing');
  });
  it('dispatches a course task with six-minute history and still enforces the approved wall-time budget',()=>{
    const input=fixture();
    const state=routeCourseBuild(input);
    const units=state.work_units!;
    expect(units).toHaveLength(1);
    const executor={model:'scripted',reasoning_effort:'high',harness_release:'course-history-fixture'};
    const preflight={target_ref:input.target.target_ref,stage:'course' as const,work_units:units,
      pending_ids:state.pending_tasks,task_bindings:state.task_bindings,quality_profile:'full' as const,
      requested_workers:1,available_agent_slots:1,
      budget:{version:'automatic_build_budget_limits.v1' as const,max_tasks:100,max_total_score:100_000_000,
        max_estimated_total_tokens:100_000_000,max_batch_score:10_000_000,max_parallel_cost:10_000_000},
      executor_provenance:executor,
      historical_performance:{version:'automatic_build_performance_history.v1' as const,
        samples:[{sample_id:'course-six-minutes',stage:'course' as const,kind:'course_task' as const,
          router_version:units[0].policy_fingerprint.router_version,...executor,service_ms:360_000}],
        lease_count:1,semantic_attempt_count:1},
      wall_budget:{version:'automatic_build_wall_budget.v1' as const,max_wall_clock_minutes:10,on_exceed:'needs_user' as const}};
    const allowed=buildAutomaticBuildPreflight(preflight);
    expect(allowed.dispatch_plan.dispatches).toHaveLength(1);
    expect(allowed.dispatch_plan.selected_dispatch_ids).toHaveLength(1);
    expect(allowed.dispatch_plan.dispatches[0].accounting.predicted_service_ms).toBe(360_000);
    expect(allowed.wall_clock.budget.status).toBe('within_budget');
    const limited=buildAutomaticBuildPreflight({...preflight,wall_budget:{...preflight.wall_budget,max_wall_clock_minutes:5}});
    expect(limited.wall_clock.budget).toMatchObject({status:'exceeded',
      violations:[{code:'max_wall_clock_minutes',actual:6,limit:5}]});
  });
  it('uses the normal frozen input/writer protocol, resumes accepted work, and closes only a reviewed lesson',()=>{
    const input=fixture();let solveInput='';
    for(let round=0;round<5;round++){
      const tasks=pending(input);expect(tasks.length).toBe(1);
      expect(()=>closeCourseBuild(input)).toThrow();
      if(tasks[0].task_kind==='solve')solveInput=tasks[0].rendered_input;
      write(input,tasks[0],candidate(tasks[0]));
      expect(pending(structuredClone(input)).some(t=>t.descriptor.work_unit_id===tasks[0].descriptor.work_unit_id)).toBe(false);
    }
    expect(solveInput).not.toContain('assessment');expect(solveInput).not.toContain('solution:');expect(solveInput).not.toContain('blueprint');
    expect(nextAutomaticBuildAction({target:input.target,stages:[routeCourseBuild(input)]},1).kind).toBe('close_stage');
    closeCourseBuild(input);expect(routeCourseBuild(input).closed).toBe(true);
    const published=JSON.parse(readFileSync(path.join(input.target.workspace_dir,'inquiry_course.json'),'utf8'));
    expect(published.course.lessons[0].experiment.kind).toBe('inquiry');
    expect(published.course.lessons[0].summary.source.source_revision).toBe(source.source_revision);
    expect(published.course.lessons).toHaveLength(1);
    published.course.lessons[0].solution='替换后的未经核验答案';
    writeFileSync(path.join(input.target.workspace_dir,'inquiry_course.json'),JSON.stringify(published));
    expect(routeCourseBuild(input).closed).toBe(false);
    closeCourseBuild(input);expect(routeCourseBuild(input).closed).toBe(true);
    expect(routeCourseBuild(input,'只构建第二章').closed).toBe(false);
  });
  it('repairs only rejected lesson content and retains the independent review trail',()=>{
    const input=fixture();for(let n=0;n<4;n++){const t=pending(input)[0];write(input,t,candidate(t));}
    const rejected=pending(input)[0];write(input,rejected,{accepted:false,checked_lids:rejected.source.passages.map(p=>p.lid),reasoning:'最终题缺少条件',issues:['题目要明确分段时间条件']});
    const repair=pending(input)[0];expect(repair.task_kind).toBe('lesson');expect(repair.rendered_input).toContain('题目要明确分段时间条件');
    expect(()=>closeCourseBuild(input)).toThrow();
    for(let n=0;n<3;n++){const t=pending(input)[0];write(input,t,candidate(t));}
    closeCourseBuild(input);
    expect(JSON.parse(readFileSync(path.join(input.target.workspace_dir,'course_build_report.json'),'utf8')).task_ids).toHaveLength(8);
  });
  it('rejects foreign source bindings and incomplete independent solutions before acceptance',()=>{
    const input=fixture();let t=pending(input)[0];write(input,t,candidate(t));t=pending(input)[0];
    const bad=candidate(t) as any;bad.lessons[0].source_lids=['999'];expect(()=>write(input,t,bad)).toThrow();
    write(input,t,candidate(t));t=pending(input)[0];write(input,t,candidate(t));t=pending(input)[0];
    expect(()=>write(input,t,{solutions:[],issues:[]})).toThrow();
  });
  it('selects courses without the retired teaching stages and resolves the shipped prompt',()=>{
    const closure=standardDeepStageClosure({id:'technical_learning',version:'technical_learning_v0'},{build_tutor:false,build_course:true});
    expect(closure).toContain('course');expect(closure).not.toContain('formal_objects');
    const policy=automaticBuildExtractionPolicy('course',resolveContentProfile('technical_learning'),'full');
    expect(resolveAutomaticBuildPromptAsset('course-builder.md').sha256).toBe(policy.prompt_sha256);
    expect(resolveAutomaticBuildExecutorPrompt('course-builder.md','task').byte_length).toBeGreaterThan(0);
    expect(resolveAutomaticBuildExecutorPrompt('course-builder.md','dispatch').byte_length).toBeGreaterThan(0);
  });
});
