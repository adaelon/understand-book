import { describe, expect, it } from "vitest";
import { desktopProviderDraft, desktopProviderStatusLabel, modelIsBound, modelSettingsDraft, modelSettingsInput, type ModelSettings } from "./desktop-provider";

describe("desktop provider settings", () => {
  it("hydrates editable metadata without ever hydrating the API key", () => {
    expect(
      desktopProviderDraft({
        configured: true,
        source: "settings",
        mode: "react",
        base_url: "https://provider.example/v1",
        model: "model-a",
        api_key_configured: true,
      }),
    ).toEqual({
      mode: "react",
      baseUrl: "https://provider.example/v1",
      model: "model-a",
      apiKey: "",
    });
  });

  it("distinguishes saved, environment, and missing configuration", () => {
    expect(desktopProviderStatusLabel(null)).toBe("尚未检测");
    expect(
      desktopProviderStatusLabel({
        configured: false,
        source: "unconfigured",
        mode: "native",
        base_url: "",
        model: "",
        api_key_configured: false,
      }),
    ).toBe("尚未配置");
    expect(
      desktopProviderStatusLabel({
        configured: true,
        source: "environment",
        mode: "native",
        base_url: "https://provider.example/v1",
        model: "model-a",
        api_key_configured: true,
      }),
    ).toBe("使用环境配置");
  });
});

describe('model catalog drafts', () => {
  const saved: ModelSettings = {
    revision: 4,
    services: [{ id: 's', name: 'Service', base_url: 'https://example.test/v1', auth: 'bearer', api_key: 'never-hydrate', api_key_configured: true, headers: { 'X-Project': 'reader' }, deployment: { kind: 'direct' } }],
    models: [{ id: 'm', name: 'Model', service_id: 's', model_id: 'model-a', api_protocol: 'openai_chat_completions', mode: 'native', capabilities: { native_tools: true }, generation: {} }],
    bindings: { default_model: 'm', teaching: 'feedback' },
  };

  it('clears secrets and copies editable maps instead of mutating loaded settings', () => {
    const draft = modelSettingsDraft(saved);
    expect(draft.services[0].api_key).toBe('');
    expect(draft.services[0].api_key_configured).toBe(true);
    draft.services[0].headers['X-Project'] = 'changed';
    draft.models[0].capabilities.native_tools = false;
    expect(saved.services[0].headers['X-Project']).toBe('reader');
    expect(saved.models[0].capabilities.native_tools).toBe(true);
  });

  it('submits blank-key preservation and explicit deletion without read-only metadata', () => {
    const draft = modelSettingsDraft(saved);
    draft.services[0].clear_api_key = true;
    const input = modelSettingsInput(draft);
    expect(input.services[0]).toMatchObject({ api_key: '', clear_api_key: true });
    expect(input.services[0]).not.toHaveProperty('api_key_configured');
    expect(input.models[0].context_window_tokens).toBeNull();
    expect(input.models[0].output_token_limit).toBeNull();
  });

  it('protects models selected by either the default or a purpose override', () => {
    expect(modelIsBound(saved, 'm')).toBe(true);
    expect(modelIsBound(saved, 'feedback')).toBe(true);
    expect(modelIsBound(saved, 'unused')).toBe(false);
  });

  it('preserves an explicit zero temperature and the selected Chat output parameter', () => {
    const draft = modelSettingsDraft(saved);
    draft.models[0].generation = { temperature: 0, output_token_parameter: 'max_completion_tokens' };
    expect(modelSettingsInput(draft).models[0].generation).toEqual({ temperature: 0, output_token_parameter: 'max_completion_tokens' });
    expect(saved.models[0].generation).toEqual({});
  });
});
