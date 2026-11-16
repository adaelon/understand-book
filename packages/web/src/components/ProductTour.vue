<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, ref, watch } from 'vue';
import { ArrowLeft, ArrowRight, CircleHelp, X } from '@lucide/vue';
import { t as uiText } from '../i18n';
import { placeTour, productTourSteps, tourStorageKey, type ProductTourContext, type ProductTourStep, type TourRect } from '../product-tour';
defineOptions({ inheritAttrs: false });

const props = defineProps<{
  context: ProductTourContext | null;
  owner: string;
  prepare?: (step: ProductTourStep) => boolean | Promise<boolean>;
  openBoards?: () => void | Promise<void>;
}>();
const opened = ref(false), changing = ref(false), index = ref(0), demo = ref(false), demoWidth = ref(4);
const steps = ref<ProductTourStep[]>([]), unavailable = ref(false);
const presentationUnavailable = ref(false);
const dialog = ref<HTMLElement>(), launch = ref<HTMLButtonElement>(), nextButton = ref<HTMLButtonElement>();
const viewport = ref({ x: 0, y: 0, width: 0, height: 0 });
const hole = ref<TourRect>({ x: 0, y: 0, width: 0, height: 0 });
const card = ref({ x: 12, y: 12 });
const step = computed(() => steps.value[index.value]);
const area = computed(() => demoWidth.value * (12 - demoWidth.value));
const seen = new Set<string>();
let target: HTMLElement | null = null, previousFocus: HTMLElement | null = null;
let app: HTMLElement | null = null, wasInert = false;
let frame = 0, request = 0, timer: ReturnType<typeof setTimeout> | undefined;
let resize: ResizeObserver | null = null, mutations: MutationObserver | null = null;
let prepared = true;

