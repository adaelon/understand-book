<script setup lang="ts">
import { t as uiText, errorMessage } from '@understand-book/web/reader/i18n';

import { ref } from 'vue';
import { isTauri, invoke } from '@tauri-apps/api/core';
const available = isTauri(), busy = ref(false), error = ref('');
async function signIn() {
  if (busy.value) return; busy.value = true; error.value = '';
  try { await invoke('desktop_sign_in'); }
  catch (failure) { error.value = errorMessage(String(failure)); }
  finally { busy.value = false; }
}
async function cancel() {
  try { await invoke('desktop_cancel_sign_in'); }
  catch (failure) { error.value = errorMessage(String(failure)); }
}
</script>
<template>
  <div v-if="available" class="desktop-sign-in">
    <button :disabled="busy" @click="signIn">{{ busy ? uiText('请在系统浏览器中确认登录…') : uiText('使用网站账号登录') }}</button>
    <button v-if="busy" @click="cancel">{{ uiText('取消') }}</button>
    <p v-if="error" role="alert">{{ error }}</p>
  </div>
</template>
<style scoped>
.desktop-sign-in { display: grid; gap: 12px; } button { padding: 12px 18px; cursor: pointer; } p { color: #a32f28; }
</style>
