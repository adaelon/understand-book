<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from 'vue';
import { accountAllowance } from '../network-client';
import AccountCostSummary from './AccountCostSummary.vue';
import CloudPlans from './CloudPlans.vue';
import { network, sceneKey } from '../network-context';
import { allowancePercent, allowanceTime, offerPercentage, paymentAmount, usageOccupation, usagePurpose, usageState, type AccountAllowanceSummary, type AccountUsagePage } from '../account-allowance';

const emit = defineEmits<{ (e: 'close'): void }>();
const dialog = ref<HTMLDialogElement | null>(null);
const summary = ref<AccountAllowanceSummary | null>(null), usage = ref<AccountUsagePage | null>(null);
const allowance = computed(() => summary.value?.current_allowance);
const percentages = computed(() => allowance.value?.balance.percentages);
const busy = ref(false), error = ref(''), loaded = ref(false), supplement = ref(false);
const showCosts = ref(false);
const showPlans = ref(false);
const limit = 20;
const operatorContact = (import.meta.env.VITE_READER_OPERATOR_CONTACT ?? '').trim();
let request = 0;
async function load(offset = 0) {
  const ticket = ++request, scene = sceneKey();
  busy.value = true; error.value = ''; loaded.value = false;
  summary.value = null; usage.value = null;
  if (!network.value.identity) { busy.value = false; return; }
  try {
    const [balance, page] = await Promise.all([accountAllowance.read(), accountAllowance.usage(limit, offset)]);
    if (ticket !== request || scene !== sceneKey()) return;
    summary.value = balance; usage.value = page; loaded.value = true;
  } catch (failure) {
    if (ticket === request && scene === sceneKey()) error.value = failure instanceof Error ? failure.message : '额度暂时无法读取';
  } finally { if (ticket === request) busy.value = false; }
}
watch(() => `${network.value.epoch}:${network.value.identity?.user_id}`, () => { supplement.value = false; void load(); }, { immediate: true, flush: 'sync' });
onMounted(() => { dialog.value?.showModal(); });
onBeforeUnmount(() => { request++; dialog.value?.close(); });
</script>

