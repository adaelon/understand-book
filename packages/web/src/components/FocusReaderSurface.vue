<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref } from 'vue';
import { ReaderPagination, type PaginationOptions } from '../reader-pagination';
import '../reader-pagination.css';

const props = defineProps<Omit<PaginationOptions, 'host'>>();
const emit = defineEmits<{ (e: 'ready', pagination: ReaderPagination): void }>();
const host = ref<HTMLElement>();
let pagination: ReaderPagination | undefined;
onMounted(() => {
  pagination = new ReaderPagination({ host: host.value!, read: index => props.read(index), render: source => props.render(source), ready: (root, source, layout) => props.ready?.(root, source, layout) ?? Promise.resolve() });
  emit('ready', pagination);
});
onBeforeUnmount(() => pagination?.dispose());
</script>

<template><section ref="host" class="focus-reader-surface" aria-label="分页正文"></section></template>
