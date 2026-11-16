// @vitest-environment happy-dom
import { act, type ReactNode } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, expect, it, vi } from 'vitest';
import { ChargesPanel } from './charges';
const read=vi.hoisted(()=>vi.fn());
const submit=vi.hoisted(()=>vi.fn());
vi.mock('@/components/data',async(original)=>({...await original<typeof import('@/components/data')>(),useData:read}));
vi.mock('@/components/operation',async(original)=>({...await original<typeof import('@/components/operation')>(),
  useOperations:()=>({busy:false,blocked:false,submit}),
  OperationDialog:({open,onSubmit,children}:{open:boolean;onSubmit:(data:FormData)=>Promise<unknown>;children:ReactNode})=>
    open?<form data-service-form onSubmit={e=>{e.preventDefault();void onSubmit(new FormData(e.currentTarget));}}>{children}<button type="submit">提交用量</button></form>:null,
}));
let root:Root|undefined,host:HTMLDivElement;
afterEach(async()=>{if(root){await act(async()=>root!.unmount());root=undefined;host.remove();}read.mockReset();submit.mockReset();});
it('reconciles processing duration through the service endpoint and preserves unknown operating cost',async()=>{
  (globalThis as typeof globalThis & {IS_REACT_ACT_ENVIRONMENT:boolean}).IS_REACT_ACT_ENVIRONMENT=true;
  const charge={entry_kind:'service',call_id:'svc-run',user_id:'A',cost_owner:'account',revision:2,state:'pending',
    model:null,purpose:'video_audio_review',quantity_ms:null,admitted_quantity_ms:2500,reserved_micro_cny:25,
    provider_cost_micro_cny:null,provider_cost_status:'pending',account_debit_micro_cny:null,created_at:1792281600,
    reports:{items:[],total:0},reconciliations:{items:[],total:0}};
  read.mockImplementation((url:string)=>({isPending:false,error:null,data:url.startsWith('/admin/charges?')?{items:[charge],total:1}:charge}));
  submit.mockResolvedValue(true);
  host=document.createElement('div');document.body.append(host);root=createRoot(host);
  await act(async()=>root!.render(<ChargesPanel userId="A"/>));
  expect(host.textContent).toContain('原声复核');
  await act(async()=>[...host.querySelectorAll('button')].find(b=>b.textContent==='查看 / 核算')!.click());
  expect(document.body.textContent).toContain('供应商成本未知');
  await act(async()=>[...document.body.querySelectorAll('button')].find(b=>b.textContent==='核对实际处理用量')!.click());
  const form=document.querySelector<HTMLFormElement>('[data-service-form]')!;
  form.querySelector<HTMLInputElement>('[name=seconds]')!.value='1.8';
  form.querySelector<HTMLInputElement>('[name=reason]')!.value='恢复执行日志';
  form.querySelector<HTMLTextAreaElement>('[name=evidence]')!.value='Whisper 实际音频时长 1.8 秒';
  await act(async()=>form.dispatchEvent(new Event('submit',{bubbles:true,cancelable:true})));
  expect(submit).toHaveBeenCalledWith('核对处理用量','/admin/service-usage/svc-run/reconcile',expect.objectContaining({
    revision:2,status:'completed',quantity_ms:1800,evidence:'Whisper 实际音频时长 1.8 秒',
  }));
});
it.each([['web_search','搜索'],['web_fetch','网页读取']])('labels %s and reconciles its unknown cost without treating it as audio duration',async(kind,label)=>{
  (globalThis as typeof globalThis & {IS_REACT_ACT_ENVIRONMENT:boolean}).IS_REACT_ACT_ENVIRONMENT=true;
  const charge={entry_kind:kind,call_id:'web-call',user_id:'A',cost_owner:'account',revision:2,state:'pending',
    model:null,purpose:kind,reserved_micro_cny:5000,provider_cost_micro_cny:null,provider_cost_status:'pending',
    account_debit_micro_cny:null,created_at:1792281600,reports:{items:[],total:0},reconciliations:{items:[],total:0}};
  read.mockImplementation((url:string)=>({isPending:false,error:null,data:url.startsWith('/admin/charges?')?{items:[charge],total:1}:charge}));
  submit.mockResolvedValue(true);
  host=document.createElement('div');document.body.append(host);root=createRoot(host);
  await act(async()=>root!.render(<ChargesPanel userId="A"/>));
  expect(host.textContent).toContain(label);expect(host.textContent).toContain('供应商成本未知');
  await act(async()=>[...host.querySelectorAll('button')].find(b=>b.textContent==='查看 / 核算')!.click());
  await act(async()=>[...document.body.querySelectorAll('button')].find(b=>b.textContent==='人工核算 / 追加更正')!.click());
  const form=document.querySelector<HTMLFormElement>('[data-service-form]')!;
  expect(form.querySelector('[name=seconds]')).toBeNull();
  form.querySelector<HTMLInputElement>('[name=cost]')!.value='0.005';
  form.querySelector<HTMLInputElement>('[name=reason]')!.value='核对请求';
  form.querySelector<HTMLTextAreaElement>('[name=evidence]')!.value='供应商请求账单';
  await act(async()=>form.dispatchEvent(new Event('submit',{bubbles:true,cancelable:true})));
  expect(submit).toHaveBeenCalledWith('人工核算费用','/admin/charges/web-call/reconcile',expect.objectContaining({provider_cost_micro_cny:5000,waive_account:false}));
});
it.each(['service', 'web_search'])('keeps the %s reconciliation form and values after a failed submission, then closes on success', async kind => {
  (globalThis as typeof globalThis & {IS_REACT_ACT_ENVIRONMENT:boolean}).IS_REACT_ACT_ENVIRONMENT=true;
  const charge={entry_kind:kind,call_id:'retry-call',user_id:'A',cost_owner:'account',revision:2,state:'pending',
    model:null,purpose:kind==='service'?'video_audio_review':kind,quantity_ms:null,admitted_quantity_ms:2500,reserved_micro_cny:25,
    provider_cost_micro_cny:null,provider_cost_status:'pending',account_debit_micro_cny:null,created_at:1792281600,
    reports:{items:[],total:0},reconciliations:{items:[],total:0}};
  read.mockImplementation((url:string)=>({isPending:false,error:null,data:url.startsWith('/admin/charges?')?{items:[charge],total:1}:charge}));
  submit.mockResolvedValueOnce(false).mockResolvedValueOnce(true);
  host=document.createElement('div');document.body.append(host);root=createRoot(host);
  await act(async()=>root!.render(<ChargesPanel userId="A"/>));
  await act(async()=>[...host.querySelectorAll('button')].find(b=>b.textContent==='查看 / 核算')!.click());
  const action=kind==='service'?'核对实际处理用量':'人工核算 / 追加更正';
  await act(async()=>[...document.body.querySelectorAll('button')].find(b=>b.textContent===action)!.click());
  const form=document.querySelector<HTMLFormElement>('[data-service-form]')!;
  const value=form.querySelector<HTMLInputElement>(kind==='service'?'[name=seconds]':'[name=cost]')!;
  value.value=kind==='service'?'1.8':'0.005';
  form.querySelector<HTMLInputElement>('[name=reason]')!.value='核对请求';
  form.querySelector<HTMLTextAreaElement>('[name=evidence]')!.value='已填写的核算依据';
  await act(async()=>form.dispatchEvent(new Event('submit',{bubbles:true,cancelable:true})));
  expect(document.querySelector('[data-service-form]')).toBe(form);
  expect(value.value).toBe(kind==='service'?'1.8':'0.005');
  expect(form.querySelector<HTMLInputElement>('[name=reason]')!.value).toBe('核对请求');
  expect(form.querySelector<HTMLTextAreaElement>('[name=evidence]')!.value).toBe('已填写的核算依据');
  await act(async()=>form.dispatchEvent(new Event('submit',{bubbles:true,cancelable:true})));
  expect(submit).toHaveBeenCalledTimes(2);
  expect(document.querySelector('[data-service-form]')).toBeNull();
});
