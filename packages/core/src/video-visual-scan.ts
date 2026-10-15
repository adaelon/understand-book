import { spawn, execFile } from 'node:child_process';
import { promisify } from 'node:util';
import { mkdirSync, readFileSync, statSync, writeFileSync } from 'node:fs';
import path from 'node:path';
import { videoTool } from './video-dependencies';
import type { VideoSourceAsset } from './generated/VideoSourceAsset';
import type { FrameLocation } from './generated/FrameLocation';

export interface VisualScanOptions {
  interval_ms: number; supplement_ms: number; change_threshold: number;
  /** Normalized teaching area; excludes a known camera/subtitle panel if configured. */
  content_rect: [number, number, number, number];
}
export const DEFAULT_VISUAL_SCAN: VisualScanOptions = {
  interval_ms: 1000, supplement_ms: 15000, change_threshold: 0.06, content_rect: [0, 0, 1, 1],
};
export interface VisualSample { pts_ms: number; change: number }
export interface VisualScan {
  options: VisualScanOptions; samples: VisualSample[]; candidate_indices: number[];
  elapsed_ms: number; decoded_bytes: number;
}
const run = promisify(execFile);
const ffmpeg = () => videoTool('ffmpeg');
export function validateScanOptions(options: VisualScanOptions) {
  const [x, y, w, h] = options.content_rect;
  if (![...options.content_rect, options.interval_ms, options.supplement_ms, options.change_threshold].every(Number.isFinite)
    || x < 0 || y < 0 || w <= 0 || h <= 0 || x + w > 1 || y + h > 1
    || options.interval_ms <= 0 || options.supplement_ms < options.interval_ms
    || options.change_threshold <= 0 || options.change_threshold > 1) throw new Error('Invalid visual scan options');
}
/** Maximum local changed-pixel fraction prevents small code edits being diluted
 * by a static full frame. No top-N ranking lets camera motion evict teaching frames. */
export function visualChange(a: Uint8Array, b: Uint8Array, width: number, height: number, rect: VisualScanOptions['content_rect']): number {
  const x0 = Math.floor(rect[0] * width), y0 = Math.floor(rect[1] * height);
  const x1 = Math.ceil((rect[0] + rect[2]) * width), y1 = Math.ceil((rect[1] + rect[3]) * height);
  let maximum = 0;
  for (let y = y0; y < y1; y += 32) for (let x = x0; x < x1; x += 32) {
    let changed = 0, count = 0;
    for (let yy = y; yy < Math.min(y + 32, y1); yy++) for (let xx = x; xx < Math.min(x + 32, x1); xx++) {
      count++; if (Math.abs(a[yy * width + xx] - b[yy * width + xx]) >= 24) changed++;
    }
    maximum = Math.max(maximum, changed / count);
  }
  return maximum;
}
export function selectVisualCandidates(samples: VisualSample[], options: VisualScanOptions): number[] {
  if (!samples.length) throw new Error('Video scan produced no frames');
  const selected = new Set([0]); let last = samples[0].pts_ms;
  for (let i = 1; i < samples.length; i++) {
    if (samples[i].change >= options.change_threshold) {
      selected.add(i - 1); selected.add(i); last = samples[i].pts_ms;
    } else if (samples[i].pts_ms - last >= options.supplement_ms) {
      selected.add(i); last = samples[i].pts_ms;
    }
  }
  selected.add(samples.length - 1);
  return [...selected].sort((a, b) => a - b);
}
/** Stream the full timeline at low resolution. select retains source PTS, unlike
 * fps, which assigns a synthetic constant frame-rate timeline. */
