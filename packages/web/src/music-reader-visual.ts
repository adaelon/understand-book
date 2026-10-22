import { createApp, defineComponent, h, ref } from 'vue';
import MusicReaderPane from './components/MusicReaderPane.vue';
import type { MusicContext, MusicQuestionContext } from './music-api';
import { musicPerformanceSnapshot, recordMusicTiming } from './music-performance';
import './style.css';
const fixture = new URLSearchParams(location.search).get('fixture') || '/api/book/music_context';
const response = await fetch(fixture);
if (!response.ok) throw new Error('请提供真实音乐资料的 context fixture');
const context: MusicContext = await response.json();
createApp(defineComponent({ setup() {
  const question = ref<MusicQuestionContext | null>(null);
  return () => h('main', { style: 'height:100dvh;max-width:1080px;margin:auto;border-inline:1px solid #e1e4d8' }, [
    h(MusicReaderPane, { context, owner: 'visual-review', question: question.value, onQuestion: (value: MusicQuestionContext) => { question.value = value; }, onClearQuestion: () => { question.value = null; } }),
  ]);
} })).mount('#app');
const metrics = document.createElement('output');
metrics.id = 'music-qa-metrics'; metrics.hidden = true; document.body.appendChild(metrics);
if (PerformanceObserver.supportedEntryTypes.includes('longtask')) new PerformanceObserver(list => { for (const entry of list.getEntries()) recordMusicTiming('browser_long_task', entry.duration); }).observe({ type: 'longtask', buffered: true });
setInterval(() => {
  const audio = document.querySelector('audio');
  metrics.textContent = JSON.stringify({ viewport: { width: innerWidth, height: innerHeight }, pages: document.querySelectorAll('.music-score-page > svg').length,
    objects: context.manifest.score_objects.length, recording_kind: context.manifest.recording.kind, audio: audio ? { current_ms: Math.round(audio.currentTime * 1000), paused: audio.paused, ready_state: audio.readyState } : null,
    timing: musicPerformanceSnapshot(), navigation: performance.getEntriesByType('navigation').map(entry => ({ duration_ms: entry.duration })),
    svg_resources: performance.getEntriesByType('resource').filter(entry => entry.name.endsWith('.svg')).map(entry => ({ duration_ms: entry.duration })) });
}, 500);
