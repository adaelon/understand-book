// @vitest-environment happy-dom
import './network-client';
import { beforeEach, afterEach, expect, it, vi } from 'vitest';
import { installIdentity, network } from './network-context';
import { ApiError } from '@understand-book/web/reader/api';
import { discardNoteDraft, keepNoteDraft, notesPanel, readNoteDrafts, saveThought, startThought, compareNoteText, registerThoughtContext, openThoughtCapture } from '@understand-book/web/reader/personal-notes';

beforeEach(() => { installIdentity({user_id:'thought-test',csrf_token:'csrf'}); readNoteDrafts().forEach(discardNoteDraft); localStorage.clear(); notesPanel.open=false; });
afterEach(() => { vi.unstubAllGlobals(); });

it('keeps original video context and device draft across material changes, and separates accounts', () => {
  const draft=startThought({book_id:'video-a',publication_id:'v1',video_time_ms:3140,label:'原来的视频'},'未写完的半句话');
  network.value.workspace={published_book_ref:{book_id:'other',publication_id:'v2'}} as never;
  draft.value.content+='，补一个想法';keepNoteDraft(draft);notesPanel.open=false;
  const recovered=readNoteDrafts()[0];
  expect(recovered.value.context.video_time_ms).toBe(3140);expect(recovered.value.context.book_id).toBe('video-a');
  expect(JSON.parse(localStorage.getItem(Object.keys(localStorage).find(key=>key.includes('thought-draft'))!)!).value.content).toContain('补一个想法');
  installIdentity({user_id:'another-reader',csrf_token:'csrf'});expect(readNoteDrafts()).toHaveLength(0);
  installIdentity({user_id:'thought-test',csrf_token:'csrf'});expect(readNoteDrafts()[0].draft_id).toBe(draft.draft_id);
});

it('defaults to the visible context once and never rebinds a resumed draft to a later page', () => {
  let context = { book_id: 'first-book', lid: '1.2', label: '原来的阅读位置' };
  const unregister = registerThoughtContext(() => context);
  try {
    const draft = openThoughtCapture();
    expect(draft.value.context).toMatchObject(context);
    context = { book_id: 'next-book', lid: '3.1', label: '后来的页面' };
    expect(openThoughtCapture().value.context).toMatchObject({ book_id: 'first-book', lid: '1.2' });
    const independent = startThought({});
    expect(independent.value.context.book_id).toBeNull();
    expect(independent.value.context.label).toBeNull();
  } finally { unregister(); }
});

it('retries the same frozen save after an unknown response and only clears an acknowledged draft', async () => {
  const draft=startThought({},'参数固定，激活可变');
  const bodies: Record<string,unknown>[]=[];
  const fetch=vi.fn(async (_url:unknown,init:RequestInit) => {bodies.push(JSON.parse(String(init.body)));if(bodies.length===1)throw new TypeError('connection lost');return new Response(JSON.stringify({note_id:'saved',current_revision:1}),{status:200});});
  vi.stubGlobal('fetch',fetch);
  await expect(saveThought(draft)).rejects.toThrow('connection lost');expect(draft.pending).not.toBeNull();expect(readNoteDrafts()).toHaveLength(1);
  await saveThought(draft);expect(bodies[1]).toEqual(bodies[0]);expect(readNoteDrafts()).toHaveLength(0);
});

it('preserves a stale revision draft for explicit merging', async () => {
  const draft=startThought({},'我的改动');draft.note_id='note';draft.base_revision=1;keepNoteDraft(draft);
  vi.stubGlobal('fetch',vi.fn(async ()=>new Response(JSON.stringify({error_code:'NOTE_REVISION_CONFLICT',category:'conflict',message:'已有新版'}),{status:409})));
  await expect(saveThought(draft)).rejects.toBeInstanceOf(ApiError);
  expect(draft.pending).toBeNull();expect(readNoteDrafts()[0].value.content).toBe('我的改动');expect(draft.base_revision).toBe(1);
});

it('persists an acknowledged note while retrying only a failed placement', async () => {
  const draft=startThought({label:'原来读到的地方'},'我自己的想法');
  draft.destination={kind:'attachment',label:'某句话下面',target:{kind:'message',session_id:'chat',turn_id:'turn',role:'assistant',quote:'原句',occurrence:0}};
  const calls:string[]=[];let fail=true;
  vi.stubGlobal('fetch',vi.fn(async(url:unknown)=>{calls.push(String(url));if(String(url).endsWith('/attach')&&fail)throw new TypeError('暂时离线');return new Response(JSON.stringify({note_id:'saved',current_revision:1}));}));
  await expect(saveThought(draft)).rejects.toThrow('想法已保存，放置未完成');
  expect(readNoteDrafts()[0].saved_note?.note_id).toBe('saved');
  expect(JSON.parse(localStorage.getItem(Object.keys(localStorage).find(key=>key.includes('thought-draft'))!)!).saved_note.note_id).toBe('saved');
  fail=false;await saveThought(draft);
  expect(calls.filter(path=>path.endsWith('/create'))).toHaveLength(1);expect(calls.filter(path=>path.endsWith('/attach'))).toHaveLength(2);expect(readNoteDrafts()).toHaveLength(0);
});

it('compares literal Chinese, formula and code without replacing old expressions', () => {
  const before='原话\n$x = 1$\n```js\na()\n```';const after='原话\n$x = 2$\n```js\nb()\n```';
  const lines=compareNoteText(before,after);
  expect(lines.filter(line=>line.kind!=='added').map(line=>line.text).join('\n')).toBe(before);
  expect(lines.filter(line=>line.kind!=='removed').map(line=>line.text).join('\n')).toBe(after);
});
