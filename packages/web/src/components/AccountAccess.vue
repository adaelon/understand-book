<script setup lang="ts">
import { t as uiText } from '../i18n';

import LanguageSwitcher from './LanguageSwitcher.vue';
import { computed, nextTick, onBeforeUnmount, onMounted, ref } from 'vue';
import { ArrowLeft, ArrowRight, Check, CircleAlert, Eye, EyeOff, LoaderCircle, Mail } from '@lucide/vue';
import { ApiError } from '../api';
import { networkFetch } from '../network-client';
import brandLogo from '../assets/brand/understand-book-logo.svg';
import { accountError, passwordHint, validPassword, type AccountView } from '../account-forms';

const props = defineProps<{ view: AccountView; token: string }>();
const emit = defineEmits<{ (e: 'login', email?: string, notice?: string): void; (e: 'reset'): void; (e: 'forgot'): void }>();
const email = ref(''), password = ref(''), confirm = ref(''), invite = ref(''), code = ref('');
const error = ref(''), notice = ref(''), busy = ref(false);
const showPassword = ref(false), showConfirm = ref(false);
const passwordTouched = ref(false), confirmTouched = ref(false), attempted = ref(false);
const passwordInput = ref<HTMLInputElement>(), confirmInput = ref<HTMLInputElement>(), codeInput = ref<HTMLInputElement>();
const passwordError = computed(() => (passwordTouched.value || attempted.value) && !validPassword(password.value) ? passwordHint : '');
const confirmError = computed(() => (confirmTouched.value || attempted.value) && password.value !== confirm.value ? uiText('两次输入的密码不一致。') : '');
const pending = ref<{ request_id: string; expires_at: number } | null>(null);
const now = ref(Date.now()), resendAt = ref(0);
const wait = computed(() => Math.max(0, Math.ceil((resendAt.value - now.value) / 1000)));
const expired = computed(() => !!pending.value && pending.value.expires_at * 1000 <= now.value);
const title = computed(() => props.view === 'register' ? (pending.value ? uiText('验证邮箱') : uiText('创建阅读账号')) : props.view === 'forgot-password' ? uiText('找回密码') : uiText('设置新密码'));
const formAction = computed(() => props.view === 'register' ? 'start' : props.view === 'forgot-password' ? 'forgot' : 'reset');
const submitLabel = computed(() => props.view === 'register' ? uiText('发送注册验证码') : props.view === 'forgot-password' ? uiText('发送重置邮件') : uiText('重置密码'));
const busyLabel = computed(() => props.view === 'reset-password' ? uiText('正在重置…') : uiText('正在发送…'));
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
    attempted.value = true;
    if (!validPassword(password.value)) { await nextTick(); passwordInput.value?.focus(); return; }
    if (password.value !== confirm.value) { await nextTick(); confirmInput.value?.focus(); return; }
  }
  const body = action === 'start' ? { email: email.value, password: password.value, invite_code: invite.value }
    : action === 'forgot' ? { email: email.value }
    : action === 'reset' ? { token: props.token, new_password: password.value }
    : action === 'resend' ? { request_id: pending.value!.request_id }
    : { request_id: pending.value!.request_id, verification_code: code.value };
  password.value = ''; confirm.value = ''; busy.value = true;
  attempted.value = false; passwordTouched.value = false; confirmTouched.value = false;
  showPassword.value = false; showConfirm.value = false;
  try {
    const result = await networkFetch<{ request_id?: string; expires_at?: number; resend_after?: number; completed?: boolean }>('POST',
      action === 'forgot' || action === 'reset' ? `/auth/password/${action}` : `/auth/register/${action}`, body);
    if (!active) return;
    if (action === 'reset') emit('reset');
    else if (action === 'forgot') { resendAt.value = Date.now() + 60_000; notice.value = uiText('如该邮箱对应可用账号，将收到重置邮件。链接 30 分钟内有效；重新申请后请使用最新邮件。'); }
    else if (result.completed) { pending.value = null; code.value = ''; emit('login', email.value, uiText('注册成功，内测期间 AI Token 免费使用，不限额度。请登录。')); }
    else { remember(result); notice.value = uiText('验证码已发送，15 分钟内有效。'); }
  } catch (failure) {
    if (!active) return;
    if (failure instanceof ApiError && failure.details) remember(failure.details);
    if (action === 'forgot') resendAt.value = Date.now() + 60_000;
    error.value = accountError(failure);
  } finally {
    if (active) {
      now.value = Date.now();
      busy.value = false;
      if (action === 'start' && pending.value) { await nextTick(); codeInput.value?.focus(); }
    }
  }
}
function restart() {
  pending.value = null; code.value = ''; password.value = ''; confirm.value = ''; invite.value = ''; error.value = ''; notice.value = '';
  attempted.value = false; passwordTouched.value = false; confirmTouched.value = false; showPassword.value = false; showConfirm.value = false;
}
onMounted(() => { timer = setInterval(() => { now.value = Date.now(); }, 1000); });
onBeforeUnmount(() => { active = false; clearInterval(timer); password.value = ''; confirm.value = ''; invite.value = ''; });
</script>

