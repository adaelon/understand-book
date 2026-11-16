// @vitest-environment happy-dom
import { beforeEach, expect, it, vi } from 'vitest';
import { ApiError, type api } from './api';
import { closedBookDraftKey, closedBookMutationKey, executeClosedBookMutation, pendingClosedBookMutation, type PendingClosedBookMutation } from './closed-book-recovery';
import { installIdentity, network, readerKey } from './network-context';
import type { ClosedBookPractice } from './generated/ClosedBookPractice';

const practice: ClosedBookPractice = { id:'practice',owner_id:'source',course:false,chat_session_id:'chat',source_id:'source',source_revision:'source-1',goal_revision:null,revision:4,status:'active',can_resume:true,target:'explain',criteria:'independent',attempt:1,assisted:false,needs_retry:false,visible_turn_ids:[],draft:'',completion_reason:'',learner_quote:'' };
beforeEach(() => { localStorage.clear();vi.restoreAllMocks(); });

it('restores the same draft after reader epoch and generation change, isolating owner, chat and attempt', () => {
  installIdentity({user_id:'learner',csrf_token:'token'});
  const beforeReader=readerKey(), key=closedBookDraftKey('learner','chat',practice);
  localStorage.setItem(key,'我的解释写到一半。');
  installIdentity({user_id:'learner',csrf_token:'new-token'});
  network.value={...network.value,readerGeneration:9};
  expect(readerKey()).not.toBe(beforeReader);
  expect(localStorage.getItem(closedBookDraftKey('learner','chat',{...practice}))).toBe('我的解释写到一半。');
  for(const other of [closedBookDraftKey('other','chat',practice),closedBookDraftKey('learner','other',practice),closedBookDraftKey('learner','chat',{...practice,attempt:2})])expect(localStorage.getItem(other)).toBeNull();
});

it.each(['draft','pause','resume','exit','start'])('replays the original %s operation after a lost response and reload, then reads current state',async action => {
  const key=closedBookMutationKey('learner','chat');
  const operation: PendingClosedBookMutation={request:{action,chat_session_id:'chat',revision:4,operation_id:'original',draft:'保留回答'},message:action==='draft'?'保留回答':undefined};
  let writes=0;
  const send=vi.fn<typeof api.closedBook>().mockImplementation(async request=>{
    if(request.action==='state')return {...practice,revision:6};
    expect(request).toEqual(operation.request);
    if(writes++===0)throw new TypeError('response lost after save');
    return {...practice,revision:5};
  });
  await expect(executeClosedBookMutation(localStorage,key,send,()=>operation)).rejects.toThrow('response lost');
  expect(pendingClosedBookMutation(localStorage,key)).toEqual(operation);
  expect(pendingClosedBookMutation(localStorage,closedBookMutationKey('another','chat'))).toBeNull();
  const create=vi.fn(()=>({...operation,request:{...operation.request,operation_id:'replacement'}}));
  const recovered=await executeClosedBookMutation(localStorage,key,send,create);
  expect(create).not.toHaveBeenCalled();
  expect(recovered.value?.revision).toBe(6);
  expect(recovered.operation).toEqual(operation);
  expect(send.mock.calls.map(([request])=>request.action)).toEqual([action,action,'state']);
  expect(pendingClosedBookMutation(localStorage,key)).toBeNull();
});

it('refreshes a rejected revision and retries the original intent with a new operation ID',async()=>{
  const key=closedBookMutationKey('learner','chat');
  const operation: PendingClosedBookMutation={request:{action:'pause',chat_session_id:'chat',operation_id:'stale',revision:4,draft:'未发出的回答'}};
  const send=vi.fn<typeof api.closedBook>().mockRejectedValueOnce(new ApiError(409,'CLOSED_BOOK_REVISION_CONFLICT','conflict','changed'))
    .mockResolvedValueOnce({...practice,revision:8}).mockResolvedValue({...practice,revision:9,status:'paused'});
  const result=await executeClosedBookMutation(localStorage,key,send,()=>operation);
  expect(result.conflict).toBe(true);expect(result.value?.revision).toBe(8);
  const pending=pendingClosedBookMutation(localStorage,key)!;
  expect(pending.request).toMatchObject({action:'pause',revision:8,draft:'未发出的回答'});
  expect(pending.request.operation_id).not.toBe('stale');
  await executeClosedBookMutation(localStorage,key,send,()=>operation);
  expect(send.mock.calls[2]![0]).toEqual(pending.request);
  expect(pendingClosedBookMutation(localStorage,key)).toBeNull();
});

it('keeps the pending receipt if the follow-up state read fails',async()=>{
  const key=closedBookMutationKey('learner','chat');
  const operation={request:{action:'pause',chat_session_id:'chat',operation_id:'saved',revision:4}};
  localStorage.setItem(key,JSON.stringify(operation));
  const send=vi.fn<typeof api.closedBook>().mockResolvedValueOnce({...practice,revision:5}).mockRejectedValueOnce(new TypeError('read unavailable'));
  await expect(executeClosedBookMutation(localStorage,key,send,()=>operation)).rejects.toThrow('read unavailable');
  expect(pendingClosedBookMutation(localStorage,key)).toEqual(operation);
});
