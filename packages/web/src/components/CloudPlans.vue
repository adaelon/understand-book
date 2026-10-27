<script setup lang="ts">
import { t as uiText } from '../i18n';

import { computed, onBeforeUnmount, ref, watch } from 'vue';
import { allowanceTime, offerPercentage, paymentAmount } from '../account-allowance';
import { accountCloudPlans, cloudPlanDurations, cloudPlanTiers, cloudRetention, cloudStorageSize, saleableCloudPlans, type AccountCloudPlans, type CloudPlanDuration, type CloudPlanProduct, type CloudStorageSummary } from '../cloud-plans';
import { network, sceneKey } from '../network-context';

const summary = ref<AccountCloudPlans | null>(null);
const storage = ref<CloudStorageSummary | null>(null), storageError = ref('');
const duration = ref<CloudPlanDuration>('day3');
const selected = ref<CloudPlanProduct | null>(null);
const busy = ref(false), error = ref('');
const products = computed(() => saleableCloudPlans(summary.value?.catalog.items ?? [], duration.value));
const contact = (import.meta.env.VITE_READER_OPERATOR_CONTACT ?? '').trim();
let request = 0;
async function load() {
  const ticket = ++request, scene = sceneKey();
  busy.value = true; error.value = ''; storageError.value = ''; summary.value = null; storage.value = null; selected.value = null;
  if (!network.value.identity) { busy.value = false; return; }
  try {
    const [plans, space] = await Promise.allSettled([accountCloudPlans.read(), accountCloudPlans.storage()]);
    if (ticket !== request || scene !== sceneKey()) return;
    if (plans.status === 'fulfilled') summary.value = plans.value;
    else error.value = plans.reason instanceof Error ? plans.reason.message : uiText('套餐暂时无法读取');
    if (space.status === 'fulfilled') storage.value = space.value;
    else storageError.value = uiText('空间用量暂时无法读取，请刷新重试。');
  } catch (failure) {
    if (ticket === request && scene === sceneKey()) error.value = failure instanceof Error ? failure.message : uiText('套餐暂时无法读取');
  } finally { if (ticket === request) busy.value = false; }
}
watch(() => `${network.value.epoch}:${network.value.identity?.user_id}`, () => { void load(); }, { immediate: true, flush: 'sync' });
watch(duration, () => { selected.value = null; });
onBeforeUnmount(() => { request++; });
</script>

