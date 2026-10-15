import { ApiError } from './api';

export const passwordHint = '密码长度为 12–1024 字节（英文字符每个 1 字节，中文通常每字 3 字节）。';
export function validPassword(value: string) { const length = new TextEncoder().encode(value).length; return length >= 12 && length <= 1024; }
export type AccountView = 'register' | 'forgot-password' | 'reset-password';
export function takeAccountLink(): { view: AccountView | null; token: string } {
  const url = new URL(location.href), value = url.searchParams.get('account');
  const view = ['register', 'forgot-password', 'reset-password'].includes(value ?? '') ? value as AccountView : null;
  const token = view === 'reset-password' ? new URLSearchParams(url.hash.slice(1)).get('token') ?? '' : '';
  if (view === 'reset-password') { url.hash = ''; history.replaceState(history.state, '', url); }
  return { view, token };
}
const messages: Record<string, string> = {
  PASSWORD_INVALID: passwordHint,
  CURRENT_PASSWORD_INVALID: '当前密码不正确，请重新输入。',
  LOGIN_RATE_LIMITED: '密码验证过于频繁，请稍后重试。',
  PASSWORD_RATE_LIMITED: '密码请求过于频繁，请稍后重试。',
  PASSWORD_RESET_INVALID: '链接已失效或已使用，请重新申请找回密码。',
  ACCOUNT_EMAIL_INVALID: '请填写有效的邮箱地址。',
  ACCOUNT_EMAIL_IN_USE: '该邮箱已有账号，请登录或找回密码。',
  INVITE_INVALID: '请核对内测码格式。',
  INVITE_UNAVAILABLE: '内测码已使用、已停用或不存在，请联系管理员。',
  REGISTRATION_EXPIRED: '注册申请已过期，请重新开始。',
  REGISTRATION_NOT_FOUND: '注册申请不存在，请重新开始。',
  REGISTRATION_CODE_INVALID: '验证码不正确，请核对邮件。',
  REGISTRATION_ATTEMPTS_EXCEEDED: '错误次数已达上限，请重发验证码。',
  REGISTRATION_RATE_LIMITED: '注册请求过于频繁，请稍后重试。',
  ACCOUNT_MAIL_RATE_LIMITED: '发送过于频繁，请等待 60 秒后重试。',
  ACCOUNT_MAIL_UNAVAILABLE: '邮件服务暂不可用，请稍后重试。',
  ACCOUNT_MAIL_REJECTED: '邮件发送失败，请稍后重发。',
  ACCOUNT_MAIL_PROVIDER_RATE_LIMITED: '邮件服务繁忙，请稍后重发。',
  ACCOUNT_MAIL_TIMEOUT: '邮件发送超时，可能已送达。请查看邮箱，或等待 60 秒后重发。',
  ACCOUNT_MAIL_DELIVERY_UNKNOWN: '邮件发送结果尚未确认，请查看邮箱，或等待 60 秒后重发。',
};
export function accountError(failure: unknown) {
  return failure instanceof ApiError ? messages[failure.errorCode] ?? '请求未完成，请检查连接后重试。'
    : '请求结果未确认，请检查连接。若刚提交新密码，可返回登录尝试新密码或重新找回。';
}
