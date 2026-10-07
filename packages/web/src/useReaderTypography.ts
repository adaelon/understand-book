import { computed, ref, watch, type Ref } from 'vue';
import { normalizeTypography, readTypography, saveTypography, typographyStyle, type ReaderTypographyPreferences } from './reader-typography';

export function useReaderTypography(owner: Ref<string | null>) {
  const preferences = ref(readTypography(owner.value));
  const saveError = ref('');
  const open = ref(false);
  watch(owner, value => { open.value = false; saveError.value = ''; preferences.value = readTypography(value); }, { flush: 'sync' });
  function update(value: ReaderTypographyPreferences) {
    preferences.value = normalizeTypography(value);
    saveError.value = saveTypography(owner.value, preferences.value) ? '' : '当前设置已应用，但未能在本设备记住。';
  }
  return { preferences, saveError, open, update, style: computed(() => typographyStyle(preferences.value)) };
}
