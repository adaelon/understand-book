<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from 'vue';
import { ApiError } from '../api';
import { networkFetch } from '../network-client';
import { accountError, passwordHint, validPassword, type AccountView } from '../account-forms';

const props = defineProps<{ view: AccountView; token: string }>();
const emit = defineEmits<{ (e: 'login', email?: string, notice?: string): void; (e: 'reset'): void; (e: 'forgot'): void }>();
const email = ref(''), password = ref(''), confirm = ref(''), invite = ref(''), code = ref('');
const error = ref(''), notice = ref(''), busy = ref(false);
const pending = ref<{ request_id: string; expires_at: number } | null>(null);
const now = ref(Date.now()), resendAt = ref(0);
const wait = computed(() => Math.max(0, Math.ceil((resendAt.value - now.value) / 1000)));
const expired = computed(() => !!pending.value && pending.value.expires_at * 1000 <= now.value);
const title = computed(() => props.view === 'register' ? (pending.value ? '验证邮箱' : '创建阅读账号') : props.view === 'forgot-password' ? '找回密码' : '设置新密码');
let active = true, timer: ReturnType<typeof setInterval>;
function remember(value: { request_id?: string; expires_at?: number; resend_after?: number }) {
  if (value.request_id && typeof value.expires_at === 'number') {
    pending.value = { request_id: value.request_id, expires_at: value.expires_at };
    resendAt.value = Date.now() + (value.resend_after ?? 60) * 1000; code.value = ''; invite.value = '';
  }
}
async function submit(action: 'start' | 'resend' | 'complete' | 'forgot' | 'reset') {
  if (busy.value) return;
  error.value = ''; notice.value = '';
  if (action === 'start' || action === 'reset') {
    if (!validPassword(password.value)) { error.value = passwordHint; return; }
    if (password.value !== confirm.value) { error.value = '两次输入的密码不一致。'; return; }
  }
  const body = action === 'start' ? { email: email.value, password: password.value, invite_code: invite.value }
    : action === 'forgot' ? { email: email.value }
    : action === 'reset' ? { token: props.token, new_password: password.value }
    : action === 'resend' ? { request_id: pending.value!.request_id }
    : { request_id: pending.value!.request_id, verification_code: code.value };
  password.value = ''; confirm.value = ''; busy.value = true;
  try {
    const result = await networkFetch<{ request_id?: string; expires_at?: number; resend_after?: number; completed?: boolean }>('POST',
      action === 'forgot' || action === 'reset' ? `/auth/password/${action}` : `/auth/register/${action}`, body);
    if (!active) return;
    if (action === 'reset') emit('reset');
    else if (action === 'forgot') { resendAt.value = Date.now() + 60_000; notice.value = '如该邮箱对应可用账号，将收到重置邮件。链接 30 分钟内有效；重新申请后请使用最新邮件。'; }
    else if (result.completed) { pending.value = null; code.value = ''; emit('login', email.value, '注册成功，请登录。材料与使用额度由管理员开通。'); }
    else { remember(result); notice.value = '验证码已发送，15 分钟内有效。'; }
  } catch (failure) {
    if (!active) return;
    if (failure instanceof ApiError && failure.details) remember(failure.details);
    if (action === 'forgot') resendAt.value = Date.now() + 60_000;
    error.value = accountError(failure);
  } finally { if (active) busy.value = false; }
}
function restart() { pending.value = null; code.value = ''; password.value = ''; confirm.value = ''; invite.value = ''; error.value = ''; notice.value = ''; }
onMounted(() => { timer = setInterval(() => { now.value = Date.now(); }, 1000); });
onBeforeUnmount(() => { active = false; clearInterval(timer); password.value = ''; confirm.value = ''; invite.value = ''; });
</script>

<template>
  <main class="account-access">
    <p class="eyebrow">UNDERSTAND BOOK</p><h1>{{ title }}</h1>
    <template v-if="view === 'register'">
      <form v-if="!pending" @submit.prevent="submit('start')">
        <p>使用管理员提供的一次性内测码注册，验证邮箱后即可登录。</p>
        <label>邮箱<input v-model="email" type="email" autocomplete="email" maxlength="254" required :disabled="busy"></label>
        <label>密码<input v-model="password" type="password" autocomplete="new-password" required :disabled="busy"></label>
        <p class="hint">{{ passwordHint }}</p>
        <label>确认密码<input v-model="confirm" type="password" autocomplete="new-password" required :disabled="busy"></label>
        <label>内测码<input v-model="invite" autocomplete="off" required :disabled="busy"></label>
        <button :disabled="busy">发送注册验证码</button>
      </form>
      <template v-else>
        <p>验证码发送至 {{ email }}。提交后完成注册。</p>
        <p v-if="expired" role="status">注册申请已过期，请重新开始。</p>
        <form @submit.prevent="submit('complete')">
          <label>邮箱验证码<input v-model="code" inputmode="numeric" autocomplete="one-time-code" pattern="[0-9]{6}" maxlength="6" required :disabled="busy || expired"></label>
          <button :disabled="busy || expired">完成注册</button>
        </form>
        <button :disabled="busy || wait > 0 || expired" @click="submit('resend')">{{ wait > 0 ? `${wait} 秒后可重发` : '重发验证码' }}</button>
        <button :disabled="busy" @click="restart">重新开始注册</button>
      </template>
    </template>
    <form v-else-if="view === 'forgot-password'" @submit.prevent="submit('forgot')">
      <p>输入已验证的账号邮箱。未绑定邮箱的老账号请联系管理员恢复。</p>
      <label>邮箱<input v-model="email" type="email" autocomplete="email" maxlength="254" required :disabled="busy"></label>
      <button :disabled="busy || wait > 0">{{ wait > 0 ? `${wait} 秒后可重发` : '发送重置邮件' }}</button>
    </form>
    <template v-else>
      <form v-if="token" @submit.prevent="submit('reset')">
        <p>提交新密码后，账号所有旧登录会话将退出。</p>
        <label>新密码<input v-model="password" type="password" autocomplete="new-password" required :disabled="busy"></label>
        <p class="hint">{{ passwordHint }}</p>
        <label>确认新密码<input v-model="confirm" type="password" autocomplete="new-password" required :disabled="busy"></label>
        <button :disabled="busy">重置密码</button>
      </form>
      <p v-else role="alert">未找到重置链接。请从邮件重新打开，或重新申请找回密码。</p>
      <button :disabled="busy" @click="emit('forgot')">重新申请找回密码</button>
    </template>
    <p v-if="error" role="alert">{{ error }}</p><p v-if="notice" role="status">{{ notice }}</p>
    <button :disabled="busy" @click="emit('login')">返回登录</button>
  </main>
</template>
<style scoped>
.account-access { box-sizing: border-box; width: min(100%, 32rem); max-height: 100%; margin: 0 auto; padding: clamp(24px, 6vh, 64px) 24px; overflow: auto; overflow-wrap: anywhere; }
.eyebrow { font-size: 12px; letter-spacing: .12em; color: var(--muted); } h1 { font-size: 28px; }
label { display: grid; gap: 8px; margin: 18px 0; } input { box-sizing: border-box; width: 100%; min-width: 0; padding: 12px; font: inherit; border: 1px solid var(--line, #bbb); border-radius: 6px; }
button { padding: 10px 16px; margin: 6px 8px 6px 0; font: inherit; cursor: pointer; } .hint { font-size: 13px; color: var(--muted); } [role="alert"] { color: #a32620; }
</style>
