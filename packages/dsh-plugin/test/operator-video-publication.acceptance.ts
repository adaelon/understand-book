import assert from 'node:assert/strict';
import {readFileSync,writeFileSync,renameSync} from 'node:fs';
import path from 'node:path';
import {request as httpRequest} from 'node:http';

const resultFile=path.resolve('target/video-operator-20261018/http-acceptance-result.json');
const result=JSON.parse(readFileSync(resultFile,'utf8'));
const fixture=await(await fetch('http://127.0.0.1:4191/fixture')).json();
const staging=path.resolve(result.root,'publication-staging'),offline=staging+'.offline';
assert.ok(staging.startsWith(path.resolve(result.root)+path.sep));
assert.ok(offline.startsWith(path.resolve(result.root)+path.sep));
const get=(url:string,headers:Record<string,string>={})=>new Promise<{status:number;body:Buffer;headers:any}>((resolve,reject)=>{
  const req=httpRequest('http://127.0.0.1:4190'+url,{headers:{Cookie:fixture.reader_cookie,...headers}},res=>{
    const chunks:Buffer[]=[];res.on('data',chunk=>chunks.push(Buffer.from(chunk)));res.on('end',()=>resolve({status:res.statusCode!,body:Buffer.concat(chunks),headers:res.headers}));
  });req.on('error',reject);req.end();
});
renameSync(staging,offline);
try{
  const base=`/api/books/${result.reference.book_id}/publications/${result.reference.publication_id}`;
  const video=await get(base+'/video_context');assert.equal(video.status,200,video.body.toString());
  const context=JSON.parse(video.body.toString());assert.ok(context.map.segments.length);assert.ok(context.translations.length);
  const media=await get(base+'/video/media',{Range:'bytes=0-31'});assert.equal(media.status,206);assert.equal(media.body.length,32);
  const manifest=await get(base+'/manifest');assert.equal(manifest.status,200,manifest.body.toString());
  result.checks.final_publication_independent_of_staging=true;
  result.final_reader={transcript_segments:context.map.segments.length,translations:context.translations.length,media_bytes:media.body.length,media_status:media.status};
  writeFileSync(resultFile,JSON.stringify(result,null,2));console.log(JSON.stringify(result.final_reader));
}finally{renameSync(offline,staging);}
