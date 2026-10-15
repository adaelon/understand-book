<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from 'vue';
import type { AnimationItem } from 'lottie-web';
import { loadYeyeAnimation, yeyeAssets, type YeyeVariant } from '../yeye-assets';

const props = withDefaults(defineProps<{
  variant?: YeyeVariant;
  /** Square display size in CSS pixels; the host chooses its available space. */
  size?: number;
  decorative?: boolean;
  label?: string;
  motion?: 'static' | 'idle' | 'waiting';
  /** Includes host regions hidden with v-show or the reader's focus layout. */
  active?: boolean;
}>(), {
  variant: 'idle',
  decorative: true,
  label: '页页，阅读伙伴',
  motion: 'static',
  active: true,
});

const size = computed(() => props.size ?? (props.variant === 'avatar' ? 32 : 160));
const root = ref<HTMLElement | null>(null);
const canvas = ref<HTMLElement | null>(null);
const still = ref<HTMLImageElement | null>(null);
const inViewport = ref(false);
const pageVisible = ref(false);
const reducedMotion = ref(true);
const playing = ref(false);
const nodding = ref(false);
const motionAllowed = computed(() => props.active && inViewport.value && pageVisible.value && !reducedMotion.value);
const eligible = computed(() => motionAllowed.value && props.variant !== 'avatar' && props.motion !== 'static');
let media: MediaQueryList | undefined;
let observer: IntersectionObserver | undefined;
let timer: ReturnType<typeof setTimeout> | undefined;
let request: AbortController | undefined;
let player: AnimationItem | undefined;
let generation = 0;
let disposed = false;

function stop() {
  generation++;
  clearTimeout(timer);
  timer = undefined;
  request?.abort();
  request = undefined;
  playing.value = false;
  nodding.value = false;
  player?.pause();
  player?.destroy();
  player = undefined;
}

function schedule() {
  if (!eligible.value || disposed) return;
  timer = setTimeout(() => { timer = undefined; void play(); }, props.motion === 'idle' ? 16_000 : 1_200);
}

async function play() {
  const current = generation;
  const valid = () => !disposed && current === generation && eligible.value;
  const variant = props.variant;
  if (variant === 'avatar') return;
  try {
    if (!player) {
      request = new AbortController();
      const [runtime, animationData] = await Promise.all([
        import('lottie-web'), loadYeyeAnimation(variant, request.signal), still.value!.decode(),
      ]);
      if (!valid()) return;
      const animation = runtime.default.loadAnimation({
        container: canvas.value!, renderer: 'svg', autoplay: false,
        loop: props.motion === 'waiting', animationData,
      });
      player = animation;
      animation.addEventListener('data_failed', () => { if (valid()) stop(); });
      animation.addEventListener('error', () => { if (valid()) stop(); });
      animation.addEventListener('complete', () => {
        if (!valid()) return;
        playing.value = false;
        schedule();
      });
      animation.addEventListener('DOMLoaded', () => {
        if (!valid()) return;
        playing.value = true;
        animation.goToAndPlay(0, true);
      });
    } else {
      playing.value = true;
      player.goToAndPlay(0, true);
    }
  } catch {
    // Static artwork and the host's status text remain usable after load failure.
    if (valid()) stop();
  }
}

// Capture at the observed terminal event, then call only after its saved facts
// arrive. Hiding/unmounting in between invalidates the callback permanently.
function prepareCompletionFeedback(): (() => void) | undefined {
  if (disposed || !motionAllowed.value || props.variant !== 'avatar') return;
  const current = generation;
  let consumed = false;
  return () => {
    if (consumed) return;
    consumed = true;
    if (disposed || generation !== current || !motionAllowed.value) return;
    nodding.value = true;
    timer = setTimeout(() => { timer = undefined; nodding.value = false; }, 720);
  };
}
defineExpose({ prepareCompletionFeedback });

watch([motionAllowed, () => props.motion, () => props.variant], () => {
  stop();
  schedule();
}, { flush: 'sync' });
function updatePageVisibility() { pageVisible.value = document.visibilityState !== 'hidden'; }
function updateMotionPreference() { reducedMotion.value = media!.matches; }
onMounted(() => {
  media = window.matchMedia('(prefers-reduced-motion: reduce)');
  updateMotionPreference();
  updatePageVisibility();
  media.addEventListener('change', updateMotionPreference);
  document.addEventListener('visibilitychange', updatePageVisibility);
  observer = new IntersectionObserver(entries => {
    inViewport.value = entries.some(entry => entry.isIntersecting && entry.intersectionRect.width > 0 && entry.intersectionRect.height > 0);
  });
  observer.observe(root.value!);
});
onBeforeUnmount(() => {
  disposed = true;
  stop();
  observer?.disconnect();
  media?.removeEventListener('change', updateMotionPreference);
  document.removeEventListener('visibilitychange', updatePageVisibility);
});
</script>

<template>
  <span
    ref="root"
    class="yeye-mascot"
    :class="{ 'yeye-nodding': nodding }"
    :data-yeye-nodding="nodding"
    :data-yeye-playing="playing"
    :data-yeye-active="eligible"
    :style="{ width: `${size}px`, height: `${size}px` }"
    :role="decorative ? undefined : 'img'"
    :aria-hidden="decorative ? 'true' : undefined"
    :aria-label="decorative ? undefined : label"
  >
    <img ref="still" :class="{ 'yeye-still-hidden': playing }" :src="yeyeAssets[variant]" :width="size" :height="size" alt="" aria-hidden="true" draggable="false" />
    <span ref="canvas" class="yeye-animation" :class="{ 'yeye-animation-visible': playing }" aria-hidden="true" />
  </span>
</template>

<style scoped>
.yeye-mascot {
  position: relative;
  display: inline-block;
  flex: none;
  vertical-align: middle;
  pointer-events: none;
  user-select: none;
}
.yeye-still-hidden { visibility: hidden; }
.yeye-nodding { animation: yeye-nod 720ms ease-in-out; transform-origin: 50% 75%; }
@keyframes yeye-nod {
  0%, 100% { transform: translateY(0) rotate(0); }
  40% { transform: translateY(2px) rotate(7deg); }
  72% { transform: translateY(-.5px) rotate(-2deg); }
}
@media (prefers-reduced-motion: reduce) { .yeye-nodding { animation: none; } }
.yeye-animation { position: absolute; inset: 0; visibility: hidden; }
.yeye-animation-visible { visibility: visible; }
.yeye-mascot img {
  display: block;
  width: 100%;
  height: 100%;
  object-fit: contain;
}
</style>
