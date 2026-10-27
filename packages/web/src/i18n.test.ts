// @vitest-environment happy-dom
import { afterEach, expect, it, vi } from 'vitest';
import { mount } from '@vue/test-utils';
import { getLocale, setLocale, t, initializeLanguage, errorMessage, localeStorageKey } from './i18n';
import LanguageSwitcher from './components/LanguageSwitcher.vue';
import AccountAccess from './components/AccountAccess.vue';
import { api } from './api';
import { network } from './network-context';
import { usePdfSelectionTranslation } from './pdf-selection-translation';

afterEach(() => { setLocale('zh-CN'); localStorage.clear(); vi.unstubAllGlobals(); });

it('persists the selected language, localizes text and preserves interpolated source content', () => {
  setLocale('en'); initializeLanguage('understand-book 阅读器');
  expect(getLocale()).toBe('en');
  expect(localStorage.getItem(localeStorageKey)).toBe('en');
  expect(document.documentElement.lang).toBe('en');
  expect(document.title).toBe('Understand Book Reader');
  expect(t('第 {0} 页', [3])).toBe('Page 3');
  expect(t('带我读《{0}》这一章', ['中文原文 {1}'])).toBe('Guide me through the chapter “中文原文 {1}”');
  expect(t('a user-supplied title')).toBe('a user-supplied title');
  expect(errorMessage('尚未翻译的服务错误', 'SERVICE_STOPPING')).toContain('SERVICE_STOPPING');
  expect(errorMessage('请填写有效的邮箱地址。')).toBe('Enter a valid email address.');
  expect(t('检索书内证据')).toBe('Searching book evidence');
  setLocale('zh-CN'); expect(t('第 {0} 页', [3])).toBe('第 3 页');
});

it('renders English registration with a language choice and leaves input untouched', async () => {
  setLocale('en');
  const wrapper = mount(AccountAccess, { props: { view: 'register', token: '' } });
  expect(wrapper.get('h1').text()).toBe('Create a reading account');
  expect(wrapper.findComponent(LanguageSwitcher).exists()).toBe(true);
  expect(wrapper.text()).toContain('Send registration code');
  await wrapper.get('input[type=email]').setValue('reader@example.com');
  expect((wrapper.get('input[type=email]').element as HTMLInputElement).value).toBe('reader@example.com');
  wrapper.unmount();
});

it('freezes the selected output language in each local model request without changing the question', async () => {
  network.value = { ...network.value, enabled: false };
  const fetcher = vi.fn(async () => Response.json({})); vi.stubGlobal('fetch', fetcher);
  setLocale('en'); await api.agentRunCreate('解释这段中文');
  setLocale('zh-CN'); await api.agentChat('Explain this English source');
  const bodies = fetcher.mock.calls.map(call => JSON.parse(String((call as unknown as [unknown, RequestInit])[1].body)));
  expect(bodies[0]).toMatchObject({ message: '解释这段中文', response_language: 'en' });
  expect(bodies[1]).toMatchObject({ message: 'Explain this English source', response_language: 'zh-CN' });
});

it('uses the UI language as the PDF translation target', async () => {
  setLocale('en');
  const translate = vi.fn(async () => ({ translation_markdown: 'Translated text', target_locale: 'en' as const }));
  const controller = usePdfSelectionTranslation(translate);
  await controller.start({ request_id: 'selection', status: 'resolved', raw_quote: '原文', resolved_quote: '原文', ranges: [] });
  expect(translate).toHaveBeenCalledWith(expect.objectContaining({ target_locale: 'en', resolved_quote: '原文' }));
  expect(controller.state.value.translation_markdown).toBe('Translated text');
});
