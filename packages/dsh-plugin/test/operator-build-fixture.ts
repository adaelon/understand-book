import assert from 'node:assert/strict';
import {harness,textResponse,toolResponse} from './harness.ts';
import {MetadataModel,objects,candidateFor} from './control-fixture.ts';

/** Controlled provider, real DSH agents and real Build Engine candidate/commit protocol. */
export async function operatorHarness(options:{onFirstCommit:()=>Promise<void>;assertAdmitted?:()=>void;completeUsage?:boolean}) {
  const inputs=new Map<string,string>();let pass1=0,first=true;
  const h=await harness(new MetadataModel(async function* (request){
    options.assertAdmitted?.();
    const r=objects(request.messages).filter(v=>v.version==='automatic_build_executor_session.v4').at(-1);
    let name:string,args:any;
    if(!r){name='ub_executor_open';args={version:'automatic_build_executor_open_request.v3',opaque_handoff_ref:JSON.stringify(request.messages).match(/abhandoff1_[a-f0-9]{64}/u)![0]};}
    else if(r.action.kind==='DELIVER_INPUT'){name='ub_executor_input_next';args=r.action.next_request;}
    else if(r.action.kind==='INPUT_BATCH'){
      const b=r.action.batch;for(const chunk of b.chunks)if(chunk.segment==='semantic_input')inputs.set(request.sessionId!,(inputs.get(request.sessionId!)??'')+chunk.payload_utf8);
      name=b.final_for_generation?'ub_executor_generation_start':'ub_executor_input_next';
      args=b.final_for_generation?{version:'automatic_build_executor_generation_start_request.v3',opaque_session_ref:b.opaque_session_ref,generation_input_ref:b.generation_input_ref,confirmed_through_ordinal:b.last_ordinal}:
        {version:'automatic_build_executor_input_next_request.v4',opaque_session_ref:b.opaque_session_ref,generation_input_ref:b.generation_input_ref,ack_through_ordinal:b.last_ordinal};
    }else if(r.action.kind==='GENERATE'){
      if(r.action.output_contract.stage==='pass1')pass1++;
      name='ub_executor_submit_candidate';args={version:'automatic_build_executor_candidate_submit.v3',opaque_session_ref:r.action.opaque_session_ref,candidate_sink_ref:r.action.candidate_sink_ref,candidate:candidateFor(r.action.output_contract,inputs.get(request.sessionId!)??'')};
    }else{assert.equal(r.action.kind,'DONE');if(first){first=false;await options.onFirstCommit();await new Promise(resolve=>setTimeout(resolve,1200));}if(options.completeUsage)yield {type:'usage',usage:{inputTokens:10,outputTokens:2,cacheReadTokens:4,cacheWriteTokens:0,totalTokens:16}};yield* textResponse('complete');return;}
    yield {type:'usage',usage:{inputTokens:10,outputTokens:2,...(options.completeUsage?{cacheReadTokens:4,cacheWriteTokens:0,totalTokens:16}:{})}};yield* toolResponse(`call-${h.model.requests.length}`,name,args);
  }));
  return {...h,pass1Generations:()=>pass1};
}
