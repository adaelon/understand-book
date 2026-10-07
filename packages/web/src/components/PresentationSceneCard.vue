<script setup lang="ts">
import { ref, watch } from 'vue';
import { api as sharedApi } from '../api';
import { bindSceneApi } from '../network-context';
const api = bindSceneApi(sharedApi);
import type { PresentationFollowUp } from '../generated/PresentationFollowUp';
const props = defineProps<{ receipt: PresentationFollowUp }>();
const emit = defineEmits<{ (e: 'locate'): void; (e: 'restore'): void }>();
const summary = ref('');
watch(() => props.receipt, async receipt => {
  summary.value = '';
  try {
    const view = await api.presentationRead(receipt.session_id, receipt.turn_id, receipt.reference, receipt);
    if (props.receipt.saved_state_ref !== receipt.saved_state_ref) return;
    summary.value = [view.restored_state?.visible_step, view.restored_state?.observed_result].filter(Boolean).join(' · ').slice(0, 240);
  } catch { summary.value = '提问现场暂时无法读取'; }
}, { immediate: true });
</script>
<template>
  <div class="scene-binding">
    <span>提问现场 · 版本 {{ receipt.reference.revision }} · 快照 {{ receipt.state_revision }}</span>
    <p v-if="summary">{{ summary }}</p>
    <button @click="emit('locate')">定位到演示</button>
    <button @click="emit('restore')">回到提问时</button>
  </div>
</template>
