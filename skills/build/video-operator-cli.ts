import { createInterface } from 'node:readline';
import { ExternalBuildCostLedger } from '../../packages/core/src/external-build-cost';
import { runOperatorVideoBuild } from './video-operator-build';

/** The first stdin line is private host configuration; later cancellation is cooperative. */
export async function runOperatorVideoCli() {
  return runOperatorMediaCli('video', runOperatorVideoBuild);
}

export async function runOperatorMediaCli(kind: 'video' | 'music', runBuild: (input: any) => Promise<unknown>) {
  const lines = createInterface({input:process.stdin});
  const abort = new AbortController();
  const emit = (body: object) => { process.stdout.write(JSON.stringify({version:`operator_${kind}_event.v1`,...body})+'\n'); };
  let sequence=0;
  const pending = new Map<number,{resolve:()=>void;reject:(error:Error)=>void}>();
  abort.signal.addEventListener('abort',()=>{for(const item of pending.values())item.reject(new Error('build_cancelled'));pending.clear();});
  let work: Promise<void> | undefined;
  await new Promise<void>(resolve => {
    lines.on('line',line=>{
      if (work) {
        const message=JSON.parse(line);
        if (message.cancel === true) abort.abort();
        if (pending.has(message.ack)) {pending.get(message.ack)!.resolve();pending.delete(message.ack);}
        return;
      }
      work=(async()=>{
        try {
          const {external_cost,...input}=JSON.parse(line);
          const ledger=new ExternalBuildCostLedger(external_cost.binding.task_ref,external_cost.binding,
            external_cost.connection,external_cost.registry_root);
          const result=await runBuild({...input,cost_ledger:ledger,signal:abort.signal,
            onProgress:(progress:unknown)=>new Promise<void>((resolve,reject)=>{
              if(abort.signal.aborted){reject(new Error('build_cancelled'));return;}
              const id=++sequence;pending.set(id,{resolve,reject});emit({progress,sequence:id});
            })});
          emit({result});
        } catch (error) { emit({error:error instanceof Error?error.message:`operator_${kind}_failed`}); process.exitCode=1; }
        finally { lines.close();process.stdin.pause();resolve(); }
      })();
    });
    lines.on('close',()=>{if(!work){process.exitCode=1;resolve();}else abort.abort();});
  });
}