<template>
  <main class="account-access">
    <div class="account-page">
      <header class="account-heading">
        <a class="brand-signature" href="/home.html"><img :src="brandLogo" class="brand-logo" width="34" height="34" alt="" /><span>Understand Book</span></a>
        <LanguageSwitcher :reset-token="view === 'reset-password' ? token : undefined" />
      </header>

      <div class="account-layout">
        <aside class="reading-invitation" :aria-label="uiText('属于你的阅读空间')">
          <p class="eyebrow"><span></span>{{ uiText('属于你的阅读空间') }}</p>
          <h2>{{ uiText('每一次阅读，') }}<br><em>{{ uiText('都更进一步。') }}</em></h2>
          <p class="reading-description">{{ uiText('串联知识，追问细节，') }}<br>{{ uiText('让理解在每一页发生。') }}</p>

          <div class="reading-art" aria-hidden="true">
            <svg viewBox="0 0 440 290" fill="none">
              <circle cx="230" cy="142" r="111" fill="#f0e8dc" />
              <circle cx="230" cy="142" r="139" stroke="#ded3c3" stroke-dasharray="2 8" />
              <path d="M43 211C84 177 145 187 208 221C267 187 326 177 388 205L388 224C323 202 265 211 208 245C145 210 88 198 43 230Z" fill="#cdb89c" opacity=".3" />
              <path d="M48 112C104 92 160 105 215 139C265 105 322 94 378 111L386 217C328 199 270 211 215 242C159 210 103 198 42 218Z" fill="#b65a17" />
              <path d="M54 105C111 91 164 105 215 136V234C159 202 108 191 49 207Z" fill="#eee4d4" stroke="#d1bca0" />
              <path d="M375 105C317 91 264 105 215 136V234C269 202 322 191 381 207Z" fill="#eee4d4" stroke="#d1bca0" />
              <path d="M61 96C117 84 169 100 215 130V226C166 192 115 180 57 194Z" fill="#fffcf4" stroke="#d1bca0" />
              <path d="M368 96C310 84 261 100 215 130V226C265 192 318 180 375 194Z" fill="#fbf6eb" stroke="#d1bca0" />
              <path d="M215 131V225" stroke="#cbb292" />
              <path d="M82 121C118 116 152 126 184 144M81 135C115 132 153 142 188 161M80 151C117 148 153 158 187 177M79 167C114 164 147 174 171 186" stroke="#d7cebd" stroke-width="3" stroke-linecap="round" />
              <path d="M81 135C106 132 132 137 151 144" stroke="#d4a274" stroke-width="7" stroke-linecap="round" opacity=".5" />
              <path d="M242 147L280 128L332 148L294 176L242 147ZM280 128L294 176" stroke="#b99c79" stroke-width="1.5" />
              <circle cx="242" cy="147" r="5" fill="#b65a17" /><circle cx="280" cy="128" r="5" fill="#f4e5d8" stroke="#b65a17" /><circle cx="332" cy="148" r="5" fill="#f4e5d8" stroke="#b65a17" /><circle cx="294" cy="176" r="5" fill="#8a9678" />
              <path d="M334 111C373 89 369 55 343 47" stroke="#b99c79" stroke-dasharray="4 5" />
              <rect x="285" y="24" width="64" height="43" rx="7" transform="rotate(-8 285 24)" fill="#fffcf4" stroke="#d5c4ab" />
              <path d="M303 38L308 43L319 29M303 53L332 49" stroke="#8a9678" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" />
              <path d="M119 72V54M110 63H128" stroke="#b65a17" stroke-width="1.5" stroke-linecap="round" />
              <circle cx="91" cy="214" r="3" fill="#b99c79" /><circle cx="367" cy="241" r="3" fill="#b99c79" />
              <path d="M244 116L260 108L264 152L255 147L247 157Z" fill="#b65a17" />
            </svg>
          </div>
          <p class="reading-caption"><span>01 / {{ uiText('阅读') }}</span><span>02 / {{ uiText('提问') }}</span><span>03 / {{ uiText('理解') }}</span></p>
        </aside>

        <section class="account-card" aria-labelledby="account-title">
          <ol v-if="view === 'register'" class="account-steps" :aria-label="uiText('注册进度')">
            <li :class="{ current: !pending, complete: pending }" :aria-current="!pending ? 'step' : undefined"><span><Check v-if="pending" :size="13" aria-hidden="true" /><template v-else>1</template></span>{{ uiText('填写信息') }}</li>
            <li :class="{ current: pending }" :aria-current="pending ? 'step' : undefined"><span>2</span>{{ uiText('验证邮箱') }}</li>
          </ol>
          <div v-if="pending" class="verification-symbol" aria-hidden="true"><Mail :size="25" :stroke-width="1.5" /></div>
          <h1 id="account-title">{{ title }}</h1>
          <p v-if="view === 'register' && !pending" class="account-intro">{{ uiText('用一个账号，开启你的下一页。') }}</p>
          <p v-else-if="pending" class="account-intro">{{ uiText('验证码已发送至') }}<strong class="verification-email">{{ email }}</strong>{{ uiText('输入邮件中的 6 位验证码，完成注册。') }}</p>
          <p v-else-if="view === 'forgot-password'" class="account-intro">{{ uiText('输入已验证的账号邮箱。未绑定邮箱的老账号请联系管理员恢复。') }}</p>
          <p v-else-if="token" class="account-intro">{{ uiText('提交新密码后，账号所有旧登录会话将退出。') }}</p>

          <div v-if="view === 'register' && !pending" class="welcome-credit">
            <strong aria-hidden="true">∞</strong>
            <div><b>{{ uiText('内测期间 AI Token 免费使用') }}</b><span>{{ uiText('完成邮箱注册，Token 不限额度。') }}</span></div>
          </div>

          <template v-if="view === 'register' && pending">
            <p v-if="expired" class="feedback feedback-error" role="status"><CircleAlert :size="17" aria-hidden="true" />{{ uiText('注册申请已过期，请重新开始。') }}</p>
            <form class="account-form verification-form" :aria-busy="busy" @submit.prevent="submit('complete')">
              <div class="field">
                <label for="account-code">{{ uiText('邮箱验证码') }}</label>
                <input id="account-code" ref="codeInput" v-model="code" class="verification-code" name="verification_code" inputmode="numeric" autocomplete="one-time-code" pattern="[0-9]{6}" maxlength="6" placeholder="000000" required :disabled="busy || expired" />
              </div>
              <button class="primary-button" :disabled="busy || expired"><LoaderCircle v-if="busy" class="spinner" :size="18" aria-hidden="true" />{{ busy ? uiText('正在验证…') : uiText('完成注册') }}<ArrowRight v-if="!busy" :size="17" aria-hidden="true" /></button>
            </form>
            <div class="verification-actions">
              <button class="text-button" :disabled="busy || wait > 0 || expired" @click="submit('resend')">{{ wait > 0 ? uiText('{0} 秒后可重发', [wait]) : uiText('重发验证码') }}</button>
              <button class="text-button" :disabled="busy" @click="restart">{{ uiText('重新开始注册') }}</button>
            </div>
          </template>

          <form v-else-if="view !== 'reset-password' || token" class="account-form" :aria-busy="busy" @submit.prevent="submit(formAction)">
            <div v-if="view !== 'reset-password'" class="field">
              <label for="account-email">{{ uiText('邮箱') }}</label>
              <input id="account-email" v-model="email" name="email" type="email" autocomplete="email" autocapitalize="none" spellcheck="false" maxlength="254" placeholder="you@example.com" required :disabled="busy" />
            </div>
            <template v-if="view !== 'forgot-password'">
              <div class="field">
                <label for="account-password">{{ view === 'register' ? uiText('密码') : uiText('新密码') }}</label>
                <div class="password-control">
                  <input id="account-password" ref="passwordInput" v-model="password" name="new_password" :type="showPassword ? 'text' : 'password'" autocomplete="new-password" :placeholder="uiText('至少 8 个字符')" required :disabled="busy" :aria-invalid="!!passwordError" aria-describedby="account-password-hint" @blur="passwordTouched = !!password" />
                  <button class="password-toggle" type="button" :disabled="busy" :aria-label="uiText('显示密码')" :aria-pressed="showPassword" aria-controls="account-password" @click="showPassword = !showPassword"><EyeOff v-if="showPassword" :size="18" aria-hidden="true" /><Eye v-else :size="18" aria-hidden="true" /></button>
                </div>
                <p id="account-password-hint" class="hint" :class="{ 'field-error': passwordError, 'field-valid': validPassword(password) }"><Check v-if="validPassword(password)" :size="14" aria-hidden="true" />{{ passwordHint }}</p>
              </div>
              <div class="field">
                <label for="account-confirm">{{ view === 'register' ? uiText('确认密码') : uiText('确认新密码') }}</label>
                <div class="password-control">
                  <input id="account-confirm" ref="confirmInput" v-model="confirm" name="confirm_password" :type="showConfirm ? 'text' : 'password'" autocomplete="new-password" :placeholder="uiText('再次输入密码')" required :disabled="busy" :aria-invalid="!!confirmError" :aria-describedby="confirmError ? 'account-confirm-error' : undefined" @blur="confirmTouched = !!confirm" />
                  <button class="password-toggle" type="button" :disabled="busy" :aria-label="uiText('显示确认密码')" :aria-pressed="showConfirm" aria-controls="account-confirm" @click="showConfirm = !showConfirm"><EyeOff v-if="showConfirm" :size="18" aria-hidden="true" /><Eye v-else :size="18" aria-hidden="true" /></button>
                </div>
                <p v-if="confirmError" id="account-confirm-error" class="hint field-error" role="alert">{{ confirmError }}</p>
              </div>
            </template>
            <div v-if="view === 'register'" class="field">
              <label for="account-invite">{{ uiText('内测码') }}<span class="label-note">{{ uiText('由邀请人提供') }}</span></label>
              <input id="account-invite" v-model="invite" class="invite-input" name="invite_code" autocomplete="off" autocapitalize="characters" spellcheck="false" maxlength="64" placeholder="XXXXX-XXXXX-XXXXX-XXXXX" required :disabled="busy" />
            </div>
            <button class="primary-button" :disabled="busy || (view === 'forgot-password' && wait > 0)"><LoaderCircle v-if="busy" class="spinner" :size="18" aria-hidden="true" />{{ busy ? busyLabel : view === 'forgot-password' && wait > 0 ? uiText('{0} 秒后可重发', [wait]) : submitLabel }}<ArrowRight v-if="!busy" :size="17" aria-hidden="true" /></button>
            <p v-if="view === 'register'" class="form-footnote">{{ uiText('验证邮箱后即可登录，内测码仅可使用一次。') }}</p>
          </form>
          <p v-else class="feedback feedback-error" role="alert"><CircleAlert :size="17" aria-hidden="true" />{{ uiText('未找到重置链接。请从邮件重新打开，或重新申请找回密码。') }}</p>

          <p v-if="error" class="feedback feedback-error" role="alert"><CircleAlert :size="17" aria-hidden="true" />{{ error }}</p>
          <p v-if="notice" class="feedback feedback-success" role="status"><Check :size="17" aria-hidden="true" />{{ notice }}</p>
          <button v-if="view === 'reset-password'" class="text-button recovery-link" :disabled="busy" @click="emit('forgot')">{{ uiText('重新申请找回密码') }}</button>

          <footer class="account-footer">
            <span v-if="view === 'register'">{{ uiText('已有账号？') }}</span>
            <button class="text-button back-to-login" :disabled="busy" @click="emit('login')"><ArrowLeft v-if="view !== 'register'" :size="15" aria-hidden="true" />{{ uiText('返回登录') }}</button>
          </footer>
        </section>
      </div>
      <footer class="page-note">Understand Book<span>·</span>{{ uiText('让阅读通向理解') }}</footer>
    </div>
  </main>
