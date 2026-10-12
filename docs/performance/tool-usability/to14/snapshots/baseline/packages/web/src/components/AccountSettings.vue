<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from 'vue';
import { ApiError } from '../api';
import { networkFetch } from '../network-client';
import { network, sceneKey } from '../network-context';
import PasswordChange from './PasswordChange.vue';

const emit = defineEmits<{ (e: 'close'): void; (e: 'bound'): void; (e: 'password-changed'): void }>();
const dialog = ref<HTMLDialogElement | null>(null);
const email = ref(''), password = ref(''), code = ref(''), error = ref(''), notice = ref('');
const busy = ref(false), pending = ref<{ request_id: string; expires_at: number } | null>(null);
const now = ref(Date.now()), resendAt = ref(0);
const wait = computed(() => Math.max(0, Math.ceil((resendAt.value - now.value) / 1000)));
const expired = computed(() => pending.value && pending.value.expires_at * 1000 <= now.value);
let active = true, timer: ReturnType<typeof setInterval>;
const messages: Record<string, string> = {
  CURRENT_PASSWORD_INVALID: '当前密码不正确，请重新输入。',
  LOGIN_RATE_LIMITED: '密码验证过于频繁，请稍后重试。',
  ACCOUNT_EMAIL_INVALID: '请填写有效的邮箱地址。',
  ACCOUNT_EMAIL_IN_USE: '该邮箱已绑定其他账号，请使用其他邮箱。',
  ACCOUNT_EMAIL_ALREADY_BOUND: '账号已有验证邮箱，请关闭设置后刷新登录信息。',
  EMAIL_BINDING_NOT_FOUND: '申请已被新的申请替换，请重新开始。',
  EMAIL_BINDING_EXPIRED: '申请已失效，请重新开始绑定。',
  EMAIL_BINDING_CODE_INVALID: '验证码不正确，请核对邮件。',
  EMAIL_BINDING_ATTEMPTS_EXCEEDED: '错误次数已达上限，请重发验证码。',
  ACCOUNT_MAIL_RATE_LIMITED: '发送过于频繁，请稍后重试。',
  ACCOUNT_MAIL_UNAVAILABLE: '邮件服务暂不可用，请稍后重试。',
  ACCOUNT_MAIL_REJECTED: '邮件发送失败，可稍后重发验证码。',
  ACCOUNT_MAIL_PROVIDER_RATE_LIMITED: '邮件服务繁忙，请稍后重发。',
  ACCOUNT_MAIL_TIMEOUT: '邮件发送超时，可能已送达。可输入收到的验证码，或稍后重发。',
  ACCOUNT_MAIL_DELIVERY_UNKNOWN: '邮件发送结果尚未确认。可输入收到的验证码，或稍后重发。',
};
function remember(value: { request_id?: string; expires_at?: number; resend_after?: number }) {
  if (value.request_id && typeof value.expires_at === 'number') {
    pending.value = { request_id: value.request_id, expires_at: value.expires_at };
    resendAt.value = Date.now() + (value.resend_after ?? 60) * 1000;
    code.value = '';
  }
}
async function submit(action: 'start' | 'resend' | 'complete') {
  const scope = sceneKey();
  busy.value = true; error.value = ''; notice.value = '';
  const body = action === 'start' ? { email: email.value, current_password: password.value }
    : action === 'resend' ? { request_id: pending.value!.request_id }
    : { request_id: pending.value!.request_id, verification_code: code.value };
  password.value = '';
  try {
    const result = await networkFetch<{ request_id?: string; expires_at?: number; resend_after?: number; completed?: boolean; email?: string; user_id?: string }>('POST', `/account/email/${action}`, body);
    if (!active || scope !== sceneKey()) return;
    const identity = network.value.identity;
    if (result.completed && identity && result.user_id === identity.user_id && result.email) {
      network.value = { ...network.value, identity: { ...identity, email: result.email } };
      pending.value = null; code.value = ''; notice.value = '邮箱已绑定，可以使用此邮箱或原账号登录。'; emit('bound');
    } else { remember(result); notice.value = '验证码已发送，15 分钟内有效。'; }
  } catch (failure) {
    if (!active || scope !== sceneKey()) return;
    if (failure instanceof ApiError) {
      if (failure.details) remember(failure.details);
      error.value = messages[failure.errorCode] ?? '请求未完成，请检查连接后重试。';
    } else error.value = '请求结果未确认，请检查连接后重试。';
  } finally { if (active) busy.value = false; }
}
function restart() { pending.value = null; code.value = ''; password.value = ''; error.value = ''; notice.value = ''; }
onMounted(() => { dialog.value?.showModal(); timer = setInterval(() => { now.value = Date.now(); }, 1000); });
onBeforeUnmount(() => { active = false; clearInterval(timer); password.value = ''; dialog.value?.close(); });
</script>

<template>
  <dialog ref="dialog" class="account-settings" aria-labelledby="account-settings-title" @cancel.prevent="emit('close')">
    <header><h2 id="account-settings-title">个人设置</h2><button type="button" @click="emit('close')">关闭</button></header>
    <p>账号：{{ network.identity?.user_id }}</p>
    <p v-if="network.identity?.email">已验证邮箱：{{ network.identity.email }}</p>
    <template v-else>
      <h3>绑定邮箱</h3><p>验证后可使用邮箱登录，保留当前账号的阅读记录、材料与额度。</p>
      <form v-if="!pending" @submit.prevent="submit('start')">
        <label>邮箱<input v-model="email" type="email" autocomplete="email" maxlength="254" required :disabled="busy"></label>
        <label>当前密码<input v-model="password" type="password" autocomplete="current-password" required :disabled="busy"></label>
        <button :disabled="busy">发送验证码</button>
      </form>
      <template v-else>
        <p>验证邮箱：{{ email }}</p>
        <p v-if="expired" role="status">验证码已过期，请重新开始绑定。</p>
        <form @submit.prevent="submit('complete')">
          <label>邮箱验证码<input v-model="code" inputmode="numeric" autocomplete="one-time-code" pattern="[0-9]{6}" maxlength="6" required :disabled="busy || !!expired"></label>
          <button :disabled="busy || !!expired">确认绑定</button>
        </form>
        <button type="button" :disabled="busy || wait > 0 || !!expired" @click="submit('resend')">{{ wait > 0 ? `${wait} 秒后可重发` : '重发验证码' }}</button>
        <button type="button" :disabled="busy" @click="restart">重新开始绑定</button>
      </template>
    </template>
    <p v-if="error" role="alert">{{ error }}</p><p v-if="notice" role="status">{{ notice }}</p>
    <PasswordChange v-if="!busy && !pending" @changed="emit('password-changed')" />
  </dialog>
</template>

<style scoped>
.account-settings { width: min(30rem, calc(100vw - 2rem)); max-height: calc(100dvh - 2rem); overflow: auto; box-sizing: border-box; border: 1px solid #ccc; border-radius: 16px; padding: 24px; color: var(--text-primary, #252525); background: var(--bg-primary, #fff); overflow-wrap: anywhere; }
.account-settings::backdrop { background: #0006; }
header { display: flex; justify-content: space-between; align-items: center; gap: 12px; }
h2 { margin: 0; } label { display: grid; gap: 6px; margin: 16px 0; } input { box-sizing: border-box; width: 100%; padding: 10px; font: inherit; } button { padding: 8px 12px; margin: 4px 6px 4px 0; cursor: pointer; } [role="alert"] { color: #a32620; }
</style>
