// @vitest-environment happy-dom
import { mount, flushPromises } from '@vue/test-utils';
import { afterEach, expect, it, vi } from 'vitest';
vi.mock('./network-client', () => ({ networkFetch: vi.fn() }));
import { networkFetch } from './network-client';
import { preparationUpload, preparations, type AccountStorage } from './personal-preparations';
import PersonalPreparations from './components/PersonalPreparations.vue';
import type { LearningWorkspace } from '@understand-book/web/reader/reader-context';

const fetchMock=vi.mocked(networkFetch);
const workspace:LearningWorkspace={
  learning_workspace_id:'workspace',goal:{text:'复习',constraints:[],revision:1,created_at:1},
  goal_history:[],members:[],archived:false,created_at:1,updated_at:1,
};
const emptyStorage=():AccountStorage=>({
  used_bytes:0,reserved_bytes:0,total_bytes:0,limit_bytes:1024*1024,max_file_bytes:1024*1024,
  available_bytes:1024*1024,can_upload:true,over_limit:false,expires_at:2_000_000_000,retained_until:null,items:[],
});
afterEach(()=>{vi.restoreAllMocks();fetchMock.mockReset();});

it('resumes with the same upload and chunk after a lost acknowledgement, and reuses creation identity',async()=>{
  const file=new File(['x'.repeat(70_000)],'notes.md');
  const session=preparationUpload(),requests:Record<string,unknown>[]=[];
  let serverBytes=0,failChunk=true,failCreate=true;
  fetchMock.mockImplementation(async(_method,_path,input)=>{
    const body=input as Record<string,unknown>;requests.push(body);
    if(body.action==='upload'){
      serverBytes=Math.max(serverBytes,Number(body.offset)+atob(String(body.chunk_base64)).length);
      if(serverBytes===65_536&&failChunk){failChunk=false;throw new Error('connection lost');}
      return {received:serverBytes};
    }
    if(failCreate){failCreate=false;throw new Error('create response lost');}
    return {task_id:body.task_id};
  });
  await expect(preparations.upload('workspace',file,()=>{},session)).rejects.toThrow('connection lost');
  expect(session.received).toBe(32_768);
  await expect(preparations.upload('workspace',file,()=>{},session)).rejects.toThrow('create response lost');
  await preparations.upload('workspace',file,()=>{},session);
  expect(new Set(requests.map(request=>request.upload_id)).size).toBe(1);
  expect(requests.filter(request=>request.action==='upload').map(request=>request.offset)).toEqual([0,32_768,32_768,65_536]);
  const creates=requests.filter(request=>request.action==='create');
  expect(creates).toHaveLength(2);
  expect(creates[0]).toEqual(creates[1]);
});

it('stops after an in-flight chunk when cancelled and never creates a preparation',async()=>{
  const session=preparationUpload(),file=new File(['x'.repeat(70_000)],'notes.md');
  let release!:()=>void;
  const gate=new Promise<void>(resolve=>{release=resolve;});
  fetchMock.mockImplementation(async()=>{
    await gate;return {received:32_768};
  });
  const result=preparations.upload('workspace',file,()=>{},session);
  await flushPromises();session.cancelled=true;release();
  await expect(result).rejects.toMatchObject({name:'AbortError'});
  expect(fetchMock).toHaveBeenCalledTimes(1);
  expect(session.received).toBe(32_768);
});

it('shows account space, permits resuming a reserved upload with zero spare space and can cancel it',async()=>{
  let current:AccountStorage={...emptyStorage(),reserved_bytes:50,total_bytes:50,limit_bytes:50,available_bytes:0,can_upload:false,
    items:[{storage_id:'reserved-upload',kind:'upload',filename:'notes.md',bytes:50,state:'reserved',workspace_id:'workspace',updated_at:1}]};
  const bodies:Record<string,unknown>[]=[];
  fetchMock.mockImplementation(async(method,path,input)=>{
    if(path==='/account/storage'){
      if(method==='POST'){bodies.push(input as Record<string,unknown>);current={...emptyStorage(),limit_bytes:50,available_bytes:50};}
      return current;
    }
    if(method==='GET')return {items:[]};
    const body=input as Record<string,unknown>;bodies.push(body);
    if(body.action==='upload')throw new Error('上传暂时中断');
    throw new Error('unexpected create');
  });
  vi.spyOn(window,'confirm').mockReturnValue(true);
  const wrapper=mount(PersonalPreparations,{props:{workspace}});
  try{
    await flushPromises();
    expect(wrapper.get('[aria-label="账号私人空间"]').text()).toContain('上传预留 50 B');
    expect(wrapper.get('.upload-button').attributes('disabled')).toBeDefined();
    await wrapper.findAll('button').find(button=>button.text()==='整理空间')!.trigger('click');
    await wrapper.findAll('button').find(button=>button.text()==='继续上传')!.trigger('click');
    const input=wrapper.get('input[type=file]');
    Object.defineProperty(input.element,'files',{value:[new File(['x'.repeat(50)],'notes.md')],configurable:true});
    await input.trigger('change');await flushPromises();
    expect(bodies.find(body=>body.action==='upload')?.upload_id).toBe('reserved-upload');
    expect(wrapper.text()).toContain('继续这次上传');
    await wrapper.findAll('button').find(button=>button.text()==='取消并释放空间')!.trigger('click');
    await flushPromises();
    expect(bodies.find(body=>body.action==='cancel')).toEqual({action:'cancel',storage_id:'reserved-upload',workspace_id:'workspace'});
    expect(wrapper.get('[aria-label="账号私人空间"]').text()).toContain('上传预留 0 B');
    expect(wrapper.get('.upload-button').attributes('disabled')).toBeUndefined();
  }finally{wrapper.unmount();}
});