export async function scanVideoVisualCandidates(file: string, options = DEFAULT_VISUAL_SCAN,
  range?: { start_ms: number; end_ms: number }): Promise<VisualScan> {
  validateScanOptions(options);
  const start = Date.now(), width = 480, height = 270, size = width * height;
  const args = ['-hide_banner', '-loglevel', 'info', '-copyts',
    ...(range ? ['-ss', String(range.start_ms / 1000), '-t', String((range.end_ms - range.start_ms) / 1000)] : []), '-i', file,
    '-map', '0:V:0', '-an', '-vf', `select='isnan(prev_selected_t)+gte(t-prev_selected_t,${options.interval_ms / 1000})',scale=${width}:${height},format=gray,showinfo=checksum=0`,
    '-fps_mode', 'passthrough', '-f', 'rawvideo', 'pipe:1'];
  const child = spawn(ffmpeg(), args, { windowsHide: true, stdio: ['ignore', 'pipe', 'pipe'] });
  const samples: VisualSample[] = [], times: number[] = [], pictures: Buffer[] = [];
  let pending = Buffer.alloc(0), log = '', tail = '', previous: Buffer | undefined, numerator = 0, denominator = 0, bytes = 0;
  const drain = () => {
    while (times.length && pictures.length) {
      const pixels = pictures.shift()!, pts_ms = times.shift()!;
      samples.push({ pts_ms, change: previous ? visualChange(previous, pixels, width, height, options.content_rect) : 0 });
      previous = pixels;
    }
  };
  child.stdout.on('data', (chunk: Buffer) => {
    bytes += chunk.length; pending = Buffer.concat([pending, chunk]);
    while (pending.length >= size) { pictures.push(Buffer.from(pending.subarray(0, size))); pending = pending.subarray(size); }
    drain();
  });
  child.stderr.on('data', (chunk: Buffer) => {
    log += chunk.toString(); tail = (tail + chunk.toString()).slice(-4000);
    const lines = log.split(/\r?\n/); log = lines.pop()!;
    for (const line of lines) {
      const base = /config in time_base: (\d+)\/(\d+)/.exec(line);
      if (base) { numerator = Number(base[1]); denominator = Number(base[2]); }
      const frame = /\bn:\s*\d+\s+pts:\s*(-?\d+)/.exec(line);
      if (frame) times.push(Number(frame[1]) * numerator / denominator * 1000);
    }
    drain();
  });
  await new Promise<void>((resolve, reject) => {
    child.on('error', reject); child.on('close', code => code === 0 ? resolve() : reject(new Error(`Visual scan failed: ${tail}`)));
  });
  if (pending.length || pictures.length || times.length || samples.some(s => !Number.isFinite(s.pts_ms))) throw new Error(`Visual scan frame/PTS mismatch: ${pending.length} bytes, ${pictures.length} pictures, ${times.length} timestamps; ${tail}`);
  return { options, samples, candidate_indices: selectVisualCandidates(samples, options), elapsed_ms: Date.now() - start, decoded_bytes: bytes };
}
export async function extractVisualFrame(file: string, source: VideoSourceAsset, requested_ms: number, output: string) {
  mkdirSync(path.dirname(output), { recursive: true });
  const { stderr } = await run(ffmpeg(), ['-hide_banner', '-loglevel', 'info', '-copyts', '-ss', String(requested_ms / 1000),
    '-i', file, '-map', '0:V:0', '-vf', 'showinfo=checksum=0', '-frames:v', '1', '-fps_mode', 'passthrough', '-y', output], { windowsHide: true });
  const base = /config in time_base: (\d+)\/(\d+)/.exec(stderr);
  const decoded = /n:\s*0\s+pts:\s*(-?\d+)\s+pts_time:.*?\bs:(\d+)x(\d+)/.exec(stderr);
  if (!base || !decoded) throw new Error('Visual frame has no actual PTS');
  const frame: FrameLocation = { video: source.video, actual_pts_ms: Number(decoded[1]) * Number(base[1]) / Number(base[2]) * 1000,
    width: Number(decoded[2]), height: Number(decoded[3]) };
  return { requested_ms, frame, png: readFileSync(output) };
}

/** Decode the selected first-at-or-after frames once. The second output carries
 * exact cropped pixels for the same bounded description cache as single reads. */
