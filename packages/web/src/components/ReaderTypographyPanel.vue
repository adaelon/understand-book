<script setup lang="ts">
import { t as uiText } from '../i18n';

import { onMounted, ref } from 'vue';
import { typographyPresets, type ReaderTypographyPreferences } from '../reader-typography';
const props = defineProps<{ preferences: ReaderTypographyPreferences; saveError: string; pdf?: boolean }>();
const emit = defineEmits<{ change: [value: ReaderTypographyPreferences]; close: [] }>();
const dialog = ref<HTMLDialogElement | null>(null);
onMounted(() => dialog.value?.showModal());
function set<K extends keyof ReaderTypographyPreferences>(key: K, value: ReaderTypographyPreferences[K]) {
  emit('change', { ...props.preferences, [key]: value });
}
function language(event: Event) {
  const mode = (event.target as HTMLSelectElement).value as ReaderTypographyPreferences['languageMode'];
  emit('change', { ...props.preferences, languageMode: mode, fontSizePx: mode === 'latin' ? 18 : 19, lineHeight: mode === 'latin' ? 1.7 : 1.85 });
}
</script>
<template>
  <dialog ref="dialog" class="reader-typography-panel" aria-labelledby="reader-typography-title" @cancel.prevent="emit('close')" @click="($event.target === dialog) && emit('close')">
    <header><h2 id="reader-typography-title">{{ uiText('阅读设置') }}</h2><button autofocus :aria-label="uiText('关闭阅读设置')" @click="emit('close')">{{ uiText('关闭') }}</button></header>
    <p v-if="pdf">{{ uiText('这些设置用于重排正文；PDF 原版保留原始排版。') }}</p>
    <div class="typography-presets" :aria-label="uiText('排版预设')">
      <button v-for="(label, key) in { book: uiText('书页'), technical: uiText('技术'), compact: uiText('紧凑') }" :key="key" @click="emit('change', { ...typographyPresets[key] })">{{ label }}</button>
    </div>
    <label>{{ uiText('字体') }}<select :aria-label="uiText('字体')" :value="preferences.font" @change="set('font', ($event.target as HTMLSelectElement).value as 'serif' | 'sans')"><option value="serif">{{ uiText('书页宋体') }}</option><option value="sans">{{ uiText('清晰黑体') }}</option></select></label>
    <label>{{ uiText('语言排版') }}<select :aria-label="uiText('语言排版')" :value="preferences.languageMode" @change="language"><option value="cjk-mixed">{{ uiText('中文 / 混排') }}</option><option value="latin">{{ uiText('英文') }}</option></select></label>
    <label>{{ uiText('字号') }} <output>{{ preferences.fontSizePx }} px</output><input :aria-label="uiText('字号')" type="range" min="17" max="22" step="1" :value="preferences.fontSizePx" @input="set('fontSizePx', Number(($event.target as HTMLInputElement).value))"></label>
    <label>{{ uiText('行距') }} <output>{{ preferences.lineHeight.toFixed(2) }}</output><input :aria-label="uiText('行距')" type="range" min="1.6" max="2" step="0.05" :value="preferences.lineHeight" @input="set('lineHeight', Number(($event.target as HTMLInputElement).value))"></label>
    <label>{{ uiText('中文字距') }} <output>{{ preferences.cjkLetterSpacingEm.toFixed(3) }} em</output><input :aria-label="uiText('中文字距')" type="range" min="0" max="0.04" step="0.005" :disabled="preferences.languageMode === 'latin'" :value="preferences.cjkLetterSpacingEm" @input="set('cjkLetterSpacingEm', Number(($event.target as HTMLInputElement).value))"></label>
    <label>{{ uiText('版心') }}<select :aria-label="uiText('版心')" :value="preferences.measure" @change="set('measure', ($event.target as HTMLSelectElement).value as ReaderTypographyPreferences['measure'])"><option value="narrow">{{ uiText('窄') }}</option><option value="standard">{{ uiText('标准') }}</option><option value="wide">{{ uiText('宽') }}</option></select></label>
    <p role="status">{{ saveError || uiText('在本设备记住设置，适用于你的所有书籍。') }}</p>
  </dialog>
</template>
<style scoped>
.reader-typography-panel { width: min(390px, calc(100vw - 32px)); max-height: calc(100dvh - 32px); overflow: auto; border: 1px solid var(--hairline); border-radius: 14px; background: var(--reader-card); color: var(--ink); padding: 22px; font: 15px/1.6 var(--sans); }
dialog::backdrop { background: rgb(25 22 18 / 25%); }
header { display: flex; align-items: center; justify-content: space-between; gap: 16px; }
h2 { font-size: 20px; margin: 0; }
label { display: grid; grid-template-columns: 1fr auto; align-items: center; gap: 8px; margin: 14px 0; }
input { grid-column: 1 / -1; width: 100%; accent-color: var(--reader-coral); }
button, select { min-height: 40px; padding: 6px 12px; border: 1px solid var(--hairline); border-radius: 6px; background: var(--reader-card); color: inherit; font: inherit; }
.typography-presets { display: flex; gap: 8px; margin-top: 18px; }
p { color: var(--reader-muted); font-size: 13px; }
</style>
