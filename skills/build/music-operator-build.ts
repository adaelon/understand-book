import { existsSync, mkdirSync, readFileSync, renameSync, writeFileSync } from 'node:fs';
import path from 'node:path';
import type { ExternalBuildCostLedger } from '../../packages/core/src/external-build-cost';
import { buildCuratedMusicWorkspace } from '../../packages/core/src/music-import';
import { musicCapabilities } from '../../packages/core/src/music-contract';

export interface OperatorMusicProgress {
  stage: string; completed: number; total: number; unit: 'tasks'; message: string;
  status: 'running' | 'completed' | 'failed' | 'cancelled'; failure?: string;
}
export interface OperatorMusicBuildInput {
  curation_path: string; workspace_dir: string; required_capabilities?: string[];
  cost_ledger: ExternalBuildCostLedger; signal?: AbortSignal;
  dependencies?: { ffprobe?: string; ffmpeg?: string };
  onProgress?: (progress: OperatorMusicProgress) => void | Promise<void>;
}
const progressFile = (workspace: string) => path.join(workspace, '.build/operator-music-progress.json');
export function readOperatorMusicProgress(workspace: string): OperatorMusicProgress | null {
  const file = progressFile(workspace);
  return existsSync(file) ? JSON.parse(readFileSync(file, 'utf8')) : null;
}
const messages: Record<string, string> = {
  music_import: '导入精选录音与谱本', music_score: '生成可定位的乐谱', music_alignment: '保存对齐结果与校准状态',
  music_events: '接纳音乐事件及片段关系', music_demonstrations: '生成明确标注的谱面合成示范',
  music_publication: '保存音乐成果', completed: '音乐成果已就绪',
};

/** Curated annotations and local rendering share the operator task, but create no model charge. */
export async function runOperatorMusicBuild(input: OperatorMusicBuildInput) {
  const workspace = path.resolve(input.workspace_dir);
  let progress = readOperatorMusicProgress(workspace);
  let pending = Promise.resolve();
  let progressFailure: unknown;
  function report(next: OperatorMusicProgress) {
    progress = next;
    const file = progressFile(workspace);
    mkdirSync(path.dirname(file), { recursive: true });
    writeFileSync(file + '.tmp', JSON.stringify(next, null, 2) + '\n');
    renameSync(file + '.tmp', file);
    pending = pending.then(() => { if (!progressFailure) return input.onProgress?.(next); })
      .catch(error => { progressFailure ??= error; });
  }
  async function flush() { await pending; if (progressFailure) throw progressFailure; }
  try {
    input.signal?.throwIfAborted();
    if (input.cost_ledger.binding.cost_owner !== 'platform') throw new Error('operator_music_requires_curated_public_source');
    await input.cost_ledger.prepare();
    report({ stage:'music_prepare',completed:0,total:1,unit:'tasks',message:'检查精选资料与已完成的构建',status:'running' });
    await flush();
    const manifest = await buildCuratedMusicWorkspace({
      curationPath: input.curation_path, bookDir: workspace, signal: input.signal, ...input.dependencies,
      onProgress: update => {
        input.signal?.throwIfAborted();
        if (progressFailure) throw progressFailure;
        report({ stage:update.phase,completed:update.completed,total:update.total,unit:'tasks',
          message:messages[update.phase] ?? '处理精选音乐资料',status:'running' });
      },
    });
    await flush();
    input.signal?.throwIfAborted();
    const available = musicCapabilities(manifest);
    const missing = (input.required_capabilities ?? ['music','music_score','music_events']).filter(capability => !available.includes(capability));
    if (missing.length) throw new Error(`音乐能力尚未就绪：${missing.join('、')}。已完成成果保留，请补齐精选资料后继续。`);
    report({ stage:'ready',completed:1,total:1,unit:'tasks',message:'音乐预构建完成，可发布共享工作区',status:'completed' });
    await flush();
    return {workspace_dir:workspace,progress:progress!,available_capabilities:available,
      required_capabilities:input.required_capabilities ?? ['music','music_score','music_events'],
      processing:{kind:'curated_local',external_model_calls:0,provider_cost_micro_cny:0,
        description:'本地谱面渲染与有来源的精选标注接纳；未执行音乐模型推断'}};
  } catch (error) {
    report({ ...(progress ?? {stage:'music_prepare',completed:0,total:0,unit:'tasks',message:'准备精选音乐资料'}),
      status:input.signal?.aborted?'cancelled':'failed',failure:error instanceof Error?error.message:String(error) });
    await pending;
    throw error;
  }
}
