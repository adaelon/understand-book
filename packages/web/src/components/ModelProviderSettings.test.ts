// @vitest-environment happy-dom
import { mount, flushPromises } from '@vue/test-utils';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { setLocale } from '../i18n';
import type { ModelSettings, ModelSettingsClient, ModelSettingsStatus } from '../desktop-provider';
import ModelProviderSettings from './ModelProviderSettings.vue';

function configured(): ModelSettingsStatus {
  return {
    configured: true, source: 'settings', settings: {
      revision: 3,
      services: [{ id: 'service-a', name: 'Service A', base_url: 'https://example.test/v1', auth: 'bearer', api_key_configured: true, headers: {}, deployment: { kind: 'direct' } }],
      models: [{ id: 'model-a', name: 'Reading model', service_id: 'service-a', model_id: 'upstream-model', api_protocol: 'openai_chat_completions', mode: 'native', capabilities: { native_tools: true }, generation: {} }],
      bindings: { default_model: 'model-a', agent: null, teaching: null, background: null },
    },
  };
}

function client(initial = configured()): ModelSettingsClient & { load: ReturnType<typeof vi.fn>; save: ReturnType<typeof vi.fn>; test: ReturnType<typeof vi.fn> } {
  return {
    load: vi.fn().mockResolvedValue(initial),
    save: vi.fn(async (settings: ModelSettings) => ({ configured: true, source: 'settings', settings: { ...settings, revision: settings.revision + 1 } })),
    test: vi.fn(async (model_config_id: string) => ({ ok: true, message: 'generated', model_config_id })),
  };
}

afterEach(() => setLocale('zh-CN'));