<template>
  <section class="cloud-plans" :aria-label="uiText('云端学习套餐')">
    <header><h3>{{ uiText('云端学习套餐') }}</h3><button type="button" :disabled="busy" @click="load()">{{ uiText('刷新套餐与权益') }}</button></header>
    <p>{{ uiText('网站与 Windows 共用账号、服务额度和个人空间。') }}</p>
    <p v-if="busy" role="status">{{ uiText('正在读取套餐与权益…') }}</p>
    <p v-if="error" role="alert">{{ error }}{{ uiText('。请点击刷新重试。') }}</p>
    <section v-if="storage" class="entitlement" :aria-label="uiText('个人云端空间')">
      <h4>{{ uiText('个人云端空间') }}</h4>
      <p>{{ uiText('空间用于保存上传的书籍、视频和课件原件。演示页、笔记等学习成果不占上传空间，保存无需存储套餐。') }}</p>
      <p>{{ uiText('已保存') }} {{ cloudStorageSize(storage.used_bytes) }} {{ uiText('· 上传中') }} {{ cloudStorageSize(storage.reserved_bytes) }}<template v-if="storage.limit_bytes !== null"> {{ uiText('· 上限') }} {{ cloudStorageSize(storage.limit_bytes) }}</template></p>
      <p v-if="storage.can_upload">{{ uiText('还能使用') }} {{ cloudStorageSize(storage.available_bytes) }}。</p>
      <p v-else-if="storage.over_limit">{{ uiText('现有资料超过当前空间上限，请在个人资料区域整理后继续上传。') }}</p>
      <p v-else>{{ uiText('当前不能新增上传，已保留的私人资料可在个人资料区域查看、导出和整理。') }}</p>
      <p v-if="!storage.can_upload && storage.retained_until">{{ uiText('私人资料保留至') }} {{ allowanceTime(storage.retained_until) }}{{ uiText('（香港时间）。') }}</p>
    </section>
    <p v-if="storageError" role="status">{{ storageError }}</p>
    <template v-if="summary">
      <section v-if="summary.current" class="entitlement" :aria-label="uiText('当前套餐')">
        <h4>{{ uiText('当前：') }}{{ summary.current.title }}</h4>
        <p>{{ cloudPlanTiers[summary.current.tier] }} · {{ cloudPlanDurations[summary.current.duration] }}</p>
        <p>{{ allowanceTime(summary.current.starts_at) }} {{ uiText('至') }} {{ allowanceTime(summary.current.expires_at) }}{{ uiText('（香港时间）') }}</p>
        <p>{{ uiText('本期套餐额度') }} {{ offerPercentage(summary.current.grant_milli_percent) }} {{ uiText('· 个人空间') }} {{ cloudStorageSize(summary.current.storage_bytes) }} {{ uiText('· 单文件上限') }} {{ cloudStorageSize(summary.current.max_file_bytes) }}</p>
        <p>{{ uiText('到期后的私人资料保留') }} {{ cloudRetention(summary.current.retention_seconds) }}{{ uiText('；可在个人资料区域查看和整理空间。') }}</p>
      </section>
      <p v-else class="empty">{{ uiText('当前没有生效的云端套餐，可以选择下方已开放的套餐。') }}</p>
      <section v-if="summary.next" class="entitlement" :aria-label="uiText('下一期套餐')">
        <h4>{{ uiText('已安排下一期：') }}{{ summary.next.title }}</h4>
        <p>{{ allowanceTime(summary.next.starts_at) }} {{ uiText('至') }} {{ allowanceTime(summary.next.expires_at) }}{{ uiText('（香港时间）') }}</p>
        <p>{{ uiText('套餐额度') }} {{ offerPercentage(summary.next.grant_milli_percent) }} {{ uiText('· 个人空间') }} {{ cloudStorageSize(summary.next.storage_bytes) }}</p>
      </section>
      <fieldset class="duration"><legend>{{ uiText('选择学习时长') }}</legend>
        <button v-for="(label, value) in cloudPlanDurations" :key="value" type="button" :aria-pressed="duration === value" @click="duration = value">{{ label }}</button>
      </fieldset>
      <p class="terms">{{ duration === 'month' ? uiText('按月开通，每期独立发放额度；月末开通按对应日期续期。') : uiText('每天为连续 24 小时，套餐额度可在整个有效期内集中使用。') }} {{ uiText('到期结束，不自动扣款；未用额度不结转。') }}</p>
      <div v-if="products.length" class="products">
        <article v-for="product in products" :key="product.product_id" class="product">
          <span>{{ cloudPlanTiers[product.tier] }}</span><h4>{{ product.title }}</h4>
          <p class="price">¥{{ paymentAmount(product.amount_fen!) }} <small>/ {{ cloudPlanDurations[product.duration] }}</small></p>
          <dl><div><dt>{{ uiText('服务额度') }}</dt><dd>{{ offerPercentage(product.grant_milli_percent!) }}</dd></div><div><dt>{{ uiText('个人空间') }}</dt><dd>{{ cloudStorageSize(product.storage_bytes!) }}</dd></div><div><dt>{{ uiText('单文件上限') }}</dt><dd>{{ cloudStorageSize(product.max_file_bytes!) }}</dd></div><div><dt>{{ uiText('到期后资料保留') }}</dt><dd>{{ cloudRetention(product.retention_seconds!) }}</dd></div></dl>
          <button type="button" @click="selected = product">{{ uiText('查看开通方式') }}</button>
        </article>
      </div>
      <p v-else role="status">{{ uiText('该时长的套餐尚未开放购买。') }}</p>
      <section v-if="selected" class="purchase" :aria-label="uiText('套餐开通方式')">
        <h4>{{ selected.title }} · ¥{{ paymentAmount(selected.amount_fen!) }}</h4>
        <p>{{ selected.instructions }}</p>
        <p v-if="contact">{{ uiText('联系方式：') }}{{ contact }}</p>
        <p>{{ uiText('请联系运营者核对套餐和到账金额。人工确认收款后开通；选择套餐不会扣款或提前授予权益。') }}</p>
        <p>{{ uiText('当前已有有效期时，新套餐会接在已安排的最后一期之后生效。开通后刷新查看确切起止时间。') }}</p>
        <p>{{ uiText('本期额度不足可在“补充额度”增加本期用量；补充额度不改变套餐到期时间。') }}</p>
        <button type="button" @click="selected = null">{{ uiText('收起开通方式') }}</button>
      </section>
      <p class="terms">{{ uiText('公共课程资料不占个人空间。个人资料准备、AI 学习和视频转写等服务共用套餐额度；已准备的公共成果不重复收取准备费用。') }}</p>
    </template>
  </section>
</template>

<style scoped>
.cloud-plans { padding: 18px 0; border-bottom: 1px solid var(--line); }
header { display: flex; align-items: center; justify-content: space-between; gap: 12px; }
h3, h4 { margin: 0; } p { line-height: 1.7; } button { font: inherit; color: var(--ink); background: var(--surface); border: 1px solid var(--line); border-radius: 6px; padding: 8px 12px; cursor: pointer; } button:disabled { opacity: .5; cursor: default; }
.entitlement, .purchase { padding: 16px; border: 1px solid var(--line); border-radius: 10px; margin: 14px 0; background: var(--surface); }
.duration { border: 0; padding: 0; display: flex; gap: 8px; flex-wrap: wrap; margin-top: 20px; } legend { margin-bottom: 10px; } [aria-pressed="true"] { border-color: var(--accent); color: var(--accent); }
.products { display: grid; grid-template-columns: repeat(auto-fit, minmax(170px, 1fr)); gap: 12px; }.product { border: 1px solid var(--line); border-radius: 10px; padding: 14px; }.product > span, .terms, dt, small { color: var(--muted); font-size: 12px; }.product h4 { margin-top: 8px; }.price { font-size: 25px; }.price small { display: block; } dl > div { display: flex; flex-wrap: wrap; justify-content: space-between; gap: 8px; margin: 10px 0; }dd { margin: 0; font-size: 13px; }.purchase p { white-space: pre-wrap; overflow-wrap: anywhere; }
</style>
