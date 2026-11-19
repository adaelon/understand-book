<script setup lang="ts">
import { onMounted, ref } from 'vue';
import { t } from '../i18n';
import { readerRequest } from '../reader-host';

interface Skill { name: string; description: string; enabled: boolean }
interface Paper { id: string; path: string; title: string }
interface Imported { book_id: string; dir?: string; status?: string; readiness?: { status?: string; route?: string }; jobs?: { status: string }[] }
const emit = defineEmits<{open: [dir: string]}>();
const skills = ref<Skill[]>([]);
const scope = ref('');
const source = ref('');
const query = ref('');
const identifier = ref('');
const attachId = ref('');
const pdfPath = ref('');
const catalog = ref<Paper[]>([]);
const imported = ref<Imported | null>(null);
const notice = ref('');
const busy = ref(false);
const error = ref('');
const call = <T,>(name: string, args: Record<string, unknown> = {}) =>
  readerRequest<T>('POST', '/local-agent/extensions', { name, arguments: args });
async function perform(work: () => Promise<void>) {
  if (busy.value) return;
  busy.value = true;
  error.value = ''; notice.value = '';
  try { await work(); } catch (cause) { error.value = cause instanceof Error ? cause.message : String(cause); }
  finally { busy.value = false; }
}
async function refreshSkills(op: string, args: Record<string, unknown> = {}) {
  const result = await call<{skills: Skill[]; scope?: string}>(op, args);
  skills.value = result.skills;
  if (result.scope) scope.value = result.scope;
}
onMounted(() => perform(() => refreshSkills('skill_list')));
async function addSkill() {
  await refreshSkills('skill_import', { source: source.value.trim() });
  source.value = '';
  notice.value = t('Skill 已安装，可在此启用。');
}
async function searchPapers() {
  const result = await call<{items: Paper[]}>('agentero_papers', { query: query.value.trim() });
  catalog.value = result.items;
  if (!result.items.length) notice.value = t('当前 Vault 中没有匹配的论文。');
}
async function importPaper(name: string, args: Record<string, unknown>) {
  imported.value = await call<Imported>(name, args);
  attachId.value = imported.value.status === 'metadata_only' ? imported.value.book_id : '';
  pdfPath.value = '';
}
</script>

<template>
  <div class="learning-extensions">
    <details open>
      <summary>{{ t('工作区教学 Skill') }}</summary>
      <p class="secondary">{{ t('启用的教学指令会提供给本范围内的后续 Agent 回合。') }} <span v-if="scope">{{ scope }}</span></p>
      <form @submit.prevent="perform(addSkill)">
        <label>{{ t('Skill 目录或 GitHub 链接') }}<input v-model="source" spellcheck="false" :disabled="busy" /></label>
        <button :disabled="busy || !source.trim()">{{ t('导入 Skill') }}</button>
      </form>
      <ul v-if="skills.length">
        <li v-for="skill in skills" :key="skill.name">
          <div><strong>{{ skill.name }}</strong><p>{{ skill.description }}</p></div>
          <button type="button" :disabled="busy" @click="perform(() => refreshSkills('skill_enable', {name: skill.name, enabled: !skill.enabled}))">{{ skill.enabled ? t('停用') : t('启用') }}</button>
          <button type="button" :disabled="busy" @click="perform(() => refreshSkills('skill_remove', {name: skill.name}))">{{ t('移除') }}</button>
        </li>
      </ul>
    </details>
    <details>
      <summary>{{ t('从 Agentero 或本机导入论文') }}</summary>
      <form @submit.prevent="perform(searchPapers)">
        <label>{{ t('在 Agentero Vault 中查找') }}<input v-model="query" :disabled="busy" /></label>
        <button :disabled="busy">{{ t('查询论文') }}</button>
      </form>
      <ul v-if="catalog.length">
        <li v-for="paper in catalog" :key="paper.path"><span>{{ paper.title }}</span>
          <button type="button" :disabled="busy" @click="perform(() => importPaper('agentero_import', {paper: paper.path}))">{{ t('导入') }}</button></li>
      </ul>
      <form @submit.prevent="perform(() => importPaper('agentero_import', {identifier: identifier.trim()}))">
        <label>{{ t('DOI 或 arXiv 标识') }}<input v-model="identifier" :disabled="busy" /></label>
        <button :disabled="busy || !identifier.trim()">{{ t('从标识导入') }}</button>
      </form>
      <form @submit.prevent="perform(() => importPaper('paper_import', {path: pdfPath.trim(), book_id: attachId.trim() || undefined}))">
        <label>{{ t('已有材料 ID（选填）') }}<input v-model="attachId" spellcheck="false" :disabled="busy" /></label>
        <label>{{ attachId.trim() ? t('为此条目补充 PDF') : t('本机 PDF 路径') }}<input v-model="pdfPath" spellcheck="false" :disabled="busy" /></label>
        <button :disabled="busy || !pdfPath.trim()">{{ t('导入 PDF') }}</button>
      </form>
      <p v-if="imported" role="status">{{ t('材料引用') }} <code>{{ imported.book_id }}</code> · {{ imported.status === 'metadata_only' ? t('已保存元数据，待补原文') : imported.jobs?.some(job => job.status === 'running') ? t('正在准备原文和阅读位置') : imported.readiness?.route === 'reader' ? t('可以开始阅读') : t('打开工作台查看准备状态') }}</p>
      <button v-if="imported?.dir" type="button" @click="emit('open', imported.dir)">{{ t('打开已导入的材料') }}</button>
    </details>
    <p v-if="notice" role="status">{{ notice }}</p><p v-if="error" role="alert" class="error">{{ error }}</p>
  </div>
</template>

<style scoped>
.learning-extensions {display:grid;gap:.85rem;margin-block:1rem}
details {border-top:1px solid var(--border,#dcd9d1);padding-top:.7rem}
summary {cursor:pointer;font-weight:600}
.secondary,li p {font-size:.86rem;color:var(--ink-soft,#625f57)}
form,li {display:flex;gap:.5rem;align-items:end;flex-wrap:wrap;margin-block:.65rem}
label,li div,li span {flex:1 1 12rem;min-width:0;overflow-wrap:anywhere}
label {display:grid;gap:.35rem;font-size:.88rem}
input,button {font:inherit;border:1px solid var(--border,#d3cfc6);border-radius:6px;color:inherit;padding:.4rem .55rem;background:var(--surface,#fff)}
button {cursor:pointer} button:disabled {cursor:default;opacity:.55}
ul {padding:0;list-style:none} li {align-items:center} li p {margin:.2rem 0} .error {color:var(--danger,#983e36)}
</style>
