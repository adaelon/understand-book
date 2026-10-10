<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref } from 'vue';
import { networkFetch } from '../network-client';
import { sceneKey } from '../network-context';
import { allowanceAmount, allowanceTime, usageOccupation, usageState, type AllowancePeriod, type AccountUsagePage } from '../account-allowance';

const emit = defineEmits<{ (e: 'close'): void }>();
const dialog = ref<HTMLDialogElement | null>(null);
const allowance = ref<AllowancePeriod | null>(null), usage = ref<AccountUsagePage | null>(null);
const busy = ref(false), error = ref(''), loaded = ref(false);
const limit = 20;
const operatorContact = (import.meta.env.VITE_READER_OPERATOR_CONTACT ?? '').trim();
let request = 0;
async function load(offset = 0) {
  const ticket = ++request, scene = sceneKey();
  busy.value = true; error.value = ''; loaded.value = false;
  allowance.value = null; usage.value = null;
  try {
    const [summary, page] = await Promise.all([
      networkFetch<{ current_allowance: AllowancePeriod | null }>('GET', '/account/allowance'),
      networkFetch<AccountUsagePage>('GET', `/account/usage?limit=${limit}&offset=${offset}`),
    ]);
    if (ticket !== request || scene !== sceneKey()) return;
    allowance.value = summary.current_allowance; usage.value = page; loaded.value = true;
  } catch (failure) {
    if (ticket === request && scene === sceneKey()) error.value = failure instanceof Error ? failure.message : '额度暂时无法读取';
  } finally { if (ticket === request) busy.value = false; }
}
onMounted(() => { dialog.value?.showModal(); void load(); });
onBeforeUnmount(() => { request++; dialog.value?.close(); });
</script>

<template>
  <dialog ref="dialog" class="account-allowance" aria-labelledby="allowance-title" @cancel.prevent="emit('close')">
    <header><h2 id="allowance-title">AI 使用额度（元）</h2><button type="button" aria-label="关闭使用额度" @click="emit('close')">关闭</button></header>
    <p class="explanation">这是用于 AI 模型调用的使用额度，与实际付款金额分别记录，不是现金余额。</p>
    <p v-if="operatorContact">额度开通与续用联系方式：{{ operatorContact }}</p>
    <button type="button" :disabled="busy" @click="load()">刷新额度与明细</button>
    <p v-if="busy" role="status">正在读取使用额度…</p>
    <p v-if="error" role="alert">{{ error }}。请点击刷新重试。</p>
    <template v-if="loaded">
      <template v-if="allowance">
        <dl class="allowance-balances">
          <div class="available"><dt>本期可用</dt><dd>{{ allowanceAmount(allowance.balance.available_micro_cny) }}</dd></div>
          <div><dt>本期授予（含调整）</dt><dd>{{ allowanceAmount(allowance.balance.granted_micro_cny) }}</dd></div>
          <div><dt>本期已用</dt><dd>{{ allowanceAmount(allowance.balance.debited_micro_cny) }}</dd></div>
          <div><dt>正在使用的预留</dt><dd>{{ allowanceAmount(allowance.balance.active_reserved_micro_cny) }}</dd></div>
          <div><dt>待核算占用</dt><dd>{{ allowanceAmount(allowance.balance.pending_micro_cny) }}</dd></div>
        </dl>
        <p>有效期：{{ allowanceTime(allowance.starts_at) }} 至 {{ allowanceTime(allowance.expires_at) }}（香港时间）</p>
        <p v-if="allowance.balance.available_micro_cny <= 0" role="status">本期可用额度已用尽{{ allowance.balance.available_micro_cny < 0 ? '，负数为已发生费用超出的部分' : '' }}，请联系为你开通账号的运营者补充额度。</p>
      </template>
      <p v-else role="status">当前没有有效额度期，可能尚未开通或已到期。请联系为你开通账号的运营者。</p>
      <p class="explanation">可用额度已扣除正在使用的预留和待核算占用。调用结果或用量尚未确认时，预留继续占用额度，待核算后更新；预留金额不是最终费用。</p>
      <p class="explanation">额度不足或到期时，仍可在现有授权范围内阅读原文、历史和已有成果。补充额度后，返回对话点击“继续任务”或发送问题；不会自动继续。</p>
      <h3>调用明细 <small>全部额度期 · {{ usage?.total ?? 0 }} 条</small></h3>
      <ul v-if="usage?.items.length" class="usage-list">
        <li v-for="item in usage.items" :key="item.call_id">
          <div class="usage-heading"><time>{{ allowanceTime(item.created_at) }}</time><strong>{{ usageState[item.state] }}</strong></div>
          <p>{{ item.model }} · {{ item.purpose }}</p>
          <dl class="usage-amounts"><div><dt>已扣减（元）</dt><dd>{{ item.account_debit_micro_cny === null && item.state !== 'pending' ? '尚未结算' : allowanceAmount(item.account_debit_micro_cny) }}</dd></div><div><dt>当前占用（元）</dt><dd>{{ allowanceAmount(usageOccupation(item)) }}</dd></div></dl>
          <details><summary>调用归属</summary><p>额度期：{{ item.period_id }}</p><p>调用：{{ item.call_id }}</p><p v-if="item.run_ref">任务：{{ item.run_ref }}</p><p v-if="item.task_ref">后台任务：{{ item.task_ref }}</p></details>
        </li>
      </ul>
      <p v-else>暂无调用记录。</p>
      <nav v-if="usage && usage.total > 0" aria-label="调用明细分页">
        <button :disabled="busy || usage.offset === 0" @click="load(Math.max(0, usage.offset - limit))">上一页</button>
        <span>{{ usage.offset + 1 }}–{{ usage.offset + usage.items.length }} / {{ usage.total }}</span>
        <button :disabled="busy || usage.offset + usage.limit >= usage.total" @click="load(usage.offset + usage.limit)">下一页</button>
      </nav>
    </template>
    <footer><button type="button" @click="emit('close')">返回阅读与对话</button></footer>
  </dialog>