function visible(selector: string) {
  return Array.from(document.querySelectorAll<HTMLElement>(selector)).find(element => {
    const bounds = element.getBoundingClientRect();
    return bounds.width > 0 && bounds.height > 0 && getComputedStyle(element).visibility !== 'hidden';
  }) ?? null;
}
function dismissed() {
  if (!props.context) return true;
  const key = tourStorageKey(props.owner, props.context);
  try { return seen.has(key) || !!localStorage.getItem(key); } catch { return seen.has(key); }
}
function remember(result: 'done' | 'skipped') {
  if (!props.context) return;
  const key = tourStorageKey(props.owner, props.context); seen.add(key);
  try { localStorage.setItem(key, result); } catch { /* This visit still remembers the dismissal. */ }
}
function readViewport() {
  const vv = window.visualViewport;
  return { x: vv?.offsetLeft ?? 0, y: vv?.offsetTop ?? 0, width: vv?.width ?? innerWidth, height: vv?.height ?? innerHeight };
}
function measure() {
  if (!opened.value || !dialog.value) return;
  viewport.value = readViewport();
  target = prepared ? visible(step.value.target) : null;
  if (!target) {
    unavailable.value = true;
    const size = dialog.value.getBoundingClientRect();
    card.value = { x: Math.max(12, (viewport.value.width - size.width) / 2), y: Math.max(12, (viewport.value.height - size.height) / 2) };
    return;
  }
  unavailable.value = false;
  presentationUnavailable.value = !!step.value.demo && target.matches(':disabled');
  const bounds = target.getBoundingClientRect(), size = dialog.value.getBoundingClientRect();
  const layout = placeTour({ x: bounds.left - viewport.value.x, y: bounds.top - viewport.value.y, width: bounds.width, height: bounds.height }, viewport.value, size);
  hole.value = layout.hole; card.value = layout.card;
}
function schedule() { cancelAnimationFrame(frame); frame = requestAnimationFrame(measure); }
function observe() {
  resize = new ResizeObserver(schedule);
  if (dialog.value) resize.observe(dialog.value);
  if (target) resize.observe(target);
  mutations = new MutationObserver(records => {
    if (records.some(record => !dialog.value?.contains(record.target))) schedule();
  });
  if (app) mutations.observe(app, { subtree: true, childList: true, attributes: true, attributeFilter: ['class', 'hidden'] });
  window.addEventListener('resize', schedule);
  window.addEventListener('scroll', schedule, true);
  window.visualViewport?.addEventListener('resize', schedule);
  window.visualViewport?.addEventListener('scroll', schedule);
  window.addEventListener('keydown', keyboard, true);
}
function stop() {
  request++; opened.value = false; changing.value = false;
  cancelAnimationFrame(frame); resize?.disconnect(); mutations?.disconnect(); resize = null; mutations = null;
  window.removeEventListener('resize', schedule); window.removeEventListener('scroll', schedule, true);
  window.visualViewport?.removeEventListener('resize', schedule); window.visualViewport?.removeEventListener('scroll', schedule);
  window.removeEventListener('keydown', keyboard, true);
  if (app) app.inert = wasInert;
  app = null; target = null;
}
function close(result: 'done' | 'skipped' = 'skipped', restoreFocus = true) {
  remember(result); stop();
  if (restoreFocus) {
    const trigger = previousFocus?.isConnected && previousFocus.getClientRects().length ? previousFocus
      : visible('.tools-toggle, [data-workspace-menu]');
    trigger?.focus({ preventScroll: true });
  }
}
async function show(position: number) {
  const current = ++request; changing.value = true; demo.value = false; unavailable.value = false; prepared = true;
  index.value = position;
  try {
    const details = document.querySelector(step.value.target)?.closest('details');
    if (details) details.open = true;
    const ready = await props.prepare?.(step.value);
    if (current !== request) return;
    prepared = ready !== false;
    await nextTick();
    if (current !== request) return;
    target = ready === false ? null : visible(step.value.target);
    if (!target) unavailable.value = true;
    else {
      target.scrollIntoView({ block: 'nearest', inline: 'nearest', behavior: 'instant' });
      resize?.observe(target);
    }
    changing.value = false;
    await nextTick();
    if (current !== request) return;
    if (target && dialog.value && viewport.value.width < 700) {
      const height = target.getBoundingClientRect().height;
      // Make room for both the real content and its explanation before cropping a large region.
      if (height > 100 && height + dialog.value.offsetHeight + 36 < viewport.value.height)
        target.scrollIntoView({ block: 'start', inline: 'nearest', behavior: 'instant' });
    }
    measure(); nextButton.value?.focus({ preventScroll: true });
  } finally { if (current === request) changing.value = false; }
}
async function start(automatic = false) {
  if (!props.context || opened.value) return;
  // Do not interrupt another dialog or a thought the user has already started typing.
  if (visible('[aria-modal="true"]') || (automatic && document.activeElement?.matches('input,textarea,[contenteditable="true"]'))) return;
  previousFocus = document.activeElement as HTMLElement | null;
  viewport.value = readViewport();
  steps.value = productTourSteps[props.context].filter(item => !item.optional || visible(item.target));
  index.value = 0; opened.value = true;
  app = document.getElementById('app'); wasInert = app?.inert ?? false;
  if (app) app.inert = true;
  await nextTick(); observe(); await show(0);
}
async function advance() {
  if (changing.value) return;
  if (index.value < steps.value.length - 1) { await show(index.value + 1); return; }
  const action = step.value.action, element = target;
  close('done', false);
  if (action === 'boards') await props.openBoards?.();
  else if (action === 'create-board') element?.click();
  else if (action === 'input') element?.querySelector<HTMLTextAreaElement>('textarea')?.focus();
  else launch.value?.focus({ preventScroll: true });
}
function keyboard(event: KeyboardEvent) {
  if (event.key === 'Escape') { event.preventDefault(); event.stopImmediatePropagation(); close(); }
  else if (event.key === 'Tab' && dialog.value) {
    const controls = Array.from(dialog.value.querySelectorAll<HTMLElement>('button:not(:disabled),input:not(:disabled)'));
    const current = controls.indexOf(document.activeElement as HTMLElement);
    if (event.shiftKey && current <= 0) { event.preventDefault(); controls.at(-1)?.focus(); }
    else if (!event.shiftKey && (current < 0 || current === controls.length - 1)) { event.preventDefault(); controls[0]?.focus(); }
    event.stopImmediatePropagation();
  }
}
watch(() => [props.context, props.owner] as const, async () => {
  clearTimeout(timer); stop();
  await nextTick();
  if (props.context && !dismissed()) timer = setTimeout(() => void start(true), 450);
}, { immediate: true, flush: 'post' });
watch(demo, async () => {
  await nextTick(); schedule();
  if (demo.value) dialog.value?.querySelector('.product-tour-demo')?.scrollIntoView({ block: 'nearest', behavior: 'instant' });
});
onBeforeUnmount(() => { clearTimeout(timer); stop(); });
</script>

