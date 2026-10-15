import { afterEach, expect, it, vi } from 'vitest';
import { mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { execFileSync } from 'node:child_process';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { inspectVideoDependencies } from '../src/video-dependencies';
import { videoExecutorArgs } from '../../../skills/build/video-executor-runtime';

const roots: string[]=[];
afterEach(()=>{vi.unstubAllEnvs();for(const root of roots.splice(0))rmSync(root,{recursive:true,force:true});});
it('reports actual absent tools/model and the command that resumes their work',async()=>{
  const root=mkdtempSync(path.join(tmpdir(),'video-missing-'));roots.push(root);
  vi.stubEnv('FFMPEG',path.join(root,'missing-ffmpeg'));
  vi.stubEnv('FFPROBE',path.join(root,'missing-ffprobe'));
  const result=await inspectVideoDependencies({python:path.join(root,'missing-python'),modelDir:root,codex:path.join(root,'missing-codex')});
  expect(result.ready).toBe(false);
  expect(result.checks.map(c=>[c.name,c.ready])).toEqual([['ffmpeg',false],['ffprobe',false],['whisper',false],['small.en',false],['codex',false]]);
  expect(result.checks.find(c=>c.name==='ffprobe')!.recovery).toContain('video-resume');
  expect(result.checks.find(c=>c.name==='small.en')!.detail).toContain('model.bin');
  expect(result.checks.find(c=>c.name==='whisper')!.recovery).toContain('video-transcribe');
});
it('launches source executor loaders from a relocated workspace and re-enters compiled commands',()=>{
  const root=mkdtempSync(path.join(tmpdir(),'video-launch-'));roots.push(root);
  const file=path.join(root,'probe.ts');writeFileSync(file,'console.log(JSON.stringify({value: 18} as {value:number}))');
  vi.stubEnv('UNDERSTAND_BOOK_SIDECAR_SELF','');
  const args=videoExecutorArgs('video-graph',file,[]);
  expect(JSON.parse(execFileSync(process.execPath,args,{cwd:root,encoding:'utf8',windowsHide:true}))).toEqual({value:18});
  vi.stubEnv('UNDERSTAND_BOOK_SIDECAR_SELF','compiled.exe');
  expect(videoExecutorArgs('video-graph','B:/~BUN/root/build.exe',['serve','workspace','batch','handoff'])).toEqual(['video-graph','serve','workspace','batch','handoff']);
  expect(videoExecutorArgs('video-discourse','C:\\app\\build.exe',['serve'])).toEqual(['video-discourse','serve']);
});
