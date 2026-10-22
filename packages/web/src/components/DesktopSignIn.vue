<script setup lang="ts">
import { ref } from 'vue';
import { isTauri, invoke } from '@tauri-apps/api/core';
const available = isTauri(), busy = ref(false), error = ref('');
async function signIn() {
  if (busy.value) return; busy.value = true; error.value = '';
  try { await invoke('desktop_sign_in'); }
  catch (failure) { error.value = String(failure); }
  finally { busy.value = false; }
}
async function cancel() {
  try { await invoke('desktop_cancel_sign_in'); }
  catch (failure) { error.value = String(failure); }
}
</script>
<template>
  <div v-if="available" class="desktop-sign-in">
    <button :disabled="busy" @click="signIn">{{ busy ? '请在系统浏览器中确认登录…' : '使用网站账号登录' }}</button>
    <button v-if="busy" @click="cancel">取消</button>
    <p v-if="error" role="alert">{{ error }}</p>
  </div>
</template>
<style scoped>
.desktop-sign-in { display: grid; gap: 12px; } button { padding: 12px 18px; cursor: pointer; } p { color: #a32f28; }
</style>
