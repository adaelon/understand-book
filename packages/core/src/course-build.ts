import { existsSync, mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import path from 'node:path';
import { z } from 'zod';
import type { AutomaticBuildStageState, AutomaticBuildTarget } from './build-orchestrator';
import type { TeachingBuildInput } from './teaching-build';
import type { AcceptedCourseRoute } from './accepted-course-route';
import { canonicalBuildJson } from './build-intent';
import { resolveContentProfile } from './content-profile';
import { evaluateModelInputBudget } from './model-input-budget';
import { AUTOMATIC_BUILD_MODEL_INPUT_BUDGET_V1 } from './automatic-build-protocol';
import { buildWorkUnitCost, createWorkUnitDescriptorV3, taskPolicyBindingForWorkUnit, type WorkUnitDescriptorV3 } from './stage-work-unit';
import { createAutomaticBuildStagePolicySet } from './automatic-build-policy-generation';
import { automaticBuildExtractionPolicy, automaticBuildGenerationArtifactPath, buildSemanticArtifactEnvelopeV3, inspectSemanticArtifact,
  semanticContractFromExtractionPolicy, writeAutomaticBuildGenerationArtifact, relocateGenerationTask,
  type SemanticArtifactEnvelopeV3, type SemanticArtifactProvenanceV2 } from './semantic-artifact';
import { buildAutomaticBuildStageBatchResult, publishAutomaticBuildArtifactSet } from './automatic-build-publication';
import { createAutomaticBuildFailureDiagnosticV3, throwExtractorSchemaError } from './extractor-contract';
import { estimateTokens } from './window';

const text = z.string().trim().min(1);
const prompt = z.object({question:text,options:z.array(z.object({id:text,text}).strict())}).strict();
const resource = z.object({kind:z.literal('resource_bounds'),instruction:text,work_gflops:z.number().positive(),read_gb:z.number().positive(),compute_tflops:z.number().positive(),bandwidth_tb_s:z.number().positive(),batch_size:z.number().int().min(1).max(256).optional()}).strict();
const mathVisualization = z.discriminatedUnion('kind', [
  z.object({kind:z.literal('convex_chord'),function:z.enum(['square','cube']),a:z.number().min(-2).max(2),b:z.number().min(-2).max(2),t:z.number().min(0).max(1)}).strict(),
  z.object({kind:z.literal('gradient_descent'),step_size:z.number().min(0).max(.16),steps:z.number().int().min(1).max(8)}).strict(),
  z.object({kind:z.literal('feasible_directions'),position:z.number().min(-.5).max(2.5),angle:z.number().min(-180).max(180)}).strict(),
]);
const inquiry = z.object({kind:z.literal('inquiry'),activity:z.enum(['reading','reasoning','practice']),instruction:text,material:text,response_prompt:text,visualization:mathVisualization.optional()}).strict();
const experiment = z.discriminatedUnion('kind',[inquiry,resource]);
const reading = z.object({before_experiment:text,after_experiment:text}).strict();
const round = z.object({title:text,prediction:prompt,experiment,explanation:text,reflection_prompt:text,reading}).strict();
const rubric = z.object({id:text,criterion:text,source_lids:z.array(text).min(1)}).strict();
const assessment = z.object({rule:z.discriminatedUnion('kind',[
  z.object({kind:z.literal('choice_set'),choices:z.array(text).min(2),correct:z.array(text).length(1)}).strict(),
  z.object({kind:z.literal('open'),items:z.array(rubric).min(1).max(12)}).strict(),
]),source_lids:z.array(text).min(1),source_sufficient:z.literal(true),feedback:z.literal('after_submit')}).strict();
export const CourseBodyZ = z.object({introduction:text,prediction:prompt,experiment,
  explanation:text,task:prompt,hint:text,assessment,solution:text,reflection_prompt:text,
  concepts:z.array(z.object({title:text,explanation:text,relation:text}).strict()).default([]),
  reading,round_title:text.optional(),follow_up_rounds:z.array(round).max(6).default([]),
}).strict();
const BlueprintZ = z.object({id:z.string().regex(/^[a-zA-Z0-9_-]+$/),question:text,objective:text,prerequisites:text,
  estimated_minutes:z.number().int().min(5).max(30),duration_basis:text,source_lids:z.array(text).min(1),blueprint:text}).strict();
const PlanZ = z.object({language:text,unit_title:text,lessons:z.array(BlueprintZ).min(1).max(8)}).strict();
const ScopeZ = z.object({title:text,question:text,route:z.enum(['chapters','theme']),unit_lids:z.array(text).min(1)}).strict();
const SolveZ = z.object({solutions:z.array(z.object({question:text,reasoning:text,answer:text}).strict()).min(2),issues:z.array(text)}).strict();
const ReviewZ = z.object({accepted:z.boolean(),checked_lids:z.array(text).min(1),reasoning:text,issues:z.array(text)}).strict();
type Blueprint = z.infer<typeof BlueprintZ>;
type Body = z.infer<typeof CourseBodyZ>;
type Plan = z.infer<typeof PlanZ>;
type Review = z.infer<typeof ReviewZ>;
type Payload = Plan | Body | z.infer<typeof SolveZ> | Review | z.infer<typeof ScopeZ>;
export interface CourseGenerationTask {
  descriptor: WorkUnitDescriptorV3; policy_generation_id:string; rendered_input:string;
  task_kind:'scope'|'plan'|'lesson'|'solve'|'review'; source:TeachingBuildInput['source']; blueprint?:Blueprint;
  course_goal?:string;
  course_route?:AcceptedCourseRoute;
}
const generation = 'course.full.v1';
const read = <T>(file:string):T|undefined => existsSync(file)?JSON.parse(readFileSync(file,'utf8')):undefined;
const json = (value:unknown) => JSON.stringify(value,null,2)+'\n';
const contracts = {
  scope: {title:'课程路线标题',question:'路线总问题',route:'chapters',unit_lids:['结构中实际存在的章节LID，按学习顺序']},
  plan: {language:'zh-CN',unit_title:'章节或单元标题',lessons:[{id:'lesson-1',question:'主问题',objective:'可观察的理解目标',prerequisites:'起点',estimated_minutes:25,duration_basis:'阅读、思考、活动、作答及反馈用时',source_lids:['实际LID'],blueprint:'来源条件、最终任务及完整解法、支持关系、活动安排'}]},
  lesson: {introduction:'核心问题及概念引入',prediction:{question:'初步判断与理由',options:[]},experiment:{kind:'inquiry',activity:'reasoning',instruction:'具体活动',material:'完整材料（Markdown）',response_prompt:'需要写出的取证或推理'},explanation:'观察后的完整解释',task:{question:'新情境应用题',options:[]},hint:'所需提示',assessment:{rule:{kind:'open',items:[{id:'criterion-1',criterion:'可接受判断、条件与等价论证',source_lids:['实际LID']}]},source_lids:['实际LID'],source_sufficient:true,feedback:'after_submit'},solution:'私有参考推理与答案',reflection_prompt:'把依据与解释联系起来',concepts:[],reading:{before_experiment:'回读原文的具体任务',after_experiment:'对照观察核验的关系'}},
  solve: {solutions:[{question:'与输入题面完全相同',reasoning:'独立求解过程与条件',answer:'结论及合理替代解'}],issues:[]},
  review: {accepted:true,checked_lids:['实际读过的LID'],reasoning:'来源、题解、支持、活动和结果揭示的具体核对',issues:[]},
};
export { BlueprintZ as CourseBlueprintZ, contracts as courseGenerationContracts, validateBody as validateCourseLessonBody };
function make(input:TeachingBuildInput, kind:CourseGenerationTask['task_kind'], id:string, source:TeachingBuildInput['source'], body:object, blueprint?:Blueprint):CourseGenerationTask {
  const policy=automaticBuildExtractionPolicy('course',resolveContentProfile(input.target.profile_id),'full');
  const rendered=canonicalBuildJson({task_kind:kind,source,output_contract:contracts[kind],
    ...(kind==='lesson'?{activity_variants:[contracts.lesson.experiment,{kind:'resource_bounds',instruction:'比较资源时间下界；说明充分重叠假设',work_gflops:120,read_gb:16,compute_tflops:60,bandwidth_tb_s:2}],
      choice_assessment:{kind:'choice_set',choices:['a','b'],correct:['a']},
      optional_rounds:{round_title:'首轮标题',follow_up_rounds:[{title:'下一轮主问题',prediction:contracts.lesson.prediction,experiment:contracts.lesson.experiment,explanation:'本轮解释',reflection_prompt:'本轮表达任务',reading:contracts.lesson.reading}]}}:{}),...body});
  const budget=evaluateModelInputBudget({...AUTOMATIC_BUILD_MODEL_INPUT_BUDGET_V1,rendered_input:rendered,router_version:policy.router_version,
    prompt_sha256:policy.prompt_sha256,stage_body_limit_tokens:22000,prompt_reserve_tokens:2048,output_reserve_tokens:8192,executor_context_floor_tokens:32768});
  if(budget.status!=='within_limit')throw new Error(`课程任务 ${id} 超过输入预算，需要缩小所选来源范围`);
  const lids=kind==='scope' ? [...new Set(input.structure.spine.flatMap(u=>u.summary.evidence_lids))] : source.passages.map(p=>p.lid);
  const descriptor=createWorkUnitDescriptorV3({target:input.target.target_ref,stage:'course',kind:'course_task',work_unit_id:`${kind}-${id}-${budget.proof.rendered_input_sha256.slice(0,16)}`,
    input_basis:{kind:'semantic_projection',projection_kind:'book_structure',source_fingerprint:source.source_revision,projection_sha256:budget.proof.rendered_input_sha256,parent_lids:lids},
    input_hash:budget.proof.rendered_input_sha256,input_budget_proof:budget.proof,policy_fingerprint:policy,evidence_lids:lids,
    cost:buildWorkUnitCost({estimated_input_tokens:budget.proof.estimated_rendered_tokens,visible_lids:lids.length,expected_output_items:1})});
  return {descriptor,policy_generation_id:generation,rendered_input:rendered,task_kind:kind,source,...(blueprint?{blueprint}:{})};
}
export function courseTaskPath(target:AutomaticBuildTarget, version:string, id:string) {return path.join(target.workspace_dir,'.build','course',version,`${encodeURIComponent(id)}.json`);}
export function freezeCourseTask(target:AutomaticBuildTarget,task:CourseGenerationTask) {
  const file=courseTaskPath(target,task.policy_generation_id,task.descriptor.work_unit_id),old=read<CourseGenerationTask>(file);
  if(old && canonicalBuildJson(relocateGenerationTask(old,target.target_ref))!==canonicalBuildJson(task))throw new Error('course frozen task changed');
  if(!old){mkdirSync(path.dirname(file),{recursive:true});writeFileSync(file,json(task),{flag:'wx'});}
}
export function readCourseTask(target:AutomaticBuildTarget,version:string,id:string) {
  const task=read<CourseGenerationTask>(courseTaskPath(target,version,id));if(!task)throw new Error('course frozen task missing');
  return relocateGenerationTask(task,target.target_ref);
}
function accepted(input:TeachingBuildInput,task:CourseGenerationTask):Payload|undefined {
  const d=task.descriptor;
  const artifact=read<SemanticArtifactEnvelopeV3<Payload>>(automaticBuildGenerationArtifactPath(input.target,'course',generation,d.work_unit_id));
  if(!artifact)return;
  const inspection=inspectSemanticArtifact(artifact,{target:input.target.target_ref,stage:'course',work_unit_id:d.work_unit_id,input_hash:d.input_hash,
    policy_generation_id:generation,semantic_contract:semanticContractFromExtractionPolicy(d.policy_fingerprint)});
  return inspection.policy_fresh && inspection.format==='v3'?artifact.payload:undefined;
}
function bindings(task:Pick<CourseGenerationTask,'source'>,lids:string[]) {
  if(new Set(lids).size!==lids.length || lids.some(lid=>!task.source.passages.some(p=>p.lid===lid)))throw new Error('课程引用必须来自本任务实际提供的原文，且不重复');
}
function validateBody(task:Pick<CourseGenerationTask,'source'>,body:Body) {
  bindings(task,body.assessment.source_lids);
  const questions=[body.prediction,...body.follow_up_rounds.map(r=>r.prediction),body.task];
  for(const p of questions)if(p.options.length===1 || new Set(p.options.map(o=>o.id)).size!==p.options.length)throw new Error('课程选项应为空或至少两个且不重复');
  if(new Set(questions.map(q=>q.question)).size!==questions.length)throw new Error('各轮需要独立的问题，不能重复同一题面');
  if(body.follow_up_rounds.length && !body.round_title)throw new Error('多轮课程需要首轮标题');
  const rule=body.assessment.rule;
  if(rule.kind==='open'){
    if(body.task.options.length || new Set(rule.items.map(i=>i.id)).size!==rule.items.length)throw new Error('开放题需要无选项题面及唯一判断项');
    rule.items.forEach(i=>{bindings(task,i.source_lids);if(i.source_lids.some(l=>!body.assessment.source_lids.includes(l)))throw new Error('判断项来源不在评分合同中');});
  }else if(canonicalBuildJson([...rule.choices].sort())!==canonicalBuildJson(body.task.options.map(o=>o.id).sort()) || !rule.choices.includes(rule.correct[0]))throw new Error('课程题面与答案选项不一致');
  for(const e of [body.experiment,...body.follow_up_rounds.map(r=>r.experiment)])if(e.kind==='resource_bounds'){
    if(!Number.isFinite(e.work_gflops/e.compute_tflops)||!Number.isFinite(e.read_gb/e.bandwidth_tb_s))throw new Error('资源实验结果必须为有限值');
  }
}
export function acceptCourseCandidate(target:AutomaticBuildTarget,task:CourseGenerationTask,candidate:unknown,provenance:SemanticArtifactProvenanceV2) {
  let payload:Payload;
  try {
    if(task.task_kind==='scope'){
      const data=JSON.parse(task.rendered_input),scope=ScopeZ.parse(candidate),units=data.structure.spine as {lid:string}[];
      if(new Set(scope.unit_lids).size!==scope.unit_lids.length || scope.unit_lids.some(lid=>!units.some(u=>u.lid===lid)))throw new Error('课程范围必须选择已有结构中的章节');payload=scope;
      if(data.whole_source && canonicalBuildJson(scope.unit_lids)!==canonicalBuildJson(units.map(u=>u.lid)))throw new Error('未限定课程范围时应覆盖全部章节并保留原顺序');
    }else if(task.task_kind==='plan'){
      const plan=PlanZ.parse(candidate);plan.lessons.forEach(b=>bindings(task,b.source_lids));
      if(new Set(plan.lessons.map(b=>b.id)).size!==plan.lessons.length)throw new Error('课标识重复');payload=plan;
    }else if(task.task_kind==='lesson'){const body=CourseBodyZ.parse(candidate);validateBody(task,body);payload=body;}
    else if(task.task_kind==='solve'){
      const solve=SolveZ.parse(candidate),questions=JSON.parse(task.rendered_input).questions as {question:string}[];
      if(solve.solutions.length!==questions.length || questions.some(q=>solve.solutions.filter(s=>s.question===q.question).length!==1))throw new Error('独立解题必须覆盖输入的每一道题');payload=solve;
    }else{
      const review=ReviewZ.parse(candidate);bindings(task,review.checked_lids);
      if(task.source.passages.some(p=>!review.checked_lids.includes(p.lid)) || review.accepted!==(review.issues.length===0))throw new Error('课程核验需要覆盖所选原文，接纳结果必须与具体问题一致');payload=review;
    }
  }catch(error){
    if(error instanceof z.ZodError)throwExtractorSchemaError(error,candidate);
    const failure=new Error(error instanceof Error?error.message:String(error));
    Object.assign(failure,{failure_diagnostic:createAutomaticBuildFailureDiagnosticV3({category:'schema',code:'semantic_output_invalid',phase:'artifact_writer',expected:failure.message.slice(0,120)})});throw failure;
  }
  return writeAutomaticBuildGenerationArtifact(target,buildSemanticArtifactEnvelopeV3({target:target.target_ref,stage:'course',work_unit_id:task.descriptor.work_unit_id,
    input_hash:task.descriptor.input_hash,policy_generation_id:generation,semantic_contract:semanticContractFromExtractionPolicy(task.descriptor.policy_fingerprint),provenance,payload}));
}
export function scopeFromAcceptedRoute(input: TeachingBuildInput, route: AcceptedCourseRoute): z.infer<typeof ScopeZ> {
  if (route.source_id !== input.source.source_id || route.source_revision !== input.source.source_revision) {
    throw new Error('已选课程路线与当前原文版本不一致，请重新核对路线');
  }
  const available = new Set(input.structure.spine.map(unit => unit.lid));
  for (const lid of route.unit_lids) {
    if (!available.has(lid)) throw new Error(`已选课程路线中的章节 ${lid} 不在书籍结构中`);
  }
  return { title: route.title, question: route.question, route: 'theme', unit_lids: route.unit_lids };
}

// Read only the shared lesson candidate. The run, learner feedback and proposal history stay private.
const ReusableTrialLessonZ = CourseBodyZ.extend({
  summary: BlueprintZ.pick({ question: true, objective: true, prerequisites: true, estimated_minutes: true, duration_basis: true })
    .extend({ source: z.object({ publication: z.object({book_id: text}), source_revision: text, start_lid: text, end_lid: text }) }).strip(),
});
const ReusableTrialArtifactZ = z.object({
  version: z.literal('course_artifact.v1'), source_id: text, source_revision: text,
  course: z.object({ lessons: z.array(ReusableTrialLessonZ).length(1) }),
});
function readTrialForRoute(input: TeachingBuildInput, route?: AcceptedCourseRoute) {
  if (!route?.trial_revision) return;
  const file = path.join(input.target.workspace_dir, '.build', 'course-trials', route.trial_revision, 'inquiry_course.json');
  const parsed = ReusableTrialArtifactZ.safeParse(read(file));
  if (!parsed.success) return;
  const artifact = parsed.data;
  const {summary, ...body} = artifact.course.lessons[0]!;
  const binding = summary.source;
  if (artifact.source_id !== route.source_id || artifact.source_revision !== route.source_revision ||
      binding.publication.book_id !== route.source_id || binding.source_revision !== route.source_revision) return;
  const start = input.source.passages.findIndex(p => p.lid === binding.start_lid);
  const end = input.source.passages.findIndex(p => p.lid === binding.end_lid);
  if (start < 0 || end < start) return;
  const passages = input.source.passages.slice(start, end + 1);
  try { validateBody({source: {...input.source, passages}}, body); }
  catch { return; } // The accepted route can still commission a fresh lesson for this section.
  const source_lids = passages.map(p => p.lid);
  const {source: _, ...description} = summary;
  return { description: {...description, source_lids}, body };
}

interface AcceptedLesson { id:string; unit_id:string; blueprint:Blueprint; body:Body; review:Review; task_ids:string[]; trial_revision?:string }
interface CourseBuildResult { tasks:CourseGenerationTask[]; lessons:AcceptedLesson[]; units:{id:string;title:string}[]; language:string; scope?:z.infer<typeof ScopeZ>; blocked?:string }
function build(input:TeachingBuildInput, goal?:string, route?:AcceptedCourseRoute):CourseBuildResult {
  const result:CourseBuildResult={tasks:[],lessons:[],units:[],language:'zh-CN'};
  const take = (task: CourseGenerationTask) => {
    if (goal) task.course_goal = goal;
    if (route) task.course_route = route;
    result.tasks.push(task);
    return accepted(input,task);
  };
  const scope = route ? scopeFromAcceptedRoute(input, route) : take(make(input,'scope','scope',{...input.source,passages:[]},
    {structure:input.structure,whole_source:!goal,request:goal??'按章、节组织本资料的课程，覆盖已有结构。'})) as z.infer<typeof ScopeZ>|undefined;
  if(!scope)return result;result.scope=scope;
  const trial = readTrialForRoute(input, route);
  for(const lid of scope.unit_lids){
    const unit=input.structure.spine.find(u=>u.lid===lid)!;
    const passages=input.source.passages.filter(p=>p.unit_lid===unit.lid);
    if(!passages.length){result.blocked=`所选章节 ${unit.lid} 没有可读取的原文`;return result;}
    // Split by complete source paragraphs; the blueprint then selects exactly the readings needed by each lesson.
    const groups:typeof passages[]=[];let size=0;
    for(const passage of passages){const tokens=estimateTokens(passage.text);if(!groups.length || size+tokens>3500){groups.push([]);size=0;}groups.at(-1)!.push(passage);size+=tokens;}
    for(const [part,group] of groups.entries()){
      const unitId=`unit-${unit.lid.replaceAll('.','-')}-${part+1}`;
      const source={...input.source,passages:group};
      const availableTrial = trial?.description.source_lids.every(lid => group.some(p => p.lid === lid)) ? trial : undefined;
      const planTask=make(input,'plan',unitId,source,{course_goal:goal,route:scope,
        ...(route ? {accepted_route:route,route_instruction:'依据已选读者起点、总问题、结课任务和示例问题，确定本单元承担的任务及其所需前提。'} : {}),
        ...(availableTrial ? {trial_candidate:availableTrial.description,trial_instruction:'仅在该课符合已选路线和本单元目标时，沿用所列问题、目标、起点、预计时间和原文范围；否则重新规划。'} : {}),
        structure:{unit,throughlines:input.structure.throughlines.filter(t=>t.lids.includes(unit.lid)),key_stops:input.structure.key_stops.filter(s=>group.some(p=>p.lid===s.lid))}});
      const plan=take(planTask) as Plan|undefined;if(!plan)continue;
      result.language=plan.language;result.units.push({id:unitId,title:plan.unit_title});
      for(const blueprint of plan.lessons){
        const id=`${unitId}-${blueprint.id}`,selected={...source,passages:group.filter(p=>blueprint.source_lids.includes(p.lid))};
        const taskId = route ? `${id}-${planTask.descriptor.input_hash.slice(0,16)}` : id;
        const description = availableTrial?.description;
        const trialBody = description && description.question === blueprint.question &&
          description.objective === blueprint.objective && description.prerequisites === blueprint.prerequisites &&
          canonicalBuildJson(description.source_lids) === canonicalBuildJson(blueprint.source_lids)
          ? availableTrial.body : undefined;
        let feedback:unknown;let done=false;
        for(let revision=0;revision<3;revision++){
          const reuseTrial = revision === 0 && trialBody !== undefined;
          const task_ids: string[] = [];
          let body: Body | undefined;
          if (reuseTrial) {
            body = trialBody;
          } else {
            const lessonTask = make(input,'lesson',`${taskId}-${revision}`,selected,{blueprint,...(feedback?{repair:feedback}:{})},blueprint);
            body = take(lessonTask) as Body | undefined;
            task_ids.push(lessonTask.descriptor.work_unit_id);
          }
          if (!body) { done = true; break; }
          // No author explanations, hints, solution or rubric enter the independent solving task.
          const solveTask=make(input,'solve',`${taskId}-${revision}`,selected,{prerequisites:blueprint.prerequisites,questions:[body.prediction,...body.follow_up_rounds.map(r=>r.prediction),body.task]});
          const solve=take(solveTask) as z.infer<typeof SolveZ>|undefined;if(!solve){done=true;break;}
          const reviewTask=make(input,'review',`${taskId}-${revision}`,selected,{blueprint,lesson:body,independent_solution:solve,
            ...(route ? {accepted_route:route,route_instruction:'核对本课是否适合已选读者起点，说明它对总问题和结课任务提供的实际支持。'} : {})});
          const review=take(reviewTask) as Review|undefined;if(!review){done=true;break;}
          if (review.accepted && solve.issues.length === 0) {
            task_ids.push(solveTask.descriptor.work_unit_id, reviewTask.descriptor.work_unit_id);
            result.lessons.push({id, unit_id:unitId, blueprint, body, review, task_ids, ...(reuseTrial ? {trial_revision:route?.trial_revision} : {})});
            done = true;
            break;
          }
          feedback={previous:body,issues:[...solve.issues,...review.issues]};
        }
        if(!done)result.blocked=`课程 ${blueprint.question} 修订三次仍有具体问题：${JSON.stringify(feedback)}`;
      }
    }
  }
  if(!result.tasks.length)result.blocked='所选结构没有可读取的课程原文';
  return result;
}
export function routeCourseBuild(input:TeachingBuildInput, goal?:string, route?:AcceptedCourseRoute):AutomaticBuildStageState {
  let result:CourseBuildResult;
  try{result=build(input,goal,route);}catch(error){return {stage:'course',closed:false,pending_tasks:[],course_blocked:error instanceof Error?error.message:String(error)};}
  const policy=automaticBuildExtractionPolicy('course',resolveContentProfile(input.target.profile_id),'full');
  const pending=result.tasks.filter(t=>!accepted(input,t));
  const closed=!result.blocked&&!pending.length&&result.lessons.length>0&&Object.entries(courseArtifacts(input,result,route)).every(([file,value])=>{
    try{return canonicalBuildJson(read(path.join(input.target.workspace_dir,file))??null)===canonicalBuildJson(value);}catch{return false;}
  });
  return {stage:'course',closed,pending_tasks:pending.map(t=>t.descriptor.work_unit_id),course_blocked:result.blocked,
    work_units:result.tasks.map(t=>t.descriptor),pending_work_units:pending.map(t=>t.descriptor),
    task_bindings:Object.fromEntries(result.tasks.map(t=>[t.descriptor.work_unit_id,taskPolicyBindingForWorkUnit(t.descriptor,generation)])),
    generation_tasks:Object.fromEntries(result.tasks.map(t=>[t.descriptor.work_unit_id,{kind:'course' as const,task:t}])),
    policy_set:createAutomaticBuildStagePolicySet({target_ref:input.target.target_ref,stage:'course',frozen_at:'2026-11-12T00:00:00.000Z',members:[{kind:'course_task',extractor:'course-builder',policy_generation_id:generation,policy_fingerprint:policy}]})};
}
function courseArtifacts(input:TeachingBuildInput,result:CourseBuildResult,route?:AcceptedCourseRoute) {
  const course={language:result.language,units:result.units,routes:[{id:'course-route',title:result.scope!.title,question:result.scope!.question,structure_ref:result.scope!.unit_lids.join(','),lesson_ids:result.lessons.map(l=>l.id)}],
    lessons:result.lessons.map(l=>({summary:{id:l.id,unit_id:l.unit_id,question:l.blueprint.question,objective:l.blueprint.objective,prerequisites:l.blueprint.prerequisites,
      estimated_minutes:l.blueprint.estimated_minutes,duration_basis:l.blueprint.duration_basis,source:{publication:{book_id:input.source.source_id,publication_id:'build'},source_revision:input.source.source_revision,
        start_lid:input.source.passages.find(p=>l.blueprint.source_lids.includes(p.lid))!.lid,end_lid:input.source.passages.filter(p=>l.blueprint.source_lids.includes(p.lid)).at(-1)!.lid,label:l.blueprint.question,structure_refs:result.scope!.unit_lids}},...l.body}))};
  return {
    'inquiry_course.json':{version:'course_artifact.v1',source_id:input.source.source_id,source_revision:input.source.source_revision,course},
    'course_build_report.json':{
      source_id:input.source.source_id, source_revision:input.source.source_revision,
      ...(route ? {accepted_route:route} : {}),
      task_ids:result.tasks.map(t=>t.descriptor.work_unit_id),
      lessons:result.lessons.map(l=>({id:l.id, review:l.review, task_ids:l.task_ids, ...(l.trial_revision ? {trial_revision:l.trial_revision} : {})})),
    },
  };
}
export function closeCourseBuild(input:TeachingBuildInput, goal?:string, route?:AcceptedCourseRoute) {
  const result=build(input,goal,route);if(result.blocked || result.tasks.some(t=>!accepted(input,t)) || !result.lessons.length)throw new Error(result.blocked??'课程制作或核验尚未完成');
  return buildAutomaticBuildStageBatchResult(publishAutomaticBuildArtifactSet({workspace_dir:input.target.workspace_dir,stage:'course',artifacts:Object.fromEntries(Object.entries(courseArtifacts(input,result,route)).map(([file,value])=>[file,json(value)]))}));
}
