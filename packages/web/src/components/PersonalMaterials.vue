<script setup lang="ts">
import { t as uiText } from '../i18n';

import { ref } from 'vue';
import { isTauri } from '@tauri-apps/api/core';
import { localMaterials, localMaterialsSupported } from '../local-materials';
import { network, type LearningWorkspace, type PublishedBookRef } from '../network-context';
import LocalMaterials from './LocalMaterials.vue';
import PersonalPreparations from './PersonalPreparations.vue';
defineProps<{ workspace: LearningWorkspace }>();
const emit = defineEmits<{ ready: []; open: [reference: PublishedBookRef]; allowance: [] }>();
const showCloud = ref(false);
const desktop = isTauri(), local = localMaterialsSupported();
</script>
<template>
  <template v-if="desktop">
    <LocalMaterials v-if="local && network.identity" :scope="{ account_id: network.identity.user_id, workspace_id: workspace.learning_workspace_id }" :workspace-name="workspace.goal.text" :archived="workspace.archived" @synced="showCloud = true; emit('ready')" @allowance="emit('allowance')" />
    <section v-else class="desktop-update" :aria-label="uiText('本机资料需要更新')"><h3>{{ uiText('本机资料') }}</h3><p>{{ uiText('本地导入不需要云端存储套餐。当前 Windows 版本尚未包含本机书库，请更新客户端后使用。') }}</p><a href="/downloads/UnderstandBookSetup.exe" download>{{ uiText('下载新版 Windows 客户端') }}</a></section>
    <button v-if="local" class="local-library-link" @click="localMaterials.openLibrary()">{{ uiText('打开本机书库（可离线使用）') }}</button>
    <details class="cloud-copy-details" :open="showCloud" @toggle="showCloud = ($event.target as HTMLDetailsElement).open">
      <summary>{{ uiText('云端副本与准备进度 · 多端使用') }}</summary>
      <p>{{ uiText('这里的文件保存在服务器，使用套餐云端空间。到期或空间不足不会影响本机资料。') }}</p>
      <PersonalPreparations v-if="showCloud" :workspace="workspace" @ready="emit('ready')" @open="emit('open', $event)" @allowance="emit('allowance')" />
    </details>
  </template>
  <PersonalPreparations v-else :workspace="workspace" @ready="emit('ready')" @open="emit('open', $event)" @allowance="emit('allowance')" />
</template>
<style scoped>
.desktop-update,.cloud-copy-details{margin:1rem 0;padding:1rem;border:1px solid var(--line,#dfd8ce);border-radius:12px}.desktop-update p,.cloud-copy-details p{line-height:1.7;color:var(--ink-muted,#786c5f)}summary{cursor:pointer}.local-library-link{padding:.5rem .8rem;border:1px solid var(--line,#dfd8ce);border-radius:8px;background:var(--paper,#fffdf8);color:inherit;cursor:pointer}
</style>
