// @vitest-environment happy-dom
import { flushPromises, mount } from '@vue/test-utils';
import { beforeEach, afterEach, expect, it, vi } from 'vitest';
import { installIdentity } from '../network-context';
import { discardNoteDraft, notesPanel, openPersonalNotes, openThoughtCapture, readNoteDrafts, registerThoughtContext, startThought } from '../personal-notes';
import PersonalNotes from './PersonalNotes.vue';
import { registerNoteDestinations, type NoteDestination } from '../note-placements';
let view:ReturnType<typeof mount>;
let unregisterContext: (() => void) | undefined;
let unregisterDestination: (() => void) | undefined;
beforeEach(()=>{installIdentity({user_id:'thought-ui',csrf_token:'csrf'});readNoteDrafts().forEach(discardNoteDraft);notesPanel.open=false;notesPanel.draftId=null;notesPanel.reference=null;
  vi.stubGlobal('fetch',vi.fn(async()=>new Response(JSON.stringify({notes:[],next_cursor:null}),{status:200})));});
afterEach(()=>{view?.unmount();unregisterContext?.();unregisterContext=undefined;unregisterDestination?.();unregisterDestination=undefined;vi.useRealTimers();vi.unstubAllGlobals();});
it('captures the exact presentation state when an independent draft is explicitly linked to the current scene', async () => {
  view = mount(PersonalNotes, { attachTo: document.body });
  await view.get('.thought-entry').trigger('click'); await flushPromises();
  await view.get('textarea').setValue('先独立写，再关联眼前这一页。');
  unregisterContext = registerThoughtContext(() => ({session_id:'session',turn_id:'turn',label:'演示 · 第二页'}));
  const destination: NoteDestination = {kind:'attachment',label:'演示 · 第二页',target:{kind:'presentation',visible_step:'第二页',receipt:{session_id:'session',turn_id:'turn',reference:{presentation_id:'presentation',revision:1},state_revision:7,saved_state_ref:'state-7'}}};
  const resolve = vi.fn(async () => destination);
  unregisterDestination = registerNoteDestinations(() => [{key:'presentation:current',label:'当前演示页',detail:'第二页',resolve}]);
  await view.get('.context-toggle').trigger('click');
  await view.get('.context-options button:first-of-type').trigger('click'); await flushPromises();
  expect(resolve).toHaveBeenCalledTimes(1);
  expect(readNoteDrafts()[0].value.context.origin).toEqual(destination.target);
  expect(readNoteDrafts()[0].destination).toBeUndefined();
  await view.get('.context-toggle').trigger('click');
  await view.get('.context-options button:last-child').trigger('click'); await flushPromises();
  expect(readNoteDrafts()[0].value.context.origin).toBeUndefined();
  expect(readNoteDrafts()[0].value.content).toBe('先独立写，再关联眼前这一页。');
});
it('shows the frozen source and lets a new global draft become independent without losing its words', async () => {
  unregisterContext = registerThoughtContext(() => ({ book_id: 'book-a', lid: '2.1', label: '学习与理解 · 第二章' }));
  view = mount(PersonalNotes, { attachTo: document.body });
  await view.get('.thought-entry').trigger('click'); await flushPromises();
  await view.get('textarea').setValue('这是我的一个联想');
  expect(view.get('.context-toggle').text()).toContain('学习与理解');
  await view.get('.context-toggle').trigger('click');
  await view.get('.context-options button:last-child').trigger('click');
  expect(view.get('.context-toggle').text()).toBe('独立想法');
  expect(readNoteDrafts()[0].value.context.book_id).toBeNull();
  expect(readNoteDrafts()[0].value.content).toBe('这是我的一个联想');
  await view.get('[aria-label="收起我的想法"]').trigger('click');
  await view.get('.thought-entry').trigger('click'); await flushPromises();
  expect(view.get('.context-toggle').text()).toBe('独立想法');
  await view.get('.context-toggle').trigger('click');
  await view.get('.context-options button:first-of-type').trigger('click');
  expect(readNoteDrafts()[0].value.context).toMatchObject({ book_id: 'book-a', lid: '2.1' });
});
it('restores the shared draft after collapse and does not submit during composition',async()=>{
  view=mount(PersonalNotes,{attachTo:document.body});
  startThought({label:'视频 · 03:14',video_time_ms:194000},'最初的半句');await flushPromises();
  expect((view.get('textarea[aria-label="想法正文"]').element as HTMLTextAreaElement).value).toBe('最初的半句');
  await view.get('textarea[aria-label="想法正文"]').setValue('最初的半句，接着写');
  await view.get('textarea[aria-label="想法正文"]').trigger('compositionstart');
  await view.get('textarea').trigger('keydown',{key:'Enter',ctrlKey:true});await flushPromises();
  expect(vi.mocked(fetch).mock.calls.every(([url])=>String(url).endsWith('/list'))).toBe(true);
  await view.get('[aria-label="收起我的想法"]').trigger('click');
  await view.get('.thought-entry').trigger('click');await flushPromises();
  expect(view.text()).toContain('视频 · 03:14');expect(readNoteDrafts()[0].value.content).toBe('最初的半句，接着写');
});

it('opens directly into the body without waiting for a library request and reuses the draft',async()=>{
  view=mount(PersonalNotes,{attachTo:document.body});
  await view.get('.thought-entry').trigger('click');await flushPromises();
  expect(document.activeElement).toBe(view.get('textarea').element);
  expect(fetch).not.toHaveBeenCalled();
  expect(view.find('[aria-modal]').exists()).toBe(false);
  await view.get('[aria-label="收起我的想法"]').trigger('click');
  openThoughtCapture();await flushPromises();
  expect(readNoteDrafts()).toHaveLength(1);
  expect(document.activeElement).toBe(view.get('textarea').element);
});

