import { createApp } from 'vue';
import { installAppViewportHeightFallback } from './app-viewport-height';
import MobileLearningPreview from './components/MobileLearningPreview.vue';
import './style.css';

installAppViewportHeightFallback();
createApp(MobileLearningPreview).mount('#app');
