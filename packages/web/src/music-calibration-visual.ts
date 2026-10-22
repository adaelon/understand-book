import { createApp, defineComponent, h, ref } from 'vue';
import MusicCalibrationPane from './components/MusicCalibrationPane.vue';
import { api } from './api';
import type { MusicAlignmentRevision } from './music-calibration';
import './style.css';
const initial = await api.musicCalibrationContext();
if (!initial) throw new Error('此入口需要当前本地、未发布的真实音乐工作区。');
createApp(defineComponent({ setup() {
  const context = ref(initial), saving = ref(false), error = ref('');
  async function save(revision: MusicAlignmentRevision) {
    saving.value = true; error.value = '';
    try { context.value = await api.musicCalibrate(revision); }
    catch (failure) { error.value = (failure as Error).message; }
    finally { saving.value = false; }
  }
  return () => h(MusicCalibrationPane, { context: context.value, saving: saving.value, saveError: error.value, onSave: save });
} })).mount('#app');
