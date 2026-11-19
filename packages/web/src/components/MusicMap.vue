<script setup lang="ts">
import { computed, ref, onMounted, onBeforeUnmount } from 'vue';
import type { MusicSourceManifest, MusicEvent } from '../../../core/src/music-contract';
import { MusicTimeline, musicTime } from '../music-playback';
import { musicRoleGroups, musicRoleKey } from '../music-roles';
const props = defineProps<{ manifest: MusicSourceManifest; position: number; paused: boolean; docked?: boolean; guideOpen?: boolean }>();
const emit = defineEmits<{ seek: [ms: number]; listen: [event: MusicEvent]; guide: [] }>();
const compactMedia = globalThis.matchMedia?.('(max-width:640px), (max-height:680px)');
const compact = ref(compactMedia?.matches ?? false), expanded = ref(!compact.value);
function resizeMap(event: MediaQueryListEvent) { compact.value = event.matches; expanded.value = !event.matches; }
onMounted(() => compactMedia?.addEventListener('change', resizeMap));
onBeforeUnmount(() => compactMedia?.removeEventListener('change', resizeMap));
function listen(event: MusicEvent) { emit('listen', event); if (compact.value) expanded.value = false; }
function seek(ms: number) { emit('seek', ms); if (compact.value) expanded.value = false; }
const whole = ref(false);
const palette = ['#a66435','#397e72','#8065a0','#b18a31','#497b9c','#a25f7b'];
const sections = computed(() => props.manifest.structure.filter(s => s.start_ms !== null && s.end_ms !== null));
const roots = computed(() => sections.value.filter(s => !s.parent_id));
const current = computed(() => roots.value.find(s => s.start_ms! <= props.position && props.position < s.end_ms!) ?? [...roots.value].reverse().find(s => s.start_ms! <= props.position) ?? roots.value[0]);
const window = computed(() => whole.value || !current.value ? { start: 0, end: props.manifest.recording.duration_ms } : { start: current.value.start_ms!, end: current.value.end_ms! });
const children = computed(() => sections.value.filter(s => s.parent_id === current.value?.section_id));
const material = computed(() => props.manifest.events.filter(e => !e.concept_tags.includes('lyrics')));
const rolePalette = computed(() => new Map(musicRoleGroups(material.value, props.manifest.events).map(group => [group.key, group.color])));
const lanes = computed(() => {
  const lanes = new Map<string, { key: string; label: string; color: string; events: MusicEvent[] }>();
  for (const e of material.value) for (const role of e.roles) {
    const theme = role.theme_id ?? role.role;
    const key = `${theme}:${role.part_id}`;
    if (!lanes.has(key)) lanes.set(key, { key, label: `${role.role} · ${props.manifest.parts.find(p => p.part_id === role.part_id)?.name ?? role.part_id}`, color: rolePalette.value.get(musicRoleKey(role))!, events: [] });
    const lane = lanes.get(key)!; if (!lane.events.includes(e)) lane.events.push(e);
  }
  return [...lanes.values()];
});
const visibleLanes = computed(() => lanes.value.map(l => ({ ...l, events: l.events.filter(e => e.start_ms < window.value.end && e.end_ms > window.value.start) })).filter(l => l.events.length));
const eventClock = computed(() => new MusicTimeline(material.value));
const active = computed(() => eventClock.value.at(props.position));
const activeSignature = computed(() => active.value.map(e => e.event_id).join('|'));
const barClock = computed(() => new MusicTimeline(props.manifest.occurrences));
const participants = computed(() => [...new Set(active.value.flatMap(e => e.roles.map(r => props.manifest.parts.find(p => p.part_id === r.part_id)?.name).filter(Boolean)))]);
const occurrence = computed(() => barClock.value.at(props.position)[0]);
const x = (ms: number) => Math.max(0, Math.min(1000, (ms - window.value.start) / (window.value.end - window.value.start) * 1000));
const globalX = (ms: number) => ms / props.manifest.recording.duration_ms * 1000;
const label = (e: MusicEvent) => `${e.title}，${musicTime(e.start_ms)}至${musicTime(e.end_ms)}。${e.listen_for}`;
// A single whole-work root uses its timed children for the compact navigator.
const dockSections = computed(() => {
  const nested = roots.value.length === 1 ? sections.value.filter(section => section.parent_id === roots.value[0].section_id) : [];
  return (nested.length ? nested : roots.value).slice().sort((a, b) => a.start_ms! - b.start_ms!);
});
const dockCurrent = computed(() => dockSections.value.find(section => section.start_ms! <= props.position && props.position < section.end_ms!)
  ?? [...dockSections.value].reverse().find(section => section.start_ms! <= props.position));
