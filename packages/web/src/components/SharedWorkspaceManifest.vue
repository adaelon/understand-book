<script setup lang="ts">
import { sharedBookKey, sharedBookLabel, sharedCapability, type SharedBook, type SharedRelease } from '../shared-workspaces';
import type { PublishedBookRef } from '../network-context';
const props = defineProps<{ release: SharedRelease; books: SharedBook[] }>();
const label = (reference: PublishedBookRef) => sharedBookLabel(reference, props.books, props.release.manifest.materials);
</script>

<template>
  <div class="shared-manifest">
    <p class="publisher">{{ release.manifest.publisher }} · 第 {{ release.version }} 版</p>
    <dl><div><dt>公开学习目标</dt><dd>{{ release.manifest.public_goal }}</dd></div><div><dt>适用范围</dt><dd>{{ release.manifest.applicability }}</dd></div><div><dt>建议基础</dt><dd>{{ release.manifest.prerequisites }}</dd></div></dl>
    <h4>预构建资料</h4>
    <ul><li v-for="material in release.manifest.materials" :key="sharedBookKey(material.published_book_ref)">{{ label(material.published_book_ref) }}<span> · {{ material.required_capabilities.map(sharedCapability).join('、') }}</span></li></ul>
    <template v-if="release.manifest.topic_links.length"><h4>主题联系</h4><ul class="parts"><li v-for="(part, index) in release.manifest.topic_links" :key="index"><strong>{{ part.title }}</strong><p>{{ part.description }}</p><small>关联资料：{{ part.materials.map(label).join('、') }}</small></li></ul></template>
    <template v-if="release.manifest.suggested_route.length"><h4>建议学习路线</h4><ol class="parts"><li v-for="(part, index) in release.manifest.suggested_route" :key="index"><strong>{{ part.title }}</strong><p>{{ part.description }}</p><small>关联资料：{{ part.materials.map(label).join('、') }}</small></li></ol></template>
    <section v-if="release.availability.status !== 'ready'" class="unavailable" aria-label="共享资料待准备项">
      <p>部分资料尚未就绪，或当前账号尚未获得完整访问权限。</p>
      <ul><template v-for="material in release.availability.materials" :key="sharedBookKey(material.published_book_ref)"><li v-if="!material.publication_available || material.missing_capabilities.length">{{ label(material.published_book_ref) }}：{{ !material.publication_available ? '该版本暂不可用' : `待准备${material.missing_capabilities.map(sharedCapability).join('、')}` }}</li></template>
        <li v-for="reference in release.availability.audience.flatMap(user => user.missing_grants)" :key="sharedBookKey(reference)">{{ label(reference) }}：尚未开通访问</li></ul>
    </section>
  </div>
</template>

<style scoped>
.shared-manifest { font-size: 14px; line-height: 1.7; overflow-wrap: anywhere; }.publisher,dt,small,li>span { color: var(--muted); } dl { display: grid; gap: 12px; } dd { margin: 2px 0 0; } h4 { margin: 20px 0 8px; font-weight: 500; } ul,ol { padding-left: 22px; }.parts li { margin-bottom: 14px; }.parts p { margin: 3px 0; }.unavailable { background: var(--surface-soft); padding: 1px 14px; border-radius: 10px; }
</style>
