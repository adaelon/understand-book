<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref, watch } from 'vue';
import { ApiError, type AccountMcpState } from '../api';
import { accountMcp } from '../network-client';
import { network } from '../network-context';

const emit = defineEmits<{ (e: 'close'): void }>();
const dialog = ref<HTMLDialogElement | null>(null);
const state = ref<AccountMcpState | null>(null), label = ref('我的 Agent');
const created = ref<{ id: string; token: string } | null>(null);
const busy = ref(false), error = ref(''), notice = ref('');
let active = true;
const owner = () => `${network.value.epoch}:${network.value.identity?.user_id}:${network.value.identity?.csrf_token}`;
const mountedOwner = owner();
const current = () => active && owner() === mountedOwner;
watch(owner, () => { state.value = null; created.value = null; error.value = ''; notice.value = ''; emit('close'); }, { flush: 'sync' });
function time(value: string | number) {
  return new Intl.DateTimeFormat('zh-CN', { timeZone: 'Asia/Hong_Kong', dateStyle: 'medium', timeStyle: 'short' })
    .format(new Date(typeof value === 'number' ? value * 1000 : value));
}
async function refresh() {
  const value = await accountMcp.read();
  if (current()) state.value = value;
}
async function perform(action: 'refresh' | 'create' | 'revoke', id?: string) {
  if (busy.value || !current()) return;
  busy.value = true; error.value = ''; notice.value = '';
  try {
    if (action === 'create') {
      created.value = null;
      const result = await accountMcp.create(label.value.trim());
      if (!current()) return;
      created.value = { id: result.credential.credential_id, token: result.token };
      if (state.value) state.value.credentials.unshift(result.credential);
      notice.value = '凭证已创建，请复制后保存到你的 Agent 客户端。';
    } else if (action === 'revoke' && id) {
      await accountMcp.revoke(id);
      if (!current()) return;
      if (created.value?.id === id) created.value = null;
      const credential = state.value?.credentials.find(item => item.credential_id === id);
      if (credential) credential.active = false;
      notice.value = '凭证已撤销。';
      await refresh();
    } else await refresh();
  } catch (failure) {
    if (!current()) return;
    error.value = failure instanceof ApiError && failure.errorCode === 'MCP_LABEL_INVALID'
      ? '名称需要包含 1–80 个字符。'
      : action === 'create' ? '创建结果未确认，请刷新列表核对；如已创建但未收到完整值，请撤销该条后重新创建。'
      : '请求未完成，请刷新核对或稍后重试。';
  } finally { if (current()) busy.value = false; }
}
async function copy(text: string) {
  try { await navigator.clipboard.writeText(text); if (current()) notice.value = '已复制。'; }
  catch { if (current()) error.value = '无法自动复制，请选中文字手动复制。'; }
}
const agentNote = '请通过 Understand Book MCP 阅读我的获准材料：先查看 book_list 选择确切版本，再检索定位并用 book_text 核实原文，按返回的来源链接引用。按需续页；以服务端 usage 为准，次数耗尽后停止重试并告知恢复时间和购买入口。';
onMounted(() => { dialog.value?.showModal(); void perform('refresh'); });
onBeforeUnmount(() => { active = false; created.value = null; state.value = null; dialog.value?.close(); });
</script>

