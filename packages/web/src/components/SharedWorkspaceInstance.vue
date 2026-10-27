<script setup lang="ts">
import { t as uiText } from '../i18n';

import { onBeforeUnmount, ref, watch } from 'vue';
import { ApiError } from '../api';
import { network, type LearningWorkspace, type PublishedBookRef } from '../network-context';
import { sharedWorkspaces } from '../network-client';
import { sharedBookKey, sharedBookLabel, type SharedBook, type SharedInstance, type SharedUpdatePreview } from '../shared-workspaces';
import SharedWorkspaceManifest from './SharedWorkspaceManifest.vue';

const props = defineProps<{ workspace: LearningWorkspace; books: SharedBook[] }>();
const emit = defineEmits<{ updated: [workspace: LearningWorkspace] }>();
const instance = ref<SharedInstance | null>(null), preview = ref<SharedUpdatePreview | null>(null);
const loading = ref(false), busy = ref(false), error = ref(''), notice = ref('');
let request = 0, operationId = '';
const label = (reference: PublishedBookRef) => sharedBookLabel(reference, props.books, preview.value?.release.manifest.materials ?? instance.value?.release.manifest.materials ?? []);
function fail(failure: unknown, ticket: number) {
  if (ticket === request && !(failure instanceof ApiError && failure.errorCode === 'CLIENT_CONTEXT_STALE')) error.value = failure instanceof Error ? failure.message : uiText('共享工作区信息暂时无法读取');
}
async function load() {
  const ticket = ++request; loading.value = true; preview.value = null; error.value = ''; notice.value = '';
  try { const value = await sharedWorkspaces.instance(props.workspace.learning_workspace_id); if (ticket === request) instance.value = value.instance ?? null; }
  catch (failure) { fail(failure, ticket); }
  finally { if (ticket === request) loading.value = false; }
}
async function showPreview() {
  if (!instance.value || busy.value) return;
  const ticket = ++request; busy.value = true; error.value = ''; notice.value = ''; preview.value = null;
  try { const value = await sharedWorkspaces.preview(props.workspace.learning_workspace_id, instance.value.latest_version); if (ticket === request) { preview.value = value; operationId = crypto.randomUUID(); } }
  catch (failure) { fail(failure, ticket); }
  finally { if (ticket === request) busy.value = false; }
}
async function update() {
  if (!preview.value || !instance.value || busy.value || props.workspace.archived || preview.value.release.availability.status !== 'ready') return;
  const ticket = request; busy.value = true; error.value = '';
  try {
    const value = await sharedWorkspaces.update(props.workspace.learning_workspace_id, operationId, instance.value.version, preview.value.release.version);
    if (ticket === request) { instance.value = value.instance; preview.value = null; notice.value = uiText('已更新共享资料与路线。你的学习目标、私人追加资料和已有记录已保留。'); emit('updated', value.workspace); }
  } catch (failure) { fail(failure, ticket); }
  finally { if (ticket === request) busy.value = false; }
}
watch(() => `${network.value.epoch}:${network.value.identity?.user_id}:${props.workspace.learning_workspace_id}`, () => { instance.value = null; busy.value = false; void load(); }, { immediate: true });
onBeforeUnmount(() => { request++; });
</script>