</template>

<style scoped>
.account-allowance { width: min(680px, calc(100vw - 24px)); max-height: calc(100dvh - 24px); box-sizing: border-box; padding: 24px; border: 1px solid var(--line); border-radius: 14px; color: var(--ink); background: var(--canvas); overflow: auto; }
.account-allowance::backdrop { background: #0006; }
header, .usage-heading, nav { display: flex; align-items: center; justify-content: space-between; gap: 12px; }
h2 { margin: 0; font-size: 21px; } h3 { margin-top: 28px; } small { font-size: 12px; font-weight: normal; }
p { line-height: 1.7; overflow-wrap: anywhere; } .explanation, dt, small { color: var(--muted); }
button { font: inherit; cursor: pointer; padding: 8px 12px; border: 1px solid var(--line); border-radius: 6px; background: var(--surface); color: var(--ink); } button:disabled { cursor: default; opacity: .5; }
.allowance-balances { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 12px; margin: 20px 0; }
.allowance-balances > div { padding: 14px; background: var(--surface); border: 1px solid var(--line); border-radius: 8px; }
.allowance-balances .available { grid-column: 1 / -1; } dd { margin: 6px 0 0; overflow-wrap: anywhere; font-variant-numeric: tabular-nums; } .available dd { font-size: 28px; }
.usage-list { padding: 0; list-style: none; } .usage-list > li { border-top: 1px solid var(--line); padding: 16px 0; }
.usage-heading { flex-wrap: wrap; } .usage-amounts { display: flex; flex-wrap: wrap; gap: 24px; } details { font-size: 12px; color: var(--muted); } summary { cursor: pointer; } footer { margin-top: 24px; }
@media (max-width: 540px) { .account-allowance { padding: 16px; } h2 { font-size: 18px; } .allowance-balances { gap: 8px; } .allowance-balances > div { padding: 10px; } nav { gap: 6px; font-size: 13px; } }
</style>
