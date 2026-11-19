<script setup lang="ts">
import { computed, onBeforeUnmount, ref, watch } from 'vue';
import { Mic, Square, X } from '@lucide/vue';
import { api as sharedApi, ApiError } from '../api';
import { bindSceneApi, readerKey } from '../reader-context';
import { getLocale, t } from '../i18n';
import { voiceOwner } from '../voice-input';

const props = defineProps<{ scope: string; disabled?: boolean }>();
const emit = defineEmits<{ insert: [text: string]; active: [busy: boolean] }>();
let api = bindSceneApi(sharedApi, readerKey);
const owner = Symbol('voice-input');
const phase = ref<'idle' | 'requesting' | 'recording' | 'transcribing' | 'review' | 'error'>('idle');
const transcript = ref(''), error = ref(''), elapsed = ref(0);
const occupied = computed(() => voiceOwner.value !== null && voiceOwner.value !== owner);
const clock = computed(() => `${Math.floor(elapsed.value / 60)}:${String(elapsed.value % 60).padStart(2, '0')}`);
let generation = 0, recorder: MediaRecorder | undefined, stream: MediaStream | undefined;
let timer: ReturnType<typeof setInterval> | undefined, request: AbortController | undefined;
let audio: Blob | undefined;
const retryable = ref(false);

function releaseMicrophone() {
  clearInterval(timer); timer = undefined;
  stream?.getTracks().forEach(track => track.stop()); stream = undefined;
}
function cancel() {
  const wasActive = phase.value !== 'idle';
  generation++;
  request?.abort(); request = undefined;
  if (recorder?.state === 'recording') recorder.stop();
  recorder = undefined; releaseMicrophone(); audio = undefined;
  retryable.value = false; transcript.value = ''; error.value = ''; phase.value = 'idle';
  if (voiceOwner.value === owner) voiceOwner.value = null;
  if (wasActive) emit('active', false);
}
function fail(message: string) {
  releaseMicrophone(); phase.value = 'error'; error.value = message;
}
function microphoneError(failure: unknown) {
  if (failure instanceof ApiError && failure.errorCode === 'VOICE_FAILED') return t('语音识别暂不可用，请重试或继续打字。');
  const name = failure instanceof Error || failure instanceof DOMException ? failure.name : '';
  if (name === 'NotAllowedError' || name === 'SecurityError') return t('没有获得麦克风权限。请在浏览器中允许使用麦克风后重试。');
  if (name === 'NotFoundError') return t('没有找到麦克风，请连接后重试。');
  if (name === 'NotReadableError') return t('麦克风暂时无法使用，请关闭占用麦克风的应用后重试。');
  return failure instanceof Error ? failure.message : t('录音没有完成，请重试。');
}
async function transcribe(token = generation) {
  if (!audio || token !== generation) return;
  phase.value = 'transcribing'; error.value = ''; request = new AbortController();
  try {
    const bytes = new Uint8Array(await audio.arrayBuffer());
    let binary = '';
    for (const byte of bytes) binary += String.fromCharCode(byte);
    if (token !== generation) return;
    const result = await api.voiceTranscribe({ audio: btoa(binary), mime_type: audio.type, locale: getLocale() }, request.signal);
    if (token !== generation) return;
    if (!result.text.trim()) { audio = undefined; retryable.value = false; fail(t('没有听清这段语音，请靠近麦克风重新录音。')); return; }
    transcript.value = result.text.trim(); phase.value = 'review'; audio = undefined; retryable.value = false;
  } catch (failure) {
    if (token === generation) { retryable.value = true; fail(t('转写暂未完成，可以重试这段录音。') + ' ' + microphoneError(failure)); }
  } finally { if (token === generation) request = undefined; }
}
function stop() {
  if (recorder?.state !== 'recording') return;
  phase.value = 'transcribing'; clearInterval(timer);
  recorder.stop(); releaseMicrophone();
}
async function start() {
  if (props.disabled || occupied.value) return;
  cancel(); voiceOwner.value = owner; emit('active', true);
  api = bindSceneApi(sharedApi, readerKey);
  phase.value = 'requesting'; elapsed.value = 0;
  const token = generation;
  try {
    if (!navigator.mediaDevices?.getUserMedia || typeof MediaRecorder === 'undefined') {
      throw new Error(t('当前浏览器无法录音。请使用 HTTPS 或本机地址，并在支持录音的浏览器中打开。'));
    }
    const status = await api.voiceStatus();
    if (token !== generation) return;
    if (!status.available) throw new Error(t('语音输入暂未启用，请先配置本地语音识别。'));
    const input = await navigator.mediaDevices.getUserMedia({ audio: { channelCount: 1, echoCancellation: true, noiseSuppression: true }, video: false });
    if (token !== generation) { input.getTracks().forEach(track => track.stop()); return; }
    stream = input;
    const mime = ['audio/webm;codecs=opus', 'audio/mp4', 'audio/ogg;codecs=opus', 'audio/webm'].find(type => MediaRecorder.isTypeSupported(type));
    recorder = new MediaRecorder(input, { ...(mime ? { mimeType: mime } : {}), audioBitsPerSecond: 48000 });
    const chunks: Blob[] = [], recording = recorder;
    recording.ondataavailable = event => { if (token === generation && event.data.size) chunks.push(event.data); };
    recording.onerror = () => {
      if (token !== generation) return;
      generation++; if (recording.state === 'recording') recording.stop(); recorder = undefined;
      fail(t('录音没有完成，请重试。'));
    };
    recording.onstop = () => {
      if (token !== generation) return;
      releaseMicrophone(); recorder = undefined;
      audio = new Blob(chunks, { type: recording.mimeType || chunks[0]?.type || mime });
      if (!audio.size) { fail(t('录音为空，请重新录音。')); return; }
      retryable.value = true; void transcribe(token);
    };
    recording.start(1000); phase.value = 'recording';
    const started = Date.now(), maxSeconds = Math.min(status.max_seconds, 90);
    timer = setInterval(() => { elapsed.value = Math.floor((Date.now() - started) / 1000); if (elapsed.value >= maxSeconds) stop(); }, 250);
  } catch (failure) { if (token === generation) fail(microphoneError(failure)); }
}
function insert() {
  if (!transcript.value.trim() || props.disabled) return;
  const value = transcript.value.trim(); cancel(); emit('insert', value);
}
watch(() => [props.scope, readerKey()], () => cancel());
watch(() => props.disabled, disabled => { if (disabled && phase.value !== 'idle') cancel(); });
onBeforeUnmount(cancel);
</script>