const exercises = computed(() => material.value.filter(event => event.concept_tags.includes('导听练习')));
const dockEvents = computed(() => exercises.value.length ? exercises.value : material.value.filter(event => !event.concept_tags.includes('谱页导听')));
const hovered = ref<string | null>(null);
const dockTitle = computed(() => hovered.value ?? dockCurrent.value?.title ?? current.value?.title ?? '沿原声自由聆听');
const number = (index: number) => String(index + 1).padStart(2, '0');
const spanStyle = (start: number, end: number) => ({ left: `${globalX(start) / 10}%`, width: `${(globalX(end) - globalX(start)) / 10}%` });
</script>

<template>
  <div v-if="docked" class="music-map music-map-dock" aria-label="全曲小地图">
    <div class="dock-heading"><span class="dock-eyebrow">全曲小地图</span><span class="dock-title" :title="dockTitle">{{ dockTitle }}</span><button class="dock-guide" :aria-expanded="!!guideOpen" aria-controls="music-work-guide" aria-label="打开地图导览" @click="emit('guide')">导览 ⌃</button></div>
    <div class="dock-tracks">
      <span class="dock-row-label">段落</span>
      <nav class="dock-sections" aria-label="全曲段落定位" v-memo="[dockSections, dockCurrent?.section_id]">
        <button v-for="(section, index) in dockSections" :key="section.section_id" :style="spanStyle(section.start_ms!, section.end_ms!)" :aria-current="section.section_id === dockCurrent?.section_id ? 'true' : undefined" :aria-label="`${number(index)} · ${section.title}，${musicTime(section.start_ms!)}，定位原声`" :title="`${section.title} · ${musicTime(section.start_ms!)}`" @pointerenter="hovered = section.title" @pointerleave="hovered = null" @focus="hovered = section.title" @blur="hovered = null" @click="seek(section.start_ms!)">{{ number(index) }}</button>
        <span v-if="!dockSections.length" class="dock-empty">自由回听</span>
      </nav>
      <template v-if="dockEvents.length">
        <span class="dock-row-label">{{ exercises.length ? '回听' : '材料' }}</span>
        <nav class="dock-events" :aria-label="exercises.length ? '回听练习定位' : '音乐材料定位'" v-memo="[dockEvents, activeSignature]">
          <button v-for="event in dockEvents" :key="event.event_id" :style="spanStyle(event.start_ms, event.end_ms)" :aria-label="label(event)" :title="label(event)" @pointerenter="hovered = event.title" @pointerleave="hovered = null" @focus="hovered = event.title" @blur="hovered = null" @click="listen(event)"></button>
        </nav>
      </template>
      <i class="dock-cursor" :style="{ transform: `translateX(${globalX(position) / 10}%)` }"></i>
    </div>
    <slot></slot>
  </div>
  <details v-else class="music-map" :open="expanded" @toggle="expanded = ($event.target as HTMLDetailsElement).open">
    <summary><span class="map-eyebrow">看音乐</span><strong>{{ current?.title ?? '作品地图' }}</strong><span class="map-clock">{{ musicTime(position) }} <span v-if="occurrence">· 第 {{ occurrence.measure }} 小节</span></span></summary>
    <div class="map-body">
      <div class="map-overview" aria-label="全章曲式地图">
        <div class="map-overview-track"><button v-for="(s,i) in roots" :key="s.section_id" :style="{left: `${globalX(s.start_ms!)/10}%`,width:`${(globalX(s.end_ms!)-globalX(s.start_ms!))/10}%`,background:palette[i%palette.length],opacity:s.section_id===current?.section_id?1:.55}" :title="`${s.title} · ${musicTime(s.start_ms!)}`" :aria-label="`${s.title}，${musicTime(s.start_ms!)}，定位原声`" @click="seek(s.start_ms!)">{{ s.title }}</button><i :style="{transform:`translateX(${globalX(position)/10}%)`}"></i></div>
      </div>
      <div class="map-detail-heading"><span>主题与声部 <small>点击色块，听对应材料</small></span><button :aria-pressed="whole" @click="whole = !whole">{{ whole ? '查看当前段落' : '查看全章' }}</button></div>
      <div class="map-lane-wrap"><div class="map-lanes" aria-label="主题材料时间图">
        <div v-for="lane in visibleLanes" :key="lane.key" v-memo="[lane, window, activeSignature]" class="map-lane">
          <span class="map-lane-name" :style="{color: lane.color}" :title="lane.label">{{ lane.label }}</span>
          <svg viewBox="0 0 1000 26" preserveAspectRatio="none" role="group" :aria-label="lane.label">
            <rect width="1000" height="26" rx="3" fill="#eeeee5" />
            <g v-for="e in lane.events" :key="e.event_id" role="button" tabindex="0" :aria-label="label(e)" @click="listen(e)" @keydown.enter.prevent="listen(e)" @keydown.space.prevent="listen(e)">
              <title>{{ label(e) }}</title><rect :x="x(e.start_ms)" y="3" :width="Math.max(4,x(e.end_ms)-x(e.start_ms))" height="20" rx="3" :fill="lane.color" :opacity="e.start_ms <= position && position < e.end_ms ? 1 : .55" :stroke="e.start_ms <= position && position < e.end_ms ? '#172f29' : 'none'" stroke-width="2" />
            </g>

          </svg>
        </div>
        <p v-if="!visibleLanes.length" class="map-empty">这一段尚无精选主题标注，可沿上方曲式地图听原声。</p>
      </div>
      <i v-if="visibleLanes.length" class="map-cursor" :style="{transform:`translateX(${x(position)/10}%)`}"></i></div>
      <div class="map-scale"><span>{{ musicTime(window.start) }}</span><span>{{ musicTime(window.end) }}</span></div>
      <div class="map-now"><i :class="{playing: !paused}"></i><span v-if="participants.length">当前标注声部：{{ participants.slice(0,4).join('、') }}{{ participants.length > 4 ? ` 等 ${participants.length} 个声部` : '' }}</span><span v-else>沿原声时间推进 · 色块表示已编选材料，空白处尚未标注</span></div>
      <nav v-if="children.length && !whole" class="map-children" aria-label="当前段落内部结构"><button v-for="s in children" :key="s.section_id" :class="{active:s.start_ms! <= position && position < s.end_ms!}" @click="seek(s.start_ms!)">{{ s.title }}</button></nav>
      <button v-if="compact" class="map-return" @click="expanded = false">返回乐谱</button>
    </div>
  </details>
