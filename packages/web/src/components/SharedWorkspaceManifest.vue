<script setup lang="ts">
import { t as uiText } from '../i18n';

import { sharedBookKey, sharedBookLabel, sharedCapability, type SharedBook, type SharedRelease } from '../shared-workspaces';
import type { PublishedBookRef } from '../reader-context';
import MaterialCover from './MaterialCover.vue';
const props = defineProps<{ release: SharedRelease; books: SharedBook[] }>();
const label = (reference: PublishedBookRef) => sharedBookLabel(reference, props.books, props.release.manifest.materials);
const book = (reference: PublishedBookRef) => props.books.find(item => sharedBookKey(item.published_book_ref) === sharedBookKey(reference));
const capabilities = (values: string[]) => [...new Set(values.map(sharedCapability))];
</script>

<template>
  <div class="shared-manifest">
    <p class="publisher">{{ release.manifest.publisher }} {{ uiText('· 第') }} {{ release.version }} {{ uiText('版') }}</p>
    <dl><div><dt>{{ uiText('公开学习目标') }}</dt><dd>{{ release.manifest.public_goal }}</dd></div><div><dt>{{ uiText('适用范围') }}</dt><dd>{{ release.manifest.applicability }}</dd></div><div><dt>{{ uiText('建议基础') }}</dt><dd>{{ release.manifest.prerequisites }}</dd></div></dl>
    <h4>{{ uiText('预构建资料') }}</h4>
    <ul class="manifest-materials"><li v-for="material in release.manifest.materials" :key="sharedBookKey(material.published_book_ref)"><div class="manifest-cover" :class="{ 'is-video': book(material.published_book_ref)?.media_kind === 'video' }"><MaterialCover :title="label(material.published_book_ref)" :cover="book(material.published_book_ref)?.cover" :video="book(material.published_book_ref)?.media_kind === 'video'" :music="book(material.published_book_ref)?.media_kind === 'music'" compact /></div><div><strong>{{ label(material.published_book_ref) }}</strong><div class="capabilities"><span v-for="capability in capabilities(material.required_capabilities)" :key="capability">{{ capability }}</span></div></div></li></ul>
    <template v-if="release.manifest.topic_links.length"><h4>{{ uiText('主题联系') }}</h4><ul class="parts"><li v-for="(part, index) in release.manifest.topic_links" :key="index"><strong>{{ part.title }}</strong><p>{{ part.description }}</p><small>{{ uiText('关联资料：') }}{{ part.materials.map(label).join('、') }}</small></li></ul></template>
    <template v-if="release.manifest.suggested_route.length"><h4>{{ uiText('建议学习路线') }}</h4><ol class="parts route-list"><li v-for="(part, index) in release.manifest.suggested_route" :key="index"><span class="route-number" aria-hidden="true">{{ String(index + 1).padStart(2, '0') }}</span><div><strong>{{ part.title }}</strong><p>{{ part.description }}</p><small>{{ uiText('关联资料：') }}{{ part.materials.map(label).join('、') }}</small></div></li></ol></template>
    <section v-if="release.manifest.external_resources?.length" :aria-label="uiText('外部链接导览')">
      <h4>{{ uiText('外部链接导览') }}</h4><p>{{ uiText('前往原网站观看或阅读。以下链接未作为已解析的工作区资料。') }}</p>
      <ul class="parts"><li v-for="(resource, index) in release.manifest.external_resources" :key="index">
        <a :href="resource.url" target="_blank" rel="noopener noreferrer">{{ resource.title }} ↗</a><p v-if="resource.description">{{ resource.description }}</p>
      </li></ul>
    </section>
    <section v-if="release.availability.status !== 'ready'" class="unavailable" :aria-label="uiText('共享资料待准备项')">
      <p>{{ uiText('部分资料尚未就绪，或当前账号尚未获得完整访问权限。') }}</p>
      <ul><template v-for="material in release.availability.materials" :key="sharedBookKey(material.published_book_ref)"><li v-if="!material.publication_available || material.missing_capabilities.length">{{ label(material.published_book_ref) }}：{{ !material.publication_available ? uiText('该版本暂不可用') : uiText('待准备{0}', [material.missing_capabilities.map(sharedCapability).join('、')]) }}</li></template>
        <li v-for="reference in release.availability.audience.flatMap(user => user.missing_grants)" :key="sharedBookKey(reference)">{{ label(reference) }}{{ uiText('：尚未开通访问') }}</li></ul>
    </section>
  </div>
</template>

<style scoped>
.shared-manifest { font-size: 14px; line-height: 1.7; overflow-wrap: anywhere; }.publisher,dt,small,li>span { color: var(--muted); } dl { display: grid; gap: 12px; } dd { margin: 2px 0 0; } h4 { margin: 20px 0 8px; font-weight: 500; } ul,ol { padding-left: 22px; }.parts li { margin-bottom: 14px; }.parts p { margin: 3px 0; }.unavailable { background: var(--surface-soft); padding: 1px 14px; border-radius: 10px; }
.manifest-materials { padding: 0; list-style: none; display: grid; grid-template-columns: repeat(auto-fit, minmax(240px, 1fr)); gap: 12px; }
.manifest-materials > li { display: flex; align-items: center; gap: 16px; padding: 16px; border: 1px solid var(--line); border-radius: 10px; background: var(--canvas); }
.manifest-materials > li > div:last-child { min-width: 0; }.manifest-materials strong { font-weight: 500; font-size: 13px; }
.manifest-cover { width: 46px; height: 69px; flex: none; }.manifest-cover.is-video { width: 96px; height: 64px; }
.capabilities { display: flex; flex-wrap: wrap; gap: 5px; margin-top: 9px; }.capabilities > span { font-size: 10px; color: #827356; padding: 2px 6px; background: #eee9de; border-radius: 4px; }
.route-list { list-style: none; padding: 0; }.route-list > li { display: flex; gap: 16px; margin: 0; padding: 14px 0; border-bottom: 1px solid var(--line); }.route-list > li:last-child { border-bottom: 0; }.route-number { flex: none; font: 400 20px/1.6 var(--mono); color: #b4986c; }.route-list strong { font-size: 13px; font-weight: 550; }.route-list p { color: #746e61; font-size: 12px; }.route-list small { display: block; margin-top: 7px; font-size: 10px; }
@media(max-width:540px){.manifest-materials { grid-template-columns: minmax(0,1fr); }.manifest-materials>li { gap: 12px; padding: 12px; }.manifest-cover.is-video { width: 72px; height: 50px; }}
</style>
