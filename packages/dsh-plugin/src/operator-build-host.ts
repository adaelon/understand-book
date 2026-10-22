import type { Context } from '@deepseek-ai/cordis';
import path from 'node:path';
import { existsSync } from 'node:fs';
import type { ExternalBuildCostConnection } from '../../core/src/external-build-cost.ts';
import { musicOperatorCommand, videoOperatorCommand } from './engine-client.ts';
import { createBuildControl, type BuildControlConfig, type OperatorBuildPlan } from './build-control.ts';

export interface OperatorBuildHostConfig {
  connection: ExternalBuildCostConnection;
  rootSessionId: string;
  video?: {
    codexExecutable: string;
    publisherExecutable: string;
    /** Offline publication store for frozen evidence; never the running Reader service root. */
    publicationServiceRoot?: string;
    pythonExecutable?: string;
    whisperModelDir?: string;
    ffmpegExecutable?: string;
    ffprobeExecutable?: string;
  };
}
interface OperatorVideoPlan {
  version: 'operator_video_plan.v1'; target_input: string; workspace_dir: string;
  review_markdown: string; build_tutor: false;
}
interface OperatorMusicPlan {
  version: 'operator_music_plan.v1'; target_input: string; workspace_dir: string;
  review_markdown: string; build_tutor: false;
}
interface Material {
  material_id: string; label: string; target_input?: string; root_dir?: string;
  pass2?: 'enabled' | 'disabled'; max_parallel?: 1 | 2 | 3;
  published_book_ref?: {book_id:string;publication_id:string};
  material_kind?: 'book' | 'video' | 'music'; build_tutor?: boolean; required_capabilities?: string[];
  plan?: OperatorBuildPlan | OperatorVideoPlan | OperatorMusicPlan; invocation_ref?: string; workspace_dir?: string;
}
interface Task {
  user_id?: string; learning_workspace_id?: string;
  task_id: string; action: 'prepare' | 'run'; materials: Material[];
  max_micro_cny: number; reserved_per_execution_micro_cny: number;
  invocation_max_micro_cny: number;
  confirmation: string; confirmed_at: number; pause_requested?: boolean;
  processing_confirmation?: string;
}

