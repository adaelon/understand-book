import { afterEach, expect, it } from 'vitest';
import { mkdtempSync, rmSync, existsSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { PersonalNoteSearch, type PersonalNoteSearchRequest } from '../src/personal-note-search';
import type { EmbeddingProvider } from '../src/embedding-provider';
const roots:string[]=[];
afterEach(()=>roots.splice(0).forEach(root=>rmSync(root,{recursive:true,force:true})));
function fixture(beforeDocument?:()=>Promise<void>){
  const root=mkdtempSync(path.join(tmpdir(),'pn-index-'));roots.push(root);
  const calls:string[][]=[];
  const vector=(text:string)=>text.includes('attention')||text.includes('表示')?[1,0]:[0,1];
  const provider:EmbeddingProvider={identity:{provider_id:'test',model_id:'test',model_revision:'1',embedding_config:{},dimensions:2},
    limits:{max_input_tokens:128,max_batch_size:8},count_tokens:text=>[...text].length+2,
    embed_documents:async texts=>{calls.push(texts);await beforeDocument?.();return{vectors:texts.map(vector)};},embed_query:async text=>({vector:vector(text)})};
  return {root,calls,provider,engine:new PersonalNoteSearch(async()=>({provider,dispose:async()=>{}}))};
}
const request=(root:string,notes:PersonalNoteSearchRequest['notes'],revision=1):PersonalNoteSearchRequest=>({kind:'personal_notes',cache_dir:root,snapshot_revision:revision,notes,query:'attention',top_k:10});

it('covers long note tails, collapses fragments, and only embeds changed current versions',async()=>{
  const {root,calls,engine}=fixture();
  const notes=[{note_id:'a',revision:1,text:'其他内容。'.repeat(150)+'表示取决于输入 attention'},{note_id:'b',revision:1,text:'烘焙步骤'}];
  expect((await engine.search(request(root,notes))).status).toBe('partial');await engine.settled();
  const first=await engine.search(request(root,notes));expect(first.status).toBe('ready');expect(first.hits[0].note_id).toBe('a');
  expect(first.hits[0].start_utf16).toBeGreaterThan(600);expect(first.hits.filter(hit=>hit.note_id==='a')).toHaveLength(1);
  const count=calls.length;await engine.search(request(root,notes));await engine.settled();expect(calls.length).toBe(count);
  const updated=[{...notes[0],revision:2,text:'新的表示 attention'},notes[1]];
  await engine.search(request(root,updated,2));await engine.settled();expect(calls.slice(count).flat()).toEqual(['新的表示 attention']);
  await engine.dispose();
  const reopened=new PersonalNoteSearch(async()=>({provider:fixture().provider,dispose:async()=>{}}));
  await reopened.search(request(root,updated,2));await reopened.settled();
  const read=await reopened.search(request(root,updated,2));expect(read.cache_hits).toBe(2);expect(read.document_tokens).toBe(0);await reopened.dispose();
});

it('does not publish late vectors after revision changes or deletion',async()=>{
  let release!:()=>void, started!:()=>void;const pending=new Promise<void>(resolve=>{release=resolve;});const begun=new Promise<void>(resolve=>{started=resolve;});
  let first=true;const {root,engine}=fixture(async()=>{if(first){first=false;started();await pending;}});
  const old=[{note_id:'a',revision:1,text:'旧的表示'}];await engine.search(request(root,old));await begun;
  const newer=[{note_id:'a',revision:2,text:'现在的表示'}];await engine.search(request(root,newer,2));release();await engine.settled();
  expect((await engine.search(request(root,newer,2))).hits[0].revision).toBe(2);
  await engine.search(request(root,[],3));expect(existsSync(path.join(root,'note-a.json'))).toBe(false);
  expect((await engine.search(request(root,old,1))).hits).toHaveLength(0);await engine.dispose();
});

it('reports an unavailable real provider without inventing ready vectors',async()=>{
  const {root}=fixture();const engine=new PersonalNoteSearch(async()=>{throw new Error('model missing');});
  await engine.search(request(root,[{note_id:'a',revision:1,text:'想法'}]));await engine.settled();
  const result=await engine.search(request(root,[{note_id:'a',revision:1,text:'想法'}]));
  expect(result.status).toBe('unavailable');expect(result.reason).toBe('model missing');expect(result.hits).toEqual([]);await engine.dispose();
});

it('indexes a later expression without changing the saved note version',async()=>{
  const {root,engine,calls}=fixture();
  const original=[{note_id:'a',revision:1,expression_count:0,text:'还没想清楚'}];
  await engine.search(request(root,original));await engine.settled();const count=calls.flat().length;
  const later=[{...original[0],expression_count:1,text:'还没想清楚\n明确回复：现在我区分了表示和参数 attention'}];
  await engine.search(request(root,later,2));await engine.settled();
  expect(calls.flat().length).toBe(count+1);expect((await engine.search(request(root,later,2))).hits[0].revision).toBe(1);await engine.dispose();
});
it('does not recreate a deleted note when an embedding batch finishes late',async()=>{
  let release!:()=>void,started!:()=>void;const pending=new Promise<void>(resolve=>{release=resolve;});const begun=new Promise<void>(resolve=>{started=resolve;});
  const {root,engine}=fixture(async()=>{started();await pending;});
  await engine.search(request(root,[{note_id:'a',revision:1,text:'表示 attention'}]));await begun;
  await engine.search(request(root,[],2));release();await engine.settled();
  expect((await engine.search(request(root,[],2))).hits).toEqual([]);expect(existsSync(path.join(root,'note-a.json'))).toBe(false);await engine.dispose();
});
it('recovers after the provider becomes available without restarting the process',async()=>{
  const {root,provider}=fixture();let available=false;
  const engine=new PersonalNoteSearch(async()=>{if(!available)throw new Error('model missing');return {provider,dispose:async()=>{}};});
  const input=request(root,[{note_id:'a',revision:1,text:'表示 attention'}]);
  await engine.search(input);await engine.settled();expect((await engine.search(input)).status).toBe('unavailable');await engine.settled();
  available=true;await engine.search(input);await engine.settled();expect((await engine.search(input)).status).toBe('ready');await engine.dispose();
});
