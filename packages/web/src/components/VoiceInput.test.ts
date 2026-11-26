// @vitest-environment happy-dom
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import { enableAutoUnmount, flushPromises, mount } from '@vue/test-utils';
import { api } from '../api';
import { voiceOwner } from '../voice-input';
import VoiceInput from './VoiceInput.vue';
import QuestionWork from './QuestionWork.vue';
import type { TutorActivity } from '../api';

enableAutoUnmount(afterEach);
const track = { stop: vi.fn() };
const stream = { getTracks: () => [track] } as unknown as MediaStream;
const getUserMedia = vi.fn();
class Recorder {
  static isTypeSupported = () => true;
  state = 'inactive'; mimeType = 'audio/webm;codecs=opus';
  ondataavailable?: (event: { data: Blob }) => void;
  onstop?: () => void;
  start() { this.state = 'recording'; }
  stop() { this.state = 'inactive'; this.ondataavailable?.({ data: new Blob(['recorded audio']) }); this.onstop?.(); }
}
beforeEach(() => {
  voiceOwner.value = null; track.stop.mockClear(); localStorage.clear();
  getUserMedia.mockReset().mockResolvedValue(stream);
  vi.stubGlobal('navigator', { mediaDevices: { getUserMedia } });
  vi.stubGlobal('MediaRecorder', Recorder);
  vi.spyOn(api, 'voiceStatus').mockResolvedValue({ available: true, max_seconds: 90 });
  vi.spyOn(api, 'voiceTranscribe').mockResolvedValue({ text: '我认为最小值是四。', duration_ms: 5000 });
});
afterEach(() => { vi.restoreAllMocks(); vi.unstubAllGlobals(); vi.useRealTimers(); });
async function record(wrapper: ReturnType<typeof mount>) {
  await wrapper.get('.voice-start').trigger('click'); await flushPromises();
  expect(wrapper.text()).toContain('正在录音');
  await wrapper.get('.voice-primary').trigger('click'); await flushPromises();
}

it('requires an explicit confirmation and inserts edited text only after releasing the microphone', async () => {
  const wrapper = mount(VoiceInput, { props: { scope: 'question-1' } });
  await record(wrapper);
  expect(track.stop).toHaveBeenCalledOnce();
  expect(wrapper.emitted('insert')).toBeUndefined();
  await wrapper.get('textarea').setValue('我认为最小值是 4，但不确定取值点。');
  await wrapper.get('.voice-primary').trigger('click');
  expect(wrapper.emitted('insert')).toEqual([['我认为最小值是 4，但不确定取值点。']]);
  expect(wrapper.emitted('active')?.at(-1)).toEqual([false]);
});

it('releases a microphone granted after cancellation without starting a recording', async () => {
  let grant!: (value: MediaStream) => void;
  getUserMedia.mockImplementation(() => new Promise(resolve => { grant = resolve; }));
  const wrapper = mount(VoiceInput, { props: { scope: 'question-1' } });
  await wrapper.get('.voice-start').trigger('click'); await flushPromises();
  await wrapper.get('.voice-cancel').trigger('click');
  grant(stream); await flushPromises();
  expect(track.stop).toHaveBeenCalledOnce();
  expect(api.voiceTranscribe).not.toHaveBeenCalled();
  expect(wrapper.find('.voice-start').exists()).toBe(true);
});

it('cancels the request and discards a late result after switching questions', async () => {
  let complete!: (value: { text: string; duration_ms: number }) => void;
  vi.mocked(api.voiceTranscribe).mockImplementation(() => new Promise(resolve => { complete = resolve; }));
  const wrapper = mount(VoiceInput, { props: { scope: 'question-1' } });
  await record(wrapper);
  const signal = vi.mocked(api.voiceTranscribe).mock.calls[0][1]!;
  await wrapper.setProps({ scope: 'question-2' });
  expect(signal.aborted).toBe(true);
  complete({ text: '上一题的思路', duration_ms: 5000 }); await flushPromises();
  expect(wrapper.find('textarea').exists()).toBe(false);
  expect(wrapper.emitted('insert')).toBeUndefined();
});

it('retries the same audio after a connection failure and treats silence as a new recording', async () => {
  vi.mocked(api.voiceTranscribe).mockRejectedValueOnce(new Error('连接中断')).mockResolvedValueOnce({ text: '', duration_ms: 3000 });
  const wrapper = mount(VoiceInput, { props: { scope: 'question-1' } });
  await record(wrapper);
  expect(wrapper.text()).toContain('重试转写');
  await wrapper.get('.voice-primary').trigger('click'); await flushPromises();
  expect(vi.mocked(api.voiceTranscribe).mock.calls[0][0]).toEqual(vi.mocked(api.voiceTranscribe).mock.calls[1][0]);
  expect(getUserMedia).toHaveBeenCalledOnce();
  expect(wrapper.text()).toContain('没有听清');
  expect(wrapper.emitted('insert')).toBeUndefined();
});

