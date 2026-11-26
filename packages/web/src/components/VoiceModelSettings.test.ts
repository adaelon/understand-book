// @vitest-environment happy-dom
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import { flushPromises, mount } from '@vue/test-utils';
import VoiceModelSettings from './VoiceModelSettings.vue';
import { api, type VoiceModelStatus } from '../api';

function makeStatus(overrides: Partial<VoiceModelStatus> = {}): VoiceModelStatus {
  return {
    runtime_available: true,
    available: false,
    can_download: true,
    source: 'none',
    model_dir: null,
    default_model_dir: 'D:\\UnderstandBook\\voice-model',
    configured_model_dir: null,
    model_name: 'Systran/faster-whisper-small',
    model_size_bytes: 483_546_902,
    download: {
      state: 'idle',
      current_file: null,
      downloaded_bytes: 0,
      total_bytes: 483_546_902,
      target_dir: null,
      source: null,
      error: null,
    },
    ...overrides,
  };
}

beforeEach(() => {
  vi.useFakeTimers();
  vi.spyOn(api, 'voiceModelStatus').mockResolvedValue(makeStatus());
  vi.spyOn(api, 'voiceModelDownload').mockResolvedValue(makeStatus({
    download: {
      state: 'downloading',
      current_file: 'model.bin',
      downloaded_bytes: 100 * 1024 * 1024,
      total_bytes: 483_546_902,
      target_dir: 'E:\\VoiceModel',
      source: 'hf-mirror',
      error: null,
    },
  }));
  vi.spyOn(api, 'voiceModelSelect').mockResolvedValue(makeStatus({
    available: true,
    source: 'configured',
    model_dir: 'E:\\ExistingWhisper',
    configured_model_dir: 'E:\\ExistingWhisper',
  }));
  vi.spyOn(api, 'voiceModelCancel').mockResolvedValue(makeStatus({
    download: {
      state: 'cancelled',
      current_file: null,
      downloaded_bytes: 0,
      total_bytes: 483_546_902,
      target_dir: null,
      source: null,
      error: '已取消下载。',
    },
  }));
  vi.spyOn(api, 'voiceModelRemove').mockResolvedValue(makeStatus());
});

afterEach(() => {
  vi.useRealTimers();
  vi.restoreAllMocks();
});

it('starts optional model download with selected mirror and target directory, and tracks progress', async () => {
  const wrapper = mount(VoiceModelSettings, { props: { native: false } });
  await flushPromises();

  expect(wrapper.text()).toContain('未下载');
  await wrapper.get('#voice-model-source').setValue('hf-mirror');
  await wrapper.get('.voice-model-dir-input').setValue('E:\\VoiceModel');
  await wrapper.get('.primary-action').trigger('click');
  await flushPromises();

  expect(api.voiceModelDownload).toHaveBeenCalledWith({
    target_dir: 'E:\\VoiceModel',
    source: 'hf-mirror',
  });
  expect(wrapper.text()).toContain('正在下载离线语音模型');
  expect(wrapper.text()).toContain('model.bin');

  vi.mocked(api.voiceModelStatus).mockResolvedValueOnce(makeStatus({
    available: true,
    source: 'configured',
    model_dir: 'E:\\VoiceModel',
    configured_model_dir: 'E:\\VoiceModel',
    download: {
      state: 'completed',
      current_file: null,
      downloaded_bytes: 483_546_902,
      total_bytes: 483_546_902,
      target_dir: 'E:\\VoiceModel',
      source: 'hf-mirror',
      error: null,
    },
  }));
  await vi.advanceTimersByTimeAsync(700);
  await flushPromises();

  expect(wrapper.text()).toContain('已就绪（离线可用）');
  expect(wrapper.text()).toContain('E:\\VoiceModel');
  wrapper.unmount();
});

it('activates an existing local model directory and allows deleting downloaded model', async () => {
  const wrapper = mount(VoiceModelSettings, { props: { native: false } });
  await flushPromises();

  await wrapper.get('.voice-model-dir-input').setValue('E:\\ExistingWhisper');
  const existingBtn = wrapper.findAll('button').find(b => b.text().includes('使用已有模型'))!;
  await existingBtn.trigger('click');
  await flushPromises();

  expect(api.voiceModelSelect).toHaveBeenCalledWith('E:\\ExistingWhisper');
  expect(wrapper.text()).toContain('已就绪（离线可用）');

  const deleteBtn = wrapper.findAll('button').find(b => b.text().includes('删除模型'))!;
  await deleteBtn.trigger('click');
  await flushPromises();
  expect(api.voiceModelRemove).toHaveBeenCalledWith(true);
  wrapper.unmount();
});
