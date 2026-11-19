<script setup lang="ts">
import { onMounted, ref } from 'vue';
import { Copy } from '@lucide/vue';
import { localRequest } from '../reader-host';
import { t } from '../i18n';

export interface PersonalAccess {
  enabled: boolean;
  materials: {book_id: string; title: string}[];
  book_ids: string[];
  write: boolean;
  navigate: boolean;
  manage: boolean;
  token?: string;
}
const props = defineProps<{ currentBook?: string }>();
const emit = defineEmits<{ changed: [access: PersonalAccess]; busy: [saving: boolean] }>();
const access = ref<PersonalAccess | null>(null);
const books = ref<string[]>([]);
const write = ref(false), navigate = ref(false), manage = ref(false);
const token = ref(''), copied = ref(false), busy = ref(false), error = ref('');
async function request(action?: 'enable' | 'revoke') {
  busy.value = true; error.value = ''; copied.value = false;
  if (action) emit('busy', true);
  try {
    const response = await localRequest<PersonalAccess>(action ? 'POST' : 'GET', '/personal/connection', action ? {
      action, ...(action === 'enable' ? {book_ids: books.value, write: write.value, navigate: navigate.value, manage: write.value && manage.value} : {})
    } : undefined);
    // The credential is returned only when first issued. Strip it before storing the status.
    const { token: issued, ...state } = response;
    access.value = state;
    token.value = issued ?? '';
    books.value = state.enabled ? [...state.book_ids] : state.materials.filter(m => m.book_id === props.currentBook).map(m => m.book_id);
    write.value = !!state.write; navigate.value = !!state.navigate; manage.value = !!state.manage;
    emit('changed', state);
  } catch (cause) { error.value = cause instanceof Error ? cause.message : String(cause); }
  finally { busy.value = false; if (action) emit('busy', false); }
}
async function copyToken() {
  try { await navigator.clipboard.writeText(token.value); copied.value = true; }
  catch { error.value = t('请选中下方凭证并复制。'); }
}
onMounted(() => request());
</script>
<template>
  <details class="personal-settings">
    <summary>{{ t('个人学习连接') }}</summary>
    <div class="personal-body">
      <p>{{ t('选择允许外部 Agent 和 CLI 访问的材料与操作。') }}</p>
      <p v-if="error" role="alert">{{ error }}</p>
      <button v-if="!access && !busy" type="button" @click="request()">{{ t('重试') }}</button>
      <template v-if="access">
        <fieldset class="personal-books" :disabled="busy || access.enabled"><legend>{{ t('可用材料') }}</legend>
          <label v-for="material in access.materials" :key="material.book_id"><input v-model="books" type="checkbox" :value="material.book_id" />{{ material.title || material.book_id }}</label>
          <span v-if="!access.materials.length">{{ t('先在本机书库中打开一份材料。') }}</span>
        </fieldset>
        <fieldset :disabled="busy || access.enabled"><legend>{{ t('允许的操作') }}</legend>
          <label><input v-model="write" type="checkbox" />{{ t('保存笔记、高亮和待续问题') }}</label>
          <label><input v-model="navigate" type="checkbox" />{{ t('在 Reader 中定位与并排原文') }}</label>
          <label><input v-model="manage" type="checkbox" :disabled="!write" />{{ t('导入本机文件与网络资源、管理 Skill') }}</label>
        </fieldset>
        <div v-if="token" class="personal-credential"><label>{{ t('连接凭证（仅此次显示）') }}<input readonly :value="token" @focus="($event.target as HTMLInputElement).select()" /></label><button type="button" @click="copyToken"><Copy :size="14"/>{{ copied ? t('已复制') : t('复制凭证') }}</button></div>
        <p v-if="access.enabled">{{ t('终端配对') }} <code>understand-book connect --token-stdin</code><br/>{{ t('MCP 启动命令') }} <code>understand-book mcp</code></p>
        <button v-if="access.enabled" type="button" :disabled="busy" @click="request('revoke')">{{ t('撤销个人连接') }}</button>
        <button v-else type="button" :disabled="busy || !books.length" @click="request('enable')">{{ t('启用个人连接') }}</button>
      </template>
    </div>
  </details>
</template>
<style scoped>
.personal-settings { font-size:.88rem; color:var(--ink); }
summary { font-weight:650; cursor:pointer; }
.personal-body,.personal-credential { display:grid; gap:10px; margin-top:10px; }
.personal-body p { margin:0; color:var(--steel); }
.personal-body p[role=alert] { color:var(--danger,#a5453b); }
fieldset { display:grid; gap:6px; border:1px solid var(--line); border-radius:8px; padding:8px 10px; }
.personal-books { max-height:12rem; overflow-y:auto; }
legend { padding:0 4px; }
label { display:flex; gap:7px; align-items:baseline; }
.personal-credential label { flex-direction:column; align-items:stretch; }
.personal-credential input { padding:6px; border:1px solid var(--line); border-radius:5px; font:inherit; }
button { display:inline-flex; align-items:center; gap:4px; justify-self:start; font:inherit; border:1px solid var(--line); background:transparent; color:var(--ink); border-radius:7px; padding:5px 10px; }
</style>
