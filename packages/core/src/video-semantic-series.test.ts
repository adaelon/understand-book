import {describe,it,expect} from 'vitest';
import {sequenceVideoSemanticSessions} from '../../../skills/build/video-semantic-series';
import type {AutomaticBuildExecutorSessionResponse as Response} from './automatic-build-executor-session';
const response=(action:Response['action']):Response=>({version:'automatic_build_executor_session.v3',action});
const open={version:'automatic_build_executor_open_request.v3',opaque_handoff_ref:'handoff'};
const wait=response({kind:'WAIT',retry_after_ms:1}),done=response({kind:'DONE',status:'committed'});

describe('bounded video semantic series',()=>{
  it('freezes the next input only after the preceding candidate has been admitted',()=>{
    let admitted=false,secondPrepared=false;
    const first=sequenceVideoSemanticSessions('handoff',[
      ()=>request=>{if((request as any).version===open.version)return wait;admitted=true;return done;},
      ()=>{expect(admitted).toBe(true);secondPrepared=true;return request=>(request as any).finish?done:wait;},
    ]);
    expect(()=>first({...open,opaque_handoff_ref:'wrong'})).toThrow('handoff mismatch');
    expect(first(open)).toEqual(wait);expect(secondPrepared).toBe(false);
    expect(first({candidate:'first'})).toEqual(wait);expect(secondPrepared).toBe(true);
    expect(first({finish:true})).toEqual(done);
  });
  it('skips already committed work and does not continue past a retryable failure',()=>{
    let laterPrepared=false;
    const failed=response({kind:'DONE',status:'retryable_failure'});
    const session=sequenceVideoSemanticSessions('handoff',[
      ()=>()=>done,()=>()=>failed,()=>{laterPrepared=true;return ()=>done;},
    ]);
    expect(session(open)).toEqual(failed);expect(laterPrepared).toBe(false);
  });
  it('keeps rejected candidates on the same task and requires an initial open',()=>{
    let laterPrepared=false;
    const session=sequenceVideoSemanticSessions('handoff',[
      ()=>request=>{if((request as any).version===open.version)return wait;throw new Error('Unsupported evidence');},
      ()=>{laterPrepared=true;return ()=>done;},
    ]);
    expect(()=>session({candidate:'early'})).toThrow('Open');
    expect(session(open)).toEqual(wait);
    expect(()=>session({candidate:'rejected'})).toThrow('Unsupported evidence');
    expect(laterPrepared).toBe(false);
  });
});
