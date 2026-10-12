<script setup lang="ts">
import { computed } from 'vue';

const props = defineProps<{ fields: { path: string; label: string; value: string }[] }>();
const title = computed(() => props.fields.find(field => /^\/(heading|title|label|name|term|step|claim|subject)$/.test(field.path)));
const paragraphs = computed(() => props.fields.filter(field => /^\/(body|detail|description|summary|text|explanation|definition|content)$/.test(field.path)));
const details = computed(() => props.fields.filter(field => field !== title.value && !paragraphs.value.includes(field)));
</script>

<template>
  <div class="record-content">
    <h5 v-if="title" class="artifact-wrap">{{ title.value }}</h5>
    <p v-for="field in paragraphs" :key="field.path" class="generic-field-value artifact-wrap">{{ field.value }}</p>
    <dl v-if="details.length" class="generic-fields">
      <template v-for="field in details" :key="field.path">
        <dt class="artifact-wrap">{{ field.label }}</dt><dd class="generic-field-value artifact-wrap">{{ field.value }}</dd>
      </template>
    </dl>
    <p v-if="!fields.length" class="record-empty">暂无内容</p>
  </div>
</template>

<style scoped>
.record-content { min-width: 0; }
h5 { margin: 0 0 .5rem; font-size: .94rem; font-weight: 650; line-height: 1.6; color: var(--ink); }
p { margin: 0 0 .6rem; font-size: .84rem; line-height: 1.8; color: var(--slate); }
p:last-child { margin-bottom: 0; }
.artifact-wrap { overflow-wrap: anywhere; min-width: 0; white-space: pre-wrap; }
.generic-fields { display: grid; grid-template-columns: minmax(3rem,auto) minmax(0,1fr); gap: .4rem .7rem; margin: .65rem 0 0; font-size: .78rem; line-height: 1.6; }
dt { color: var(--muted); } dd { margin: 0; color: var(--ink); }
.record-empty { color: var(--muted); }
</style>
