import type { BookRef } from './types';
export interface OperatorMaterial {
  material_id:string;label:string;state:string;published_book_ref?:BookRef|null;
  target_input?:string;root_dir?:string;pass2?:'enabled'|'disabled';max_parallel?:number;
  material_kind?:'book'|'video'|'music';build_course?:boolean;
  course_goal?:string|null;
  course?:{lessons:{id:string;question:string;objective:string;estimated_minutes:number}[]};
  invocation_ref?:string|null;workspace_dir?:string|null;
  required_capabilities?:string[];
  plan?:{review_markdown:string;model_runtime?:{provider:string;model:string;max_output_tokens?:number;reasoning_effort?:string|null}}|null;
  observation?:{progress?:{stage:string;completed?:number;total?:number|null;unit?:string;percent?:number;message?:string};waiting_for_decision?:boolean;[key:string]:unknown}|null;
}
export interface OperatorBuild {
  task_id:string;name:string;state:string;revision:number;created_at:number;updated_at:number;
  materials:OperatorMaterial[];visible_to_users:string[];max_micro_cny:number;reserved_per_execution_micro_cny:number;
  pause_requested:boolean;error?:string|null;release?:{release_id:string;version:number}|null;
  budget?:{max_micro_cny:number;provider_cost_micro_cny:number;estimated_cost_micro_cny?:number;pending_micro_cny:number;reserved_micro_cny:number;remaining_micro_cny:number;unknown_count:number}|null;
}
export const buildState=(state:string)=>({draft:'待准备计划',pending:'待准备',planned:'计划已准备',reused:'复用已有成果',complete:'构建完成，等待导入',imported:'成果已导入',queued_prepare:'等待构建宿主准备',preparing:'正在准备计划',awaiting_confirmation:'等待确认计划',queued:'等待构建宿主执行',queued_run:'等待构建宿主执行',running:'正在构建',interrupted:'已中断，可继续',ready:'成果已就绪',published:'已发布共享工作区',paused:'已暂停'}[state]??state);
