<script setup lang="ts">
import { computed } from 'vue';
import { Play, Square, X, BookOpen } from '@lucide/vue';
import { pianoExample, pianoPitches, pianoPlaying, pianoLoading, pianoError, pitchName, playPiano, stopPiano } from '../music-example-player';
import { t } from '../i18n';
const base = import.meta.env.BASE_URL;
const emit = defineEmits<{ source: [sourceId: string] }>();
const keys = computed(() => {
  const pitches = pianoExample.value?.notes.map(note => note.midi_pitch) ?? [60];
  const first = Math.floor(Math.min(...pitches) / 12) * 12;
  const last = Math.min(127, Math.max(first + 12, Math.ceil((Math.max(...pitches) + 1) / 12) * 12));
  let white = 0;
  const keys = Array.from({ length: last - first + 1 }, (_, index) => {
    const pitch = first + index, black = [1, 3, 6, 8, 10].includes(pitch % 12);
    return { pitch, black, position: black ? white - 0.32 : white++ };
  });
  return { keys, white };
});
</script>

<template>
  <section v-if="pianoExample" class="music-example-piano" :aria-label="t('钢琴试听')" data-reader-selection-ignore>
    <div class="piano-heading"><div><strong>{{ pianoExample.title }}</strong><small>Yamaha C5 · FLAC · {{ t('示范速度') }} ♩ = {{ pianoExample.tempo_bpm }} · <a :href="`${base}piano/salamander-v3/credits.html`" target="_blank" rel="noopener">{{ t('音色来源') }}</a></small></div><button type="button" :aria-label="t('收起试听')" @click="stopPiano(true)"><X :size="17" /></button></div>
    <div class="piano-status"><span :role="pianoLoading ? 'status' : undefined">{{ pianoLoading ? t('正在准备钢琴音色…') : pianoPitches.length ? pianoPitches.map(pitchName).join(' + ') : pianoPlaying ? t('休止') : t('点琴键也能单听') }}</span><div><button v-if="pianoExample.source_ref_id" type="button" @click="emit('source', pianoExample.source_ref_id)"><BookOpen :size="13" />{{ t('查看谱面') }}</button><button type="button" @click="pianoPlaying || pianoLoading ? stopPiano() : playPiano(pianoExample)"><Square v-if="pianoPlaying || pianoLoading" :size="13" /><Play v-else :size="13" />{{ pianoPlaying || pianoLoading ? t('停止') : t('重听') }}</button></div></div>
    <div class="piano-keys" role="group" :aria-label="t('钢琴键盘')">
      <button v-for="key in keys.keys" :key="key.pitch" type="button" class="piano-key" :class="{ black: key.black, sounding: pianoPitches.includes(key.pitch) }" :style="{ left: `${key.position / keys.white * 100}%`, width: `${(key.black ? 0.64 : 1) / keys.white * 100}%` }" :aria-label="pitchName(key.pitch)" :aria-pressed="pianoPitches.includes(key.pitch)" :data-pitch="key.pitch" @click="playPiano(pianoExample, key.pitch)"><span>{{ key.black ? '' : pitchName(key.pitch) }}</span></button>
    </div>
    <p v-if="pianoError" class="piano-error" role="alert">{{ pianoError }}</p>
  </section>
</template>

<style scoped>
.music-example-piano{flex-shrink:0;position:relative;z-index:4;padding:12px 16px 14px;background:var(--paper,#fcfaf4);border-top:1px solid var(--line,#dfdfd2);box-shadow:0 -5px 18px #21382a08;color:var(--ink,#263e35)}
.piano-heading,.piano-status,.piano-status>div{display:flex;align-items:center;justify-content:space-between;gap:12px}.piano-heading>div{display:flex;flex-direction:column;gap:3px;min-width:0}.piano-heading strong{font-size:12px;font-weight:500;overflow-wrap:anywhere}.piano-heading small{font-size:11px;color:var(--steel,#707868)}
.piano-heading a{color:inherit;text-underline-offset:2px}
.music-example-piano button{font:inherit;cursor:pointer}.piano-heading>button{border:0;background:transparent;padding:6px;color:inherit;display:grid;place-items:center}.piano-status{font-size:12px;margin:9px 0}.piano-status>span{min-height:20px;font-variant-numeric:tabular-nums}.piano-status button{display:inline-flex;align-items:center;gap:4px;border:0;background:transparent;color:inherit;padding:5px 0;white-space:nowrap}
.piano-keys{position:relative;height:100px}.piano-key{position:absolute;top:0;height:100%;padding:0;border:1px solid #c7c5b9;border-radius:0 0 4px 4px;background:linear-gradient(#fffdf5,#f6f2e7);color:#364234;box-sizing:border-box}.piano-key span{position:absolute;bottom:7px;left:0;right:0;font-size:10px;text-align:center}.piano-key.black{height:62%;z-index:1;border-color:#1f2d27;background:linear-gradient(#39443e,#1e2c26);box-shadow:0 2px 2px #0004}.piano-key.sounding{background:#c6d8b8;border-color:#668259;box-shadow:inset 0 -4px 0 #668259}.piano-key.black.sounding{background:#6d9164;border-color:#4b7043}.piano-error{font-size:12px;color:#a3402c;margin:8px 0 0}@media(max-width:480px){.music-example-piano{padding:10px 12px}.piano-keys{height:86px}.piano-key span{font-size:9px}}
</style>
