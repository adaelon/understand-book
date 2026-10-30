import {cpSync,existsSync,mkdirSync,readFileSync,readdirSync,writeFileSync} from 'node:fs';
import path from 'node:path';
import assert from 'node:assert/strict';
import {pathToFileURL} from 'node:url';
const mode=process.argv[2];
if(!['baseline','candidate'].includes(mode))throw new Error('Expected baseline or candidate');
const root=path.resolve('tmp/music-incremental-20261028');
const {buildCuratedMusicWorkspace}=await import(pathToFileURL(path.resolve('packages/core/src/music-import.ts')).href);
const results=[];
for(const name of ['schubert','beethoven']){
 const seed=path.join(root,'seed',name);
 if(!existsSync(seed))cpSync(path.resolve('.understand-book/music-calibration-qa',name),seed,{recursive:true});
 const bookDir=path.join(root,mode,name);
 if(existsSync(bookDir))throw new Error('Use a new benchmark output directory');
 cpSync(seed,bookDir,{recursive:true});
 const curationPath=path.resolve('evals/music',`${name}-curation.json`);
 const historyFile=path.join(bookDir,'.build/music/calibrations.json'),history=readFileSync(historyFile);
 const initial=JSON.parse(readFileSync(path.join(seed,'music_source_manifest.json'),'utf8'));
 const media=readFileSync(path.join(bookDir,initial.recording.media_path));
 const runs=[];
 let stable;
 for(let iteration=0;iteration<4;iteration++){
   const start=performance.now();
   const manifest=await buildCuratedMusicWorkspace({curationPath,bookDir});
   const elapsed_ms=performance.now()-start;
   assert.deepEqual(readFileSync(historyFile),history);
   assert.deepEqual(readFileSync(path.join(bookDir,manifest.recording.media_path)),media);
   for(const key of ['occurrences','note_alignments','coverage'])assert.deepEqual(manifest[key],initial[key],`${name}/${key}`);
   if(stable)assert.deepEqual(manifest,stable); else stable=manifest;
   const report=JSON.parse(readFileSync(path.join(bookDir,'music/build-report.json'),'utf8'));
   runs.push({iteration,phase:iteration===0?'first_after_clone':'repeat',elapsed_ms,report});
   console.log(JSON.stringify({mode,work:name,iteration,elapsed_ms}));
 }
 if(mode==='candidate'){
   const baseline=path.join(root,'baseline',name);
   const files=['music_source_manifest.json','base.json','source.txt','music/curation.json','music/score/parsed.json','music/score/render-input.musicxml','music/score/derived.mei','music/score/score-synthesis.mid',...stable.pages.map(p=>p.svg_path),...stable.demonstrations.map(d=>d.media_path)].filter(Boolean);
   const compare=(file)=>{
     const actual=readFileSync(path.join(bookDir,file)),expected=readFileSync(path.join(baseline,file));
     if(file==='music/score/derived.mei'){
       const normalized=bytes=>bytes.toString('utf8').replace(/<application\b[^>]*>[\s\S]*?<\/application>/g,block=>
         /<name\b[^>]*>Verovio<\/name>/.test(block)?block.replace(/\sisodate="[^"]*"/,''):block);
       assert.equal(normalized(actual),normalized(expected),`${name}/${file} (only Verovio generation timestamp ignored)`);
     }else assert.deepEqual(actual,expected,`${name}/${file}`);
   };
   for(const file of files)compare(file);
   for(const file of readdirSync(path.join(bookDir,'music/score')).filter(f=>f.endsWith('.svg')))assert.deepEqual(readFileSync(path.join(bookDir,'music/score',file)),readFileSync(path.join(baseline,'music/score',file)),`${name}/${file}`);
 }
 results.push({work:name,source_curation:curationPath,workspace:bookDir,score_objects:stable.score_objects.length,pages:stable.pages.length,demonstrations:stable.demonstrations.length,history_count:JSON.parse(history).length,calibration_preserved:true,audio_preserved:true,outputs_equal:mode==='candidate',runs});
 mkdirSync(path.join(root,'results'),{recursive:true});
 writeFileSync(path.join(root,'results',`${mode}.json`),JSON.stringify({mode,node:process.version,real_model:false,results},null,2)+'\n');
}
