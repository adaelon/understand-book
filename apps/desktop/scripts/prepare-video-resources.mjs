import { copyFileSync, existsSync, mkdirSync, writeFileSync } from 'node:fs';
import { execFileSync } from 'node:child_process';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const repo=path.resolve(path.dirname(fileURLToPath(import.meta.url)),'../../..');
const target=path.resolve(process.argv[2]||path.join(repo,'apps/desktop/src-tauri/resources/video'));
const ffmpeg=process.env.FFMPEG||execFileSync('where.exe',['ffmpeg'],{encoding:'utf8',windowsHide:true}).trim().split(/\r?\n/)[0];
const ffprobe=process.env.FFPROBE||path.join(path.dirname(ffmpeg),'ffprobe.exe');
const license=process.env.UNDERSTAND_BOOK_FFMPEG_LICENSE||path.resolve(path.dirname(ffmpeg),'../LICENSE.txt');
for(const file of [ffmpeg,ffprobe,license])if(!existsSync(file))throw new Error(`Video release dependency missing: ${file}. Set FFMPEG, FFPROBE and UNDERSTAND_BOOK_FFMPEG_LICENSE.`);
mkdirSync(path.join(target,'bin'),{recursive:true});
copyFileSync(ffmpeg,path.join(target,'bin/ffmpeg.exe'));copyFileSync(ffprobe,path.join(target,'bin/ffprobe.exe'));
copyFileSync(license,path.join(target,'FFmpeg-LICENSE.txt'));
copyFileSync(path.join(repo,'evals/video/requirements-whisper.txt'),path.join(target,'requirements-whisper.txt'));
copyFileSync(path.join(repo,'docs/视频安装与整讲验收-VI18.md'),path.join(target,'README.md'));
writeFileSync(path.join(target,'versions.json'),JSON.stringify({ffmpeg:execFileSync(ffmpeg,['-version'],{encoding:'utf8',windowsHide:true}).split(/\r?\n/)[0],
  ffprobe:execFileSync(ffprobe,['-version'],{encoding:'utf8',windowsHide:true}).split(/\r?\n/)[0],whisper_model:'small.en',model_revision:'d1d751a5f8271d482d14ca55d9e2deeebbae577f'},null,2)+'\n');
console.log(target);
