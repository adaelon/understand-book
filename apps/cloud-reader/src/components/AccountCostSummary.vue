<script setup lang="ts">
import { t as uiText } from '@understand-book/web/reader/i18n';

import { onBeforeUnmount, ref, watch } from 'vue';
import { accountAllowance } from '../network-client';
import { network, sceneKey } from '@understand-book/web/reader/reader-context';
import { allowancePercent, allowanceTime, usagePurpose, usageState, type AccountUsagePage } from '../account-allowance';
import { accountCostCategories, accountCostQuery, type AccountCostFilter, type AccountCostReport, type AccountCostSummary } from '../account-costs';

const report = ref<AccountCostReport | null>(null), busy = ref(false), error = ref('');
const mode = ref<'account' | 'tasks' | 'workspaces' | 'memory'>('account');
const selected = ref<{ title: string; filter: AccountCostFilter } | null>(null), details = ref<AccountUsagePage | null>(null);
let request = 0, detailRequest = 0;
async function load(offset = 0) {
  const ticket = ++request, scene = sceneKey(); busy.value = true; error.value = ''; report.value = null;
  try { const value = await accountAllowance.costs(offset); if (ticket === request && scene === sceneKey()) report.value = value; }
  catch (failure) { if (ticket === request && scene === sceneKey()) error.value = failure instanceof Error ? failure.message : uiText('汇总暂时无法读取'); }
  finally { if (ticket === request) busy.value = false; }
}
async function drill(title: string, filter: AccountCostFilter, offset = 0) {
  const ticket = ++detailRequest, scene = sceneKey(); selected.value = { title, filter }; details.value = null; error.value = '';
  try { const value = await accountAllowance.costUsage(accountCostQuery(filter, offset)); if (ticket === detailRequest && scene === sceneKey()) details.value = value; }
  catch (failure) { if (ticket === detailRequest && scene === sceneKey()) error.value = failure instanceof Error ? failure.message : uiText('明细暂时无法读取'); }
}
function closeDetail() { detailRequest++; selected.value = null; details.value = null; }
function rows(): { key: string; title: string; summary: AccountCostSummary; filter?: AccountCostFilter; related?: string[]; reference?: string }[] {
  if (!report.value) return [];
  if (mode.value === 'account') return [{ key: 'account', title: uiText('账号合计'), summary: report.value.account }];
  if (mode.value === 'tasks') return report.value.tasks.items.map(task => ({ key: task.parent_task_ref, title: `${[...new Set(task.purposes.map(usagePurpose))].join('、') || uiText('学习任务')} · ${allowanceTime(task.latest_at)}`, summary: task, filter: { parent_task_ref: task.parent_task_ref }, related: [...new Set(task.related_learning_workspace_ids.map(id => report.value!.workspaces.find(ws => ws.learning_workspace_id === id)?.workspace_goal || uiText('其他来源工作区')))], reference: task.parent_task_ref }));
  if (mode.value === 'workspaces') return report.value.workspaces.map(ws => ({ key: ws.learning_workspace_id, title: ws.workspace_goal || uiText('学习工作区'), summary: ws, filter: { learning_workspace_id: ws.learning_workspace_id }, reference: ws.learning_workspace_id }));
  return report.value.account_level.map((summary, i) => ({ key: String(i), title: uiText('账号级整理'), summary, filter: { account_level: true } }));
}
watch(() => `${network.value.epoch}:${network.value.identity?.user_id}`, () => { closeDetail(); mode.value = 'account'; void load(); }, { immediate: true, flush: 'sync' });
onBeforeUnmount(() => { request++; detailRequest++; });
</script>

