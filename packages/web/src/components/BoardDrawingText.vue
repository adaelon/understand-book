<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue';
import 'katex/dist/katex.min.css';
import { parseBoardMath } from '../board-math';
import { revealDrawingLines, type DrawingAppearance } from '../board-canvas-appearance';

const props = defineProps<{
  text: string;
  width: number;
  height: number;
  shape?: 'rectangle' | 'ellipse';
  appearance: DrawingAppearance;
  progress: number;
}>();

const content = ref<HTMLElement>();
const fontSize = ref(18);
const measured = ref(false);
const overflowing = ref(false);
const document = computed(() => parseBoardMath(props.text));
const nativeLines = computed(() => revealDrawingLines(props.appearance, props.progress));
const heading = computed(() => props.appearance.lines[0]?.weight === 620);
const baseFontSize = computed(() => props.shape ? 18 : 22);
const area = computed(() => {
  const width = Math.max(1, props.shape === 'ellipse' ? props.width * .7 : props.width - (props.shape ? 36 : 16));
  const height = Math.max(1, props.shape === 'ellipse' ? props.height * .7 : props.height - (props.shape ? 20 : 12));
  return { x: (props.width - width) / 2, y: (props.height - height) / 2, width, height };
});
const richLines = computed(() => document.value.lines.map(line => ({
  segments: line.segments.map(segment => {
    let offset = segment.start;
    const characters = segment.kind === 'text' ? Array.from(segment.source, text => {
      const start = offset; offset += text.length;
      return { text, start };
    }) : [];
    return { ...segment, characters };
  }),
})));
const characterEnds = computed(() => {
  let offset = 0;
  return Array.from(props.text, character => { offset += character.length; return offset; });
});
const visibleOffset = computed(() => {
  const count = Math.ceil(characterEnds.value.length * Math.max(0, Math.min(1, props.progress)));
  return count ? characterEnds.value[count - 1] : 0;
});

// Only content, available space, or font loading triggers a fit. Playback changes
// visibility inside the already measured text, including each complete formula.
function fitContent() {
  const node = content.value;
  if (!node || !node.offsetWidth) return;
  const fits = (size: number) => {
    node.style.fontSize = `${size}px`;
    return node.scrollWidth <= area.value.width + .5 && node.scrollHeight <= area.value.height + .5;
  };
  let size = baseFontSize.value;
  if (!fits(size)) {
    let low = .5, high = size;
    for (let pass = 0; pass < 7; pass++) {
      const middle = (low + high) / 2;
      if (fits(middle)) low = middle;
      else high = middle;
    }
    size = Math.max(.5, Math.floor(low * 4) / 4);
  }
  overflowing.value = !fits(size);
  fontSize.value = size;
  measured.value = true;
}
let mounted = false, queued = false;
async function scheduleFit() {
  if (!mounted || queued) return;
  queued = true;
  await nextTick();
  queued = false;
  if (mounted) fitContent();
}
watch([() => props.text, () => props.width, () => props.height, () => props.shape, heading], () => {
  measured.value = false;
  void scheduleFit();
}, { flush: 'post' });
onMounted(() => {
  mounted = true;
  void scheduleFit();
  globalThis.document.fonts?.addEventListener('loadingdone', scheduleFit);
});
onBeforeUnmount(() => {
  mounted = false;
  globalThis.document.fonts?.removeEventListener('loadingdone', scheduleFit);
});
</script>

<template>
  <text v-if="!document.hasMath" class="board-native-text" data-board-drawing-text :fill="appearance.textColor" :text-anchor="shape ? 'middle' : 'start'">
    <tspan v-for="(line, index) in nativeLines" :key="index" :x="line.x" :y="line.y" :font-size="line.fontSize" :font-weight="line.weight">{{ line.text }}</tspan>
  </text>
  <foreignObject v-else data-board-drawing-text :x="area.x" :y="area.y" :width="area.width" :height="area.height" class="board-rich-object">
    <div xmlns="http://www.w3.org/1999/xhtml" class="board-rich-viewport" :class="{ 'is-shape': shape }" :style="{ color: appearance.textColor }">
      <div ref="content" data-board-rich-text :data-board-rich-ready="measured" :data-board-rich-overflow="overflowing" :data-board-rich-font-size="fontSize" class="board-rich-content" :style="{ fontSize: `${fontSize}px` }" aria-hidden="true">
        <div v-for="(line, index) in richLines" :key="index" class="board-rich-line" :class="{ 'is-heading': heading && index === 0 }">
          <template v-for="(segment, segmentIndex) in line.segments" :key="segmentIndex">
            <span v-if="segment.kind === 'math'" class="board-rich-math" :class="{ 'is-display': segment.displayMode }" data-board-math :style="{ visibility: visibleOffset > segment.start ? 'visible' : 'hidden' }" v-html="segment.html"></span>
            <template v-else><span v-for="character in segment.characters" :key="character.start" class="board-rich-character" :style="{ visibility: visibleOffset > character.start ? 'visible' : 'hidden' }">{{ character.text }}</span></template>
          </template>
          <br v-if="!line.segments.length" />
        </div>
      </div>
    </div>
  </foreignObject>
</template>

<style scoped>
.board-native-text{font-family:var(--sans);font-weight:400;user-select:none}
.board-rich-object{overflow:visible;pointer-events:none;user-select:none}
.board-rich-viewport{width:100%;height:100%;display:flex;align-items:flex-start;justify-content:flex-start;font-family:var(--sans);font-weight:400}
.board-rich-viewport.is-shape{align-items:center;justify-content:center;text-align:center}
.board-rich-content{width:100%;flex:none;line-height:1.45;overflow-wrap:anywhere}
.board-rich-line{min-height:1.45em;white-space:pre-wrap}
.board-rich-line.is-heading{font-size:1.166667em;font-weight:620;line-height:1.35;min-height:1.35em;margin-bottom:.166667em}
.board-rich-math{display:inline-block;white-space:nowrap;vertical-align:baseline;overflow-wrap:normal}
.board-rich-math.is-display{display:block}
.board-rich-math :deep(.katex){font-size:1.08em;line-height:1.2}
.board-rich-math :deep(.katex-display){margin:.12em 0;text-align:inherit}
</style>