<template>
  <div class="voice-input" :class="{ opened: phase !== 'idle' }">
    <button v-if="phase === 'idle'" type="button" class="voice-start" :disabled="disabled || occupied" @click="start">
      <Mic :size="16" aria-hidden="true" /><span>{{ t('语音输入') }}</span>
    </button>
    <template v-else>
      <div class="voice-heading">
        <span class="voice-state" role="status"><i v-if="phase === 'recording'" class="recording-dot" />{{ phase === 'recording' ? t('正在录音') : phase === 'requesting' ? t('正在打开麦克风…') : phase === 'transcribing' ? t('正在转成文字…') : phase === 'review' ? t('检查识别文字') : t('语音输入') }}<time v-if="phase === 'recording'">{{ clock }} / 1:30</time></span>
        <button type="button" class="voice-cancel" :aria-label="t('取消语音输入')" @click="cancel"><X :size="16" aria-hidden="true" /></button>
      </div>
      <p v-if="error" class="voice-error" role="alert">{{ error }}</p>
      <template v-if="phase === 'review'">
        <textarea v-model="transcript" :aria-label="t('识别结果')" rows="3" />
        <p class="voice-hint">{{ t('可以修改识别文字，确认后补到输入框末尾。') }}</p>
      </template>
      <div class="voice-actions">
        <button v-if="phase === 'recording'" type="button" class="voice-primary" @click="stop"><Square :size="13" fill="currentColor" aria-hidden="true" />{{ t('停止并转文字') }}</button>
        <button v-if="phase === 'review'" type="button" class="voice-primary" :disabled="!transcript.trim()" @click="insert">{{ t('加入输入框') }}</button>
        <button v-if="phase === 'error' && retryable" type="button" class="voice-primary" @click="transcribe()">{{ t('重试转写') }}</button>
        <button v-if="phase === 'review' || phase === 'error'" type="button" @click="start">{{ t('重新录音') }}</button>
      </div>
    </template>
  </div>
</template>

<style scoped>
.voice-input { min-width: 0; color: var(--ink); font-size: .8rem; }
.voice-input.opened { width: 100%; box-sizing: border-box; border: 1px solid var(--line); border-radius: 10px; padding: .7rem .8rem; background: var(--canvas); }
button { display: inline-flex; gap: .4rem; align-items: center; justify-content: center; min-height: 36px; padding: .35rem .6rem; border: 1px solid var(--line); border-radius: 7px; font: inherit; color: var(--brand-green-deep); background: transparent; cursor: pointer; }
button:disabled { opacity: .5; cursor: default; }
.voice-start { border-color: transparent; padding-inline: .15rem .4rem; }
.voice-heading,.voice-state { display: flex; gap: .5rem; align-items: center; }
.voice-heading { justify-content: space-between; }
.voice-state { flex-wrap: wrap; font-weight: 600; }
time { font-size: .75rem; color: var(--steel); font-variant-numeric: tabular-nums; font-weight: 400; }
.voice-cancel { min-width: 32px; min-height: 32px; padding: .25rem; border: 0; color: var(--slate); }
.recording-dot { width: 7px; height: 7px; border-radius: 50%; background: var(--reader-coral); }
.voice-actions { display: flex; flex-wrap: wrap; gap: .5rem; }
.voice-actions:not(:empty) { margin-top: .5rem; }
.voice-primary { border-color: var(--brand-green); background: color-mix(in srgb,var(--brand-green) 8%,var(--canvas)); }
textarea { display: block; width: 100%; box-sizing: border-box; margin-top: .5rem; border: 1px solid var(--line); border-radius: 7px; padding: .5rem .65rem; font: inherit; font-size: 16px; line-height: 1.6; color: var(--ink); background: var(--canvas); resize: vertical; }
.voice-hint,.voice-error { line-height: 1.6; margin: .4rem 0 0; overflow-wrap: anywhere; }
.voice-hint { color: var(--steel); font-size: .75rem; }.voice-error { color: var(--reader-coral); }
button:focus-visible,textarea:focus-visible { outline: 2px solid var(--brand-green); outline-offset: 2px; }
</style>
