import fs from 'node:fs';
import path from 'node:path';
const root=path.dirname(new URL(import.meta.url).pathname.replace(/^\/([A-Za-z]:)/,'$1'));
const env=fs.readFileSync(path.resolve(root,'../../../.env'),'utf8');
const value=name=>env.match(new RegExp(`^${name}=(.*)$`,'m'))?.[1].trim().replace(/^['"]|['"]$/g,'');
const files=['input.json','common.md','B-api.md','C-api.md','vendor/konva-10.7.0.min.js','vendor/LICENSE'];
fs.writeFileSync(path.join(root,'frozen.json'),JSON.stringify({
  frozen_at:new Date().toISOString(),head:'c2ff3e1',source:'frozen-source/tracked.patch plus copied untracked production modules and experiment adapter',
  model:value('FLUID_LLM_MODEL'),provider_mode:value('UNDERSTAND_BOOK_PROVIDER')||'native',temperature:0,
  output_limit:'existing Runtime: per-request actual value in timing-*-start.json; experimental default 8000',
  order:['B1','C1','B2','C2','B3','C3'],stop:'contract §6; inspect each run before starting the next',
  library:{name:'Konva',version:'10.7.0',license:'MIT',source:'https://unpkg.com/konva@10.7.0/konva.min.js',docs:'https://konvajs.org/docs/overview.html'},
  files:Object.fromEntries(files.map(name=>[name,{bytes:fs.statSync(path.join(root,name)).size}])),
  input_tokens:'Not separately exposed by current adapter; record Provider total tokens, unknown split. No tokenizer estimate is reported as actual usage.',
  assembly:'C marker replaced in returned write arguments before existing Runtime write; persisted candidate and subsequent tool-call history contain the actual library. Full resulting Provider token cost is included.',
  preflight:'author + three previews + Reader mouse/touch drag while held + save/reopen/follow-up passed',
},null,2));
