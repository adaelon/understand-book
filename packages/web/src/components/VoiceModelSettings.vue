<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from 'vue';
import { Download, FolderOpen, Trash2, X } from '@lucide/vue';
import { api, type VoiceModelStatus } from '../api';
import { t } from '../i18n';

const props = defineProps<{ native?: boolean }>();
const emit = defineEmits<{ busy: [active: boolean] }>();

const status = ref<VoiceModelStatus | null>(null);
const loading = ref(false);
const acting = ref(false);
const error = ref('');
const source = ref<'huggingface' | 'hf-mirror'>('huggingface');
const targetDir = ref('');
let pollTimer: ReturnType<typeof setInterval> | undefined;

const downloading = computed(() =>
  ['downloading', 'verifying', 'cancelling'].includes(status.value?.download.state ?? '')
);
const progressPercent = computed(() => {
  const dl = status.value?.download;
  if (!dl || !dl.total_bytes) return 0;
  return Math.min(100, Math.round((dl.downloaded_bytes / dl.total_bytes) * 100));
});
const modelSizeMb = computed(() =>
  Math.round((status.value?.model_size_bytes ?? 483_546_902) / (1024 * 1024))
);
const downloadedMb = computed(() =>
  ((status.value?.download.downloaded_bytes ?? 0) / (1024 * 1024)).toFixed(1)
);
const totalMb = computed(() =>
  ((status.value?.download.total_bytes ?? 483_546_902) / (1024 * 1024)).toFixed(1)
);
const effectiveTargetDir = computed(() =>
  targetDir.value.trim() || status.value?.configured_model_dir || status.value?.default_model_dir || ''
);
const stateSummary = computed(() => {
  if (!status.value) return loading.value ? t('正在检测...') : t('尚未检测');
  const dlState = status.value.download.state;
  if (dlState === 'downloading') return t('正在下载离线语音模型');
  if (dlState === 'verifying') return t('正在校验模型完整性...');
  if (dlState === 'cancelling') return t('正在取消下载...');
  if (status.value.available) {
    if (status.value.source === 'env') return t('已就绪（环境变量配置）');
    return t('已就绪（离线可用）');
  }
  if (!status.value.runtime_available) return t('当前环境未内置语音运行环境');
  return t('未下载（按需安装，约 461 MB）');
});

function stopPolling() {
  if (pollTimer !== undefined) {
    clearInterval(pollTimer);
    pollTimer = undefined;
  }
}

function syncPolling() {
  if (downloading.value) {
    emit('busy', true);
    if (pollTimer === undefined) {
      pollTimer = setInterval(() => {
        void refresh(true);
      }, 600);
    }
  } else {
    stopPolling();
    emit('busy', acting.value);
  }
}

async function refresh(silent = false) {
  if (!silent) {
    loading.value = true;
    error.value = '';
  }
  try {
    const next = await api.voiceModelStatus();
    status.value = next;
    if (!targetDir.value && (next.configured_model_dir || next.default_model_dir)) {
      targetDir.value = next.configured_model_dir || next.default_model_dir || '';
    }
    if (next.download.state === 'failed' && next.download.error) {
      error.value = next.download.error;
    }
    syncPolling();
  } catch (cause) {
    if (!silent) error.value = cause instanceof Error ? cause.message : String(cause);
    stopPolling();
    emit('busy', false);
  } finally {
    if (!silent) loading.value = false;
  }
}

async function pickFolder(title: string): Promise<string | null> {
  if (!props.native) return null;
  const { open } = await import('@tauri-apps/plugin-dialog');
  const selected = await open({
    directory: true,
    multiple: false,
    title,
  });
  return typeof selected === 'string' && selected.trim() ? selected : null;
}

async function chooseTargetDirectory() {
  error.value = '';
  try {
    const selected = await pickFolder(t('选择离线语音模型存放目录'));
    if (selected) targetDir.value = selected;
  } catch (cause) {
    error.value = cause instanceof Error ? cause.message : String(cause);
  }
}

async function useExistingDirectory() {
  error.value = '';
  acting.value = true;
  emit('busy', true);
  try {
    const selected = props.native
      ? await pickFolder(t('选择已有的离线语音模型目录'))
      : targetDir.value.trim();
    if (!selected) return;
    status.value = await api.voiceModelSelect(selected);
    targetDir.value = status.value.configured_model_dir || selected;
  } catch (cause) {
    error.value = cause instanceof Error ? cause.message : String(cause);
  } finally {
    acting.value = false;
    syncPolling();
  }
}

async function startDownload() {
  error.value = '';
  acting.value = true;
  emit('busy', true);
  try {
    status.value = await api.voiceModelDownload({
      target_dir: effectiveTargetDir.value || undefined,
      source: source.value,
    });
    syncPolling();
  } catch (cause) {
    error.value = cause instanceof Error ? cause.message : String(cause);
  } finally {
    acting.value = false;
    syncPolling();
  }
}

async function cancelDownload() {
  error.value = '';
  try {
    status.value = await api.voiceModelCancel();
    syncPolling();
  } catch (cause) {
    error.value = cause instanceof Error ? cause.message : String(cause);
  }
}

