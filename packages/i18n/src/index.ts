import english from './en.json';

export type Locale = 'zh-CN' | 'en';
export const localeStorageKey = 'understand-book.locale';
const catalog: Readonly<Record<string, string>> = english;

function initialLocale(): Locale {
  if (typeof window === 'undefined') return 'zh-CN';
  const requested = new URLSearchParams(window.location.search).get('lang');
  if (requested === 'en' || requested === 'zh-CN') {
    try { window.localStorage.setItem(localeStorageKey, requested); } catch { /* The URL still selects the language when storage is unavailable. */ }
    return requested;
  }
  try { return window.localStorage.getItem(localeStorageKey) === 'en' ? 'en' : 'zh-CN'; }
  catch { return 'zh-CN'; }
}

let locale: Locale = initialLocale();
if (typeof document !== 'undefined') document.documentElement.lang = locale;
export function getLocale(): Locale { return locale; }

/** Source-language keys keep the Chinese copy alongside its call site. */
export function t(source: string, values: readonly unknown[] = []): string {
  const message = locale === 'en' ? catalog[source] ?? source : source;
  return message.replace(/\{(\d+)\}/g, (placeholder, index: string) =>
    Number(index) < values.length ? String(values[Number(index)] ?? '') : placeholder);
}

export function setLocale(next: Locale): void {
  locale = next;
  if (typeof window !== 'undefined') {
    try { window.localStorage.setItem(localeStorageKey, next); } catch { /* The language also travels in the page URL. */ }
    document.documentElement.lang = next;
  }
}

/** Reload also refreshes module-level labels and keeps every frontend in one language. */
export function switchLocale(next: Locale, resetToken?: string): void {
  setLocale(next);
  const url = new URL(window.location.href);
  url.searchParams.set('lang', next);
  if (resetToken) url.hash = new URLSearchParams({ token: resetToken }).toString();
  window.location.assign(url.href);
}

export function initializeLanguage(title: string, description?: string): void {
  document.documentElement.lang = locale;
  document.title = t(title);
  document.querySelector('.app-bootstrap [role="status"]')?.setAttribute('aria-label', locale === 'en' ? 'Loading' : '正在加载');
  if (description) document.querySelector('meta[name="description"]')?.setAttribute('content', t(description));
}

/** Server diagnostics keep their code; untranslated Chinese must not become English UI copy. */
export function errorMessage(message: string, code?: string): string {
  const translated = t(message);
  if (locale !== 'en' || !/[\u3400-\u9fff]/.test(translated)) return translated;
  return `The request could not be completed${code ? ` (${code})` : ''}. Please try again.`;
}

export function formatLocale(): string { return locale === 'en' ? 'en-US' : 'zh-CN'; }

/** Inline presentation frames have an opaque origin and receive only their small UI catalog. */
export function translationsFor(sources: readonly string[]): Record<string, string> {
  return Object.fromEntries(sources.map(source => [source, t(source)]));
}