it('stops at the recording limit and excludes a second microphone owner', async () => {
  vi.useFakeTimers();
  const first = mount(VoiceInput, { props: { scope: 'question-1' } });
  const second = mount(VoiceInput, { props: { scope: 'question-2' } });
  await first.get('.voice-start').trigger('click'); await flushPromises();
  expect(second.get('button').attributes('disabled')).toBeDefined();
  await vi.advanceTimersByTimeAsync(90_000); await flushPromises();
  expect(track.stop).toHaveBeenCalledOnce();
  expect(api.voiceTranscribe).toHaveBeenCalledOnce();
  await first.get('.voice-cancel').trigger('click');
  expect(second.get('button').attributes('disabled')).toBeUndefined();
});

it('recovers from denied permissions and releases the microphone on unmount', async () => {
  getUserMedia.mockRejectedValueOnce(new DOMException('denied', 'NotAllowedError'));
  const wrapper = mount(VoiceInput, { props: { scope: 'question-1' } });
  await wrapper.get('.voice-start').trigger('click'); await flushPromises();
  expect(wrapper.text()).toContain('没有获得麦克风权限');
  await wrapper.get('.voice-actions button').trigger('click'); await flushPromises();
  wrapper.unmount();
  expect(track.stop).toHaveBeenCalledOnce();
  expect(api.voiceTranscribe).not.toHaveBeenCalled();
  expect(voiceOwner.value).toBeNull();
});

it('explains an unconfigured recognizer before requesting microphone permission', async () => {
  vi.mocked(api.voiceStatus).mockResolvedValue({ available: false, max_seconds: 90 });
  const wrapper = mount(VoiceInput, { props: { scope: 'question-1' } });
  await wrapper.get('.voice-start').trigger('click'); await flushPromises();
  expect(wrapper.text()).toContain('语音输入暂未启用');
  expect(getUserMedia).not.toHaveBeenCalled();
  await wrapper.get('.voice-cancel').trigger('click');
  expect(wrapper.emitted('active')?.at(-1)).toEqual([false]);
});

it('offers a direct shortcut to settings when the optional voice model can be downloaded', async () => {
  vi.mocked(api.voiceStatus).mockResolvedValue({ available: false, max_seconds: 90, runtime_available: true, can_download: true });
  const wrapper = mount(VoiceInput, { props: { scope: 'question-1' } });
  await wrapper.get('.voice-start').trigger('click'); await flushPromises();
  expect(wrapper.text()).toContain('离线语音模型尚未下载');
  expect(getUserMedia).not.toHaveBeenCalled();
  await wrapper.get('.voice-actions .voice-primary').trigger('click');
  expect(wrapper.emitted('open-settings')).toHaveLength(1);
});

it('stops recording when the containing input is hidden or disabled', async () => {
  const wrapper = mount(VoiceInput, { props: { scope: 'question-1' } });
  await wrapper.get('.voice-start').trigger('click'); await flushPromises();
  await wrapper.setProps({ disabled: true });
  expect(track.stop).toHaveBeenCalledOnce();
  expect(api.voiceTranscribe).not.toHaveBeenCalled();
  expect(voiceOwner.value).toBeNull();
});

it('appends speech to the latest calculation draft without asking or submitting for the student', async () => {
  const activity = { delivery_ref: 'calculation', status: 'active', move: { move_id: 'q', actions: ['submit'], answer_input: { kind: 'worked_solution' } } } as TutorActivity;
  const ensureDisplayed = vi.fn();
  const wrapper = mount(QuestionWork, { props: { activity, busy: false, ensureDisplayed } });
  const editor = wrapper.get('textarea[data-workspace-input="tutor"]');
  await editor.setValue('先看约束 x ≥ 3。');
  await record(wrapper);
  await editor.setValue('先看约束 x ≥ 3。\n我刚补充了可行域。');
  await wrapper.get('.voice-primary').trigger('click');
  expect((editor.element as HTMLTextAreaElement).value).toBe('先看约束 x ≥ 3。\n我刚补充了可行域。\n我认为最小值是四。');
  expect(ensureDisplayed).not.toHaveBeenCalled();
  expect(wrapper.emitted('change')?.at(-1)?.[0]).toContain('我刚补充了可行域');
  wrapper.unmount();
  const restored = mount(QuestionWork, { props: { activity, busy: false, ensureDisplayed } });
  expect((restored.get('textarea').element as HTMLTextAreaElement).value).toContain('我认为最小值是四。');
});
