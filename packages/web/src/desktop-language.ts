import { invoke, isTauri } from '@tauri-apps/api/core';
import { getLocale } from './i18n';

/** Keep native menus and navigation between local and cloud origins in the selected language. */
export function initializeDesktopLanguage(): void {
  if (isTauri()) void invoke('desktop_set_language', { language: getLocale() }).catch(console.error);
}
