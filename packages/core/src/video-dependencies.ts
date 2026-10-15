import { execFile } from 'node:child_process';
import { promisify } from 'node:util';
import { existsSync, statSync } from 'node:fs';
import path from 'node:path';

const run = promisify(execFile);
export function videoTool(name: 'ffmpeg' | 'ffprobe') {
  if (process.env[name.toUpperCase()]) return process.env[name.toUpperCase()]!;
  const sidecar = process.env.UNDERSTAND_BOOK_SIDECAR_SELF;
  if (sidecar) {
    const bundled = path.join(path.dirname(sidecar), 'resources/video/bin', name + (process.platform === 'win32' ? '.exe' : ''));
    if (existsSync(bundled)) return bundled;
  }
  return name;
}
export interface VideoDependency {
  name: string; ready: boolean; detail: string; recovery: string;
}

export async function inspectVideoDependencies(options: { python?: string; modelDir?: string; codex?: string } = {}) {
  const checks: VideoDependency[] = [];
  async function executable(name: string, command: string, args: string[], recovery: string) {
    try {
      const result = await run(command, args, { windowsHide: true, timeout: 30000, maxBuffer: 1024 * 1024 });
      checks.push({ name, ready: true, detail: result.stdout.trim().split(/\r?\n/)[0] || command, recovery });
    } catch (error) {
      checks.push({ name, ready: false, detail: `${command}: ${(error as Error).message}`.slice(0, 1400), recovery });
    }
  }
  await executable('ffmpeg', videoTool('ffmpeg'), ['-version'], '安装 FFmpeg，设置 FFMPEG 为可执行文件路径，再运行 video-doctor；原命令可继续恢复。');
  await executable('ffprobe', videoTool('ffprobe'), ['-version'], '安装 FFprobe，设置 FFPROBE 为可执行文件路径，再运行 video-doctor；用 video-resume 恢复导入。');
  const python = options.python || process.env.VIDEO_WHISPER_PYTHON || 'python';
  await executable('whisper', python, ['-c', 'import faster_whisper; from importlib.metadata import version; print("faster-whisper " + version("faster-whisper"))'],
    '安装 Python 及分发包 requirements-whisper.txt 中的依赖，设置 VIDEO_WHISPER_PYTHON，再运行 video-transcribe 恢复缺块。');
  const modelDir = options.modelDir || process.env.VIDEO_WHISPER_MODEL_DIR;
  const missing = ['model.bin', 'config.json', 'tokenizer.json', 'vocabulary.txt'].filter(name => {
    const file = modelDir && path.join(modelDir, name);
    return !file || !existsSync(file) || !statSync(file).isFile() || statSync(file).size === 0;
  });
  checks.push({ name: 'small.en', ready: !missing.length, detail: modelDir ? (missing.length ? `缺少 ${missing.join(', ')}：${modelDir}` : path.resolve(modelDir)) : 'VIDEO_WHISPER_MODEL_DIR 未配置',
    recovery: '将 faster-whisper small.en 模型下载到本地，设置 VIDEO_WHISPER_MODEL_DIR；重新运行 video-doctor 和原 video-transcribe 命令。' });
  if (options.codex) await executable('codex', options.codex, ['--version'], '提供已登录且支持原生图片的 Codex CLI 可执行文件，再恢复原语义任务。');
  const ready = (names: string[]) => names.every(name => checks.some(c => c.name === name && c.ready));
  return { version: 'video_dependencies.v1', platform: process.platform, checks,
    capabilities: { import: ready(['ffmpeg', 'ffprobe']), transcript: ready(['ffmpeg', 'whisper', 'small.en']),
      frames: ready(['ffmpeg', 'ffprobe']), audio_review: ready(['ffmpeg', 'whisper', 'small.en']),
      semantic_executor: options.codex ? ready(['codex']) : null },
    ready: checks.every(c => c.ready) };
}
