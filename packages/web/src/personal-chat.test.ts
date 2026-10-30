// @vitest-environment happy-dom
import { afterEach, expect, it, vi } from 'vitest';
import { installIdentity } from './network-context';
import { openPersonalChat, personalChat } from './personal-chat';

afterEach(() => vi.unstubAllGlobals());

it('keeps words typed while a new discussion is still being created', async () => {
  installIdentity({user_id:'personal-chat-draft',csrf_token:'csrf'});
  localStorage.clear();
  let finish!: (response: Response) => void;
  vi.stubGlobal('fetch',vi.fn(async (url: unknown) => String(url).endsWith('/chat_create')
    ? await new Promise<Response>(resolve => {finish=resolve;})
    : new Response(JSON.stringify({session_id:'new-session',title:'新讨论',turns:[]}))));
  const opening=openPersonalChat([{note_id:'idea',revision:1}]);
  expect(personalChat.open).toBe(true);expect(personalChat.opening).toBe(true);
  personalChat.text='这是我刚刚输入的补充，不能被默认问题覆盖。';
  finish(new Response(JSON.stringify({session_id:'new-session'})));
  await opening;
  expect(personalChat.opening).toBe(false);
  expect(personalChat.text).toBe('这是我刚刚输入的补充，不能被默认问题覆盖。');
  expect(JSON.parse(localStorage.getItem('understand-book:personal-chat:personal-chat-draft:new-session')!).text).toBe(personalChat.text);
});
