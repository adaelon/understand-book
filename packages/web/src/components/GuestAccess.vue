<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref } from 'vue';
import { ArrowRight, BookOpen, Check } from '@lucide/vue';
import { t as uiText } from '../i18n';
import { ApiError } from '../api';
import { accountError } from '../account-forms';
import { networkFetch } from '../network-client';
import type { NetworkIdentity } from '../network-context';
import LanguageSwitcher from './LanguageSwitcher.vue';
import brandLogo from '../assets/brand/understand-book-logo.svg';

const emit = defineEmits<{ (e: 'entered', identity: NetworkIdentity): void; (e: 'login'): void }>();
const invite = ref(''), error = ref(''), checking = ref(true), busy = ref(false);
let active = true;
async function restore(): Promise<boolean> {
  try {
    const identity = await networkFetch<NetworkIdentity>('GET', '/auth/me');
    if (active) emit('entered', identity);
    return true;
  } catch (failure) {
    if (failure instanceof ApiError && failure.status === 401) return false;
    throw failure;
  }
}
async function enter() {
  if (busy.value || checking.value || !invite.value.trim()) return;
  busy.value = true; error.value = '';
  try {
    // A previous response may have installed the cookie even if its body was lost.
    if (await restore()) return;
    const identity = await networkFetch<NetworkIdentity>('POST', '/auth/guest', { invite_code: invite.value.trim() });
    invite.value = '';
    if (active) emit('entered', identity);
  } catch (failure) {
    if (active) error.value = failure instanceof ApiError ? accountError(failure) : uiText('连接暂时中断。请重试；已开通的体验会自动恢复。');
  } finally { if (active) busy.value = false; }
}
onMounted(async () => {
  try { await restore(); }
  catch { if (active) error.value = uiText('连接暂时中断。请重试；已开通的体验会自动恢复。'); }
  finally { if (active) checking.value = false; }
});
onBeforeUnmount(() => { active = false; invite.value = ''; });
</script>

<template>
  <main class="guest-access">
    <header class="guest-heading"><a class="brand-signature" href="/home.html"><img :src="brandLogo" width="36" height="36" alt="" /><span>Understand Book</span></a><LanguageSwitcher /></header>
    <section class="guest-card" aria-labelledby="guest-title">
      <span class="guest-eyebrow"><BookOpen :size="16" />{{ uiText('邀请制体验') }}</span>
      <h1 id="guest-title">{{ uiText('从一个问题，开始读懂。') }}</h1>
      <p class="guest-intro">{{ uiText('输入内测码，以游客身份直接体验个性化阅读与学习。') }}</p>
      <div class="guest-benefits"><span><Check :size="16" />{{ uiText('无需注册') }}</span><span><Check :size="16" />{{ uiText('示例材料已备好') }}</span></div>
      <div class="guest-credit"><strong>¥5<span>{{ uiText('体验额度') }}</span></strong><p>{{ uiText('每枚内测码赠送一次，有效期 30 天。') }}</p></div>
      <p v-if="checking" role="status">{{ uiText('正在恢复你的体验…') }}</p>
      <form v-else @submit.prevent="enter">
        <label for="guest-invite">{{ uiText('内测码') }}</label>
        <input id="guest-invite" v-model="invite" name="invite_code" autocomplete="off" autocapitalize="characters" spellcheck="false" maxlength="64" placeholder="XXXXX-XXXXX-XXXXX-XXXXX" required :disabled="busy" :aria-invalid="!!error" aria-describedby="guest-help guest-error" />
        <p id="guest-help" class="guest-hint">{{ uiText('使用邀请人发给你的内测码，即可进入资料库。') }}</p>
        <p v-if="error" id="guest-error" class="guest-error" role="alert">{{ error }}</p>
        <button class="guest-submit" :disabled="busy || !invite.trim()">{{ busy ? uiText('正在开通体验…') : uiText('开始体验') }}<ArrowRight v-if="!busy" :size="18" /></button>
      </form>
      <p class="guest-retention">{{ uiText('同一浏览器可继续上次的阅读。请保留浏览器数据；退出游客会话后，原内测码无法再次使用。') }}</p>
      <button class="guest-login" type="button" :disabled="busy" @click="emit('login')">{{ uiText('已有账号？登录继续') }}</button>
    </section>
  </main>
</template>

<style scoped>
.guest-access { box-sizing: border-box; width: min(100%, 760px); height: 100%; margin: 0 auto; padding: clamp(20px, 5vh, 48px) clamp(20px, 5vw, 40px); overflow-y: auto; color: var(--ink); }
.guest-heading { display: flex; align-items: center; justify-content: space-between; gap: 12px; flex-wrap: wrap; }
.brand-signature { display: inline-flex; align-items: center; gap: 10px; text-decoration: none; color: inherit; }
.guest-card { max-width: 440px; margin: clamp(32px, 7vh, 72px) auto 0; }
.guest-eyebrow, .guest-benefits, .guest-benefits span { display: flex; align-items: center; gap: 8px; }
.guest-eyebrow { font-size: 13px; color: var(--steel, #6c6a64); }
h1 { font: 400 clamp(27px, 5vw, 36px)/1.45 var(--font-display); font-synthesis: none; margin: 18px 0 12px; }
p { line-height: 1.75; margin: 0; }
.guest-intro { color: var(--slate, #3d3d3a); }
.guest-benefits { gap: 22px; flex-wrap: wrap; font-size: 13px; margin: 20px 0; }
.guest-benefits svg { color: var(--accent, #6b7658); }
.guest-credit { border: 1px solid var(--line, #ddd); border-radius: 12px; padding: 18px 20px; background: var(--surface, #f5f4f0); margin-bottom: 26px; }
.guest-credit strong { font-size: 30px; font-weight: 500; }
.guest-credit strong span { margin-left: 12px; font-size: 14px; font-weight: 400; }
.guest-credit p, .guest-hint, .guest-retention { font-size: 12px; color: var(--steel, #6c6a64); }
label { display: block; font-size: 14px; margin-bottom: 10px; }
input { box-sizing: border-box; width: 100%; padding: 14px 12px; border: 1px solid var(--line, #bbb); border-radius: 8px; background: var(--canvas, #fff); color: inherit; font: inherit; font-size: 16px; }
input:focus-visible { outline: 2px solid var(--accent, #6b7658); outline-offset: 2px; }
.guest-hint { margin-top: 8px; }
.guest-error { color: #a32620; font-size: 13px; margin-top: 12px; }
button { font: inherit; cursor: pointer; }
button:disabled { opacity: .55; cursor: default; }
.guest-submit { width: 100%; margin-top: 20px; padding: 14px 18px; display: flex; justify-content: center; align-items: center; gap: 12px; background: var(--ink, #292d29); color: var(--canvas, #fff); border: 0; border-radius: 8px; }
.guest-retention { margin: 18px 0; }
.guest-login { padding: 8px 0; color: var(--steel, #6c6a64); border: 0; background: transparent; text-decoration: underline; text-underline-offset: 4px; }
</style>
