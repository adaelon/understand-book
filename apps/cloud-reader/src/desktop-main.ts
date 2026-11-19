import { t as uiText } from '@understand-book/web/reader/i18n';
import { createApp } from 'vue';
import DesktopLibrary from './components/DesktopLibrary.vue';
import 'katex/dist/katex.min.css';
import { initializeLanguage } from '@understand-book/web/reader/i18n';
import { initializeDesktopLanguage } from '@understand-book/web/reader/desktop-language';
initializeLanguage(uiText('Understand Book — 本机书库'));
initializeDesktopLanguage();
createApp(DesktopLibrary).mount('#app');
