<script setup lang="ts">
import { onBeforeUnmount, ref, watch } from 'vue';
import { ArrowUpRight, Sparkles } from '@lucide/vue';
import { t } from '../i18n';
import { ApiError } from '../api';
import { inquiryCourse } from '../inquiry-course';
import { network, type PublishedBookRef } from '../reader-context';
const props=defineProps<{workspaceId:string;disabled?:boolean}>();
defineEmits<{open:[reference?:PublishedBookRef]}>();
const completed=ref(0),total=ref(0),recent=ref(''),reference=ref<PublishedBookRef>(),error=ref('');
let generation=0;
async function load(){
  const token=++generation;completed.value=0;total.value=0;recent.value='';reference.value=undefined;error.value='';
  try{
    const catalog=await inquiryCourse.catalog(props.workspaceId);if(token!==generation)return;
    total.value=catalog.course?.lessons.length??0;
    completed.value=catalog.course?.lessons.filter(l=>catalog.progress.some(p=>p.lesson_id===l.id&&p.release_version===catalog.release_version&&p.status==='completed')).length??0;
    const latest=catalog.progress.find(p=>p.release_version===catalog.release_version&&p.status!=='skipped');
    if(latest){const lesson=await inquiryCourse.read(props.workspaceId,latest.run_id);if(token!==generation)return;
      reference.value=lesson.summary.source.publication;
      const record=lesson.reviews.filter(r=>r.displayed).flatMap(r=>r.understanding).at(-1);
      recent.value=record?.learner_summary?.headline||record?.demonstrated[0]||record?.difficulties[0]||'';
    }
  }catch(e){if(token===generation&&!(e instanceof ApiError&&e.status===404))error.value=t('进展暂时无法读取');}
}
watch(()=>`${network.value.epoch}:${props.workspaceId}`,()=>void load(),{immediate:true});
onBeforeUnmount(()=>generation++);
</script>
<template>
  <section class="understanding-entry" :aria-label="t('我的理解与进展')">
    <div class="entry-title"><span>{{t('我的理解')}}</span><strong>{{recent || t('把想明白的地方，连起来。')}}</strong><small v-if="recent"><Sparkles :size="12"/>{{t('最近一课的理解')}}</small><small v-else>{{t('看看已有的理解、当前卡点和下一步。')}}</small></div>
    <div v-if="total" class="entry-progress"><div><span>{{t('课程进度')}}</span><strong>{{t('{0} / {1} 节',[completed,total])}}</strong></div><progress :value="completed" :max="total" :aria-label="t('课程完成进度')"/></div>
    <button :disabled="disabled" @click="$emit('open',reference)">{{t('查看理解地图')}}<ArrowUpRight :size="16"/></button>
    <p v-if="error" role="status">{{error}} <button @click="load">{{t('重试')}}</button></p>
  </section>
</template>
<style scoped>
.understanding-entry{display:grid;grid-template-columns:minmax(0,1.5fr) minmax(140px,.65fr) auto;gap:24px;align-items:center;border-block:1px solid var(--line);padding:22px 0;margin:22px 0;color:var(--ink)}.entry-title>span{display:block;font:500 12px/1.5 var(--sans);color:var(--brand-accent);margin-bottom:7px}.entry-title>strong{display:block;font:500 18px/1.7 var(--font-display, var(--sans));overflow-wrap:anywhere}.entry-title>small{display:flex;align-items:center;gap:6px;color:var(--steel);font:11px/1.7 var(--sans);margin-top:7px}.entry-progress>div{display:flex;align-items:baseline;justify-content:space-between;gap:10px;font:12px/1.7 var(--sans)}.entry-progress span{color:var(--steel)}.entry-progress strong{font-size:15px;font-weight:500}progress{display:block;width:100%;height:5px;margin-top:9px;appearance:none;border:0;background:var(--surface-soft);border-radius:3px;overflow:hidden}progress::-webkit-progress-bar{background:var(--surface-soft)}progress::-webkit-progress-value{background:var(--brand-accent)}button{display:inline-flex;align-items:center;gap:8px;border:1px solid var(--line);border-radius:8px;background:var(--surface);color:var(--ink);font:500 12px/1.5 var(--sans);padding:10px 14px;min-height:44px;cursor:pointer}button:disabled{opacity:.5;cursor:default}button:focus-visible{outline:2px solid var(--brand-accent);outline-offset:3px}p{grid-column:1/-1;font-size:12px;color:var(--steel);margin:0}p button{background:transparent;border:0;min-height:34px;padding:4px 10px}@media(max-width:760px){.understanding-entry{grid-template-columns:1fr auto;gap:16px}.entry-title{grid-column:1/-1}.entry-title>strong{font-size:17px}}@media(max-width:360px){.understanding-entry{grid-template-columns:1fr}.entry-title{grid-column:1}.understanding-entry>button{justify-self:start}}
</style>
