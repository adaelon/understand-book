import fs from 'node:fs';
import path from 'node:path';

// Selection only. The Runtime remains the authority for protocol, auth and capability validation.
export function selectTrialProvider(env, settingsPath, modelConfigId) {
  const filename = settingsPath || env.UNDERSTAND_BOOK_MODEL_SETTINGS;
  const selectedId = modelConfigId || env.UNDERSTAND_BOOK_MODEL_CONFIG_ID;
  if (selectedId && !filename) throw new Error('--model-config-id requires --model-settings.');
  if (filename) {
    const absolute = path.resolve(filename);
    const settings = JSON.parse(fs.readFileSync(absolute, 'utf8'));
    const id = selectedId || settings.bindings?.agent || settings.bindings?.default_model;
    const model = settings.models?.find(model => model.id === id);
    if (!model) throw new Error(`Model configuration not found: ${id ?? '(default)'}`);
    const service = settings.services?.find(service => service.id === model.service_id);
    if (!service) throw new Error(`Service configuration not found: ${model.service_id}`);
    const origin = new URL(service.base_url).origin;
    return {
      direct: true, base: service.base_url, model: model.model_id, configuredModel: model.model_id,
      metadata: { model_config_id: id, service_id: service.id, revision: settings.revision,
        api_protocol: model.api_protocol, provider_origin: origin },
      environment: { UNDERSTAND_BOOK_MODEL_SETTINGS: absolute, UNDERSTAND_BOOK_MODEL_CONFIG_ID: id },
    };
  }
  const base = env.OPENCODE_BASE_URL;
  const configuredModel = env.FLUID_LLM_MODEL;
  if (!base || !env.OPENCODE_API_KEY || !configuredModel) {
    throw new Error('Supply --model-settings and optionally --model-config-id, or the existing OPENCODE_BASE_URL, OPENCODE_API_KEY and FLUID_LLM_MODEL configuration.');
  }
  const model = new URL(base).origin === 'https://api.deepseek.com'
    && ['deepseek-v4-flash', 'deepseek-v4-flash-vision-exp'].includes(configuredModel)
    ? 'deepseek-flash' : configuredModel;
  return { direct: false, base, model, configuredModel,
    metadata: { api_protocol: 'legacy_auto', provider_origin: new URL(base).origin }, environment: {} };
}

export function positiveLimit(value, fallback, name) {
  if (value === undefined) return fallback;
  const parsed = Number(value);
  if (!Number.isSafeInteger(parsed) || parsed <= 0) throw new Error(`${name} must be a positive integer.`);
  return parsed;
}

export function trialCostBudget(maxCostCny, ratesFile) {
  if (maxCostCny === undefined && ratesFile === undefined) return null;
  if (maxCostCny === undefined || ratesFile === undefined) throw new Error('--max-cost-cny and --rates-file must be supplied together.');
  const amount = String(maxCostCny);
  if (!/^\d+(?:\.\d{1,6})?$/.test(amount)) throw new Error('--max-cost-cny must be a positive CNY amount with at most six decimal places.');
  const [whole, fraction = ''] = amount.split('.');
  const micro = BigInt(whole) * 1000000n + BigInt(fraction.padEnd(6, '0'));
  if (micro <= 0n || micro > BigInt(Number.MAX_SAFE_INTEGER)) throw new Error('--max-cost-cny is outside the supported positive amount range.');
  const filename = path.resolve(ratesFile);
  fs.accessSync(filename, fs.constants.R_OK);
  return {
    max_cost_micro_cny: Number(micro), rates_file: filename,
    environment: { MODEL_TRIAL_MAX_COST_MICRO_CNY: String(micro), MODEL_TRIAL_RATES_FILE: filename },
  };
}
