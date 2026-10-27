import { t as uiText } from './i18n';
export interface DesktopAuthorizationRequest { redirect_uri: string; code_challenge: string; state: string }
export function desktopAuthorization(search: string): DesktopAuthorizationRequest | null {
  const params = new URLSearchParams(search);
  if (params.get('desktop_authorize') !== '1') return null;
  const redirect_uri = params.get('redirect_uri') ?? '';
  const code_challenge = params.get('code_challenge') ?? '', state = params.get('state') ?? '';
  const callback = new URL(redirect_uri);
  if (callback.protocol !== 'http:' || callback.hostname !== '127.0.0.1' || !callback.port
    || callback.pathname !== '/callback' || callback.search || callback.hash || callback.username || callback.password
    || !/^[A-Za-z0-9_-]{43}$/.test(code_challenge) || !/^[A-Za-z0-9_-]{43}$/.test(state)) {
    throw new Error(uiText('桌面登录链接已失效，请返回应用重新登录。'));
  }
  return { redirect_uri, code_challenge, state };
}
export function cancelDesktopAuthorization(request: DesktopAuthorizationRequest): string {
  const callback = new URL(request.redirect_uri);
  callback.searchParams.set('error', 'access_denied'); callback.searchParams.set('state', request.state);
  return callback.toString();
}
