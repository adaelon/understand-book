import { createApp } from 'vue';
import LocalPdfReader from '../../src/components/LocalPdfReader.vue';
const style = document.createElement('style');
style.textContent = 'html,body,#app{height:100%;margin:0}body{font-family:system-ui,sans-serif;background:#fcfaf6}#app{display:flex;flex-direction:column}';
document.head.appendChild(style);
createApp(LocalPdfReader, { url: '/test-source.pdf', materialId: 'fixture-pdf' }).mount('#app');
