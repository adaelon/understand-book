<script setup lang="ts">
import { ref } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import LocalMaterials from './LocalMaterials.vue';
const error = ref(''), signingIn = ref(false);
async function cloud(signIn = false) {
  error.value = ''; signingIn.value = signIn;
  try { await invoke(signIn ? 'desktop_sign_in' : 'desktop_open_cloud'); }
  catch (reason) { error.value = String(reason); }
  finally { signingIn.value = false; }
}
</script>
<template>
  <main class="desktop-library">
    <header><div><p class="eyebrow">UNDERSTAND BOOK</p><h1>本机书库</h1><p>资料留在自己的电脑，打开就能读。</p></div><nav><button @click="cloud()">进入云端学习</button><button :disabled="signingIn" @click="cloud(true)">{{ signingIn ? '请在浏览器确认…' : '登录网站账号' }}</button><button v-if="signingIn" @click="invoke('desktop_cancel_sign_in')">取消登录</button></nav></header>
    <p v-if="error" role="alert">{{ error }}</p>
    <LocalMaterials />
    <p class="hint">在云端工作区中点击“同步到云端”，才会上传资料并检查套餐。登录账号不会自动上传本机文件。</p>
  </main>
</template>
<style>
html{background:#f3eee5;color:#302d27;font-family:"Segoe UI","Microsoft YaHei",sans-serif}body{margin:0}button{font:inherit}.desktop-library{max-width:1120px;margin:0 auto;padding:40px 28px}.desktop-library>header{display:flex;justify-content:space-between;align-items:center;gap:2rem}.desktop-library h1{font-size:30px;letter-spacing:.04em;margin:.5rem 0}.desktop-library .eyebrow{letter-spacing:.16em;font-size:12px;color:#8a592f}.desktop-library nav{display:flex;gap:.6rem;flex-wrap:wrap}.desktop-library nav button{padding:.7rem 1rem;border:1px solid #dbd1c2;border-radius:9px;background:#fffdf8;cursor:pointer}.desktop-library .hint{color:#786c5f;font-size:.85rem;line-height:1.8}.desktop-library>header p:not(.eyebrow){color:#786c5f}.desktop-library>p[role=alert]{color:#a33825}@media(max-width:800px){.desktop-library>header{align-items:flex-start;flex-direction:column}}
</style>
