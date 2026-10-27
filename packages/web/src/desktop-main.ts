import { t as uiText } from './i18n';
import { createApp } from 'vue';
import DesktopLibrary from './components/DesktopLibrary.vue';
import 'katex/dist/katex.min.css';
import { initializeLanguage } from './i18n';
import { initializeDesktopLanguage } from './desktop-language';
initializeLanguage(uiText('Understand Book — 本机书库'));
initializeDesktopLanguage();
createApp(DesktopLibrary).mount('#app');
