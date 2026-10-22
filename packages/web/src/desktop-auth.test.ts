import { describe, expect, it } from 'vitest';
import { cancelDesktopAuthorization, desktopAuthorization } from './desktop-auth';
const valid = new URLSearchParams({ desktop_authorize: '1', redirect_uri: 'http://127.0.0.1:49152/callback', code_challenge: 'A'.repeat(43), state: 'B'.repeat(43) });
describe('native browser authorization', () => {
  it('preserves the callback and state for consent and cancellation', () => {
    const request = desktopAuthorization(`?${valid}`)!;
    const cancelled = new URL(cancelDesktopAuthorization(request));
    expect(cancelled.origin).toBe('http://127.0.0.1:49152');
    expect(cancelled.searchParams.get('state')).toBe('B'.repeat(43));
    expect(cancelled.searchParams.get('error')).toBe('access_denied');
    expect(desktopAuthorization('?account=login')).toBeNull();
  });
  it('does not send browser consent to external or malformed callbacks', () => {
    for (const redirect_uri of ['https://example.com/callback', 'http://localhost:49152/callback', 'http://127.0.0.1:49152/other']) {
      const params = new URLSearchParams(valid); params.set('redirect_uri', redirect_uri);
      expect(() => desktopAuthorization(`?${params}`)).toThrow();
    }
    const params = new URLSearchParams(valid); params.set('code_challenge', '');
    expect(() => desktopAuthorization(`?${params}`)).toThrow();
  });
});