<template>
  <dialog ref="dialog" class="account-mcp" aria-labelledby="account-mcp-title" @cancel.prevent="emit('close')">
    <header><h2 id="account-mcp-title">MCP 接入</h2><button type="button" @click="emit('close')">关闭</button></header>
    <p>让你的 Agent 检索和阅读获准材料。所有客户端共用本账号的次数。</p>
    <p>账号：{{ network.identity?.email || network.identity?.user_id }}</p>
    <p v-if="error" role="alert">{{ error }}</p><p v-if="notice" role="status">{{ notice }}</p>
    <button type="button" :disabled="busy" @click="perform('refresh')">刷新</button>
    <p v-if="busy" role="status">正在处理…</p>
    <template v-if="state">
      <section aria-label="连接设置">
        <h3>连接你的 Agent</h3>
        <p>选择支持 Streamable HTTP 和自定义 Authorization 请求头的客户端。</p>
        <label>接入地址<input readonly :value="state.mcp_url"></label>
        <button type="button" @click="copy(state.mcp_url)">复制地址</button>
        <p>Authorization：<code>Bearer &lt;你的接入凭证&gt;</code></p>
        <form @submit.prevent="perform('create')">
          <label>凭证名称<input v-model="label" required maxlength="80" :disabled="busy" placeholder="例如：我的 Agent"></label>
          <button :disabled="busy || !label.trim()">创建凭证</button>
        </form>
        <div v-if="created" class="new-credential">
          <p>完整凭证仅本次显示，关闭后无法再次查看。</p>
          <label>新凭证<textarea readonly :value="created.token" spellcheck="false" /></label>
          <button type="button" @click="copy(created.token)">复制凭证</button>
          <button type="button" @click="created = null">我已保存，隐藏凭证</button>
        </div>
        <ul class="credentials">
          <li v-for="credential in state.credentials" :key="credential.credential_id">
            <strong>{{ credential.label }}</strong> · {{ credential.active ? '可用' : credential.revoked_at ? '已撤销' : '已失效' }}
            <small>创建于 {{ time(credential.created_at) }}</small>
            <button v-if="credential.active" type="button" :disabled="busy" @click="perform('revoke', credential.credential_id)">撤销 {{ credential.label }}</button>
          </li>
        </ul>
        <p v-if="!state.credentials.length">尚未创建接入凭证。</p>
        <p>凭证仅用于 MCP 读取与次数查询。改密或账号授权失效后需重新创建；退出网页不会撤销凭证。</p>
        <details><summary>交给 Agent 的阅读说明</summary><p>{{ agentNote }}</p><button type="button" @click="copy(agentNote)">复制阅读说明</button></details>
      </section>
      <section aria-label="MCP 次数">
        <h3>调用次数</h3>
        <p>免费剩余 <strong>{{ state.usage.free_remaining }}</strong> 次 · 已购可用 <strong>{{ state.usage.paid_remaining }}</strong> 次</p>
        <p>当月已用 {{ state.usage.free_used }} / {{ state.usage.free_granted }} 次免费额度。</p>
        <p>免费恢复：{{ time(state.usage.free_resets_at) }}（香港时间）</p>
        <p v-if="state.usage.free_remaining === 0 && state.usage.paid_remaining === 0">次数已用完，内容调用暂停；可等待免费恢复或购买调用包，目录和次数查询仍可使用。</p>
        <ul class="packs">
          <li v-for="pack in state.usage.call_packs" :key="pack.pack_id">
            <strong>{{ pack.pack_count }} 包 · 可用 {{ pack.available_units }} 次</strong>
            <span>已用 {{ pack.used_units }} 次；未用 {{ pack.remaining_units }} 次</span>
            <span>开通 {{ time(pack.opened_at) }} · 到期 {{ time(pack.expires_at) }}（香港时间）</span>
            <span v-if="pack.available_units === 0 && pack.remaining_units > 0">已不在有效期内，未用次数不可使用。</span>
          </li>
        </ul>
        <p v-if="!state.usage.call_packs.length">尚无已购调用包。</p>
      </section>
      <section aria-label="购买调用包">
        <h3>购买调用包</h3>
        <p>每包 10 元，包含 1,000 次内容调用，可重复购买。请联系向你提供内测资格的管理员，提供本页账号、购买包数，并按管理员给出的付款方式付款；管理员确认收款后开通。</p>
        <p>开通后有效 12 个日历月，剩余次数在有效期内跨月保留。先用免费次数，再用最早到期的有效调用包；全部耗尽后停止内容调用。</p>
        <p>每月免费 1,000 次，每月 1 日香港时间 00:00 恢复，未用免费次数不累计。成功的空检索、每一续页和重复请求均计次；目录、次数查询和失败请求不计次。</p>
        <p>购买增加调用次数，材料权限需由管理员另行开通。MCP 次数与托管模型使用额度分别计算。</p>
      </section>
    </template>
  </dialog>
</template>

<style scoped>
.account-mcp { width: min(44rem, calc(100vw - 2rem)); max-height: calc(100dvh - 2rem); overflow: auto; box-sizing: border-box; border: 1px solid var(--border-color, #ccc); border-radius: 16px; padding: 24px; color: var(--text-primary, #252525); background: var(--bg-primary, #fff); overflow-wrap: anywhere; }
.account-mcp::backdrop { background: #0006; }
header { display: flex; justify-content: space-between; align-items: center; gap: 12px; }
h2 { margin: 0; } section { border-top: 1px solid var(--border-color, #ddd); margin-top: 20px; padding-top: 8px; }
label { display: grid; gap: 6px; margin: 12px 0; } input, textarea { box-sizing: border-box; width: 100%; padding: 10px; font: inherit; }
button { padding: 8px 12px; margin: 4px 6px 4px 0; cursor: pointer; } li { margin: 12px 0; } small, .packs span { display: block; margin-top: 4px; }
.new-credential { padding: 12px; background: var(--bg-secondary, #f3f4f3); border-radius: 8px; } [role="alert"] { color: #a32620; }
</style>