export async function extractVisualFrames(file: string, source: VideoSourceAsset, times: number[], directory: string,
  rect: VisualScanOptions['content_rect']) {
  mkdirSync(directory, {recursive:true});
  const ordered=[...new Set(times)].sort((a,b)=>a-b);
  if (!ordered.length) throw new Error('No frame requests');
  const x=Math.round(rect[0]*source.width), y=Math.round(rect[1]*source.height);
  const w=Math.round(rect[2]*source.width), h=Math.round(rect[3]*source.height), size=w*h*3;
  const expression=ordered.map(t=>`gte(t,${t/1000})*(isnan(prev_selected_t)+lt(prev_selected_t,${t/1000}))`).join('+');
  // A filter file avoids the Windows command-line length limit on an entire lecture.
  const filter=path.join(directory,'selection.fffilter');
  writeFileSync(filter,`[0:V:0]select='${expression}',showinfo=checksum=0,split=2[full][crop];[crop]format=rgb24,crop=${w}:${h}:${x}:${y}[pixels]`);
  const child=spawn(ffmpeg(),['-hide_banner','-loglevel','info','-copyts','-ss',String(ordered[0]/1000),'-i',file,
    '-filter_complex_script',filter,'-map','[full]','-frames:v',String(ordered.length),'-fps_mode','passthrough','-threads','1','-y',path.join(directory,'f%05d.png'),
    '-map','[pixels]','-frames:v',String(ordered.length),'-fps_mode','passthrough','-f','rawvideo','pipe:1'],{windowsHide:true,stdio:['ignore','pipe','pipe']});
  let pending=Buffer.alloc(0), log='', tail='', numerator=0, denominator=0;
  const pts:number[]=[], contentIds:string[]=[], contents:Array<{id:string;pixels:Buffer}>=[];
  child.stdout.on('data',(bytes:Buffer)=>{
    pending=Buffer.concat([pending,bytes]);
    while(pending.length>=size){
      const pixels=Buffer.from(pending.subarray(0,size));pending=pending.subarray(size);
      const id=`f${String(contentIds.length+1).padStart(5,'0')}`, previous=contents.find(c=>c.pixels.equals(pixels));
      contentIds.push(previous?.id??id);
      if(!previous){contents.push({id,pixels});if(contents.length>32)contents.shift();}
    }
  });
  child.stderr.on('data',(bytes:Buffer)=>{
    log+=bytes.toString();tail=(tail+bytes.toString()).slice(-4000);
    const lines=log.split(/\r?\n/);log=lines.pop()!;
    for(const line of lines){
      const base=/config in time_base: (\d+)\/(\d+)/.exec(line);if(base){numerator=Number(base[1]);denominator=Number(base[2]);}
      const frame=/\bn:\s*\d+\s+pts:\s*(-?\d+)/.exec(line);if(frame)pts.push(Number(frame[1])*numerator/denominator*1000);
    }
  });
  await new Promise<void>((resolve,reject)=>{child.on('error',reject);child.on('close',code=>code===0?resolve():reject(new Error(`Frame preparation failed: ${tail}`)));});
  if(pending.length||pts.length!==contentIds.length)throw new Error('Frame/pixel stream mismatch');
  let request=0;
  const frames=pts.map((actual_pts_ms,i)=>{
    const requested_ms=ordered[request];
    if(requested_ms===undefined||actual_pts_ms<requested_ms)throw new Error('Unexpected prepared frame PTS');
    while(request<ordered.length&&ordered[request]<=actual_pts_ms)request++;
    const id=`f${String(i+1).padStart(5,'0')}`, png=path.join(directory,id+'.png');
    return {id,requested_ms,frame:{video:source.video,actual_pts_ms,width:source.width,height:source.height},path:png,png_bytes:statSync(png).size,content_id:contentIds[i]};
  });
  if(request!==ordered.length)throw new Error('Video contains no frame at or after the last requested position');
  return frames;
}
/** Compare actual native pixels in the described region; no digest or fuzzy
 * equivalence can erase a one-character code change. Used only to skip a model description. */
export async function visualContentPixels(png: string, source: VideoSourceAsset, rect: VisualScanOptions['content_rect']): Promise<Buffer> {
  const x = Math.round(rect[0] * source.width), y = Math.round(rect[1] * source.height);
  const w = Math.round(rect[2] * source.width), h = Math.round(rect[3] * source.height);
  const { stdout } = await run(ffmpeg(), ['-v', 'error', '-i', png, '-vf', `format=rgb24,crop=${w}:${h}:${x}:${y}`,
    '-frames:v', '1', '-f', 'rawvideo', 'pipe:1'], { windowsHide: true, encoding: 'buffer', maxBuffer: source.width * source.height * 4 });
  return stdout;
}