<template>
  <button v-if="context" ref="launch" v-bind="$attrs" type="button" class="product-tour-launch" :aria-label="uiText('新手引导')" :title="uiText('新手引导')" @click="start()"><CircleHelp :size="18" aria-hidden="true" /><span>{{ uiText('新手引导') }}</span></button>
  <Teleport to="body">
    <div v-if="opened && step" class="product-tour-overlay" :style="{ left: `${viewport.x}px`, top: `${viewport.y}px`, width: `${viewport.width}px`, height: `${viewport.height}px` }">
      <template v-if="!unavailable">
        <div class="product-tour-shade" :style="{ inset: `0 0 auto 0`, height: `${hole.y}px` }" />
        <div class="product-tour-shade" :style="{ left: 0, top: `${hole.y}px`, width: `${hole.x}px`, height: `${hole.height}px` }" />
        <div class="product-tour-shade" :style="{ left: `${hole.x + hole.width}px`, top: `${hole.y}px`, right: 0, height: `${hole.height}px` }" />
        <div class="product-tour-shade" :style="{ left: 0, top: `${hole.y + hole.height}px`, right: 0, bottom: 0 }" />
        <div class="product-tour-spotlight" :style="{ left: `${hole.x}px`, top: `${hole.y}px`, width: `${hole.width}px`, height: `${hole.height}px` }" />
      </template>
      <div v-else class="product-tour-shade product-tour-full-shade" />
      <section ref="dialog" class="product-tour-card" role="dialog" aria-modal="true" aria-labelledby="product-tour-title" aria-describedby="product-tour-description" :data-tour-step="step.id" :style="{ left: `${card.x}px`, top: `${card.y}px`, maxHeight: `${Math.max(160, viewport.height - 96)}px` }" @keydown.stop>
        <header><span>{{ uiText('认识这个空间') }} · {{ index + 1 }} / {{ steps.length }}</span><button type="button" class="product-tour-skip" @click="close()">{{ uiText('跳过引导') }}<X :size="14" aria-hidden="true" /></button></header>
        <div class="product-tour-copy" aria-live="polite">
          <h2 id="product-tour-title">{{ uiText(step.title) }}</h2>
          <p id="product-tour-description">{{ uiText(step.demo && presentationUnavailable ? '当前演示制作暂不可用。可以先体验下面的示例，了解可交互的讲解是什么样。' : step.body) }}</p>
          <p v-if="unavailable" class="product-tour-hint" role="status">{{ uiText('这个位置暂时不可见，可以继续或稍后从“新手引导”重看。') }}</p>
          <p v-else-if="step.hint" class="product-tour-hint">{{ uiText(step.hint) }}</p>
          <template v-if="step.demo">
            <button class="product-tour-demo-toggle" type="button" :aria-expanded="demo" @click="demo = !demo">{{ demo ? uiText('收起交互示例') : uiText('看看演示效果') }}</button>
            <div v-if="demo" class="product-tour-demo">
              <h3>{{ uiText('同样长的围栏，面积会一样吗？') }}</h3>
              <div class="product-tour-garden"><div :style="{ width: `${(12 - demoWidth) * 13}px`, height: `${demoWidth * 13}px` }"><span>{{ area }} m²</span></div></div>
              <label for="product-tour-width">{{ uiText('改变宽度') }}<output>{{ uiText('{0} 米', [demoWidth]) }}</output></label>
              <input id="product-tour-width" v-model.number="demoWidth" type="range" min="1" max="11" step="1" />
              <p aria-live="polite">{{ uiText('围栏始终是 24 米，面积现在是 {0} 平方米。', [area]) }}</p>
              <small>{{ uiText('这是交互效果示例。实际演示会围绕你的问题生成。') }}</small>
            </div>
          </template>
        </div>
        <footer><button type="button" class="product-tour-back" :disabled="index === 0 || changing" @click="show(index - 1)"><ArrowLeft :size="14" aria-hidden="true" />{{ uiText('上一步') }}</button><button ref="nextButton" type="button" class="product-tour-next" :disabled="changing" @click="advance()">{{ uiText(step.next || (index === steps.length - 1 ? '开始使用' : '下一步')) }}<ArrowRight :size="15" aria-hidden="true" /></button></footer>
      </section>
    </div>
  </Teleport>
</template>