describe('model service settings', () => {
  it('loads metadata with a blank credential field and never performs an implicit test', async () => {
    const api = client();
    const wrapper = mount(ModelProviderSettings, { props: { client: api } });
    await flushPromises();
    expect((wrapper.get('[data-testid=service-key]').element as HTMLInputElement).value).toBe('');
    expect(wrapper.get('[data-testid=service-key]').attributes('placeholder')).toBe('留空保留已保存密钥');
    expect(wrapper.get('[data-testid=save-model-settings]').attributes('disabled')).toBeDefined();
    expect(api.test).not.toHaveBeenCalled();
    expect(api.save).not.toHaveBeenCalled();
  });

  it('creates a no-key local service with an explicitly declared native-tool model', async () => {
    const api = client({ configured: false, source: 'unconfigured', settings: null });
    const wrapper = mount(ModelProviderSettings, { props: { client: api } });
    await flushPromises();
    await wrapper.get('button').trigger('click');
    await wrapper.get('[data-testid=service-name]').setValue('Local model');
    await wrapper.get('[data-testid=service-url]').setValue('http://localhost:11434/v1');
    await wrapper.get('[data-testid=service-auth]').setValue('none');
    await wrapper.get('[data-testid=model-id]').setValue('local-tool-model');
    await wrapper.get('[data-testid=save-model-settings]').trigger('click');
    await flushPromises();
    expect(api.save).not.toHaveBeenCalled();
    expect(wrapper.get('[role=alert]').text()).toContain('必须明确选择支持原生工具调用');
    await wrapper.get('[data-testid=model-native-tools]').setValue('true');
    await wrapper.get('[data-testid=save-model-settings]').trigger('click');
    await flushPromises();
    expect(api.save).toHaveBeenCalledOnce();
    const input = api.save.mock.calls[0][0] as ModelSettings;
    expect(input.services[0]).toMatchObject({ auth: 'none', api_key: '', base_url: 'http://localhost:11434/v1' });
    expect(input.models[0]).toMatchObject({ name: 'local-tool-model', capabilities: { native_tools: true } });
    expect(input.bindings.default_model).toBe(input.models[0].id);
    expect(api.test).not.toHaveBeenCalled();
    expect(wrapper.text()).toContain('已保存并应用');
  });

  it('keeps failed-save input and offers an explicit draft test', async () => {
    const api = client();
    api.save.mockRejectedValue(new Error('Settings file is not writable'));
    const wrapper = mount(ModelProviderSettings, { props: { client: api } });
    await flushPromises();
    await wrapper.get('[data-testid=model-id]').setValue('replacement');
    expect(wrapper.get('[data-testid=test-model]').text()).toBe('测试草稿文本');
    await wrapper.get('[data-testid=save-model-settings]').trigger('click');
    await flushPromises();
    expect(wrapper.get('[role=alert]').text()).toContain('Settings file is not writable');
    expect((wrapper.get('[data-testid=model-id]').element as HTMLInputElement).value).toBe('replacement');
    expect(wrapper.text()).toContain('有未保存的更改');
    expect(api.test).not.toHaveBeenCalled();
  });

  it('tests a candidate without saving, replacing the default or discarding the draft', async () => {
    const initial = configured();
    const api = client(initial);
    const wrapper = mount(ModelProviderSettings, { props: { client: api } });
    await flushPromises();
    await wrapper.get('[data-testid=model-id]').setValue('candidate-model');
    await wrapper.get('[data-testid=test-model]').trigger('click');
    await flushPromises();
    expect(api.test).toHaveBeenCalledWith('model-a', expect.objectContaining({
      models: [expect.objectContaining({model_id:'candidate-model'})],
      services: [expect.objectContaining({api_key:''})],
    }));
    expect(api.save).not.toHaveBeenCalled();
    expect(initial.settings!.models[0].model_id).toBe('upstream-model');
    expect((wrapper.get('[data-testid=model-id]').element as HTMLInputElement).value).toBe('candidate-model');
    expect(wrapper.get('[role=status]').text()).toContain('草稿文本生成通过，配置尚未保存');
    expect(wrapper.text()).toContain('有未保存的更改');
  });

  it('drops Chat-only and legacy reasoning parameters when switching the draft protocol', async () => {
    const initial = configured();
    initial.settings!.models[0].generation = { output_token_parameter:'max_completion_tokens', temperature:0 };
    initial.settings!.models[0].agent_reasoning_effort = 'high';
    const api = client(initial);
    const wrapper = mount(ModelProviderSettings, { props: {client:api} });
    await flushPromises();
    await wrapper.get('[data-testid=model-protocol]').setValue('anthropic_messages');
    await wrapper.get('[data-testid=test-model]').trigger('click');
    await flushPromises();
    const input = api.test.mock.calls[0][1] as ModelSettings;
    expect(input.models[0]).toMatchObject({ api_protocol:'anthropic_messages', agent_reasoning_effort:null, generation:{temperature:0} });
    expect(input.models[0].generation).not.toHaveProperty('output_token_parameter');
    expect(api.save).not.toHaveBeenCalled();
  });

  it('tests only the selected saved model and labels text-only coverage', async () => {
    const api = client();
    const wrapper = mount(ModelProviderSettings, { props: { client: api } });
    await flushPromises();
    await wrapper.get('[data-testid=test-model]').trigger('click');
    await flushPromises();
    expect(api.test).toHaveBeenCalledWith('model-a');
    expect(api.save).not.toHaveBeenCalled();
    expect(wrapper.get('[role=status]').text()).toBe('文本生成通过。工具调用和结构化输出尚未测试。');
    expect(wrapper.emitted('busy')).toEqual([[true], [false]]);
  });

  it('shows a failed connection test and retains the model selection', async () => {
    const api = client();
    api.test.mockRejectedValue(new Error('HTTP 401: check API key'));
    const wrapper = mount(ModelProviderSettings, { props: { client: api } });
    await flushPromises();
    await wrapper.get('[data-testid=test-model]').trigger('click');
    await flushPromises();
    expect(wrapper.get('[role=alert]').text()).toContain('HTTP 401');
    expect((wrapper.get('[data-testid=model-id]').element as HTMLInputElement).value).toBe('upstream-model');
  });

  it('requires replacing purpose assignments before deleting a model', async () => {
    const initial = configured();
    initial.settings!.models.push({ ...initial.settings!.models[0], id: 'model-b', name: 'Second model' });
    const wrapper = mount(ModelProviderSettings, { props: { client: client(initial) } });
    await flushPromises();
    expect(wrapper.get('[data-testid=delete-model]').attributes('disabled')).toBeDefined();
    await wrapper.get('[data-testid=default-model]').setValue('model-b');
    expect(wrapper.get('[data-testid=delete-model]').attributes('disabled')).toBeUndefined();
    await wrapper.get('[data-testid=delete-model]').trigger('click');
    expect(wrapper.findAll('[data-testid=default-model] option')).toHaveLength(1);
    expect(wrapper.get('[data-testid=default-model] option').text()).toBe('Second model');
  });

  it('does not offer a new catalog after a failed configuration read', async () => {
    const api = client();
    api.load.mockRejectedValue(new Error('Invalid saved model settings'));
    const wrapper = mount(ModelProviderSettings, { props: { client: api } });
    await flushPromises();
    expect(wrapper.get('[role=alert]').text()).toContain('Invalid saved model settings');
    expect(wrapper.find('[data-testid=save-model-settings]').exists()).toBe(false);
    expect(wrapper.text()).not.toContain('连接你的第一个模型服务');
  });

  it('renders an English settings screen with the same explicit test contract', async () => {
    setLocale('en');
    const wrapper = mount(ModelProviderSettings, { props: { client: client() } });
    await flushPromises();
    expect(wrapper.text()).toContain('Default model');
    expect(wrapper.get('[data-testid=test-model]').text()).toBe('Test text generation');
    expect(wrapper.text()).not.toMatch(/[\u3400-\u9fff]/);
  });
});