<template>
  <section v-if="instance || error" class="shared-instance" :aria-label="uiText('共享工作区个人实例')" :aria-busy="loading || busy">
    <p v-if="error" role="alert">{{ error }} <button :disabled="busy || loading" @click="load">{{ uiText('重新读取共享信息') }}</button></p>
    <template v-if="instance">
      <div class="heading"><div><span class="eyebrow">{{ uiText('共享工作区 · 我的个人实例') }}</span><h3>{{ instance.release.manifest.name }}</h3><p>{{ uiText('当前使用第') }} {{ instance.version }} {{ uiText('版') }}<span v-if="instance.latest_version > instance.version"> {{ uiText('· 有第') }} {{ instance.latest_version }} {{ uiText('版可选') }}</span></p></div><button :disabled="loading || busy" @click="load">{{ uiText('检查共享版本') }}</button></div>
      <p v-if="instance.release.archived" class="hint">{{ uiText('共享入口已归档，你已建立的工作区继续保留，资料按现有访问权限使用。') }}</p>
      <p v-if="notice" role="status">{{ notice }}</p>
      <details class="original-route"><summary>{{ uiText('查看所选版本的公开目标与路线') }}</summary><SharedWorkspaceManifest :release="instance.release" :books="books" /></details>
      <button v-if="instance.latest_version > instance.version && !preview" :disabled="busy || loading || workspace.archived" @click="showPreview">{{ uiText('查看新版变化') }}</button>
      <section v-if="preview" class="shared-update" :aria-label="uiText('共享工作区更新预览')">
        <h4>{{ uiText('第') }} {{ instance.version }} {{ uiText('版 → 第') }} {{ preview.release.version }} {{ uiText('版') }}</h4>
        <ul class="change-list">
          <li v-for="reference in preview.changes.added_materials" :key="sharedBookKey(reference)">{{ uiText('新增资料：') }}{{ label(reference) }}</li>
          <li v-for="reference in preview.changes.removed_materials" :key="sharedBookKey(reference)">{{ uiText('移出公共资料：') }}{{ label(reference) }}</li>
          <li v-if="preview.changes.public_goal_changed">{{ uiText('公开学习目标已调整。') }}</li><li v-if="preview.changes.composition_changed">{{ uiText('主题联系或建议路线已调整。') }}</li>
          <li v-if="preview.changes.required_capabilities_changed">{{ uiText('资料清单或所需能力已调整。') }}</li><li v-if="preview.changes.description_changed">{{ uiText('名称、适用范围或建议基础已调整。') }}</li>
          <li v-if="!preview.changes.added_materials.length && !preview.changes.removed_materials.length && !preview.changes.public_goal_changed && !preview.changes.composition_changed && !preview.changes.required_capabilities_changed && !preview.changes.description_changed">{{ uiText('公共说明有所更新，资料版本与路线保持。') }}</li>
        </ul>
        <p>{{ uiText('你的学习目标继续为：') }}<strong>{{ workspace.goal.text }}</strong></p>
        <p v-if="preview.preserved_private_materials.length">{{ uiText('保留私人追加资料：') }}{{ preview.preserved_private_materials.map(label).join('、') }}。</p>
        <p>{{ uiText('已有聊天、笔记和理解记录保留当时的资料版本。选择更新后，再次打开资料即可使用新版。') }}</p>
        <details><summary>{{ uiText('查看新版完整内容') }}</summary><SharedWorkspaceManifest :release="preview.release" :books="books" /></details>
        <details v-if="preview.changes.added_materials.length || preview.changes.removed_materials.length" class="exact-versions"><summary>{{ uiText('核对资料版本') }}</summary><ul><li v-for="reference in preview.changes.added_materials" :key="`add:${sharedBookKey(reference)}`">{{ uiText('新增') }} {{ label(reference) }}：{{ reference.publication_id }}</li><li v-for="reference in preview.changes.removed_materials" :key="`remove:${sharedBookKey(reference)}`">{{ uiText('移出') }} {{ label(reference) }}：{{ reference.publication_id }}</li></ul></details>
        <p v-if="preview.release.availability.status !== 'ready'" role="status">{{ uiText('新版资料或访问权限尚未完整就绪，暂时无法更新。') }}</p>
        <div class="actions"><button :disabled="busy || workspace.archived || preview.release.availability.status !== 'ready'" @click="update">{{ busy ? uiText('正在更新…') : uiText('选择更新到此版本') }}</button><button :disabled="busy" @click="preview = null">{{ uiText('保留当前版本') }}</button></div>
      </section>
    </template>
  </section>
</template>

<style scoped>
.shared-instance { margin: 18px 0; padding: 18px; border: 1px solid var(--line); border-radius: 12px; background: var(--surface-soft); }.heading { display: flex; flex-wrap: wrap; gap: 12px; align-items: center; justify-content: space-between; }.heading>div { min-width: 0; }h3,h4 { margin: 5px 0; font-size: 16px; font-weight: 500; overflow-wrap: anywhere; }.eyebrow,.hint { color: var(--muted); font-size: 12px; }p,li { font-size: 13px; line-height: 1.8; overflow-wrap: anywhere; }button { font: inherit; border: 1px solid var(--line); border-radius: 8px; color: var(--ink); background: var(--canvas); padding: 8px 11px; min-height: 40px; cursor: pointer; }button:disabled { opacity: .5; cursor: default; }details { margin: 12px 0; }summary { cursor: pointer; padding: 6px 0; font-size: 13px; }.shared-update { margin-top: 18px; padding-top: 14px; border-top: 1px solid var(--line); }.actions { display: flex; flex-wrap: wrap; gap: 8px; margin-top: 16px; }.actions button:first-child { background: var(--brand-accent); color: var(--brand-on-accent); }.change-list { padding-left: 22px; }@media(max-width:540px){.shared-instance { padding: 14px; }}
</style>