<template>
  <dialog ref="dialog" class="account-allowance" aria-labelledby="allowance-title" @cancel.prevent="emit('close')">
    <header><h2 id="allowance-title">AI 使用额度</h2><button type="button" aria-label="关闭使用额度" @click="emit('close')">关闭</button></header>
    <div class="allowance-toolbar"><span>学习对话、记忆整理与资料准备共用账号额度</span><button type="button" :disabled="busy" @click="load()">刷新额度与明细</button></div>
    <button type="button" :aria-expanded="showPlans" @click="showPlans = !showPlans">{{ showPlans ? '收起套餐与权益' : '套餐与权益 · 月付 / 冲刺' }}</button>
    <CloudPlans v-if="showPlans" />
    <p v-if="busy" role="status">正在读取使用额度…</p>
    <p v-if="error" role="alert">{{ error }}。请点击刷新重试。</p>
    <template v-if="loaded">
      <template v-if="allowance">
        <section class="allowance-overview" aria-label="当前额度">
          <dl class="allowance-balances">
            <div class="available"><dt>可用额度</dt><dd>{{ allowancePercent(percentages?.available) }}</dd></div>
            <div><dt>本期已用</dt><dd>{{ allowancePercent(percentages?.debited) }}</dd></div>
          </dl>
          <p class="allowance-expiry">{{ allowanceTime(allowance.expires_at) }} 到期 · 香港时间</p>
        </section>
        <p v-if="!percentages" role="status">本期尚未配置百分比基准，请联系运营者完成配置；原使用记录仍保留。</p>
        <dl v-else class="allowance-calculation">
          <div><dt>正在使用的预留</dt><dd>{{ allowancePercent(percentages.active_reserved) }}</dd></div>
          <div><dt>待核算占用</dt><dd>{{ allowancePercent(percentages.pending) }}</dd></div>
        </dl>
        <p v-if="percentages?.active_reserved.nonzero || percentages?.pending.nonzero" class="allowance-occupation">预留与待核算占用已经从可用额度中扣除，核算后会更新。占用不等于最终消耗。</p>
        <p v-if="percentages?.deficit.nonzero" role="status">已发生费用超出剩余额度 {{ allowancePercent(percentages.deficit) }}，可用额度显示为 0%。</p>
        <p v-if="percentages && !percentages.available.nonzero" role="status">本期可用额度已用尽，请补充额度后继续。</p>
      </template>
      <p v-else role="status">{{ summary?.status === 'expired' ? '上一额度期已到期。' : summary?.status === 'scheduled' ? '下一额度期尚未生效。' : summary?.status === 'closed' ? '上一额度期已关闭。' : '当前没有有效额度期。' }}请联系为你开通账号的运营者。</p>
      <p v-if="!allowance && summary?.latest_allowance" class="explanation">最近额度期：{{ allowanceTime(summary.latest_allowance.starts_at) }} 至 {{ allowanceTime(summary.latest_allowance.expires_at) }}（香港时间）</p>
      <button type="button" :aria-expanded="supplement" @click="supplement = !supplement">补充额度</button>
      <section v-if="supplement" class="supplement" aria-label="补充额度">
        <h3>补充 AI 使用额度</h3>
        <ul v-if="summary?.replenishment.offers.length" class="usage-list">
          <li v-for="offer in summary.replenishment.offers" :key="offer.offer_id">
            <strong>{{ offer.title }} · 增加 {{ offerPercentage(offer.delta_milli_percent) }}</strong>
            <p>支付 ¥{{ paymentAmount(offer.amount_fen) }} · {{ allowanceTime(allowance!.expires_at) }} 到期（香港时间）</p>
            <p>{{ offer.instructions }}</p>
          </li>
        </ul>
        <p v-else>当前暂无可直接选择的补充方案。</p>
        <p>请联系运营者，确认实际支付金额、增加的额度百分比和有效期。人工确认收款后，额度会加入账号；可点击“刷新额度与明细”查看。</p>
        <p v-if="summary?.replenishment.contact || operatorContact">联系方式：{{ summary?.replenishment.contact || operatorContact }}</p>
        <p v-else>请联系为你开通账号的运营者。</p>
        <p>补充额度只增加可用比例，100% 的基准与旧明细保持不变，余额可以超过 100%。</p>
      </section>
      <details class="allowance-details"><summary>额度怎么算</summary>
        <p class="explanation">同一额度期以固定标准额度为 100%，补充额度不会改变历史用量的比例。这是 AI 服务的使用额度。</p>
        <dl v-if="allowance" class="allowance-calculation"><div><dt>本期授予（含调整）</dt><dd>{{ allowancePercent(percentages?.granted) }}</dd></div></dl>
        <p v-if="allowance">有效期：{{ allowanceTime(allowance.starts_at) }} 至 {{ allowanceTime(allowance.expires_at) }}（香港时间）</p>
        <p class="explanation">小额非零消耗显示“低于 0.1%”，仍完整计入本期合计。不同额度期分别使用原期基准，不合并成一个百分比。</p>
      </details>
      <details class="allowance-details"><summary>额度不足或到期后怎么办</summary>
        <p class="explanation">额度不足或到期时，仍可在现有授权范围内阅读原文、历史和已有成果。补充额度后，返回对话点击“继续任务”或发送问题；不会自动继续。</p>
      </details>
      <button type="button" :aria-expanded="showCosts" @click="showCosts = !showCosts">{{ showCosts ? '收起任务与工作区汇总' : '按任务与工作区查看' }}</button>
      <AccountCostSummary v-if="showCosts" />
      <h3>使用记录 <small>全部额度期 · {{ usage?.total ?? 0 }} 条</small></h3>
      <details v-if="usage?.period_groups.length" class="allowance-details"><summary>按原额度期查看合计</summary>
        <section v-for="group in usage.period_groups" :key="group.period_id" class="period-group">
          <p>额度期 {{ group.period_id }} · 基准 {{ group.allowance_basis?.version ?? '待配置' }}</p>
          <dl class="allowance-calculation">
            <div><dt>已用</dt><dd>{{ allowancePercent(group.percentages?.debited) }}</dd></div>
            <div><dt>正在使用的预留</dt><dd>{{ allowancePercent(group.percentages?.active_reserved) }}</dd></div>
            <div><dt>待核算占用</dt><dd>{{ allowancePercent(group.percentages?.pending) }}</dd></div>
          </dl>
          <p v-if="group.unknown_provider_cost_count">{{ group.unknown_provider_cost_count }} 次调用费用仍待核算。</p>
        </section>
      </details>
      <ul v-if="usage?.items.length" class="usage-list">
        <li v-for="item in usage.items" :key="item.call_id">
          <div class="usage-heading"><time>{{ allowanceTime(item.created_at) }}</time><strong>{{ usageState[item.state] }}</strong></div>
          <p>{{ usagePurpose(item.purpose) }}<span v-if="item.entry_kind === 'service' && item.quantity_ms != null"> · {{ (item.quantity_ms / 1000).toFixed(1) }} 秒</span></p>
          <dl class="usage-amounts"><div><dt>已用额度</dt><dd>{{ item.account_debit_micro_cny === null ? (item.state === 'pending' ? '待核算' : item.state === 'released' ? '0%' : '尚未结算') : allowancePercent(item.allowance_percentages?.debited) }}</dd></div><div><dt>当前占用</dt><dd>{{ usageOccupation(item) }}</dd></div></dl>
          <details><summary>原额度期与任务</summary><p>额度期：{{ item.period_id }} · 基准 {{ item.allowance_basis?.version ?? '待配置' }}</p><p>调用：{{ item.call_id }}</p><p v-if="item.run_ref">任务：{{ item.run_ref }}</p><p v-if="item.task_ref">后台任务：{{ item.task_ref }}</p></details>
        </li>
      </ul>
      <p v-else>暂无使用记录。</p>
      <nav v-if="usage && usage.total > 0" aria-label="使用记录分页">
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
.allowance-toolbar { display: flex; align-items: center; justify-content: space-between; gap: 12px; margin: 18px 0; font-size: 12px; color: var(--muted); }
.allowance-overview { padding: 24px; margin: 20px 0; background: var(--surface); border: 1px solid var(--line); border-radius: 12px; }
.allowance-balances { display: grid; grid-template-columns: 1.4fr 1fr; align-items: end; gap: 16px; margin: 0; }
.allowance-balances > div { min-width: 0; }
.allowance-balances dt { font-size: 13px; color: var(--steel); }
dd { margin: 6px 0 0; overflow-wrap: anywhere; font-variant-numeric: tabular-nums; } .available dd { font-size: 38px; line-height: 1.2; color: var(--accent); }
.allowance-balances > div:not(.available) dd { font-size: 23px; }
.allowance-expiry { margin: 18px 0 0; padding-top: 14px; border-top: 1px solid var(--line); font-size: 12px; color: var(--steel); }
.allowance-occupation { color: var(--muted); font-size: 13px; }
.allowance-details { border-bottom: 1px solid var(--line); padding: 12px 0; font-size: 13px; }
.allowance-calculation { display: grid; gap: 8px; }.allowance-calculation > div { display: flex; justify-content: space-between; gap: 16px; }.allowance-calculation dd { margin: 0; }
.supplement { margin-top: 16px; padding: 16px; border: 1px solid var(--line); border-radius: 10px; } .supplement h3 { margin-top: 0; } .period-group { padding: 10px 0; border-bottom: 1px solid var(--line); }
.usage-list { padding: 0; list-style: none; } .usage-list > li { border-top: 1px solid var(--line); padding: 16px 0; }
.usage-heading { flex-wrap: wrap; } .usage-amounts { display: flex; flex-wrap: wrap; gap: 24px; } details { font-size: 12px; color: var(--muted); } summary { cursor: pointer; } footer { margin-top: 24px; }
@media (max-width: 540px) {
  .account-allowance { padding: 16px; }
  h2 { font-size: 18px; }
  .allowance-balances { grid-template-columns: 1fr; align-items: start; gap: 18px; }
  .allowance-balances dd { white-space: nowrap; }
  .allowance-overview { padding: 18px; }
  .allowance-toolbar { flex-wrap: wrap; }
  .allowance-toolbar button { white-space: nowrap; flex-shrink: 0; }
  nav { gap: 6px; font-size: 13px; }
}
</style>
