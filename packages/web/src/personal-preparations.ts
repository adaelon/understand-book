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
const endpoint=(workspace:string)=>`/learning-workspaces/${encodeURIComponent(workspace)}/preparations`;
export const preparations={
  list:(workspace:string)=>networkFetch<{items:Preparation[]}>('GET',endpoint(workspace)),
  act:(workspace:string,input:Record<string,unknown>)=>networkFetch<Preparation>('POST',endpoint(workspace),{operation_id:crypto.randomUUID(),...input}),
  async upload(workspace:string,file:File,progress:(received:number)=>void) {
    const upload_id=crypto.randomUUID();
    for(let offset=0;offset<file.size;offset+=32*1024) {
      const bytes=new Uint8Array(await file.slice(offset,offset+32*1024).arrayBuffer());
      let binary='';for(let start=0;start<bytes.length;start+=8192) binary+=String.fromCharCode(...bytes.subarray(start,start+8192));
      const result=await networkFetch<{received:number}>('POST',endpoint(workspace),{action:'upload',upload_id,filename:file.name,total:file.size,offset,chunk_base64:btoa(binary)});
      progress(result.received);
    }
    return preparations.act(workspace,{action:'create',task_id:crypto.randomUUID(),upload_id});
  },
};
