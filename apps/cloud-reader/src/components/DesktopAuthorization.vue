<script setup lang="ts">
import { t as uiText } from '@understand-book/web/reader/i18n';

import { ref } from 'vue';
import { network } from '@understand-book/web/reader/reader-context';
import { desktopAuthorization, cancelDesktopAuthorization, type DesktopAuthorizationRequest } from '../desktop-auth';
const busy = ref(false), error = ref('');
let request: DesktopAuthorizationRequest | null = null;
try { request = desktopAuthorization(location.search); } catch (failure) { error.value = (failure as Error).message; }
async function approve() {
  const identity = network.value.identity;
  if (!request || !identity || busy.value) return;
  busy.value = true; error.value = '';
  try {
    // Consent belongs to the account. Restoring a reading workspace in the
    // background must not discard an already-issued authorization code.
    const response = await fetch('/api/auth/desktop/authorize', {
      method: 'POST', credentials: 'same-origin', cache: 'no-store',
      headers: { 'Content-Type': 'application/json', 'X-CSRF-Token': identity.csrf_token },
      body: JSON.stringify(request),
    });
    const result = await response.json();
    if (!response.ok) throw new Error(result.message ?? uiText('暂时无法连接 Windows，请重试。'));
    if (network.value.identity?.csrf_token !== identity.csrf_token) throw new Error(uiText('账号已切换，请重新确认登录。'));
    location.assign(result.redirect_url);
  } catch (failure) { error.value = failure instanceof Error ? failure.message : String(failure); busy.value = false; }
}
function cancel() { if (request) location.assign(cancelDesktopAuthorization(request)); }
</script>
<template>
  <aside v-if="request || error" class="desktop-authorization" :aria-label="uiText('授权 Windows 登录')">
    <h2>{{ uiText('连接 Windows 版 Understand Book') }}</h2>
    <p v-if="!network.identity">{{ uiText('请先登录网站账号，然后确认连接。Windows 版将使用同一份课程、额度和学习记录。') }}</p>
    <template v-else>
      <p>{{ uiText('使用') }} {{ network.identity.email || network.identity.user_id }} {{ uiText('登录 Windows 应用。') }}</p>
      <button :disabled="busy" @click="approve">{{ busy ? uiText('正在连接…') : uiText('确认登录 Windows') }}</button>
    </template>
    <button v-if="request" :disabled="busy" @click="cancel">{{ uiText('取消') }}</button>
    <p v-if="error" role="alert">{{ error }}</p>
  </aside>
</template>
<style scoped>
.desktop-authorization { position: relative; z-index: 50; margin: 16px auto; max-width: 640px; padding: 20px; border: 1px solid #c6bda9; border-radius: 16px; background: #fffaf0; color: #342f27; }
h2 { margin: 0 0 12px; font-size: 18px; } button { padding: 9px 15px; margin: 0 8px 0 0; cursor: pointer; }
</style>
