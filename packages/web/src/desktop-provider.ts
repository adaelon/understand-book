import { t as uiText } from './i18n';
import type { ApiProtocol } from './generated/ApiProtocol';
import type { AuthMethod } from './generated/AuthMethod';
import type { Deployment } from './generated/Deployment';
import type { ModelService } from './generated/ModelService';
import type { ModelDefinition } from './generated/ModelDefinition';
import type { ModelSettings as RuntimeModelSettings } from './generated/ModelSettings';
export type { ModelCapabilities } from './generated/ModelCapabilities';
export type { GenerationParameters } from './generated/GenerationParameters';
export type { PurposeBindings } from './generated/PurposeBindings';
export type DesktopProviderMode = "native" | "react";

export interface DesktopProviderStatus {
  configured: boolean;
  source: "settings" | "environment" | "unconfigured";
  mode: DesktopProviderMode;
  base_url: string;
  model: string;
  api_key_configured: boolean;
}

export interface DesktopProviderDraft {
  mode: DesktopProviderMode;
  baseUrl: string;
  model: string;
  apiKey: string;
}

export function desktopProviderDraft(status: DesktopProviderStatus): DesktopProviderDraft {
  return {
    mode: status.mode,
    baseUrl: status.base_url,
    model: status.model,
    apiKey: "",
  };
}

export function desktopProviderStatusLabel(status: DesktopProviderStatus | null): string {
  if (!status) return uiText('尚未检测');
  if (!status.configured) return uiText('尚未配置');
  return status.source === "settings" ? uiText('已保存并应用') : uiText('使用环境配置');
}

export type ModelApiProtocol = ApiProtocol;
export type ModelAuthentication = AuthMethod;
export type ModelDeployment = Deployment;
export type ModelDefinitionSettings = ModelDefinition;

/** The host omits credentials on read; blank write values retain the saved key. */
export interface ModelServiceSettings extends Omit<ModelService, 'api_key' | 'clear_api_key'> {
  api_key?: ModelService['api_key'];
  api_key_configured?: boolean;
  clear_api_key?: boolean;
}

/** Configuration fields come from Rust; only the credential view is UI-specific. */
export interface ModelSettings extends Omit<RuntimeModelSettings, 'services'> {
  services: ModelServiceSettings[];
}

export interface ModelSettingsStatus {
  configured: boolean;
  source: DesktopProviderStatus["source"];
  settings: ModelSettings | null;
}

export interface ModelTestResult {
  ok: boolean;
  message: string;
  model_config_id: string;
}

/** Both native and HTTP hosts supply the same settings operations to the editor. */
export interface ModelSettingsClient {
  load(): Promise<ModelSettingsStatus>;
  save(input: ModelSettings): Promise<ModelSettingsStatus>;
  test(modelConfigId: string, input?: ModelSettings): Promise<ModelTestResult>;
}

export function emptyModelSettings(): ModelSettings {
  return { revision: 0, services: [], models: [], bindings: { default_model: "" } };
}

/** Never hydrate a credential into a form, including after a successful save. */
export function modelSettingsDraft(settings: ModelSettings | null): ModelSettings {
  if (!settings) return emptyModelSettings();
  return {
    ...settings,
    services: settings.services.map(service => ({ ...service, headers: { ...service.headers }, deployment: { ...(service.deployment ?? { kind: "direct" }) }, api_key: "", clear_api_key: false })),
    models: settings.models.map(model => ({ ...model, capabilities: { ...model.capabilities }, generation: { ...model.generation } })),
    bindings: { ...settings.bindings },
  };
}

export function modelSettingsInput(settings: ModelSettings): ModelSettings {
  return {
    ...settings,
    services: settings.services.map(({ api_key_configured: _configured, ...service }) => ({ ...service, headers: { ...service.headers }, deployment: { ...(service.deployment ?? { kind: "direct" }) } })),
    models: settings.models.map(model => ({
      ...model, capabilities: { ...model.capabilities },
      context_window_tokens: typeof model.context_window_tokens === "number" ? model.context_window_tokens : null,
      output_token_limit: typeof model.output_token_limit === "number" ? model.output_token_limit : null,
      generation: {
        ...model.generation,
        temperature: typeof model.generation?.temperature === "number" ? model.generation.temperature : undefined,
      },
    })),
    bindings: { ...settings.bindings },
  };
}

export function modelIsBound(settings: ModelSettings, modelId: string): boolean {
  return Object.values(settings.bindings).some(id => id === modelId);
}
