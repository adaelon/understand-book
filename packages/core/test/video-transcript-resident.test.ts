import { afterEach, expect, it, vi } from 'vitest';
import { execFileSync } from 'node:child_process';
import { existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { importVideo } from '../src/video-import';
import { buildVideoTranscript, readVideoBuild } from '../src/video-transcript';
import { TranscriptServiceLedger } from '../src/service-usage';

const roots: string[] = [];
afterEach(() => { vi.unstubAllEnvs(); for (const root of roots.splice(0)) rmSync(root, { recursive: true, force: true }); });

async function fixture(control: Record<string, unknown> = {}) {
  const root = mkdtempSync(path.join(tmpdir(), 'resident-whisper-')); roots.push(root);
  const workspace = path.join(root, '视频 workspace'), trace = path.join(root, 'trace.jsonl');
  mkdirSync(path.join(root, 'faster_whisper-0.0.0.dist-info'));
  writeFileSync(path.join(root, 'faster_whisper-0.0.0.dist-info/METADATA'), 'Name: faster-whisper\nVersion: 0.0.0\n');
  writeFileSync(path.join(root, 'control.json'), JSON.stringify(control));
  writeFileSync(path.join(root, 'faster_whisper.py'), `
import json, os, time, wave
from pathlib import Path
from dataclasses import dataclass
@dataclass
class Word:
    word: str = " Speech."
    start: float = 0.5
    end: float = 1.0
    probability: float = 0.9
@dataclass
class Segment:
    text: str
    start: float
    end: float
    words: list
@dataclass
class Info:
    duration: float
class WhisperModel:
    def __init__(self, model_dir, **kwargs):
        assert kwargs == {"device":"cpu", "compute_type":"int8", "cpu_threads":8}
        self.root = Path(model_dir)
        self.record({"event":"load"})
        time.sleep(0.03)
    def record(self, value):
        with (self.root / "trace.jsonl").open("a", encoding="utf-8") as target:
            target.write(json.dumps(dict(value, pid=os.getpid())) + "\\n")
    def transcribe(self, audio, **kwargs):
        assert kwargs == {"language":"en", "task":"transcribe", "beam_size":5, "temperature":0,
            "word_timestamps":True, "condition_on_previous_text":False, "vad_filter":True,
            "vad_parameters":{"min_silence_duration_ms":500}}
        chunk = Path(audio).stem
        workspace = Path(audio).parents[2]
        previous = json.loads((workspace / "video_build.json").read_text("utf-8"))["chunks"]
        self.record({"event":"transcribe", "chunk":chunk, "states":[c["status"] for c in previous],
            "saved":[p.stem for p in (workspace / "transcript/raw").glob("*.json")]})
        control = json.loads((self.root / "control.json").read_text("utf-8"))
        if chunk == control.get("fail"):
            raise RuntimeError("fixture recognition failure")
        if chunk == control.get("crash"):
            os._exit(7)
        if chunk == control.get("wait"):
            time.sleep(30)
        with wave.open(audio, "rb") as source:
            duration = source.getnframes() / source.getframerate()
        return iter([Segment(" Speech.",0.5,1.0,[Word()])]), Info(duration)
`);
  vi.stubEnv('PYTHONPATH', root);
  vi.stubEnv('PYTHONDONTWRITEBYTECODE', '1');
  const input = path.join(root, 'input.mp4');
  execFileSync(process.env.FFMPEG || 'ffmpeg', ['-v','error','-f','lavfi','-i','color=s=160x90:r=10:d=9',
    '-f','lavfi','-i','anullsrc=r=16000:cl=mono','-t','9','-c:v','libx264','-pix_fmt','yuv420p','-c:a','aac',input], { windowsHide: true });
  await importVideo(input, workspace);
  const requests: Array<{url: string; body: any}> = [], charges = new Map<string, {state: string}>();
  let denied = false, loseReceipt = false;
  const fetcher = (async (url: any, init: any) => {
    const body = init.body ? JSON.parse(init.body) : undefined;
    requests.push({url: String(url), body});
    if (String(url).endsWith('/service-admit')) {
      if (denied) return Response.json({error_code:'SERVICE_RATE_UNAVAILABLE'}, {status:409});
      charges.set(body.usage_id, {state:'sent'});
      return Response.json({admitted:true, usage_id:body.usage_id});
    }
    if (String(url).endsWith('/service-receipts')) {
      const charge = charges.get(body.usage_id)!;
      charge.state = body.status === 'completed' ? 'settled' : 'pending';
      if (loseReceipt) { loseReceipt = false; throw new Error('response lost'); }
      return Response.json({charge});
    }
    return Response.json({charge:charges.get(String(url).split('/').at(-1)!)});
  }) as typeof fetch;
  const meter = (confirmation = 'start') => new TranscriptServiceLedger('task', workspace,
    {base_url:'http://fixture',cookie:'operator',origin:'http://fixture',csrf_token:'token'}, fetcher, confirmation);
  return {root, workspace, requests, charges, meter, options:{modelDir:root, chunkMs:3000, contextMs:0},
    events: (): any[] => existsSync(trace) ? readFileSync(trace,'utf8').trim().split('\n').map(line=>JSON.parse(line)) : [],
    control: (value: Record<string, unknown>) => writeFileSync(path.join(root,'control.json'),JSON.stringify(value)),
    deny: () => { denied = true; }, loseReceipt: () => { loseReceipt = true; },
    raw: (id: string) => readFileSync(path.join(workspace,`transcript/raw/${id}.json`)),
  };
}

it('loads once and saves and settles each of three chunks before requesting the next', async () => {
  const f = await fixture();
  vi.stubEnv('PYTHONIOENCODING', 'ascii');
  const build = await buildVideoTranscript(f.workspace, {...f.options, serviceMeter:f.meter()});
  expect(build.stage).toBe('ready');
  expect(f.events().filter(e=>e.event==='load')).toHaveLength(1);
  expect(new Set(f.events().map(e=>e.pid)).size).toBe(1);
  const calls = f.events().filter(e=>e.event==='transcribe');
  expect(calls.map(e=>e.chunk)).toEqual(['chunk-000','chunk-001','chunk-002']);
  expect(calls[1].saved).toEqual(['chunk-000']);
  expect(calls[2].states).toEqual(['completed','completed','running']);
  expect(build.chunks.map(c=>JSON.parse(f.raw(c.id).toString()).load_ms)).toEqual([expect.any(Number),0,0]);
  expect(f.requests.map(r=>r.url.split('/').at(-1))).toEqual(['service-admit','service-receipts','service-admit','service-receipts','service-admit','service-receipts']);
  expect(f.requests.filter(r=>r.body?.status==='completed').map(r=>r.body.quantity_ms)).toEqual([3000,3000,3000]);
  // The existing vi3 recovery evaluation uses the adapter's file-based one-shot entry.
  const request = path.join(f.root, 'request.json'), output = path.join(f.root, 'one-shot.json');
  writeFileSync(request, JSON.stringify({audio:path.join(f.workspace,'.build/video/chunk-000.wav'),model_dir:f.root}));
  execFileSync(process.env.VIDEO_WHISPER_PYTHON || 'python', [path.resolve('packages/core/src/video-whisper.py'),request,output], {windowsHide:true});
  expect(JSON.parse(readFileSync(output,'utf8')).segments).toEqual(JSON.parse(f.raw('chunk-000').toString()).segments);
}, 30_000);

it('continues after one recognition failure and resumes only its missing raw response after usage release', async () => {
  const f = await fixture({fail:'chunk-001'});
  const first = await buildVideoTranscript(f.workspace, {...f.options, serviceMeter:f.meter()});
  expect(first.chunks.map(c=>c.status)).toEqual(['completed','failed','completed']);
  const saved = f.raw('chunk-000');
  const failedReceipt = f.requests.find(r=>r.body?.status==='unknown')!;
  expect(failedReceipt.body.evidence.chunk_id).toBe('chunk-001');
  await buildVideoTranscript(f.workspace, {...f.options, serviceMeter:f.meter()});
  expect(f.events().filter(e=>e.event==='load')).toHaveLength(1);
  f.charges.get(failedReceipt.body.usage_id)!.state = 'released'; f.control({});
  const second = await buildVideoTranscript(f.workspace, {...f.options, serviceMeter:f.meter()});
  expect(second.stage).toBe('ready');
  expect(f.events().filter(e=>e.event==='load')).toHaveLength(2);
  expect(f.events().filter(e=>e.event==='transcribe').map(e=>e.chunk)).toEqual(['chunk-000','chunk-001','chunk-002','chunk-001']);
  expect(f.raw('chunk-000')).toEqual(saved);
}, 30_000);

it('does not start Python or load a model when every chunk is denied admission', async () => {
  const f = await fixture(); f.deny();
  const build = await buildVideoTranscript(f.workspace, {...f.options, serviceMeter:f.meter()});
  expect(build.stage).toBe('failed'); expect(f.events()).toEqual([]);
  expect(build.chunks.every(c=>c.failure?.includes('SERVICE_RATE_UNAVAILABLE'))).toBe(true);
}, 30_000);

it('cancels the resident process during a chunk, retains completed work and unknown usage, and resumes missing chunks', async () => {
  const f = await fixture({wait:'chunk-001'}), controller = new AbortController();
  const promise = buildVideoTranscript(f.workspace, {...f.options, serviceMeter:f.meter(), signal:controller.signal});
  // Attach the rejection assertion before aborting so no rejection goes unobserved.
  const canceled = expect(promise).rejects.toThrow();
  await vi.waitFor(() => expect(f.events().some(e=>e.chunk==='chunk-001')).toBe(true), {timeout:15_000,interval:20});
  controller.abort(); await canceled;
  expect(readVideoBuild(f.workspace)!.chunks.map(c=>c.status)).toEqual(['completed','failed','pending']);
  expect(f.requests.filter(r=>r.body?.status==='unknown')).toHaveLength(1);
  expect(f.events().filter(e=>e.event==='transcribe').map(e=>e.chunk)).toEqual(['chunk-000','chunk-001']);
  const saved = f.raw('chunk-000'); f.control({});
  const resumed = await buildVideoTranscript(f.workspace, {...f.options, serviceMeter:f.meter('continue')});
  expect(resumed.stage).toBe('ready'); expect(f.raw('chunk-000')).toEqual(saved);
  expect(f.events().filter(e=>e.event==='load')).toHaveLength(2);
  expect(f.events().filter(e=>e.event==='transcribe').map(e=>e.chunk)).toEqual(['chunk-000','chunk-001','chunk-001','chunk-002']);
}, 30_000);

it('replays a lost receipt from its saved raw response without another model load', async () => {
  const f = await fixture(); f.loseReceipt();
  const first = await buildVideoTranscript(f.workspace, {...f.options, serviceMeter:f.meter()});
  expect(first.chunks.map(c=>c.status)).toEqual(['failed','completed','completed']);
  const second = await buildVideoTranscript(f.workspace, {...f.options, serviceMeter:f.meter()});
  expect(second.stage).toBe('ready');
  expect(f.events().filter(e=>e.event==='load')).toHaveLength(1);
  const receipts = f.requests.filter(r=>r.url.endsWith('/service-receipts'));
  expect(receipts.at(-1)!.body).toEqual(receipts[0].body);
}, 30_000);

it('reports an exited worker without hanging or rerunning completed chunks', async () => {
  const f = await fixture({crash:'chunk-001'});
  const first = await buildVideoTranscript(f.workspace, {...f.options, serviceMeter:f.meter()});
  expect(first.chunks.map(c=>c.status)).toEqual(['completed','failed','pending']);
  expect(first.chunks[1].failure).toContain('Whisper exited (7)');
  expect(f.events().filter(e=>e.event==='load')).toHaveLength(1);
  expect(f.requests.filter(r=>r.url.endsWith('/service-admit'))).toHaveLength(2);
  f.control({});
  const second = await buildVideoTranscript(f.workspace, {...f.options, serviceMeter:f.meter('continue')});
  expect(second.stage).toBe('ready');
  expect(f.events().filter(e=>e.event==='transcribe').map(e=>e.chunk)).toEqual(['chunk-000','chunk-001','chunk-001','chunk-002']);
}, 30_000);