<template>
  <section class="cost-summary" :aria-label="uiText('任务与工作区用量')">
    <div class="heading"><h3>{{ uiText('任务与工作区用量') }}</h3><button :disabled="busy" @click="closeDetail(); load()">{{ uiText('刷新汇总') }}</button></div>
    <p>{{ uiText('前台回答与后续整理归入原任务。账号级整理单独列出，相关工作区不会重复分摊。各额度期使用原基准，分别显示比例。') }}</p>
    <nav :aria-label="uiText('用量分组')"><button v-for="item in ([['account',uiText('账号合计')],['tasks',uiText('按任务')],['workspaces',uiText('按工作区')],['memory',uiText('账号级整理')]] as const)" :key="item[0]" :aria-pressed="mode === item[0]" @click="mode = item[0]; closeDetail()">{{ item[1] }}</button></nav>
    <p v-if="busy" role="status">{{ uiText('正在读取汇总…') }}</p><p v-if="error" role="alert">{{ error }}</p>
    <template v-if="report">
      <article v-for="row in rows()" :key="row.key" class="cost-row">
        <div class="heading"><h4>{{ row.title }}</h4><button v-if="row.filter" @click="drill(row.title, row.filter)">{{ uiText('查看调用明细') }}</button></div>
        <p>{{ row.summary.request_count }} {{ uiText('次调用') }}<span v-if="row.summary.unknown_cost_requests"> · {{ row.summary.unknown_cost_requests }} {{ uiText('次费用尚未确认') }}</span></p>
        <dl :aria-label="uiText('调用分类')"><div v-for="[kind, label] in accountCostCategories" :key="kind" :aria-label="uiText('{0}调用', [label])"><dt>{{ label }}</dt><dd>{{ row.summary.cost_breakdown[kind].request_count }} {{ uiText('次调用') }}<span v-if="row.summary.cost_breakdown[kind].estimated_cost_requests"> · {{ row.summary.cost_breakdown[kind].estimated_cost_requests }} {{ uiText('次成本为估计') }}</span><span v-if="row.summary.cost_breakdown[kind].unknown_cost_requests"> · {{ row.summary.cost_breakdown[kind].unknown_cost_requests }} {{ uiText('次费用尚未确认') }}</span><span v-if="row.summary.cost_breakdown[kind].pending_requests"> · {{ row.summary.cost_breakdown[kind].pending_requests }} {{ uiText('次待核算') }}</span><span v-if="row.summary.cost_breakdown[kind].reconciliation_requests"> · {{ row.summary.cost_breakdown[kind].reconciliation_requests }} {{ uiText('次待核对') }}</span></dd></div></dl>
        <p v-if="row.related?.length">{{ uiText('关联工作区：') }}{{ row.related.join('、') }}{{ uiText('。关联仅说明来源，不追加扣减。') }}</p>
        <details v-if="row.reference"><summary>{{ uiText('核对任务标识') }}</summary><p>{{ row.reference }}</p></details>
        <section v-for="group in row.summary.period_groups" :key="group.period_id" class="cost-period">
          <p>{{ group.starts_at != null && group.expires_at != null ? uiText('{0} 至 {1}', [allowanceTime(group.starts_at), allowanceTime(group.expires_at)]) : uiText('原额度期') }}</p>
          <dl><div><dt>{{ uiText('已用额度') }}</dt><dd>{{ allowancePercent(group.percentages?.debited) }}</dd></div><div><dt>{{ uiText('正在使用的预留') }}</dt><dd>{{ allowancePercent(group.percentages?.active_reserved) }}</dd></div><div><dt>{{ uiText('待核算占用') }}</dt><dd>{{ allowancePercent(group.percentages?.pending) }}</dd></div></dl>
          <details><summary>{{ uiText('核对原额度期与基准') }}</summary><p>{{ group.period_id }} · {{ group.allowance_basis?.version ?? uiText('待配置') }}</p></details>
        </section>
        <p v-if="!row.summary.period_groups.length">{{ uiText('暂无用量记录。') }}</p>
      </article>
      <p v-if="!rows().length">{{ uiText('暂无该类用量记录。') }}</p>
      <nav v-if="mode === 'tasks' && report.tasks.total" :aria-label="uiText('汇总任务分页')"><button :disabled="busy || !report.tasks.offset" @click="closeDetail(); load(Math.max(0, report.tasks.offset - 20))">{{ uiText('上一页任务') }}</button><span>{{ report.tasks.offset + 1 }}–{{ report.tasks.offset + report.tasks.items.length }} / {{ report.tasks.total }}</span><button :disabled="busy || report.tasks.offset + report.tasks.limit >= report.tasks.total" @click="closeDetail(); load(report.tasks.offset + 20)">{{ uiText('下一页任务') }}</button></nav>
      <p class="updated">{{ uiText('更新于') }} {{ allowanceTime(report.as_of) }}{{ uiText('（香港时间）；后续核算会更新原任务。') }}</p>
    </template>
    <section v-if="selected" class="cost-detail" :aria-label="uiText('所选用量明细')">
      <div class="heading"><h4>{{ selected.title }} {{ uiText('· 调用明细') }}</h4><button @click="closeDetail">{{ uiText('关闭明细') }}</button></div>
      <ul v-if="details"><li v-for="item in details.items" :key="item.call_id"><strong>{{ usagePurpose(item.purpose) }} · {{ usageState[item.state] }}</strong><p>{{ uiText('已用：') }}{{ item.account_debit_micro_cny === null ? uiText('尚未核算') : allowancePercent(item.allowance_percentages?.debited) }} · {{ allowanceTime(item.created_at) }}</p><p v-if="item.provider_cost_status === 'pending' || item.needs_reconciliation">{{ uiText('费用依据仍待核对。') }}</p><details><summary>{{ uiText('核对调用与原额度期') }}</summary><p>{{ uiText('调用') }} {{ item.call_id }} {{ uiText('· 额度期') }} {{ item.period_id }} {{ uiText('· 基准') }} {{ item.allowance_basis?.version ?? uiText('待配置') }}</p></details></li></ul>
      <p v-else-if="!error" role="status">{{ uiText('正在读取明细…') }}</p>
      <nav v-if="details?.total" :aria-label="uiText('所选用量分页')"><button :disabled="!details.offset" @click="drill(selected.title, selected.filter, Math.max(0, details.offset - 20))">{{ uiText('上一页明细') }}</button><span>{{ details.offset + 1 }}–{{ details.offset + details.items.length }} / {{ details.total }}</span><button :disabled="details.offset + details.limit >= details.total" @click="drill(selected.title, selected.filter, details.offset + 20)">{{ uiText('下一页明细') }}</button></nav>
    </section>
  </section>
</template>

<style scoped>
.cost-summary { margin: 18px 0; padding: 16px; border: 1px solid var(--line); border-radius: 10px; } h3,h4 { margin: 0; overflow-wrap: anywhere; } .heading, nav { display: flex; flex-wrap: wrap; align-items: center; justify-content: space-between; gap: 8px; } nav { justify-content: flex-start; margin: 12px 0; } p { font-size: 13px; line-height: 1.7; overflow-wrap: anywhere; } button { font: inherit; padding: 7px 10px; color: var(--ink); background: var(--surface); border: 1px solid var(--line); border-radius: 6px; cursor: pointer; } button[aria-pressed=true] { border-color: var(--accent); color: var(--accent); } button:disabled { opacity: .5; } .cost-row,.cost-detail { border-top: 1px solid var(--line); padding: 16px 0; } .cost-period { padding: 6px 0; } dl { display: grid; gap: 6px; font-size: 13px; } dl>div { display: flex; justify-content: space-between; gap: 16px; } dd { margin: 0; } .updated,dt { color: var(--muted); } ul { padding: 0; list-style: none; } li { padding: 10px 0; border-bottom: 1px solid var(--line); }
</style>
