import assert from 'node:assert/strict';
import { test } from 'node:test';
import { mkdtempSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { ExternalBuildCostLedger, readExternalBuildCostBinding } from '../../core/src/external-build-cost.ts';
import { createExecutorHost } from '../src/executor-host.ts';
import { harness, ScriptedModel, probeRuntime, textResponse, toolResponse } from './harness.ts';

for(const mode of ['complete','denied','unknown','receipt-replay'] as const) test(`B2 per-provider-call ${mode}`,async()=>{
  const events:{path:string;body:any}[]=[],charges=new Map<string,any>();let blocked=false,offline=mode==='receipt-replay';
  const fetcher:typeof fetch=async(url,init)=>{
    const route=String(url),body=init?.body?JSON.parse(String(init.body)):undefined;events.push({path:route,body});
    if(route.endsWith('/metered-admit')){
      assert.equal(body.granularity,'calls'); // The server derives the reservation from configured rates.
      assert.ok(body.request.max_tokens>0);
      if(mode==='denied')return new Response(JSON.stringify({error_code:'BUILD_BUDGET_EXHAUSTED'}),{status:409});
      charges.set(body.call_id,{state:'sent'});return Response.json({admitted:true,call_id:body.call_id});
    }
    if(route.includes('/charges/'))return charges.has(route.split('/').at(-1)!)?Response.json({charge:charges.get(route.split('/').at(-1)!)}):new Response('{}',{status:404});
    if(route.endsWith('/metered-receipts')){
      if(offline)return new Response('{}',{status:503});
      charges.set(body.call_id,{state:body.status==='unknown'?'pending':'settled'});
    }
    return Response.json({});
  };
  const binding=readExternalBuildCostBinding({version:'external_build_cost.v1',metering:'provider_calls_v1',cost_owner:'account',user_id:'A',learning_workspace_id:'space',max_micro_cny:10000,reserved_per_execution_micro_cny:100,confirmation:'explicit'}, {profile_id:'dsh_native_v4',profile_revision:1});
  const directory=mkdtempSync(path.join(tmpdir(),'ub-b2-meter-'));
  const ledger=new ExternalBuildCostLedger('personal-task',binding,{base_url:'http://fixture',cookie:'test',csrf_token:'test',origin:'http://fixture'},directory,fetcher);
  await ledger.prepare();let calls=0;
  const h=await harness(new ScriptedModel(async function*(){
    assert.equal(events.filter(e=>e.path.endsWith('/metered-admit')).length,++calls,'every actual provider call follows admission');
    if(mode==='unknown')throw new Error('transport disconnected');
    const output=calls===1?toolResponse('read','fixture_read',{}):textResponse('done');
    yield* output.slice(0,-1);yield {type:'usage',usage:{inputTokens:10,cacheReadTokens:4,cacheWriteTokens:0,outputTokens:3,totalTokens:17}};yield output.at(-1)!;
  }));
  const host=createExecutorHost(h.ctx,'understand-book-executor',async b=>{
    b.modelCost={ledger,executionId:'owned-child',logicalId:'slot',blocked:()=>{blocked=true;}};
    b.agent!.ctx.tools.register({name:'fixture_read',description:'Read one input',parameters:{type:'object',properties:{}},output:{schema:{},render:()=>[{type:'text',text:'input'}]},execute:async()=>({ok:true})});
  });
  try{
    const run=await host.launch({parent:h.parent,signal:AbortSignal.timeout(10000),opaque_handoff_ref:`abhandoff1_${'1'.repeat(64)}`,runtime:probeRuntime()});await run.terminal;await run.dispose();
    if(mode==='denied'){assert.equal(calls,0);assert.equal(blocked,true);}
    else{
      assert.equal(calls,mode==='complete'?2:1);
      const receipts=events.filter(e=>e.path.endsWith('/metered-receipts'));assert.equal(receipts.length,calls);
      if(mode==='unknown')assert.equal(receipts[0].body.status,'unknown');
      else {assert.equal(receipts[0].body.usage.input_tokens,14);assert.equal(receipts[0].body.usage.cached_input_tokens,4);assert.equal(receipts[0].body.usage.total_tokens,17);}
      if(mode==='receipt-replay'){assert.equal(blocked,true);offline=false;await ledger.prepare();assert.deepEqual(events.filter(e=>e.path.endsWith('/metered-receipts')).at(-1)!.body,receipts[0].body);assert.equal(calls,1);}
    }
  }finally{await host.dispose();await h.dispose();}
});