it('leaves background shortcuts alone and does not trap Tab or dismiss an IME composition',async()=>{
  view=mount(PersonalNotes,{attachTo:document.body});
  startThought({},'读到这里想到的');await flushPromises();
  window.dispatchEvent(new KeyboardEvent('keydown',{key:'Enter',ctrlKey:true,bubbles:true}));
  window.dispatchEvent(new KeyboardEvent('keydown',{key:'Escape',bubbles:true}));
  await flushPromises();expect(fetch).not.toHaveBeenCalled();expect(notesPanel.open).toBe(true);
  const tab=new KeyboardEvent('keydown',{key:'Tab',bubbles:true,cancelable:true});
  view.get('.thought-actions .primary').element.dispatchEvent(tab);expect(tab.defaultPrevented).toBe(false);
  await view.get('textarea').trigger('compositionstart');
  await view.get('textarea').trigger('keydown',{key:'Escape'});expect(notesPanel.open).toBe(true);
  await view.get('textarea').trigger('compositionend');
  await view.get('textarea').trigger('keydown',{key:'Escape'});expect(notesPanel.open).toBe(false);
});

it('saves the captured source, collapses on acknowledgement and shows a receipt',async()=>{
  vi.stubGlobal('fetch',vi.fn(async()=>new Response(JSON.stringify({note_id:'saved',current_revision:1}))));
  view=mount(PersonalNotes,{attachTo:document.body});
  startThought({label:'正在读的原文',book_id:'book-a',lid:'2.1'},'先保留这个疑问');await flushPromises();
  await view.get('textarea').trigger('keydown',{key:'Enter',ctrlKey:true});await flushPromises();
  expect(notesPanel.open).toBe(false);expect(readNoteDrafts()).toHaveLength(0);
  expect(view.get('[role="status"]').text()).toContain('已保存');
  const body=JSON.parse(String(vi.mocked(fetch).mock.calls[0][1]?.body));
  expect(body.value.context).toMatchObject({book_id:'book-a',lid:'2.1'});
});

it('keeps a failed save visible with the original text for retry',async()=>{
  vi.stubGlobal('fetch',vi.fn(async()=>{throw new TypeError('暂时离线');}));
  view=mount(PersonalNotes,{attachTo:document.body});
  startThought({},'尚未保存的解释');await flushPromises();
  await view.get('.thought-actions .primary').trigger('click');await flushPromises();
  expect(notesPanel.open).toBe(true);
  expect((view.get('textarea').element as HTMLTextAreaElement).value).toBe('尚未保存的解释');
  expect(view.get('[role="alert"]').text()).toContain('暂时离线');
  expect(view.get('.thought-actions .primary').text()).toContain('重试');
});

it('dismisses each save receipt after three seconds, including consecutive saves with the same message', async () => {
  vi.useFakeTimers({ toFake: ['setTimeout', 'clearTimeout'] });
  vi.stubGlobal('fetch', vi.fn(async () => Response.json({ note_id: 'saved', current_revision: 1 })));
  view = mount(PersonalNotes, { attachTo: document.body });
  startThought({}, '第一条想法'); await flushPromises();
  await view.get('.thought-actions .primary').trigger('click'); await flushPromises();
  expect(view.get('.toolbar-notice').text()).toContain('已保存');
  await vi.advanceTimersByTimeAsync(2000);
  startThought({}, '第二条想法'); await flushPromises();
  await view.get('.thought-actions .primary').trigger('click'); await flushPromises();
  await vi.advanceTimersByTimeAsync(1000);
  expect(view.get('.toolbar-notice').text()).toContain('已保存');
  await vi.advanceTimersByTimeAsync(2000);
  expect(view.find('.toolbar-notice').exists()).toBe(false);
  expect(view.find('.thought-entry').exists()).toBe(true);
});

it('a slow save cannot clear the newer draft opened while waiting',async()=>{
  let finish!:(response:Response)=>void;
  vi.stubGlobal('fetch',vi.fn(async(url:unknown)=>String(url).endsWith('/create') ? await new Promise<Response>(resolve=>{finish=resolve;}) : new Response(JSON.stringify({notes:[],next_cursor:null}))));
  view=mount(PersonalNotes,{attachTo:document.body});startThought({},'先保存的想法');await flushPromises();
  await view.get('.thought-actions .primary').trigger('click');await flushPromises();
  startThought({},'保存期间另起的新表达');await flushPromises();
  finish(new Response(JSON.stringify({note_id:'old',current_revision:1})));await flushPromises();
  expect((view.get('textarea[aria-label="想法正文"]').element as HTMLTextAreaElement).value).toBe('保存期间另起的新表达');
  expect(readNoteDrafts().map(draft=>draft.value.content)).toEqual(['保存期间另起的新表达']);
});

it('a save acknowledgement cannot replace the library opened while it was pending',async()=>{
  let finish!:(response:Response)=>void;
  vi.stubGlobal('fetch',vi.fn(async(url:unknown)=>String(url).endsWith('/create') ? await new Promise<Response>(resolve=>{finish=resolve;}) : new Response(JSON.stringify({notes:[],next_cursor:null}))));
  view=mount(PersonalNotes,{attachTo:document.body});startThought({},'先保存，再找之前的想法');await flushPromises();
  await view.get('.thought-actions .primary').trigger('click');await flushPromises();
  openPersonalNotes();await flushPromises();
  finish(new Response(JSON.stringify({note_id:'old',current_revision:1})));await flushPromises();
  expect(notesPanel.view).toBe('library');expect(view.find('.thought-navigation').exists()).toBe(true);
  expect(view.find('textarea').exists()).toBe(false);
});
