import { afterEach, expect, it } from 'vitest';
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import path from 'node:path';
import { tmpdir } from 'node:os';
import { execFileSync } from 'node:child_process';
import { TranscriptServiceLedger } from '../src/service-usage';
import { buildVideoTranscript, planVideoChunks } from '../src/video-transcript';
import { importVideo } from '../src/video-import';

const roots:string[]=[];
afterEach(()=>{for(const root of roots.splice(0)) rmSync(root,{recursive:true,force:true});});
function fixture() {
  const root=mkdtempSync(path.join(tmpdir(),'service-usage-'));roots.push(root);
  const requests:Array<{url:string;body:any}>=[];
  const charges=new Map<string,{state:string;receipt?:unknown}>();
  let loseReceipt=false,deny=false;
  const fetcher=(async(url:any,init:any)=>{
    const body=init.body?JSON.parse(init.body):undefined;requests.push({url:String(url),body});
    if(String(url).endsWith('/service-admit')) {
      if(deny) return new Response(JSON.stringify({error_code:'SERVICE_RATE_UNAVAILABLE'}),{status:409});
      charges.set(body.usage_id,{state:'sent'});
      return Response.json({admitted:true,usage_id:body.usage_id});
    }
    if(String(url).endsWith('/service-receipts')) {
      const c=charges.get(body.usage_id)!;c.state=body.status==='completed'?'settled':body.status==='unknown'?'pending':'released';c.receipt=body;
      if(loseReceipt){loseReceipt=false;throw new Error('response lost');}
      return Response.json({charge:c});
    }
    return Response.json({charge:charges.get(String(url).split('/').at(-1)!)});
  }) as typeof fetch;
  const connection={base_url:'http://fixture',cookie:'operator',origin:'http://fixture',csrf_token:'token'};
  return {root,requests,charges,create:(confirmation?:string)=>new TranscriptServiceLedger('task',root,connection,fetcher,confirmation),
    loseReceipt:()=>{loseReceipt=true;},deny:()=>{deny=true;}};
}
it('replays a persisted completion after response loss without running recognition twice',async()=>{
  const f=fixture(),chunk=planVideoChunks(2500)[0],output=path.join(f.root,'raw.json');
  let calls=0;
  const execute=async()=>{calls++;writeFileSync(output,JSON.stringify({info:{duration:2.5},segments:[]}));};
  f.loseReceipt();
  await expect(f.create().transcribe(chunk,output,execute)).rejects.toThrow('response lost');
  expect(calls).toBe(1);
  await f.create().transcribe(chunk,output,execute);
  expect(calls).toBe(1);
  const admissions=f.requests.filter(r=>r.url.endsWith('/service-admit'));
  const receipts=f.requests.filter(r=>r.url.endsWith('/service-receipts'));
  expect(admissions).toHaveLength(1);
  expect(receipts).toHaveLength(2);
  expect(receipts[1].body).toEqual(receipts[0].body);
  expect(receipts[0].body.quantity_ms).toBe(2500);
  expect(receipts[0].body.evidence.decoded_duration_ms).toBe(2500);
});
it('denies recognition before launching when the rate or parent allowance is unavailable',async()=>{
  const f=fixture();f.deny();let called=false;
  await expect(f.create().transcribe(planVideoChunks(2000)[0],path.join(f.root,'raw.json'),async()=>{called=true;})).rejects.toThrow('SERVICE_RATE_UNAVAILABLE');
  expect(called).toBe(false);
});
it('admits fractional ffprobe timestamps as whole milliseconds and settles decoded audio duration',async()=>{
  const f=fixture(),output=path.join(f.root,'raw.json');
  await f.create().transcribe(planVideoChunks(2500.25)[0],output,async()=>{
    writeFileSync(output,JSON.stringify({info:{duration:2.50025},segments:[]}));
  });
  expect(f.requests.find(r=>r.url.endsWith('/service-admit'))!.body.quantity_ms).toBe(2501);
  expect(f.requests.find(r=>r.url.endsWith('/service-receipts'))!.body.quantity_ms).toBe(2500);
});
it('retains interrupted usage and only retries after its old reservation is explicitly released',async()=>{
  const f=fixture(),chunk=planVideoChunks(3000)[0],output=path.join(f.root,'raw.json');let calls=0;
  const fail=async()=>{calls++;throw new Error('Whisper stopped');};
  await expect(f.create().transcribe(chunk,output,fail)).rejects.toThrow('Whisper stopped');
  await expect(f.create().transcribe(chunk,output,fail)).rejects.toThrow('SERVICE_USAGE_UNRESOLVED');
  expect(calls).toBe(1);
  const id=f.requests.find(r=>r.url.endsWith('/service-admit'))!.body.usage_id;
  f.charges.get(id)!.state='released';
  await f.create().transcribe(chunk,output,async()=>{calls++;writeFileSync(output,JSON.stringify({info:{duration:3},segments:[]}));});
  expect(calls).toBe(2);
  const admissions=f.requests.filter(r=>r.url.endsWith('/service-admit'));
  expect(admissions).toHaveLength(2);
  expect(admissions[1].body.logical_usage_id).toBe(admissions[0].body.logical_usage_id);
  expect(admissions[1].body.attempt).toBe(2);
  expect(admissions[1].body.usage_id).not.toBe(id);
});
it('records missing duration as unknown instead of inventing zero cost or token usage',async()=>{
  const f=fixture(),chunk=planVideoChunks(2000)[0],output=path.join(f.root,'raw.json');
  await expect(f.create().transcribe(chunk,output,async()=>{writeFileSync(output,'{"segments":[]}');})).rejects.toThrow('SERVICE_USAGE_DURATION_MISSING');
  const receipt=f.requests.find(r=>r.url.endsWith('/service-receipts'))!.body;
  expect(receipt.status).toBe('unknown');
  expect(receipt.quantity_ms).toBeUndefined();
  expect(readFileSync(path.join(f.root,'.build/service-usage/chunk-000.json'),'utf8')).toContain('unknown');
});
it('requires a new explicit continue confirmation before repeating an interrupted physical attempt',async()=>{
  const f=fixture(),chunk=planVideoChunks(2000)[0],output=path.join(f.root,'raw.json');let calls=0;
  await expect(f.create('start-confirmation').transcribe(chunk,output,async()=>{calls++;throw new Error('paused');})).rejects.toThrow('paused');
  await expect(f.create('start-confirmation').transcribe(chunk,output,async()=>{calls++;})).rejects.toThrow('SERVICE_USAGE_UNRESOLVED');
  expect(calls).toBe(1);
  await f.create('continue-confirmation').transcribe(chunk,output,async()=>{calls++;writeFileSync(output,'{"info":{"duration":2},"segments":[]}');});
  expect(calls).toBe(2);
  expect([...f.charges.values()].map(v=>v.state)).toEqual(['pending','settled']);
  const admissions=f.requests.filter(r=>r.url.endsWith('/service-admit'));
  expect(admissions[1].body.processing_confirmation).toBe('continue-confirmation');
  expect(admissions[1].body.attempt).toBe(2);
});
it('meters the real transcript execution path and resumes publication after losing the receipt response',async()=>{
  const f=fixture(),source=path.join(f.root,'input.mp4');
  execFileSync(process.env.FFMPEG||'ffmpeg',['-v','error','-f','lavfi','-i','color=s=160x90:r=10:d=2',
    '-f','lavfi','-i','anullsrc=r=16000:cl=mono','-t','2','-c:v','libx264','-pix_fmt','yuv420p','-c:a','aac',source],{windowsHide:true});
  await importVideo(source,f.root);
  let calls=0;
  const recognize=async(_audio:string,output:string)=>{
    calls++;writeFileSync(output,JSON.stringify({engine:'faster-whisper',model:'small.en',language:'en',info:{duration:2},
      segments:[{text:' Read.',start:.5,end:1,words:[{word:' Read.',start:.5,end:1,probability:.9}]}]}));
  };
  f.loseReceipt();
  const failed=await buildVideoTranscript(f.root,{recognize,serviceMeter:f.create()});
  expect(failed.stage).toBe('failed');expect(calls).toBe(1);
  const resumed=await buildVideoTranscript(f.root,{recognize,serviceMeter:f.create()});
  expect(resumed.stage).toBe('ready');expect(calls).toBe(1);
  expect(f.requests.filter(r=>r.url.endsWith('/service-admit'))).toHaveLength(1);
  expect(f.requests.filter(r=>r.url.endsWith('/service-receipts'))).toHaveLength(2);
  expect(readFileSync(path.join(f.root,'source.txt'),'utf8')).toContain('Read.');
},30_000);
