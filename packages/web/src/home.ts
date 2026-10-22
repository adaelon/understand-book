import { createApp } from 'vue';
import ProductHome from './components/ProductHome.vue';
import './home.css';

createApp(ProductHome).mount('#home');

// The public root doubles as the returning reader's entry. Explicit homepage
// and section links remain available for browsing the product introduction.
if (location.pathname === '/' && !location.hash) {
  void fetch('/api/auth/me', { credentials: 'same-origin', cache: 'no-store' })
    .then(response => { if (response.ok && !location.hash) location.replace('/index.html'); })
    .catch(() => {});
}
