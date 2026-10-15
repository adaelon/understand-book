import { createServer } from 'node:http';
import { spawn } from 'node:child_process';
import { readFileSync, writeFileSync, mkdirSync } from 'node:fs';
import { resolve, join } from 'node:path';
import assert from 'node:assert/strict';
import { question, save, pngSize } from './image-experiment.mjs';

const mode=process.argv[2] || 'record';
assert(['record','live'].includes(mode));
if(mode==='live') for(const key of ['OPENCODE_API_KEY','OPENCODE_BASE_URL','FLUID_LLM_MODEL']) assert(process.env[key],`Missing ${key}`);
const workspace=resolve('evals/video/generated/vi2/moved-workspace');
const link=JSON.parse(readFileSync(join(workspace,'video_publication.json'),'utf8'));
const publication=resolve(workspace,link.directory);
const manifest=JSON.parse(readFileSync(join(publication,'video_source_manifest.json'),'utf8'));
const transcript=JSON.parse(readFileSync(join(publication,manifest.transcript_map_path),'utf8'));
const inputs=JSON.parse(readFileSync('evals/video/results/vi0c-20261010/inputs.json','utf8'));
const out=resolve(`evals/video/generated/vi5/${mode}`);mkdirSync(out,{recursive:true});
let currentDir;
const env={...process.env};let server;
if(mode==='record'){
  server=createServer(async(req,res)=>{
    let body='';for await(const chunk of req)body+=chunk;
    writeFileSync(join(currentDir,'http-body.json'),body);
    const answer='RECORDING ONLY: no model inference.';
    const content=env.UNDERSTAND_BOOK_PROVIDER==='react'?JSON.stringify({final:answer}):answer;
    res.writeHead(200,{'Content-Type':'application/json'});
    res.end(JSON.stringify({choices:[{message:{content}}]}));
  });
  await new Promise(r=>server.listen(0,'127.0.0.1',r));
  Object.assign(env,{OPENCODE_API_KEY:'vi5-local-recorder',OPENCODE_BASE_URL:`http://127.0.0.1:${server.address().port}`,FLUID_LLM_MODEL:'vi5-recorder'});
}
const cases={single_a:[555123],single_b:[650000],crop_a:[555123],crop_b:[650000],progressive:[600000,615000,630000,650000],matrix:[1200000,1210000],capacity8:[600000,607000,614000,621000,628000,635000,642000,649000]};
const checks=[];
try{
 for(const provider of mode==='record'?['native','react']:[env.UNDERSTAND_BOOK_PROVIDER||'native']){
  env.UNDERSTAND_BOOK_PROVIDER=provider;
  for(const [name,sampling] of Object.entries(cases)){
    currentDir=join(out,provider,name);mkdirSync(currentDir,{recursive:true});
    const read={location:{video:manifest.source.video,start_ms:sampling[0],end_ms:sampling.at(-1)+1000},sampling};
    if(name.startsWith('crop'))read.region=inputs.frames[name==='crop_a'?'f555123_crop':'f650000_crop'].rect_normalized;
    const input=join(currentDir,'input.json');save(input,{question,read});
    const executable=resolve(process.env.CARGO_TARGET_DIR||'target',`debug/examples/video_frames_probe${process.platform==='win32'?'.exe':''}`);
    const child=spawn(executable,[publication,input,currentDir],{env,windowsHide:true,stdio:['ignore','inherit','inherit']});
    const exit=await new Promise((r,j)=>{child.on('close',r);child.on('error',j);});assert.equal(exit,0,`${provider}/${name}`);
    const body=JSON.parse(readFileSync(join(currentDir,'request.json'),'utf8'));
    if(mode==='record') assert.deepEqual(body,JSON.parse(readFileSync(join(currentDir,'http-body.json'),'utf8')));
    const content=body.messages.at(-1).content;
    const images=content.filter(x=>x.type==='image_url');assert.equal(images.length,sampling.length);
    const observations=JSON.parse(readFileSync(join(currentDir,'observations.json'),'utf8'));
    const metadata=JSON.parse(readFileSync(join(currentDir,'read.json'),'utf8'));
    const run=JSON.parse(readFileSync(join(currentDir,'run.json'),'utf8'));
    assert.equal(observations.length,sampling.length);
    for(let i=0;i<images.length;i++){
      const png=readFileSync(join(currentDir,`frame-${i}.png`));
      assert.equal(images[i].image_url.url,`data:image/png;base64,${png.toString('base64')}`);assert.equal(images[i].image_url.detail,'high');
      assert.deepEqual(pngSize(png),{width:metadata.frames[i].image_width,height:metadata.frames[i].image_height});
      const frame=metadata.frames[i].read.frame;
      assert.equal(frame.width,1920);assert.equal(frame.height,1080);assert(frame.actual_pts_ms>=sampling[i]&&frame.actual_pts_ms-sampling[i]<34);
      assert.deepEqual(observations[i].publication,link.reference);
      assert.equal(observations[i].model_request_id,JSON.parse(content[i*2].text).model_request_id);
      assert.equal(observations[i].frames.length,read.region?0:1);assert.equal(observations[i].regions.length,read.region?1:0);
    }
    assert.deepEqual(metadata.transcript,transcript.segments.filter(s=>s.location.start_ms<read.location.end_ms&&s.location.end_ms>read.location.start_ms));
    checks.push({provider,name,semantic_inference:mode==='live',read:metadata,observations,run,request_bytes:Buffer.byteLength(JSON.stringify(body))});
    console.log(JSON.stringify({provider,name,frames:images.length,read_ms:run.read_ms}));
  }
 }
 const summary={mode,semantic_inference:mode==='live',publication:link.reference,video:manifest.source.video,checks};
 save(join(out,'verification.json'),summary);
 if(mode==='record')save(resolve('evals/video/results/vi5-20261010/verification.json'),summary);
}finally{server?.close();}