async function removeModel() {
  error.value = '';
  acting.value = true;
  emit('busy', true);
  try {
    status.value = await api.voiceModelRemove(true);
    targetDir.value = status.value.default_model_dir || '';
  } catch (cause) {
    error.value = cause instanceof Error ? cause.message : String(cause);
  } finally {
    acting.value = false;
    syncPolling();
  }
}

defineExpose({ refresh });
onMounted(() => { void refresh(); });
onBeforeUnmount(() => {
  stopPolling();
  emit('busy', false);
});
</script>

<template>
  <div class="voice-model-settings">
    <div class="desktop-settings-row">
      <div>
        <strong>{{ t('离线语音识别模型') }}</strong>
        <p>{{ stateSummary }}</p>
      </div>
      <div class="voice-model-top-actions">
        <button
          v-if="downloading"
          type="button"
          :disabled="status?.download.state === 'cancelling'"
          @click="cancelDownload"
        >
          <X :size="15" aria-hidden="true" />
          <span>{{ t('取消下载') }}</span>
        </button>
        <button
          v-else-if="status?.can_download && !status.available"
          type="button"
          class="primary-action"
          :disabled="acting || loading"
          @click="startDownload"
        >
          <Download :size="15" aria-hidden="true" />
          <span>{{ t('下载模型') }} ({{ modelSizeMb }} MB)</span>
        </button>
        <button
          v-else-if="status?.available && status.source !== 'env' && status.source !== 'bundled'"
          type="button"
          :disabled="acting || loading"
          @click="removeModel"
        >
          <Trash2 :size="15" aria-hidden="true" />
          <span>{{ t('删除模型') }}</span>
        </button>
      </div>
    </div>

    <p class="desktop-settings-message">
      {{ t('用于问答框的本机离线语音转文字（中/英文）。模型按需下载，不占用安装包体积，可存放到其他盘符。') }}
    </p>

    <div v-if="downloading" class="voice-model-progress" role="status" aria-live="polite">
      <div class="voice-model-progress-track">
        <div class="voice-model-progress-fill" :style="{ width: `${progressPercent}%` }" />
      </div>
      <div class="voice-model-progress-meta">
        <span>{{ status?.download.current_file || 'model.bin' }}</span>
        <span>{{ downloadedMb }} / {{ totalMb }} MB ({{ progressPercent }}%)</span>
      </div>
    </div>

    <div v-if="status?.can_download && status.source !== 'env'" class="voice-model-controls">
      <div class="voice-model-field">
        <label class="voice-model-label" for="voice-model-source">{{ t('下载源') }}</label>
        <select id="voice-model-source" v-model="source" :disabled="downloading || acting">
          <option value="huggingface">{{ t('HuggingFace（官方源）') }}</option>
          <option value="hf-mirror">{{ t('HF-Mirror（国内镜像）') }}</option>
        </select>
      </div>
      <div class="voice-model-dir-row">
        <input
          v-model="targetDir"
          type="text"
          class="voice-model-dir-input"
          :aria-label="t('模型存放目录')"
          :placeholder="status.default_model_dir || ''"
          :disabled="downloading || acting"
        />
        <button
          v-if="native"
          type="button"
          :disabled="downloading || acting"
          @click="chooseTargetDirectory"
        >
          <FolderOpen :size="15" aria-hidden="true" />
          <span>{{ t('更改目录') }}</span>
        </button>
        <button
          type="button"
          :disabled="downloading || acting"
          @click="useExistingDirectory"
        >
          <span>{{ t('使用已有模型') }}</span>
        </button>
      </div>
    </div>

    <p v-if="status?.model_dir" class="desktop-settings-path">{{ status.model_dir }}</p>
    <p v-if="error" class="book-picker-error" role="alert">{{ error }}</p>
  </div>
</template>

<style scoped>
.voice-model-settings { display: grid; gap: .55rem; }
.voice-model-top-actions { display: flex; flex-wrap: wrap; gap: .45rem; align-items: center; }
.voice-model-settings button { display: inline-flex; align-items: center; gap: .35rem; }
.voice-model-progress { display: grid; gap: .3rem; }
.voice-model-progress-track { height: 7px; border-radius: 999px; background: color-mix(in srgb, var(--line) 75%, transparent); overflow: hidden; }
.voice-model-progress-fill { height: 100%; border-radius: 999px; background: var(--brand-green); transition: width .2s ease; }
.voice-model-progress-meta { display: flex; justify-content: space-between; gap: .5rem; font-size: .78rem; color: var(--steel); font-variant-numeric: tabular-nums; }
.voice-model-controls { display: grid; gap: .5rem; }
.voice-model-field { display: flex; align-items: center; gap: .5rem; flex-wrap: wrap; }
.voice-model-label { font-size: .8rem; color: var(--steel); }
.voice-model-field select { border: 1px solid var(--line); border-radius: 7px; padding: .3rem .55rem; font: inherit; font-size: .82rem; color: var(--ink); background: var(--canvas); }
.voice-model-dir-row { display: flex; gap: .45rem; flex-wrap: wrap; align-items: center; }
.voice-model-dir-input { flex: 1 1 220px; min-width: 0; border: 1px solid var(--line); border-radius: 7px; padding: .35rem .55rem; font: inherit; font-size: .8rem; color: var(--ink); background: var(--canvas); }
</style>