</template>
<style scoped>
.account-access { width: 100%; height: 100%; overflow-y: auto; overflow-wrap: anywhere; color: #302c26; background: #f7f4ee; font: 400 14px/1.6 var(--sans, 'Microsoft YaHei', sans-serif); }
.account-page { display: flex; flex-direction: column; width: min(100%, 1140px); min-height: 100%; margin: 0 auto; padding: 26px 40px 18px; }
.account-heading { display: flex; align-items: center; justify-content: space-between; gap: 16px; }
.brand-signature { font-size: 19px; letter-spacing: -.4px; }
.account-layout { display: grid; grid-template-columns: minmax(0, 1fr) minmax(0, 460px); align-items: center; gap: clamp(40px, 6vw, 88px); flex: 1; padding: 28px 0; }
.reading-invitation { padding: 8px 0 18px; }
.eyebrow { display: flex; align-items: center; gap: 10px; margin: 0 0 22px; color: #786d5d; font-size: 12px; letter-spacing: .13em; }
.eyebrow > span { width: 6px; height: 6px; border-radius: 50%; background: var(--brand-accent); }
.reading-invitation h2 { margin: 0; font: 400 clamp(36px, 3.7vw, 48px)/1.45 var(--font-display); font-synthesis: none; letter-spacing: .025em; }
.reading-invitation h2 em { color: var(--brand-accent); font-style: normal; }
.reading-description { margin: 22px 0 0; color: #776e61; font-size: 15px; line-height: 1.95; }
.reading-art { margin: 16px -12px 0; }
.reading-art svg { display: block; width: 100%; max-height: 290px; }
.reading-caption { display: flex; justify-content: center; gap: 26px; margin: 12px 0 0; color: #8a7d6b; font-size: 11px; letter-spacing: .08em; }
.account-card { min-width: 0; padding: 26px 32px 18px; border: 1px solid #e7dfd3; border-radius: 18px; background: #fffdf9; box-shadow: 0 16px 48px #48341a06, 0 2px 6px #48341a03; }
.account-steps { display: flex; align-items: center; gap: 22px; padding: 0; margin: 0 0 18px; list-style: none; color: #8b8276; font-size: 12px; }
.account-steps li { display: flex; align-items: center; gap: 7px; }
.account-steps li + li::before { content: ''; width: 30px; height: 1px; margin-right: 8px; background: #e3d9cc; }
.account-steps li > span { display: grid; place-items: center; width: 22px; height: 22px; border: 1px solid #e3d9cc; border-radius: 50%; font-size: 11px; }
.account-steps .current, .account-steps .complete { color: var(--brand-accent-deep); }
.account-steps .current > span { color: #fff; border-color: var(--brand-accent); background: var(--brand-accent); }
.account-steps .complete > span { background: var(--brand-accent-soft); border-color: var(--brand-accent-soft); }
h1 { margin: 0; font: 400 30px/1.4 var(--font-display); font-synthesis: none; }
.account-intro { margin: 8px 0 18px; color: #7b7267; font-size: 13px; line-height: 1.8; }
.welcome-credit { display: flex; align-items: center; gap: 16px; margin: 16px 0 18px; padding: 12px 16px; border: 1px solid #efdfc8; border-radius: 9px; background: #faf2e5; }
.welcome-credit > strong { flex: 0 0 auto; color: var(--brand-accent-deep); font: 400 34px/1.2 Georgia, serif; letter-spacing: -1px; }
.welcome-credit small { margin-right: 2px; font-size: 19px; }
.welcome-credit b, .welcome-credit span { display: block; }
.welcome-credit b { color: #6d4c2a; font-size: 12px; font-weight: 500; }
.welcome-credit span { margin-top: 2px; color: #8a765e; font-size: 11px; }
.account-form { display: grid; gap: 12px; }
.field { min-width: 0; }
label { display: flex; justify-content: space-between; align-items: baseline; gap: 8px; margin-bottom: 6px; font-size: 13px; font-weight: 500; line-height: 1.4; }
.label-note { color: #8a8072; font-size: 11px; font-weight: 400; }
input { display: block; box-sizing: border-box; width: 100%; min-width: 0; height: 46px; padding: 11px 13px; border: 1px solid #ded7cc; border-radius: 7px; background: #fffefa; color: #302c26; font: inherit; font-size: 15px; transition: border-color .15s, box-shadow .15s; }
input::placeholder { color: #a29a8e; opacity: 1; font-size: 13px; }
input:hover:not(:disabled) { border-color: #c4b39c; }
input:focus { outline: none; border-color: var(--brand-accent); box-shadow: 0 0 0 3px #b65a1712; }
input[aria-invalid='true'] { border-color: #b44e3d; }
input:disabled { background: #f5f1eb; color: #8a8072; }
.password-control { position: relative; }
.password-control input { padding-right: 46px; }
button { font: inherit; cursor: pointer; touch-action: manipulation; }
button:disabled { cursor: default; opacity: .5; }
button:focus-visible, a:focus-visible { outline: 2px solid var(--brand-accent); outline-offset: 4px; }
.password-toggle { position: absolute; top: 1px; right: 1px; display: grid; place-items: center; width: 44px; height: 44px; padding: 0; border: 0; border-radius: 6px; background: transparent; color: #8a8072; }
.password-toggle:hover:enabled { color: var(--brand-accent-deep); background: #f4ede3; }
.hint { display: flex; align-items: center; gap: 5px; margin: 6px 0 0; color: #857b6e; font-size: 11px; line-height: 1.6; }
.hint.field-error { color: #a13d30; }
.hint.field-valid { color: #627449; }
.invite-input { font-family: var(--mono, monospace); font-size: 13px; letter-spacing: .025em; }
.invite-input::placeholder { font-size: 12px; }
.primary-button { display: flex; justify-content: center; align-items: center; gap: 12px; width: 100%; min-height: 46px; margin-top: 4px; padding: 11px 16px; border: 1px solid var(--brand-accent); border-radius: 7px; background: var(--brand-accent); color: #fff; font-size: 14px; font-weight: 500; transition: background .15s, border-color .15s; }
.primary-button:hover:enabled { background: var(--brand-accent-deep); border-color: var(--brand-accent-deep); }
.form-footnote { margin: -6px 0 0; color: #8a8072; font-size: 11px; text-align: center; }
.account-footer { display: flex; align-items: center; justify-content: center; flex-wrap: wrap; gap: 8px; margin-top: 16px; padding-top: 12px; border-top: 1px solid #eee8df; color: #8a8072; font-size: 12px; }
.text-button { min-height: 36px; padding: 6px 0; border: 0; background: transparent; color: var(--brand-accent-deep); font-size: 12px; }
.text-button:hover:enabled { text-decoration: underline; text-underline-offset: 4px; }
.back-to-login { display: inline-flex; align-items: center; gap: 7px; }
.page-note { display: flex; justify-content: center; gap: 10px; margin-top: auto; color: #978b79; font-size: 10px; letter-spacing: .035em; }
.verification-symbol { display: grid; place-items: center; width: 52px; height: 52px; margin: 26px 0 20px; border: 1px solid #efdfc8; border-radius: 14px; background: #faf2e5; color: var(--brand-accent); }
.verification-email { display: block; margin: 6px 0; color: #4d4234; font-weight: 500; }
.verification-form { margin-top: 26px; }
.verification-code { height: 54px; text-align: center; font: 24px var(--mono, monospace); letter-spacing: .45em; padding-left: calc(13px + .45em); }
.verification-code::placeholder { font-size: 24px; color: #c4b7a3; }
.verification-actions { display: flex; flex-wrap: wrap; justify-content: space-between; gap: 8px 16px; margin-top: 12px; }
.feedback { display: flex; align-items: flex-start; gap: 8px; padding: 11px 12px; margin: 18px 0 0; border-radius: 7px; font-size: 12px; line-height: 1.7; }
.feedback svg { flex: 0 0 auto; margin-top: 2px; }
.feedback-error { background: #fbede8; color: #9a3d2f; }
.feedback-success { background: #f0f3e9; color: #5b6d43; }
.recovery-link { display: block; margin: 12px auto 0; }
.spinner { animation: account-spin .9s linear infinite; }
@keyframes account-spin { to { transform: rotate(360deg); } }
@media (prefers-reduced-motion: reduce) { .spinner { animation: none; } input, .primary-button { transition: none; } }
@media (max-width: 900px) {
  .account-page { max-width: 560px; padding: 24px 24px 18px; }
  .account-layout { display: block; flex: 0 0 auto; padding: 30px 0 24px; }
  .reading-invitation { display: none; }
  .page-note { margin-top: auto; }
}
@media (max-width: 480px) {
  .account-page { padding: 20px 18px 16px; }
  .account-heading { gap: 8px; }
  .brand-signature { gap: 7px; font-size: 16px; }
  .brand-logo { width: 29px; height: 29px; }
  .account-layout { padding: 24px 0 20px; }
  .account-card { padding: 24px 22px 14px; border-radius: 14px; }
  .account-steps { margin-bottom: 16px; gap: 12px; font-size: 11px; }
  .account-steps li + li::before { width: 22px; margin-right: 4px; }
  h1 { font-size: 28px; }
  .account-form { gap: 12px; }
  .account-intro { margin-bottom: 18px; }
  .welcome-credit { padding: 11px 12px; gap: 12px; margin: 18px 0 20px; }
  .welcome-credit > strong { font-size: 30px; }
  input { font-size: 16px; }
  .account-footer { margin-top: 16px; padding-top: 10px; }
}
@media (max-width: 360px) {
  .account-page { padding-inline: 12px; }
  .account-card { padding-inline: 18px; }
  .brand-signature { font-size: 15px; }
  .welcome-credit { gap: 9px; }
  .welcome-credit span { font-size: 10px; }
}
</style>
