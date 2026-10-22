import { networkFetch } from './network-client';
import type { AllowancePercentage } from './account-allowance';
import type { PublishedBookRef } from './network-context';
export interface Preparation {
  task_id:string; name:string; learning_workspace_id:string; state:string; revision:number; error:string|null; pause_requested:boolean;
  allowance:null|{period_id:string;expires_at:number;available:AllowancePercentage;maximum:AllowancePercentage;remaining:AllowancePercentage|null;};
  usage_periods:{period_id:string;expires_at:number;used:AllowancePercentage|null;pending:AllowancePercentage|null;reserved:AllowancePercentage|null}[];
  estimate:{status:string;message:string;lower?:AllowancePercentage;upper?:AllowancePercentage;partial?:boolean;per_call_maximum?:AllowancePercentage};
  materials:{label:string;kind:'book'|'video';state:string;published_book_ref:PublishedBookRef|null;plan_ready:boolean;progress:null|{message:string;completed:number;total:number;unit:string;status:string}}[];
}
export interface StorageItem {
  storage_id:string;kind:'upload';filename:string;bytes:number;state:'reserved'|'used';
  workspace_id:string;task_id?:string|null;book_id?:string|null;publication_id?:string|null;updated_at:number;download_url?:string|null;
}
export interface AccountStorage {
  used_bytes:number;reserved_bytes:number;total_bytes:number;limit_bytes:number|null;max_file_bytes:number|null;
  available_bytes:number;can_upload:boolean;over_limit:boolean;expires_at:number|null;retained_until:number|null;items:StorageItem[];
}
export const accountStorage={
  read:()=>networkFetch<AccountStorage>('GET','/account/storage'),
  act:(action:'cancel'|'delete'|'cleanup',item?:StorageItem)=>networkFetch<AccountStorage>('POST','/account/storage',{
    action,...(item?{storage_id:item.storage_id,workspace_id:item.workspace_id}:{}),
  }),
};
export interface PreparationUpload {
  upload_id:string;task_id:string;operation_id:string;received:number;cancelled:boolean;saving:boolean;
}
export function preparationUpload(upload_id:string=crypto.randomUUID()):PreparationUpload {
  return {upload_id,task_id:crypto.randomUUID(),operation_id:crypto.randomUUID(),received:0,cancelled:false,saving:false};
}
export interface PreparationSource {
  name: string; size: number;
  slice(start: number, end: number): { arrayBuffer(): Promise<ArrayBuffer> };
}
const endpoint=(workspace:string)=>`/learning-workspaces/${encodeURIComponent(workspace)}/preparations`;
export const preparations={
  list:(workspace:string)=>networkFetch<{items:Preparation[];supported_formats?:string[]}>('GET',endpoint(workspace)),
  act:(workspace:string,input:Record<string,unknown>)=>networkFetch<Preparation>('POST',endpoint(workspace),{operation_id:crypto.randomUUID(),...input}),
  async upload(workspace:string,file:PreparationSource,progress:(received:number)=>void,session=preparationUpload()) {
    const {upload_id}=session;
    session.cancelled=false;
    while(session.received<file.size) {
      if(session.cancelled)throw new DOMException('上传已停止','AbortError');
      const offset=session.received;
      const bytes=new Uint8Array(await file.slice(offset,offset+32*1024).arrayBuffer());
      let binary='';for(let start=0;start<bytes.length;start+=8192) binary+=String.fromCharCode(...bytes.subarray(start,start+8192));
      const result=await networkFetch<{received:number}>('POST',endpoint(workspace),{action:'upload',upload_id,filename:file.name,total:file.size,offset,chunk_base64:btoa(binary)});
      session.received=result.received;
      progress(result.received);
    }
    if(session.cancelled)throw new DOMException('上传已停止','AbortError');
    session.saving=true;
    try {
      return await preparations.act(workspace,{action:'create',task_id:session.task_id,operation_id:session.operation_id,upload_id});
    } finally { session.saving=false; }
  },
};