/** Books use the DSH root; media builds retain their own processing inside the same task queue. */
export function createOperatorBuildHost(ctx: Context, config: BuildControlConfig, presetId: string,
  host: OperatorBuildHostConfig, fetcher: typeof fetch = fetch) {
  const shutdown = new AbortController();
  let running: Promise<boolean> | undefined;
  let loop: Promise<void> | undefined;
  const c = host.connection;
  async function request(suffix: string, body?: unknown) {
    const response = await fetcher(`${c.base_url.replace(/\/$/u, '')}/api/admin/operator-builds${suffix}`, {
      method: body === undefined ? 'GET' : 'POST', headers: {'Content-Type':'application/json', Cookie:c.cookie,
        Origin:c.origin, 'X-CSRF-Token':c.csrf_token}, ...(body === undefined ? {} : {body:JSON.stringify(body)}),
      signal: AbortSignal.timeout(15000),
    });
    if (!response.ok) {
      const failure=await response.json().catch(()=>undefined);
      throw new Error(failure?.message?`${failure.message} (${failure.error_code??response.status})`:`operator_build_http_${response.status}`);
    }
    return response.json();
  }
  const owner = {host_session_id:host.rootSessionId};
  async function processOne(): Promise<boolean> {
    if (shutdown.signal.aborted) return false;
    const agent = ctx.agents.list().find(a => a.id === host.rootSessionId && !(a.options.subagentDepth ?? 0));
    if (!agent) return false;
    const {task} = await request('/next', owner) as {task:Task|null};
    if (!task) return false;
    const endpoint = `/${encodeURIComponent(task.task_id)}`;
    const cancellation = new AbortController();
    const signal = AbortSignal.any([shutdown.signal, cancellation.signal]);
    let probing = false;
    const timer = setInterval(() => {
      if (probing || signal.aborted) return;
      probing = true;
      void request(endpoint).then(value => { if (value.pause_requested) cancellation.abort(); })
        .catch(() => cancellation.abort()).finally(() => { probing = false; });
    }, 1000);
    const control = createBuildControl(ctx, {...config, externalCost:{connection:c, binding:{
      version:'external_build_cost.v1', cost_owner:task.user_id ? 'account' : 'platform', task_ref:task.task_id,
      ...(task.user_id ? {user_id:task.user_id,learning_workspace_id:task.learning_workspace_id,metering:'provider_calls_v1' as const} : {}),
      max_micro_cny:task.max_micro_cny, reserved_per_execution_micro_cny:task.reserved_per_execution_micro_cny,
      confirmation:task.confirmation || `prepare:${task.task_id}`,
    }}}, presetId);
    try {
      for (const material of task.materials) {
        if (material.published_book_ref) continue;
        if (signal.aborted) throw new Error('build_cancelled');
        const base = `${endpoint}/materials/${encodeURIComponent(material.material_id)}`;
        if (material.material_kind === 'music') {
          if (task.user_id) throw new Error('operator_music_requires_curated_public_source');
          if (material.build_tutor) throw new Error('operator_music_tutor_unsupported');
          const plan = material.plan as OperatorMusicPlan | undefined ?? {
            version:'operator_music_plan.v1' as const, target_input:material.target_input!, build_tutor:false as const,
            workspace_dir:path.resolve(material.root_dir!,task.task_id,`music-${material.material_id}`),
            review_markdown:'音乐预构建：读取精选资料清单 → 导入真实录音与 MusicXML → 生成谱面 → 接纳有依据的对齐、音乐事件和听辨素材 → 检查能力。已有成果继续复用；本地处理不调用音乐模型，不产生供应商调用费用。',
          };
          if (plan.version !== 'operator_music_plan.v1') throw new Error('operator_music_plan_invalid');
          if (!material.plan) await request(`${base}/progress`, {...owner,state:'planned',plan,workspace_dir:plan.workspace_dir});
          if (task.action === 'prepare') continue;
          const invocation = material.invocation_ref ?? `music:${task.task_id}:${material.material_id}`;
          const progress = async (value: unknown) => {
            const updated=await request(`${base}/progress`, {...owner,state:'running',invocation_ref:invocation,
              workspace_dir:plan.workspace_dir,observation:{progress:value}});
            if (updated.pause_requested) cancellation.abort();
          };
          await progress({stage:'music_prepare',completed:0,total:0,unit:'tasks',message:'读取精选音乐资料与已有成果'});
          const result = await musicOperatorCommand(config, {curation_path:plan.target_input,workspace_dir:plan.workspace_dir,
            required_capabilities:material.required_capabilities ?? ['music','music_score','music_events'],
            external_cost:{binding:{version:'external_build_cost.v1',cost_owner:'platform',task_ref:task.task_id,
              max_micro_cny:task.max_micro_cny,reserved_per_execution_micro_cny:task.reserved_per_execution_micro_cny,
              confirmation:task.confirmation},connection:c,registry_root:config.driverRoot},
            dependencies:{ffmpeg:host.video?.ffmpegExecutable,ffprobe:host.video?.ffprobeExecutable}},progress,signal);
          await request(`${base}/progress`, {...owner,state:'complete',invocation_ref:invocation,
            workspace_dir:plan.workspace_dir,observation:result});
          await request(`${base}/import`, {...owner,invocation_ref:invocation,workspace_dir:plan.workspace_dir});
          continue;
        }
        const video = material.material_kind === 'video' || /\.mp4$/iu.test(material.target_input ?? '')
          || !!material.target_input && ['video_import.json','video_source_manifest.json'].some(file=>existsSync(path.join(material.target_input!,file)));
        if (video) {
          if (material.build_tutor) throw new Error('operator_video_tutor_unsupported');
          if (!host.video) throw new Error('operator_video_configuration_required');
          const plan = material.plan as OperatorVideoPlan | undefined ?? {
            version:'operator_video_plan.v1' as const, target_input:path.resolve(material.target_input!), build_tutor:false as const,
            workspace_dir:['video_import.json','video_source_manifest.json'].some(file=>existsSync(path.join(material.target_input!,file)))
              ? path.resolve(material.target_input!) : path.resolve(material.root_dir!,task.task_id,`video-${material.material_id}`),
            review_markdown:task.user_id ? '个人视频：导入 → 按音频时长计量转写 → 原文阅读。转写与模型共用已确认的任务额度，已完成片段继续复用。' : '视频预构建：导入 → 转写 → 翻译 → 画面索引 → 概念图谱 → 原讲解结构。默认不生成 Tutor 教学材料；按已完成工作显示进度。',
          };
          if (plan.version !== 'operator_video_plan.v1') throw new Error('operator_video_plan_invalid');
          if (!material.plan) await request(`${base}/progress`, {...owner,state:'planned',plan,workspace_dir:plan.workspace_dir});
          if (task.action === 'prepare') continue;
          const invocation = material.invocation_ref ?? `video:${task.task_id}:${material.material_id}`;
          const progress = async (value: unknown) => {
            const updated=await request(`${base}/progress`, {...owner,state:'running',invocation_ref:invocation,
              workspace_dir:plan.workspace_dir,observation:{progress:value}});
            if (updated.pause_requested) cancellation.abort();
          };
          await progress({stage:'preparing',completed:0,total:0,unit:'项',message:'正在读取已完成的构建进度'});
          const binding = {
            version:'external_build_cost.v1',cost_owner:task.user_id ? 'account' : 'platform',task_ref:task.task_id,
            ...(task.user_id ? {user_id:task.user_id,learning_workspace_id:task.learning_workspace_id} : {}),
            max_micro_cny:task.max_micro_cny,reserved_per_execution_micro_cny:task.reserved_per_execution_micro_cny,
            confirmation:task.confirmation,
          };
          const result = await videoOperatorCommand(config, {input_path:plan.target_input,workspace_dir:plan.workspace_dir,
            processing_confirmation:task.processing_confirmation,
            personal_source_only:!!task.user_id,codex_executable:host.video.codexExecutable,publisher_executable:host.video.publisherExecutable,
            publication_service_root:host.video.publicationServiceRoot ?? path.join(config.driverRoot,'video-publications',task.task_id),
            external_cost:{binding,connection:c,registry_root:config.driverRoot},
            dependencies:{ffmpeg:host.video.ffmpegExecutable,ffprobe:host.video.ffprobeExecutable,
              python:host.video.pythonExecutable,modelDir:host.video.whisperModelDir}},progress,signal);
          await request(`${base}/progress`, {...owner,state:'complete',invocation_ref:invocation,
            workspace_dir:plan.workspace_dir,observation:result});
          await request(`${base}/import`, {...owner,invocation_ref:invocation,workspace_dir:plan.workspace_dir});
          continue;
        }
        const input = {target_input:material.target_input!,root_dir:material.root_dir!,pass2:material.pass2 ?? 'disabled',
          max_parallel:material.max_parallel ?? 1,build_tutor:material.build_tutor ?? false};
        if (task.action === 'prepare') {
          if (!material.plan) {
            const plan = await control.prepareOperator(agent, signal, input);
            await request(`${base}/progress`, {...owner,state:'planned',plan,workspace_dir:plan.workspace_dir});
          }
          continue;
        }
        if (!material.plan || 'version' in material.plan) throw new Error('operator_build_plan_missing');
        let invocation = material.invocation_ref;
        if (!invocation) {
          const result = await control.createOperatorInvocation(agent, signal, input, material.plan,
            {id:task.confirmation,confirmed_at:new Date(task.confirmed_at * 1000).toISOString(),max_micro_cny:task.invocation_max_micro_cny}) as {invocation_ref:string};
          invocation = result.invocation_ref;
        }
        await request(`${base}/progress`, {...owner,state:'running',invocation_ref:invocation,workspace_dir:material.plan.workspace_dir});
        const result = await control.run(agent, signal, invocation, true, async boundary => {
          await request(`${base}/progress`, {...owner,state:'running',invocation_ref:invocation,workspace_dir:material.plan!.workspace_dir,
            observation:boundary?{waiting_for_decision:true,...boundary}:null});
        }) as {step?:{action:{kind:string}};yielded?:boolean};
        const complete = result.step?.action.kind === 'DONE';
        await request(`${base}/progress`, {...owner,state:complete?'complete':'interrupted',invocation_ref:invocation,
          workspace_dir:material.plan.workspace_dir,observation:result});
        if (!complete) {
          await request(`${endpoint}/finish`, {...owner,state:'interrupted'});
          return true;
        }
        await request(`${base}/import`, {...owner,invocation_ref:invocation,workspace_dir:material.plan.workspace_dir});
      }
      await request(`${endpoint}/finish`, {...owner,state:task.action==='prepare'?'awaiting_confirmation':'ready'});
    } catch (failure) {
      await request(`${endpoint}/finish`, {...owner,state:'interrupted',error:signal.aborted?'build_cancelled':
        failure instanceof Error?failure.message:'operator_build_failed'});
    } finally {
      clearInterval(timer);
      await control.dispose();
    }
    return true;
  }
  function processNext() {
    return running ??= processOne().finally(() => {running=undefined;});
  }
  return {processNext, start() {
    loop ??= (async () => {
      while (!shutdown.signal.aborted) {
        await processNext().catch(() => undefined);
        if (!shutdown.signal.aborted) await new Promise<void>(resolve => {
          const done = () => {clearTimeout(timer);shutdown.signal.removeEventListener('abort',done);resolve();};
          const timer = setTimeout(done,2000);shutdown.signal.addEventListener('abort',done,{once:true});
        });
      }
    })();
  }, async dispose() {shutdown.abort();await Promise.allSettled([running,loop]);}};
}