<style scoped>
.product-tour-launch{display:inline-flex;align-items:center;justify-content:center;gap:7px;min-height:40px;padding:8px 11px;border:1px solid var(--line);border-radius:8px;background:transparent;color:var(--steel);font:13px var(--sans);cursor:pointer;white-space:nowrap}
.product-tour-launch:hover{color:var(--brand-accent)}
.product-tour-overlay{position:fixed;z-index:20000;isolation:isolate;overflow:hidden;color:var(--ink);font-family:var(--sans)}
.product-tour-overlay *{box-sizing:border-box}
.product-tour-shade{position:absolute;background:rgba(8,12,10,.78);pointer-events:none}
.product-tour-full-shade{inset:0}
.product-tour-spotlight{position:absolute;pointer-events:none;border:2px solid #f4cf96;border-radius:10px;box-shadow:0 0 18px #f4cf9626}
.product-tour-card{position:absolute;display:flex;flex-direction:column;width:min(352px,calc(100% - 24px));padding:22px;border:1px solid var(--line);border-radius:13px;background:var(--reader-card,var(--canvas));box-shadow:0 18px 70px #0005;overflow:hidden}
.product-tour-card header{display:flex;align-items:center;justify-content:space-between;gap:8px;margin-bottom:16px;flex-shrink:0}
.product-tour-card header>span{font-size:11px;color:var(--brand-accent)}
.product-tour-card button{display:inline-flex;align-items:center;justify-content:center;gap:6px;font:inherit;font-size:12px;cursor:pointer}
.product-tour-card button:focus-visible,.product-tour-demo input:focus-visible,.product-tour-launch:focus-visible{outline:2px solid var(--brand-accent);outline-offset:3px}
.product-tour-skip{padding:5px 0;border:0;background:transparent;color:var(--steel);white-space:nowrap}
.product-tour-copy{min-height:0;overflow:auto;overscroll-behavior:contain;padding:0 2px 3px}
.product-tour-copy h2{font-family:var(--font-display);font-size:22px;font-weight:500;line-height:1.55;margin:0 0 14px;overflow-wrap:anywhere}
.product-tour-copy p{font-size:14px;line-height:1.95;margin:0 0 14px;overflow-wrap:anywhere}
.product-tour-copy .product-tour-hint{font-size:12px;line-height:1.85;padding:11px 13px;border-radius:7px;background:var(--surface-soft);color:var(--steel);margin-bottom:0}
.product-tour-card footer{display:flex;align-items:center;justify-content:space-between;gap:12px;margin-top:19px;flex-shrink:0}
.product-tour-back{padding:9px 0;border:0;color:var(--steel);background:transparent;white-space:nowrap}
.product-tour-back:disabled{opacity:.35;cursor:default}
.product-tour-next{min-height:42px;max-width:78%;padding:10px 13px;border:0;border-radius:8px;background:var(--brand-accent);color:var(--brand-on-accent,#fff);line-height:1.5;text-align:center}
.product-tour-next:disabled{opacity:.65;cursor:wait}
.product-tour-demo-toggle{border:0;background:transparent;color:var(--brand-accent);padding:11px 0;margin-top:5px;min-height:40px}
.product-tour-demo{margin-top:2px;padding:14px;border:1px solid var(--line);border-radius:9px;background:var(--canvas)}
.product-tour-demo h3{font-size:13px;font-weight:500;margin:0 0 10px;line-height:1.8}
.product-tour-garden{height:150px;display:grid;place-items:center;background:var(--surface-soft);border-radius:6px;margin-bottom:12px}
.product-tour-garden>div{display:grid;place-items:center;border:2px solid var(--brand-accent);background:var(--brand-accent-soft)}
.product-tour-garden span{font-size:11px;white-space:nowrap;background:var(--reader-card);padding:2px 4px;color:var(--ink)}
.product-tour-demo label{display:flex;align-items:center;justify-content:space-between;gap:12px;font-size:12px}
.product-tour-demo input{width:100%;margin:12px 0;accent-color:var(--brand-accent)}
.product-tour-demo p{font-size:12px;line-height:1.8;margin-bottom:8px;font-variant-numeric:tabular-nums}
.product-tour-demo small{display:block;font-size:11px;line-height:1.8;color:var(--steel)}
@media(max-width:600px){.product-tour-launch{width:38px;min-width:38px;min-height:44px;padding:8px!important}.product-tour-launch span{display:none}.product-tour-card{padding:18px}.product-tour-copy h2{font-size:20px}.product-tour-copy p{font-size:13px}.product-tour-card header{margin-bottom:12px}.product-tour-skip{min-height:36px}.product-tour-card footer button{min-height:44px}}
@media(max-height:650px){.product-tour-card{padding:15px}.product-tour-card header{margin-bottom:8px}.product-tour-card footer{margin-top:12px}.product-tour-copy h2{font-size:19px;margin-bottom:9px}.product-tour-copy p{font-size:12px;line-height:1.8}.product-tour-garden{height:90px}.product-tour-garden>div{transform:scale(.6)}.product-tour-demo{padding:10px}}
</style>
