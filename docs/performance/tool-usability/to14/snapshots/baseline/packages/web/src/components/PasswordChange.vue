<script setup lang="ts">
import { onBeforeUnmount, ref } from 'vue';
import { networkFetch } from '../network-client';
import { accountError, passwordHint, validPassword } from '../account-forms';
const emit = defineEmits<{ (e: 'changed'): void }>();
const open = ref(false), current = ref(''), password = ref(''), confirm = ref(''), error = ref(''), busy = ref(false);
let active = true;
async function submit() {
  error.value = '';
  if (!validPassword(password.value)) { error.value = passwordHint; return; }
  if (password.value !== confirm.value) { error.value = '两次输入的密码不一致。'; return; }
  const body = { current_password: current.value, new_password: password.value };
  current.value = ''; password.value = ''; confirm.value = ''; busy.value = true;
  try { await networkFetch('POST', '/account/password', body); if (active) emit('changed'); }
  catch (failure) { if (active) error.value = accountError(failure); }
  finally { if (active) busy.value = false; }
}
onBeforeUnmount(() => { active = false; current.value = ''; password.value = ''; confirm.value = ''; });
</script>
<template>
  <section>
    <button v-if="!open" @click="open = true">修改密码</button>
    <form v-else @submit.prevent="submit">
      <h3>修改密码</h3><p>修改后所有旧登录会话将退出，请使用新密码重新登录。</p>
      <label>当前密码<input v-model="current" type="password" autocomplete="current-password" required :disabled="busy"></label>
      <label>新密码<input v-model="password" type="password" autocomplete="new-password" required :disabled="busy"></label>
      <p>{{ passwordHint }}</p>
      <label>确认新密码<input v-model="confirm" type="password" autocomplete="new-password" required :disabled="busy"></label>
      <button :disabled="busy">保存新密码</button>
      <p v-if="error" role="alert">{{ error }}</p>
    </form>
  </section>
</template>
<style scoped>
section { border-top: 1px solid var(--line, #ddd); margin-top: 20px; padding-top: 12px; } label { display: grid; gap: 6px; margin: 16px 0; } input { box-sizing: border-box; width: 100%; padding: 10px; font: inherit; } button { padding: 8px 12px; cursor: pointer; } [role="alert"] { color: #a32620; }
</style>