</template>

<style scoped>
.music-map-dock{flex:1;min-width:0;background:transparent!important;border:0!important;color:#eee8d6;container-type:inline-size}
.dock-heading{height:18px;display:flex;align-items:center;gap:12px;font-size:11px}
.dock-eyebrow{flex-shrink:0;color:#d7e0ce}.dock-title{color:#e8cda7;overflow:hidden;white-space:nowrap;text-overflow:ellipsis;min-width:0}
.dock-heading .dock-guide{margin-left:auto;padding:0 5px;flex-shrink:0;font-size:10px;min-height:22px;color:#e8e9dc;border-color:#718172}
.dock-tracks{position:relative;display:grid;grid-template-columns:28px 1fr;grid-template-rows:22px;grid-auto-rows:13px;align-items:stretch}
.dock-row-label{display:flex;align-items:center;color:#becbb9;font-size:10px}
.dock-sections,.dock-events{position:relative;min-width:0;isolation:isolate}
.dock-sections button,.dock-events button{position:absolute;top:0;height:100%;min-height:0!important;padding:0 1px;border:0;border-radius:0;box-sizing:border-box;background:transparent;color:#2f3428;font:11px Georgia,serif;display:flex;align-items:center;justify-content:center}
.dock-sections button::before,.dock-events button::before{content:'';position:absolute;inset:2px 1px;z-index:-1;border-radius:2px;background:#c49b7f}
.dock-sections button:nth-child(even)::before{background:#b98e79}
.dock-sections button[aria-current=true]::before{background:#efd6a9}
.dock-sections button:hover::before,.dock-sections button:focus-visible::before{background:#f6dcb2}
.dock-events{background:linear-gradient(transparent 47%,#637a69 47% 55%,transparent 55%)}
.dock-events button::before{inset:4px 1px;background:#a7baa7}
.dock-events button:hover::before,.dock-events button:focus-visible::before{background:#eed5aa}
.dock-cursor{position:absolute;left:28px;right:0;top:0;bottom:0;pointer-events:none;will-change:transform}
.dock-cursor::before{content:'';display:block;height:100%;width:2px;background:#f2ebcf}
.dock-empty{font-size:11px;color:#b9c6b2}
.map-lane-wrap{position:relative;overflow:hidden;--map-label:120px}.map-cursor{position:absolute;top:0;bottom:0;left:var(--map-label);right:0;pointer-events:none;will-change:transform;z-index:1}.map-cursor::before,.map-overview-track i::before{content:"";display:block;width:2px;height:100%;background:#172f29}.music-map{flex-shrink:0;background:#faf9f2;border-bottom:1px solid #dedfd4;color:#29443a;font-family:var(--sans,sans-serif)}
summary{display:flex;align-items:center;gap:12px;padding:10px 24px;cursor:pointer;list-style:none;font-size:13px}summary::after{content:'⌄';margin-left:6px}.map-eyebrow{color:#9b6036;font-size:11px;letter-spacing:1px}.map-clock{margin-left:auto;color:#6e7868;font-size:11px}.map-body{padding:0 24px 10px}.map-lanes{max-height:112px;overflow:auto;scrollbar-width:thin}.map-children{max-height:32px;overflow:auto}.map-overview-track{position:relative;height:36px;overflow:hidden}.map-overview-track button{position:absolute;top:0;height:34px;color:white;border:1px solid #faf9f2;overflow:hidden;text-overflow:ellipsis;white-space:nowrap;padding:0 5px;font-size:11px;text-align:left}.map-overview-track i{position:absolute;top:0;bottom:0;width:100%;pointer-events:none;will-change:transform}.map-overview svg{width:100%;height:38px;display:block}svg g{cursor:pointer}svg g:focus{outline:2px solid #315c4b;outline-offset:-2px}text{font-size:12px;fill:white;pointer-events:none}.map-detail-heading{display:flex;justify-content:space-between;align-items:center;font-size:12px;margin:9px 0 6px}small{font-size:10px;color:#7c8275;margin-left:6px}button{font:inherit;color:inherit;background:transparent;border:1px solid #d9decf;border-radius:5px;padding:3px 8px;cursor:pointer}.map-lane{display:grid;grid-template-columns:110px 1fr;gap:10px;align-items:center;margin:4px 0}.map-lane-name{font-size:11px;overflow:hidden;white-space:normal;line-height:1.35}.map-lane svg{width:100%;height:26px;overflow:hidden}.map-return{display:block;margin:8px 0 0 auto;font-size:11px}.map-scale{display:flex;justify-content:space-between;margin-left:120px;color:#7b8274;font-size:10px}.map-now{display:flex;align-items:center;gap:6px;font-size:11px;margin-top:6px;color:#647460}.map-now i{width:6px;height:6px;background:#9eaa91;border-radius:50%;flex-shrink:0}.map-now i.playing{background:#b57832;box-shadow:0 0 0 3px #b578321f}.map-empty{font-size:12px;color:#79816e;margin:8px 0}.map-children{display:flex;gap:4px;flex-wrap:wrap;margin-top:7px;font-size:10px}.map-children .active{background:#e6ecda;border-color:#a7b693}@media(max-width:640px){.map-lane-wrap{--map-label:86px}summary{padding:9px 12px;gap:8px}.map-body{padding:0 12px 8px}.map-lanes{max-height:86px}.map-lane{grid-template-columns:80px 1fr;gap:6px}.map-scale{margin-left:86px}.map-clock{font-size:10px}.map-detail-heading small{display:none}text{font-size:14px}.map-now{line-height:1.4}}
</style>
