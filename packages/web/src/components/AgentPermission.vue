<script setup lang="ts">
import { onMounted, ref } from 'vue';
import { t } from '../i18n';
import { localRequest } from '../reader-host';

interface Permission {
  request_id: string;
  title: string;
  details?: string;
  options: { optionId: string; name: string; kind: string }[];
}
const props = defineProps<{ turnId: string }>();
const permission = ref<Permission | null>(null);
const busy = ref(false);
const error = ref('');
async function load() {
  busy.value = true;
  error.value = '';
  try {
    const response = await localRequest<{ permission: Permission | null }>('POST', '/local-agent/permission', {
      action: 'get', turn_id: props.turnId
    });
    permission.value = response.permission;
  } catch (cause) { error.value = cause instanceof Error ? cause.message : String(cause); }
  finally { busy.value = false; }
}
async function respond(optionId: string | null) {
  if (!permission.value || busy.value) return;
  busy.value = true;
  error.value = '';
  try {
    await localRequest('POST', '/local-agent/permission', {
      action:'respond', turn_id:props.turnId, request_id:permission.value.request_id, option_id:optionId
    });
    permission.value = null;
  } catch (cause) { error.value = cause instanceof Error ? cause.message : String(cause); }
  finally { busy.value = false; }
}
onMounted(load);
</script>
<template>
  <section v-if="permission || error" class="agent-permission" :aria-label="t('Agent 请求确认')">
    <strong>{{ t('Agent 请求确认') }}</strong>
    <template v-if="permission">
      <p>{{ permission.title }}</p>
      <details v-if="permission.details"><summary>{{ t('查看操作参数') }}</summary><pre>{{ permission.details }}</pre></details>
      <div class="permission-actions">
        <button v-for="option in permission.options" :key="option.optionId" :disabled="busy" @click="respond(option.optionId)">{{ option.name }}</button>
        <button :disabled="busy" @click="respond(null)">{{ t('取消此请求') }}</button>
      </div>
    </template>
    <div v-if="error" role="alert">{{ error }} <button :disabled="busy" @click="load">{{ t('重新读取') }}</button></div>
  </section>
</template>
<style scoped>
.agent-permission { display:grid; gap:8px; padding:12px; margin:8px 0; background:var(--paper); border:1px solid var(--line); border-radius:9px; font-size:.86rem; }
.agent-permission p { margin:0; overflow-wrap:anywhere; }
summary { cursor:pointer; color:var(--steel); }
pre { white-space:pre-wrap; overflow-wrap:anywhere; max-height:12rem; overflow:auto; }
.permission-actions { display:flex; gap:6px; flex-wrap:wrap; }
button { border:1px solid var(--line); border-radius:6px; background:transparent; color:var(--ink); font:inherit; padding:4px 8px; cursor:pointer; }
</style>
