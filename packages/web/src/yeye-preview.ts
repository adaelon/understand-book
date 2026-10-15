import { createApp } from 'vue';
import YeyePreview from './components/YeyePreview.vue';

// The production smoke build uses the Reader's stylesheet preload transform.
document.querySelectorAll<HTMLLinkElement>('link[data-app-styles]').forEach(style => {
  style.rel = 'stylesheet';
});
createApp(YeyePreview).mount('#app');
