<script setup lang="ts">
import { t as uiText } from '../i18n';

import { inject, ref, watch } from 'vue';
import { api as sharedApi } from '../api';
import { bindSceneApi } from '../network-context';
const api = bindSceneApi(sharedApi);
import { sharePresentationKey } from '../reading-share';
import { captureRetainedPresentation, presentationShareSource } from '../presentation-share';
import type { PresentationFollowUp } from '../generated/PresentationFollowUp';
const props = defineProps<{ receipt: PresentationFollowUp; memId?: string }>();
const emit = defineEmits<{ (e: 'locate'): void; (e: 'restore'): void }>();
const summary = ref('');
const sharePresentation = inject(sharePresentationKey, null);
const exporting = ref(false), exportError = ref('');
async function shareRecorded() {
  if (!sharePresentation || !props.memId || exporting.value) return;
  const memId = props.memId, turnId = props.receipt.turn_id;
  exporting.value = true; exportError.value = '';
  try {
    await sharePresentation(async () => {
      const view = await api.notePresentationRead(memId, true);
      const scene = await captureRetainedPresentation(view, (text, ids) => api.notePresentationObserve(memId, text, ids));
      return presentationShareSource(view, scene, id => api.agentSourceResolve(turnId, id, memId), true);
    });
  } catch (failure) { if (memId === props.memId) exportError.value = failure instanceof Error ? failure.message : String(failure); }
  finally { exporting.value = false; }
}
watch(() => props.memId, () => { exportError.value = ''; });
watch(() => props.receipt, async receipt => {
  summary.value = '';
  try {
    const view = props.memId ? await api.notePresentationRead(props.memId, true) : await api.presentationRead(receipt.session_id, receipt.turn_id, receipt.reference, receipt);
    if (props.receipt.saved_state_ref !== receipt.saved_state_ref) return;
    summary.value = [view.restored_state?.visible_step, view.restored_state?.observed_result].filter(Boolean).join(' · ').slice(0, 240);
  } catch { summary.value = uiText('提问时的演示状态暂时无法读取'); }
}, { immediate: true });
</script>
<template>
  <div class="scene-binding">
    <span>{{ memId ? uiText('记录时的演示') : uiText('提问时的演示') }}</span>
    <p v-if="summary">{{ summary }}</p>
    <button @click="emit('locate')">{{ memId ? uiText('打开演示') : uiText('定位到演示') }}</button>
    <button v-if="memId && sharePresentation" :disabled="exporting" @click="shareRecorded">{{ exporting ? uiText('正在恢复并取图…') : uiText('分享记录时的图解') }}</button>
    <p v-if="exportError" role="alert">{{ exportError }}</p>
    <button @click="emit('restore')">{{ memId ? uiText('回到记录时') : uiText('回到提问时') }}</button>
  </div>
</template>
