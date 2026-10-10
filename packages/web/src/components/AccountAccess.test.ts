// @vitest-environment happy-dom
import { mount, flushPromises } from '@vue/test-utils';
import { afterEach, expect, it, vi } from 'vitest';
import AccountAccess from './AccountAccess.vue';
import PasswordChange from './PasswordChange.vue';
import { installIdentity, network } from '../network-context';
import { takeAccountLink, validPassword } from '../account-forms';
const wrappers: ReturnType<typeof mount>[] = [];
afterEach(() => { wrappers.splice(0).forEach(w => w.unmount()); vi.useRealTimers(); vi.unstubAllGlobals(); sessionStorage.clear(); localStorage.clear(); history.replaceState({}, '', '/'); network.value = { ...network.value, identity: null, workspace: null, enabled: false }; });
function access(view: 'register' | 'forgot-password' | 'reset-password', token = '') { const w = mount(AccountAccess, { props: { view, token } }); wrappers.push(w); return w; }
const button = (w: ReturnType<typeof mount>, text: string) => w.findAll('button').find(b => b.text() === text)!;
const pending = { request_id: 'request', expires_at: Math.floor(Date.now()/1000) + 900, resend_after: 60 };
async function fillRegistration(w: ReturnType<typeof mount>) {
  const inputs=w.findAll('input');
  for (const [i,value] of ['reader@example.com','fixture-password','fixture-password','ABCDE-ABCDE-ABCDE-ABCDE'].entries()) await inputs[i].setValue(value);
}
it('registration keeps a timed-out request, resends, reports wrong codes and returns to login without storing secrets', async () => {
  vi.useFakeTimers();
  const calls: string[]=[];
  vi.stubGlobal('fetch',vi.fn(async (url:string,init:RequestInit) => {
    calls.push(url);
    if(url.endsWith('/start')) return Response.json({...pending,error_code:'ACCOUNT_MAIL_TIMEOUT'},{status:503});
    if(url.endsWith('/resend')) return Response.json(pending);
    if(JSON.parse(String(init.body)).verification_code!=='123456') return Response.json({error_code:'REGISTRATION_CODE_INVALID'},{status:400});
    return Response.json({completed:true});
  }));
  const w=access('register'); await fillRegistration(w); await w.get('form').trigger('submit'); await flushPromises();
  expect(w.text()).toContain('可能已送达'); expect(w.find('input[type=password]').exists()).toBe(false);
  expect(button(w,'60 秒后可重发').attributes('disabled')).toBeDefined();
  vi.advanceTimersByTime(61_000); await flushPromises(); await button(w,'重发验证码').trigger('click'); await flushPromises();
  await w.get('input').setValue('000000'); await w.get('form').trigger('submit'); await flushPromises(); expect(w.text()).toContain('验证码不正确');
  await w.get('input').setValue('123456'); await w.get('form').trigger('submit'); await flushPromises();
  expect(w.emitted('login')?.[0][0]).toBe('reader@example.com'); expect(calls).toHaveLength(4);
  for(const storage of [localStorage,sessionStorage]) { expect(JSON.stringify(storage)).not.toContain('fixture-password'); expect(JSON.stringify(storage)).not.toContain('ABCDE'); }
});
it('registration failures clear passwords and expired verification can restart', async () => {
  vi.useFakeTimers(); let failed=true;
  vi.stubGlobal('fetch',vi.fn(async () => failed ? Response.json({error_code:'INVITE_UNAVAILABLE'},{status:409}) : Response.json(pending)));
  const w=access('register'); await fillRegistration(w); await w.get('form').trigger('submit'); await flushPromises();
  expect(w.text()).toContain('内测码已使用'); expect(w.findAll('input[type=password]').every(i=>(i.element as HTMLInputElement).value==='')).toBe(true);
  failed=false; await fillRegistration(w); await w.get('form').trigger('submit'); await flushPromises();
  vi.advanceTimersByTime(901_000); await flushPromises(); expect(button(w,'完成注册').attributes('disabled')).toBeDefined();
  await button(w,'重新开始注册').trigger('click'); expect(w.findAll('input[type=password]')).toHaveLength(2);
});
it('forgot has a uniform receipt and reset links are consumed from the address before a POST', async () => {
  history.replaceState({},'', '/?account=reset-password#token=mail-secret');
  expect(takeAccountLink()).toEqual({view:'reset-password',token:'mail-secret'}); expect(location.hash).toBe('');
  const fetcher=vi.fn(async()=>Response.json({accepted:true})); vi.stubGlobal('fetch',fetcher);
  const w=access('forgot-password'); await w.get('input').setValue('missing@example.com'); await w.get('form').trigger('submit'); await flushPromises();
  expect(w.text()).toContain('如该邮箱对应可用账号');
  const reset=access('reset-password','mail-secret'); expect(fetcher).toHaveBeenCalledTimes(1);
  for(const input of reset.findAll('input')) await input.setValue('new-password-123');
  await reset.get('form').trigger('submit'); await flushPromises();
  expect(fetcher.mock.calls).toHaveLength(2); expect(reset.emitted('reset')).toHaveLength(1);
  expect(reset.findAll('input').every(i=>(i.element as HTMLInputElement).value==='')).toBe(true);
  expect(JSON.stringify(sessionStorage)+JSON.stringify(localStorage)).not.toContain('mail-secret');
});
it('invalid reset token offers recovery and password validation follows byte lengths', async () => {
  expect(validPassword('中文密码')).toBe(true); expect(validPassword('a'.repeat(1025))).toBe(false);
  vi.stubGlobal('fetch',vi.fn(async()=>Response.json({error_code:'PASSWORD_RESET_INVALID'},{status:409})));
  const w=access('reset-password','old-token');
  for(const input of w.findAll('input')) await input.setValue('new-password-123');
  await w.get('form').trigger('submit'); await flushPromises(); expect(w.text()).toContain('链接已失效');
  await button(w,'重新申请找回密码').trigger('click'); expect(w.emitted('forgot')).toHaveLength(1);
  const missing=access('reset-password'); expect(missing.find('form').exists()).toBe(false); expect(missing.text()).toContain('未找到重置链接');
});
it('personal password changes clear form fields on errors and success emits only for the current identity',async()=>{
  installIdentity({user_id:'A',csrf_token:'csrf'});
  let wrong=true;
  vi.stubGlobal('fetch',vi.fn(async (_url:string,init:RequestInit)=>{
    expect(init.headers).toMatchObject({'X-CSRF-Token':'csrf'});
    return wrong ? Response.json({error_code:'CURRENT_PASSWORD_INVALID'},{status:400}) : Response.json({completed:true});
  }));
  const w=mount(PasswordChange); wrappers.push(w); await button(w,'修改密码').trigger('click');
  const fill=async()=>{ for(const input of w.findAll('input')) await input.setValue('fixture-password'); await w.get('form').trigger('submit'); await flushPromises(); };
  await fill(); expect(w.text()).toContain('当前密码不正确'); expect(network.value.identity?.user_id).toBe('A');
  expect(w.findAll('input').every(i=>(i.element as HTMLInputElement).value==='')).toBe(true);
  wrong=false; await fill(); expect(w.emitted('changed')).toHaveLength(1);
});
