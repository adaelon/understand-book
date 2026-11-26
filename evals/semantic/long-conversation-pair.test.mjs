import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { createBudget, inputUsage, pairedOrder } from './long-conversation-pair.mjs';
import { actionEvidence, report, samplingStatus } from './long-conversation-report.mjs';

const request = {model:'deepseek-flash', max_tokens:8192, messages:[{role:'user', content:'整理'}]};
const admit = (b,req,row,start=0) => b.admit(req,row,start,Infinity,req.model);

test('paired batch reserves in-flight cost across samples and settles each request once', () => {
  const b=createBudget(0.12,2);
  const first={request};
  assert.equal(admit(b,request,first),null);
  assert.equal(admit(b,request,{request},1),'BATCH_COST_LIMIT');
  assert.equal(b.consumed(),first.reservation);
  first.usage={prompt_tokens:200,completion_tokens:10,prompt_cache_hit_tokens:100};
  assert.equal(b.consumed(),284);
  assert.equal(admit(b,request,{request}),null);
  assert.equal(admit(b,request,{request}),'TASK_SEND_LIMIT');
});

test('long band uses only actual single-request usage; zero and absent cache are distinct', () => {
  assert.equal(inputUsage({prompt_tokens:126100, total_tokens:150000}),126100);
  assert.equal(inputUsage({total_tokens:150000}),null);
  assert.equal(inputUsage({input_tokens:10,cache_read_input_tokens:100001,cache_creation_input_tokens:0}),100011);
  assert.equal(inputUsage({input_tokens:10}),null);
});

test('the first four and remaining four each cover both lengths and both models', () => {
  const order=pairedOrder();
  for(const group of [order.slice(0,4),order.slice(4)]) {
    assert.equal(new Set(group.map(({model,band})=>model+band)).size,4);
  }
});

test('reject unsupported pictures on Pro and missing output caps before dispatch', () => {
  const b=createBudget(10,5);
  const pro={...request,model:'deepseek-v4-pro',
    messages:[{role:'user',content:[{type:'image_url',image_url:{url:'data:image/png;base64,AA=='}}]}]};
  assert.equal(admit(b,pro,{}),'PROVIDER_IMAGE_UNSUPPORTED');
  assert.equal(admit(b,{...request,max_tokens:null},{}),'UNBOUND_OUTPUT_LIMIT');
  assert.equal(b.ledger.length,0);
});

test('a separate higher-output experiment reserves every additional possible output token', () => {
  const baseline=createBudget(10,1), expanded=createBudget(10,1,16384);
  const first={request}, second={request:{...request,max_tokens:16384}};
  assert.equal(admit(baseline,second.request,second),'UNBOUND_OUTPUT_LIMIT');
  assert.equal(admit(baseline,request,first),null);
  assert.equal(admit(expanded,second.request,second),null);
  const extraInputBytes=Buffer.byteLength(JSON.stringify(second.request))-Buffer.byteLength(JSON.stringify(request));
  assert.equal(expanded.consumed()-baseline.consumed(),8192*8+extraInputBytes*2);
});

test('only a new call and its later matching receipt count in a resumed long history', () => {
  const call=(id)=>({id,function:{name:'presentation_author',arguments:'{"operation":"deliver"}'}});
  const result=(id)=>({role:'tool',tool_call_id:id,content:JSON.stringify({
    status:'error',model_body:{error_code:'PRESENTATION_INSPECTION_REQUIRED'}
  })});
  const accepted={status:200,reservation:1,usage:{prompt_tokens:100}};
  const rows=[
    {...accepted,ordinal:2,request:{messages:[result('old')]},response:{choices:[{message:{tool_calls:[call('new')]}}]}},
    {ordinal:3,request:{messages:[result('new')]},status:402,blocked:'BATCH_COST_LIMIT'},
    {ordinal:4,request:{messages:[result('new')]},reservation:1,error:'provider transport or JSON error'},
    {...accepted,ordinal:5,request:{messages:[result('old'),result('new')]}},
    {...accepted,ordinal:6,request:{messages:[result('new')]}}
  ];
  const actions=actionEvidence(rows);
  assert.equal(actions.length,1);
  assert.equal(actions[0].observed_in,5);
  assert.equal(actions[0].delivery_unknown_in,4);
  assert.equal(actions[0].diagnostic,'PRESENTATION_INSPECTION_REQUIRED');
  assert.equal(actionEvidence(rows.slice(0,3))[0].observed_in,null);
  assert.deepEqual(rows.map(samplingStatus),['sampled','not_sent','delivery_unknown','sampled','sampled']);
  assert.equal(samplingStatus({status:400,reservation:1}),'upstream_rejected');
  assert.equal(samplingStatus({status:200,scripted:true}),'not_sent');
});

test('a locally started Reader with no provider-confirmed main generation cannot form a paired sample', t => {
  const root=fs.mkdtempSync(path.join(os.tmpdir(),'paired-report-'));
  t.after(()=>fs.rmSync(root,{recursive:true,force:true}));
  const write=(dir,name,data)=>fs.writeFileSync(path.join(dir,name),JSON.stringify(data));
  const rows=[
    {ordinal:1,blocked:'BATCH_COST_LIMIT',status:402},
    {ordinal:1,reservation:100,error:'provider transport or JSON error'}
  ];
  const samples=rows.map((record,index)=>{
    const id=String(index),dir=path.join(root,id);
    fs.mkdirSync(dir);
    write(dir,'sample.json',{first_ordinal:1,repeat:1,band:'long'});
    write(dir,'provider.json',[record]);
    return {id};
  });
  write(root,'run.json',{samples});
  const summary=report(root);
  assert.equal(summary.confirmed_first_main_samples,0);
  assert.equal(summary.aborted_or_unstarted,8);
  assert.equal(summary.comparisons.length,0);
  assert.equal(summary.samples[1].unknown_usage_reserved_micro_cny,100);
});
