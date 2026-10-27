import { t as uiText } from './i18n';
import { createApp } from 'vue';
import ProductHome from './components/ProductHome.vue';
import './home.css';
import { initializeLanguage } from './i18n';

initializeLanguage(uiText('Understand Book — 阅读、视频与音乐的学习工作区'), uiText('Understand Book，把阅读、视频与音乐放进围绕目标的学习工作区。对照原文提问、框选视频画面、看谱听音乐，让材料、笔记与对话接起每一次理解。'));

createApp(ProductHome).mount('#home');

// The public root doubles as the returning reader's entry. Explicit homepage
// and section links remain available for browsing the product introduction.
if (location.pathname === '/' && !location.hash) {
  void fetch('/api/auth/me', { credentials: 'same-origin', cache: 'no-store' })
    .then(response => { if (response.ok && !location.hash) location.replace('/index.html'); })
    .catch(() => {});
}
