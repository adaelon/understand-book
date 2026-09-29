import fs from 'node:fs';
import path from 'node:path';
const library=fs.readFileSync(new URL('vendor/konva-10.7.0.min.js',import.meta.url),'utf8');
for(const directory of process.argv.slice(2)){
  const files=fs.readdirSync(directory).sort();
  const read=name=>JSON.parse(fs.readFileSync(path.join(directory,name),'utf8'));
  const responses=files.filter(n=>/^response-\d+\.json$/.test(n)).map(read);
  const requests=files.filter(n=>/^request-\d+\.json$/.test(n)).map(read);
  const calls=responses.flatMap(r=>r.tool_calls??[]).filter(c=>c.name==='presentation.author').map(c=>JSON.parse(c.arguments));
  const receipts=new Map();
  requests.forEach((request,index)=>{
    for(const message of request.messages??[]){
      if(message.role!=='Tool')continue;
      let result;try{result=JSON.parse(message.content)}catch{continue}
      if(!receipts.has(message.tool_call_id))receipts.set(message.tool_call_id,{request:index,...result});
    }
  });
  const assemblies=files.filter(n=>/^write-\d+-assembly.json$/.test(n)).map(read);
  const historyWrites=requests.flatMap((request,index)=>(request.messages??[])
    .flatMap(message=>message.tool_calls??[]).filter(call=>call.name==='presentation.author')
    .flatMap(call=>{const args=JSON.parse(call.arguments);return args.operation==='write'&&typeof args.html==='string'
      ?[{request:index,id:call.id,bytes:Buffer.byteLength(args.html),contains_fixed_library:args.html.includes(library)}]:[]}));
  const outcome=read('outcome.json');
  const report={summary:read('summary.json'),delivered:files.includes('view.json'),incomplete:outcome.incomplete??null,warning:outcome.warning??null,
    history_writes:historyWrites,
    history_contains_fixed_library:historyWrites.some(write=>write.contains_fixed_library),
    tokens:responses.reduce((s,r)=>s+(r.usage??0),0),unknown_usage:responses.filter(r=>r.usage==null).length+Math.max(0,requests.length-responses.length),
    usage_split:'unknown',operations:Object.fromEntries(['write','preview','deliver'].map(op=>[op,calls.filter(c=>c.operation===op).length])),
    writes:assemblies,scene_bytes:assemblies.map(a=>a.raw_bytes),
    scene_js_bytes:files.filter(n=>/^write-\d+-raw.html$/.test(n)).map(n=>Buffer.byteLength([...fs.readFileSync(path.join(directory,n),'utf8').matchAll(/<script\b[^>]*>([\s\S]*?)<\/script>/gi)].map(m=>m[1]).join('\n'))),
    successful_preview_receipts:[...receipts.values()].filter(r=>['preview_ready_for_inspection','preview_environment_recorded'].includes(r.model_body?.status)),
    readbacks:[...receipts.values()].filter(r=>r.model_body?.reading),
    instruction_revisions:[...new Set(requests.flatMap(r=>(r.instruction_assets??[]).filter(a=>a.asset_id==='resident-agent.skill.presentation-method').map(a=>a.revision)))],
    request_output_limits:files.filter(n=>/^timing-\d+-start.json$/.test(n)).map(n=>read(n).output_token_limit),
  };
  fs.writeFileSync(path.join(directory,'audit.json'),JSON.stringify(report,null,2));
  console.log(JSON.stringify({...report,successful_preview_receipts:report.successful_preview_receipts.length,readbacks:report.readbacks.length},null,2));
}
