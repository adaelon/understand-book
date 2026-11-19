<script setup lang="ts">
import { computed, onMounted, ref, useId, watch } from 'vue';
import { Check, ChevronRight, Cloud, Cpu, KeyRound, List, Plus, Save, Settings2, Trash2 } from '@lucide/vue';
import { hasModelDirectory, providerForAddress, providerPresets, type ProviderPreset } from '../model-provider-presets';
import { t as uiText } from '../i18n';
import {
  emptyModelSettings, modelIsBound, modelSettingsDraft, modelSettingsInput,
  type ModelApiProtocol, type ModelDefinitionSettings, type ModelDeployment, type ModelSettingsClient, type ModelSettingsStatus,
} from '../desktop-provider';

const props = defineProps<{ client: ModelSettingsClient }>();
const nativeToolsHintId = useId();
const modelInputId = useId();
function providerHost(address: string) { return new URL(address).hostname; }
const emit = defineEmits<{ busy: [value: boolean]; saved: [value: ModelSettingsStatus] }>();
const settings = ref(emptyModelSettings());
const status = ref<ModelSettingsStatus | null>(null);
const loading = ref(true);
const saving = ref(false);
const testing = ref(false);
const error = ref('');
const testResult = ref('');
const testFailed = ref(false);
const selectedServiceId = ref('');
const selectedModelId = ref('');
const savedInput = ref('');
const headersText = ref('');
const choosingProvider = ref(false);
const catalog = ref<string[] | null>(null);
const catalogLoading = ref(false);
const catalogMessage = ref('');
let catalogRequest = 0;
const selectedService = computed(() => settings.value.services.find(service => service.id === selectedServiceId.value));
const serviceModels = computed(() => settings.value.models.filter(model => model.service_id === selectedServiceId.value));
const selectedModel = computed(() => serviceModels.value.find(model => model.id === selectedModelId.value));
const defaultModel = computed(() => settings.value.models.find(model => model.id === settings.value.bindings.default_model));
const preferredServiceModel = computed(() => defaultModel.value?.service_id === selectedServiceId.value
  ? defaultModel.value.id : serviceModels.value[0]?.id ?? '');
const newService = computed(() => !!selectedService.value && !status.value?.settings?.services.some(service => service.id === selectedServiceId.value));
const catalogProtocol = computed(() => selectedModel.value?.api_protocol === 'legacy_auto'
  ? providerForAddress(selectedService.value?.base_url ?? '')?.protocol ?? 'openai_chat_completions'
  : selectedModel.value?.api_protocol);
const canListModels = computed(() => props.client.listModels && catalogProtocol.value && selectedService.value?.deployment?.kind === 'direct'
  && ['none','bearer','api_key'].includes(selectedService.value.auth) && hasModelDirectory(catalogProtocol.value));
const busy = computed(() => saving.value || testing.value);
const changed = computed(() => JSON.stringify(modelSettingsInput(settings.value)) !== savedInput.value);
const sourceLabel = computed(() => {
  if (loading.value) return uiText('正在读取...');
  if (changed.value) return uiText('有未保存的更改');
  if (!status.value?.configured) return uiText('尚未配置');
  return status.value.source === 'environment' ? uiText('使用环境配置') : uiText('已保存并应用');
});
const protocols: { id: ModelApiProtocol; label: string }[] = [
  { id: 'openai_chat_completions', label: 'OpenAI Chat Completions' },
  { id: 'openai_responses', label: 'OpenAI Responses' },
  { id: 'anthropic_messages', label: 'Anthropic Messages' },
  { id: 'gemini_generate_content', label: 'Gemini GenerateContent' },
  { id: 'gemini_interactions', label: 'Gemini Interactions' },
  { id: 'ollama_chat', label: 'Ollama Chat' },
  { id: 'cohere_v2_chat', label: 'Cohere V2 Chat' },
  { id: 'bedrock_converse', label: 'AWS Bedrock Converse' },
  { id: 'deepseek_messages', label: 'DeepSeek Messages' },
  { id: 'legacy_auto', label: uiText('沿用原有路由') },
];
watch(busy, value => emit('busy', value));
watch(selectedServiceId, () => {
  selectedModelId.value = preferredServiceModel.value;
  headersText.value = Object.entries(selectedService.value?.headers ?? {}).map(([name, value]) => `${name}: ${value}`).join('\n');
});
watch(settings, () => { testResult.value = ''; }, { deep: true });
watch(() => [selectedServiceId.value, selectedService.value?.base_url, selectedService.value?.api_key, selectedService.value?.clear_api_key,
  selectedService.value?.auth, selectedService.value?.deployment?.kind, headersText.value, selectedModel.value?.api_protocol], () => {
  catalogRequest++; catalog.value = null; catalogMessage.value = ''; catalogLoading.value = false;
}, { flush: 'sync' });

function accept(result: ModelSettingsStatus) {
  status.value = result;
  settings.value = modelSettingsDraft(result.settings);
  if (!settings.value.services.some(service => service.id === selectedServiceId.value)) {
    selectedServiceId.value = defaultModel.value?.service_id ?? settings.value.services[0]?.id ?? '';
  }
  if (!serviceModels.value.some(model => model.id === selectedModelId.value)) selectedModelId.value = preferredServiceModel.value;
  headersText.value = Object.entries(selectedService.value?.headers ?? {}).map(([name, value]) => `${name}: ${value}`).join('\n');
  savedInput.value = JSON.stringify(modelSettingsInput(settings.value));
}

async function load() {
  loading.value = true;
  error.value = '';
  try { accept(await props.client.load()); }
  catch (cause) { error.value = String(cause instanceof Error ? cause.message : cause); }
  finally { loading.value = false; }
}
onMounted(load);

function id(prefix: string) { return `${prefix}-${crypto.randomUUID()}`; }
function addService(preset: ProviderPreset) {
  const serviceId = id('service');
  settings.value.services.push({ id: serviceId, name: preset.id === 'custom' ? uiText('自定义服务') : preset.name, base_url: preset.address, auth: preset.auth, api_key: '', headers: {}, deployment: { kind: 'direct' } });
  selectedServiceId.value = serviceId;
  choosingProvider.value = false;
  error.value = '';
  addModel(serviceId, preset.protocol);
}
function addModel(serviceId = selectedServiceId.value, protocol = selectedModel.value?.api_protocol ?? providerForAddress(selectedService.value?.base_url ?? '')?.protocol ?? 'openai_chat_completions') {
  if (!serviceId) return;
  const model: ModelDefinitionSettings = {
    id: id('model'), name: '', service_id: serviceId, model_id: '',
    api_protocol: protocol, mode: 'native', capabilities: { native_tools: null }, generation: {},
  };
  settings.value.models.push(model);
  selectedModelId.value = model.id;
  if (!settings.value.bindings.default_model) settings.value.bindings.default_model = model.id;
}
function cancelNewService() {
  const serviceId = selectedServiceId.value;
  if (!newService.value) return;
  const removed = new Set(serviceModels.value.map(model => model.id));
  settings.value.models = settings.value.models.filter(model => model.service_id !== serviceId);
  settings.value.services = settings.value.services.filter(service => service.id !== serviceId);
  const bindings = settings.value.bindings;
  if (removed.has(bindings.default_model)) bindings.default_model = settings.value.models[0]?.id ?? '';
  for (const purpose of ['agent','teaching','background'] as const) if (removed.has(bindings[purpose] ?? '')) bindings[purpose] = null;
  selectedServiceId.value = defaultModel.value?.service_id ?? settings.value.services[0]?.id ?? '';
  error.value = '';
}
async function fetchModels() {
  const service = selectedService.value, model = selectedModel.value;
  if (!service || !model || !catalogProtocol.value || !props.client.listModels) return;
  updateHeaders();
  if (error.value) return;
  const request = ++catalogRequest;
  catalogLoading.value = true; catalogMessage.value = ''; catalog.value = null;
  try {
    const result = await props.client.listModels(service, catalogProtocol.value);
    if (request !== catalogRequest) return;
    catalog.value = result.models;
    if (!result.models.length) catalogMessage.value = uiText('服务没有返回型号，可手动填写模型 ID。');
    else if (result.has_more) catalogMessage.value = uiText('已列出服务返回的第一页，其他型号可手动填写。');
  } catch (cause) {
    if (request === catalogRequest) catalogMessage.value = uiText('未能读取型号，可手动填写模型 ID。') + ' ' + (cause instanceof Error ? cause.message : String(cause));
  } finally { if (request === catalogRequest) catalogLoading.value = false; }
}
function setDeployment(event: Event) {
  if (!selectedService.value) return;
  const kind = (event.target as HTMLSelectElement).value as ModelDeployment['kind'];
  if (kind === 'azure_deployment') selectedService.value.deployment = { kind, api_version: '' };
  else if (kind === 'vertex') selectedService.value.deployment = { kind, project: '', location: 'global' };
  else if (kind === 'bedrock') selectedService.value.deployment = { kind, region: '' };
  else selectedService.value.deployment = { kind };
}
function requiresKey(auth: string) { return auth === 'bearer' || auth === 'api_key'; }
function changedProtocol() {
  const model = selectedModel.value;
  if (model && !['legacy_auto', 'deepseek_messages'].includes(model.api_protocol) && model.capabilities.reasoning_effort !== true) model.agent_reasoning_effort = null;
  if (model?.generation && !['legacy_auto', 'openai_chat_completions'].includes(model.api_protocol)) delete model.generation.output_token_parameter;
}
function removeModel(model: ModelDefinitionSettings) {
  if (modelIsBound(settings.value, model.id)) { error.value = uiText('此模型仍被用途使用，请先选择替代模型。'); return; }
  settings.value.models = settings.value.models.filter(candidate => candidate.id !== model.id);
  selectedModelId.value = serviceModels.value[0]?.id ?? '';
  error.value = '';
}
function removeService() {
  if (serviceModels.value.length) { error.value = uiText('请先移除或转移此服务下的模型。'); return; }
  settings.value.services = settings.value.services.filter(service => service.id !== selectedServiceId.value);
  selectedServiceId.value = settings.value.services[0]?.id ?? '';
  error.value = '';
}
function updateHeaders() {
  if (!selectedService.value) return;
  const headers: Record<string, string> = {};
  for (const line of headersText.value.split('\n')) {
    if (!line.trim()) continue;
    const colon = line.indexOf(':');
    if (colon <= 0) { error.value = uiText('额外请求头请按每行“名称: 值”填写。'); return; }
    headers[line.slice(0, colon).trim()] = line.slice(colon + 1).trim();
  }
  selectedService.value.headers = headers;
  error.value = '';
}
function modelLabel(model: ModelDefinitionSettings) { return model.name.trim() || model.model_id.trim() || uiText('未命名模型'); }
function validate(): string {
  if (!settings.value.models.length) return uiText('请添加一个服务和模型。');
  for (const service of settings.value.services) {
    if (!service.name.trim() || !service.base_url.trim()) return uiText('请填写服务名称和服务地址。');
    try { const url = new URL(service.base_url); if (!['http:', 'https:'].includes(url.protocol)) throw new Error(); }
    catch { return uiText('服务地址必须是有效的 HTTP 或 HTTPS 地址。'); }
    if (requiresKey(service.auth) && !service.api_key?.trim() && (!service.api_key_configured || service.clear_api_key)) return uiText('请填写 API Key，或选择适用的认证方式。');
  }
  for (const model of settings.value.models) {
    if (!model.model_id.trim()) return uiText('请填写每个模型的模型 ID。');
  }
  const agent = settings.value.models.find(model => model.id === (settings.value.bindings.agent || settings.value.bindings.default_model));
  if (agent && agent.mode === 'native' && agent.api_protocol !== 'legacy_auto' && agent.capabilities.native_tools !== true) return uiText('用于主 Agent 的模型必须明确选择支持原生工具调用。');
  return '';
}
async function save() {
  updateHeaders();
  if (error.value) return;
  error.value = validate();
  if (error.value) return;
  saving.value = true;
  try {
    const input = modelSettingsInput(settings.value);
    input.models.forEach(model => { if (!model.name.trim()) model.name = model.model_id.trim(); });
    const result = await props.client.save(input);
    accept(result);
    emit('saved', result);
  } catch (cause) { error.value = String(cause instanceof Error ? cause.message : cause); }
  finally { saving.value = false; }
}
async function test() {
  if (!selectedModel.value) return;
  testResult.value = '';
  testFailed.value = false;
  updateHeaders();
  if (error.value) return;
  const draft = changed.value;
  let input;
  if (draft) {
    error.value = validate();
    if (error.value) return;
    input = modelSettingsInput(settings.value);
    input.models.forEach(model => { if (!model.name.trim()) model.name = model.model_id.trim(); });
  }
  testing.value = true;
  try {
    const result = draft ? await props.client.test(selectedModel.value.id, input) : await props.client.test(selectedModel.value.id);
    testFailed.value = !result.ok;
    testResult.value = result.ok ? (draft ? uiText('草稿文本生成通过，配置尚未保存。工具调用和结构化输出尚未测试。') : uiText('文本生成通过。工具调用和结构化输出尚未测试。')) : result.message;
  } catch (cause) { testFailed.value = true; testResult.value = String(cause instanceof Error ? cause.message : cause); }
  finally { testing.value = false; }
}
</script>

<template>
  <section class="model-settings" data-testid="model-settings" :aria-busy="loading || busy">
    <header class="model-settings-heading">
      <div><h4>{{ uiText('AI 模型') }}</h4><p>{{ uiText('选择阅读、教学和后台任务使用的模型。') }}</p></div>
      <span class="model-settings-status" :class="{ dirty: changed }">{{ sourceLabel }}</span>
    </header>
    <p v-if="loading" role="status">{{ uiText('正在读取...') }}</p>
    <template v-else-if="status">
      <div v-if="!settings.services.length || choosingProvider" class="provider-picker">
        <div class="model-section-heading"><h5>{{ uiText('选择服务商') }}</h5><button v-if="settings.services.length" type="button" @click="choosingProvider = false">{{ uiText('取消') }}</button></div>
        <div class="provider-grid">
          <button v-for="provider in providerPresets" :key="provider.id" type="button" class="provider-choice" :data-testid="'new-provider-' + provider.id" @click="addService(provider)">
            <span class="provider-mark" :data-provider="provider.id" aria-hidden="true"><Cpu v-if="provider.id === 'ollama'" :size="20" /><Settings2 v-else-if="provider.id === 'custom'" :size="20" /><Cloud v-else :size="20" /></span>
            <span class="provider-label"><strong>{{ provider.id === 'custom' ? uiText('自定义服务') : provider.name }}</strong><small>{{ provider.id === 'custom' ? uiText('填写自己的 API 地址') : provider.id === 'ollama' ? uiText('本机运行') : providerHost(provider.address) }}</small></span>
            <ChevronRight :size="15" aria-hidden="true" />
          </button>
        </div>
      </div>
      <template v-else>
        <label v-if="settings.models.some(model => model.model_id.trim())" class="model-default"><span>{{ uiText('默认模型') }}</span>
          <select v-model="settings.bindings.default_model" data-testid="default-model" :disabled="busy">
            <optgroup v-for="service in settings.services" :key="service.id" :label="service.name">
              <option v-for="model in settings.models.filter(item => item.service_id === service.id)" :key="model.id" :value="model.id">{{ modelLabel(model) }}</option>
            </optgroup>
          </select>
        </label>
        <div class="model-layout">
          <aside class="connections">
            <div class="model-section-heading"><h5>{{ uiText('模型服务') }}</h5></div>
            <div class="model-service-list" role="group" :aria-label="uiText('模型服务')">
              <button v-for="service in settings.services" :key="service.id" type="button" :aria-pressed="service.id === selectedServiceId" :disabled="busy" @click="selectedServiceId = service.id">
                <span><strong>{{ service.name }}</strong><small>{{ settings.models.filter(item => item.service_id === service.id).length }} {{ uiText('个模型') }}</small></span>
                <ChevronRight :size="14" aria-hidden="true" />
              </button>
            </div>
            <button type="button" class="add-provider" :disabled="busy" @click="choosingProvider = true"><Plus :size="15" />{{ uiText('添加服务') }}</button>
          </aside>
          <fieldset v-if="selectedService" class="model-editor" :disabled="busy">
            <legend>{{ selectedService.name }}</legend>
            <div class="connection-title"><h5><KeyRound :size="15" />{{ uiText('服务连接') }}</h5><button v-if="newService" type="button" @click="cancelNewService">{{ uiText('取消添加') }}</button></div>
            <div class="model-fields connection-fields">
              <label v-if="requiresKey(selectedService.auth)"><span>API Key</span><input v-model="selectedService.api_key" type="password" autocomplete="new-password" :placeholder="selectedService.api_key_configured ? uiText('留空保留已保存密钥') : uiText('输入 API Key')" data-testid="service-key" /></label>
              <label class="model-field-wide"><span>{{ uiText('服务地址') }}</span><input v-model="selectedService.base_url" type="url" placeholder="https://provider.example/v1" data-testid="service-url" /></label>
            </div>
          <details class="model-advanced"><summary>{{ uiText('高级连接选项') }}</summary>
            <div class="model-fields">
              <label><span>{{ uiText('服务名称') }}</span><input v-model="selectedService.name" data-testid="service-name" /></label>
              <label><span>{{ uiText('认证方式') }}</span><select v-model="selectedService.auth" data-testid="service-auth"><option value="bearer">Bearer API Key</option><option value="api_key">{{ uiText('API Key 请求头') }}</option><option value="none">{{ uiText('无需认证') }}</option><option value="google_adc">Google ADC</option><option value="azure_developer_tools">Azure Developer Tools</option><option value="azure_managed_identity">Azure Managed Identity</option><option value="azure_workload_identity">Azure Workload Identity</option></select></label>
            </div>
            <label><span>{{ uiText('部署方式') }}</span><select :value="selectedService.deployment?.kind ?? 'direct'" @change="setDeployment"><option value="direct">{{ uiText('直接连接') }}</option><option value="azure_v1">Azure / Foundry v1</option><option value="azure_deployment">Azure Deployment</option><option value="vertex">Google Vertex</option><option value="bedrock">AWS Bedrock</option></select></label>
            <div v-if="selectedService.deployment?.kind === 'azure_deployment'" class="model-fields model-cloud-fields"><label class="model-field-wide"><span>API Version</span><input v-model="selectedService.deployment.api_version" placeholder="api-version" /></label></div>
            <div v-if="selectedService.deployment?.kind === 'vertex'" class="model-fields model-cloud-fields"><label><span>Google Cloud Project</span><input v-model="selectedService.deployment.project" /></label><label><span>{{ uiText('区域') }}</span><input v-model="selectedService.deployment.location" placeholder="global" /></label><label class="model-field-wide"><span>{{ uiText('配额项目（可选）') }}</span><input v-model="selectedService.deployment.quota_project" /></label></div>
            <div v-if="selectedService.deployment?.kind === 'bedrock'" class="model-cloud-fields"><label><span>{{ uiText('AWS 区域') }}</span><input v-model="selectedService.deployment.region" placeholder="us-east-1" /></label></div>
            <label class="model-field-wide"><span>{{ uiText('额外请求头') }}</span><textarea v-model="headersText" rows="2" placeholder="X-Project: my-project" @change="updateHeaders" /><small>{{ uiText('每行填写“名称: 值”，认证信息请使用上方密钥字段。') }}</small></label>
            <label v-if="selectedService.api_key_configured" class="model-checkbox"><input v-model="selectedService.clear_api_key" type="checkbox" />{{ uiText('删除此服务已保存的密钥') }}</label>
            <button type="button" class="model-danger" :disabled="serviceModels.length > 0" :title="serviceModels.length ? uiText('请先移除或转移此服务下的模型。') : ''" @click="removeService"><Trash2 :size="14" />{{ uiText('删除服务') }}</button>
          </details>
          <div class="model-section-heading"><h5>{{ uiText('此服务的模型') }}</h5><button type="button" :disabled="serviceModels.some(model => !model.model_id.trim())" @click="addModel()"><Plus :size="14" />{{ uiText('添加模型') }}</button></div>
          <div v-if="serviceModels.length > 1 || selectedModel?.model_id" class="model-model-list" role="group" :aria-label="uiText('此服务的模型')"><button v-for="model in serviceModels" :key="model.id" type="button" :aria-pressed="model.id === selectedModelId" @click="selectedModelId = model.id"><Check v-if="model.id === settings.bindings.default_model" :size="14" /><span>{{ modelLabel(model) }}</span></button></div>
          <div v-if="selectedModel" class="model-detail">
            <div class="model-fields">
              <div class="model-field-wide catalog-field">
                <div class="field-heading"><label :for="modelInputId">{{ uiText('模型 ID') }}</label><button v-if="canListModels" type="button" data-testid="list-models" :disabled="busy || catalogLoading || !selectedService.base_url.trim()" @click="fetchModels"><List :size="14" />{{ catalogLoading ? uiText('正在读取…') : uiText('从服务获取型号') }}</button></div>
                <select v-if="catalog?.length" :aria-label="uiText('可用型号')" data-testid="available-models" :value="catalog.includes(selectedModel.model_id) ? selectedModel.model_id : ''" @change="selectedModel.model_id = ($event.target as HTMLSelectElement).value"><option value="" disabled>{{ uiText('选择服务返回的型号') }}</option><option v-for="modelId in catalog" :key="modelId" :value="modelId">{{ modelId }}</option></select>
                <input :id="modelInputId" v-model="selectedModel.model_id" :placeholder="uiText('填写或选择模型 ID')" data-testid="model-id" />
                <small v-if="catalogMessage" role="status">{{ catalogMessage }}</small>
              </div>
              <label class="model-field-wide"><span>{{ uiText('原生工具能力') }}</span><select v-model="selectedModel.capabilities.native_tools" :aria-label="uiText('原生工具能力')" :aria-describedby="nativeToolsHintId" data-testid="model-native-tools"><option :value="undefined" hidden>{{ uiText('未声明') }}</option><option :value="null">{{ uiText('未声明') }}</option><option :value="true">{{ uiText('支持') }}</option><option :value="false">{{ uiText('不支持') }}</option></select><small :id="nativeToolsHintId">{{ uiText('阅读问答需要工具调用。请根据模型说明手动选择；原有路由保留原配置。') }}</small></label>
            </div>
            <details class="model-advanced"><summary>{{ uiText('模型能力与预算') }}</summary>
              <div class="model-fields">
                <label><span>{{ uiText('显示名称') }}</span><input v-model="selectedModel.name" :placeholder="selectedModel.model_id || uiText('可选，便于区分模型')" data-testid="model-name" /></label>
                <label class="model-field-wide"><span>{{ uiText('API 协议') }}</span><select v-model="selectedModel.api_protocol" data-testid="model-protocol" @change="changedProtocol"><option v-for="protocol in protocols" :key="protocol.id" :value="protocol.id">{{ protocol.label }}</option></select></label>
              </div>
              <p>{{ uiText('以下能力由你手动配置，请与所选模型的服务说明核对。') }}</p>
              <div class="model-capabilities"><label v-for="capability in ([{key: 'parallel_tools', label: uiText('并行工具调用')}, {key: 'json_object', label: 'JSON Object'}, {key: 'json_schema', label: 'JSON Schema'}, {key: 'images', label: uiText('图像输入')}, {key: 'continuation', label: uiText('私有推理续接')}, {key: 'streaming', label: uiText('流式文本')}, {key: 'streaming_tools', label: uiText('流式工具调用')}, {key: 'reasoning_effort', label: uiText('推理强度参数')}] as const)" :key="capability.key"><span>{{ capability.label }}</span><select v-model="selectedModel.capabilities[capability.key]" @change="changedProtocol"><option :value="undefined" hidden>{{ uiText('未声明') }}</option><option :value="null">{{ uiText('未声明') }}</option><option :value="true">{{ uiText('支持') }}</option><option :value="false">{{ uiText('不支持') }}</option></select></label></div>
              <div class="model-fields">
                <label class="model-field-wide"><span>{{ uiText('AI 工具调用方式') }}</span><select v-model="selectedModel.mode"><option value="native">{{ uiText('原生工具调用') }}</option><option value="react">{{ uiText('文本工具调用（ReAct）') }}</option></select></label>
                <label><span>{{ uiText('上下文窗口（token）') }}</span><input v-model.number="selectedModel.context_window_tokens" type="number" min="1" step="1" :placeholder="uiText('沿用模型配置')" /></label>
                <label><span>{{ uiText('输出上限（token）') }}</span><input v-model.number="selectedModel.output_token_limit" type="number" min="1" step="1" :placeholder="uiText('沿用模型配置')" /></label>
                <label v-if="selectedModel.generation"><span>{{ uiText('温度（可选）') }}</span><input v-model.number="selectedModel.generation.temperature" type="number" min="0" max="2" step="0.1" :placeholder="uiText('沿用服务默认值')" /></label>
                <label v-if="selectedModel.generation && ['legacy_auto', 'openai_chat_completions'].includes(selectedModel.api_protocol)"><span>{{ uiText('输出预算参数') }}</span><select v-model="selectedModel.generation.output_token_parameter"><option :value="undefined">{{ uiText('自动') }}</option><option value="max_tokens">max_tokens</option><option value="max_completion_tokens">max_completion_tokens</option></select></label>
                <label v-if="selectedModel.api_protocol === 'deepseek_messages' || selectedModel.api_protocol === 'legacy_auto' || selectedModel.capabilities.reasoning_effort === true"><span>{{ uiText('推理强度') }}</span><select v-model="selectedModel.agent_reasoning_effort"><option :value="undefined" hidden>{{ uiText('沿用模型配置') }}</option><option :value="null">{{ uiText('沿用模型配置') }}</option><option value="low">Low</option><option value="high">High</option><option value="max">Max</option></select></label>
              </div>
              <button type="button" class="model-danger" :disabled="modelIsBound(settings, selectedModel.id)" :title="modelIsBound(settings, selectedModel.id) ? uiText('此模型仍被用途使用，请先选择替代模型。') : ''" data-testid="delete-model" @click="removeModel(selectedModel)"><Trash2 :size="14" />{{ uiText('删除模型') }}</button>
              <p v-if="modelIsBound(settings, selectedModel.id)" class="model-hint">{{ uiText('此模型仍被用途使用，请先选择替代模型。') }}</p>
            </details>
            <div class="model-test-row"><button type="button" :disabled="testing || saving" data-testid="test-model" @click="test">{{ testing ? uiText('正在测试文本生成…') : changed ? uiText('测试草稿文本') : uiText('测试文本生成') }}</button><small>{{ changed ? uiText('仅测试当前草稿，不会保存或更改默认模型。') : uiText('测试会向所选服务发送一条简短请求。') }}</small></div>
          </div>
          </fieldset>
        </div>
        <details class="model-advanced model-purpose"><summary>{{ uiText('按用途选择模型') }}</summary>
          <p>{{ uiText('未指定的用途继承默认模型。') }}</p>
          <label v-for="purpose in ([{key: 'agent', label: uiText('主 Agent')}, {key: 'teaching', label: uiText('教学与练习反馈')}, {key: 'background', label: uiText('后台任务')}] as const)" :key="purpose.key"><span>{{ purpose.label }}</span><select v-model="settings.bindings[purpose.key]" :disabled="busy"><option :value="null">{{ uiText('使用默认模型') }}</option><option :value="undefined" hidden>{{ uiText('使用默认模型') }}</option><option v-for="model in settings.models" :key="model.id" :value="model.id">{{ modelLabel(model) }}</option></select></label>
        </details>
        <div class="model-save-row"><p>{{ uiText('API Key 明文保存在当前用户设置中。') }}</p><button type="button" class="model-primary" :disabled="busy || !changed" data-testid="save-model-settings" @click="save"><Save :size="16" />{{ saving ? uiText('应用中') : uiText('保存并应用') }}</button></div>
      </template>
    </template>
    <p v-if="testResult" :role="testFailed ? 'alert' : 'status'" class="model-feedback" :class="{ failure: testFailed }">{{ testResult }}</p>
    <div v-if="error" role="alert" class="model-feedback failure"><p>{{ error }}</p><button v-if="!status && !loading" type="button" @click="load">{{ uiText('重新读取设置') }}</button></div>
  </section>
</template>

<style scoped>
.model-settings { container-type: inline-size; color: var(--ink, #202928); font-size: .85rem; }
.model-settings-heading, .model-section-heading, .model-save-row, .model-test-row { display: flex; align-items: center; justify-content: space-between; gap: .75rem; }
.model-settings-heading { align-items: flex-start; margin-bottom: 1rem; }
h4, h5 { margin: 0; font-size: .9rem; font-weight: 650; }
h5 { font-size: .8rem; }
.model-settings-heading p, .model-hint, .model-advanced p, .model-save-row p { margin: .3rem 0 0; color: var(--steel, #64736f); font-size: .75rem; line-height: 1.5; }
.model-settings-status { max-width: 45%; flex: 0 0 auto; border-radius: 4px; padding: .25rem .4rem; background: var(--canvas, #f2f5f1); color: var(--steel, #64736f); font-size: .7rem; line-height: 1.4; }
.model-settings-status.dirty { background: #fff4df; color: #80591d; }
.model-settings button { display: inline-flex; align-items: center; justify-content: center; gap: .35rem; min-height: 34px; padding: .4rem .65rem; border: 1px solid var(--line, #d9e0dc); border-radius: 5px; background: var(--model-control-bg, white); color: inherit; font: inherit; cursor: pointer; }
.model-settings button:disabled { opacity: .5; cursor: default; }
.model-settings button.model-primary { background: var(--ink, #263b34); color: var(--model-on-primary, white); border-color: var(--ink, #263b34); }
.model-settings button.model-danger { color: #9a3d30; margin-top: .65rem; }
.model-settings button:focus-visible, .model-settings input:focus-visible, .model-settings select:focus-visible, .model-settings textarea:focus-visible, summary:focus-visible { outline: 2px solid var(--accent, #498067); outline-offset: 2px; }
.model-settings label { display: grid; gap: .35rem; min-width: 0; font-size: .75rem; color: var(--steel, #64736f); }
.model-settings input:not([type=checkbox]), .model-settings select, .model-settings textarea { width: 100%; min-width: 0; box-sizing: border-box; min-height: 36px; border: 1px solid var(--line, #d9e0dc); border-radius: 5px; padding: .45rem .55rem; background: var(--model-control-bg, white); color: var(--ink, #202928); font: inherit; font-size: .82rem; }
.model-settings input:disabled, .model-settings select:disabled { background: var(--canvas, #f2f5f1); }
.model-settings small { font-size: .72rem; line-height: 1.5; font-weight: 400; color: var(--steel, #64736f); }
.model-default { padding: .8rem; border: 1px solid var(--line, #d9e0dc); border-radius: 6px; background: var(--canvas, #f6f8f5); }
.model-section-heading { margin: 1rem 0 .5rem; }
.model-section-heading button { font-size: .75rem; min-height: 30px; }
.model-service-list, .model-model-list { display: flex; flex-wrap: wrap; gap: .4rem; margin-bottom: .7rem; }
.model-service-list button, .model-model-list button { min-width: 0; max-width: 100%; font-size: .75rem; }
.model-service-list span, .model-model-list span { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.model-service-list svg, .model-model-list svg { flex-shrink: 0; }
.model-service-list [aria-pressed=true], .model-model-list [aria-pressed=true] { border-color: var(--accent, #498067); color: var(--accent, #38684e); background: var(--model-selected-bg, #f2f7f1); }
.model-editor { min-width: 0; margin: 0; padding: .8rem; border: 1px solid var(--line, #d9e0dc); border-radius: 6px; }
.model-editor legend { padding: 0 .3rem; color: var(--steel, #64736f); font-size: .72rem; }
.model-fields { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: .7rem; }
.model-field-wide { grid-column: 1 / -1; }
.model-cloud-fields { margin: .65rem 0; }
.model-capabilities { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: .65rem; margin: .7rem 0; }
.model-advanced { margin-top: .8rem; }
.model-advanced summary { cursor: pointer; color: var(--steel, #64736f); font-size: .75rem; }
.model-advanced[open] summary { margin-bottom: .7rem; }
.model-settings .model-checkbox { display: flex; align-items: center; gap: .4rem; margin-top: .65rem; }
.model-checkbox input { margin: 0; }
.model-test-row { justify-content: flex-start; flex-wrap: wrap; margin-top: .85rem; }
.model-test-row button { font-size: .75rem; }
.model-purpose { padding: .7rem 0; border-bottom: 1px solid var(--line, #d9e0dc); }
.model-purpose label { margin-top: .6rem; }
.model-save-row { margin-top: .8rem; }
.model-save-row p { margin: 0; }
.model-save-row button { flex-shrink: 0; }
.model-feedback { margin: .8rem 0 0; padding: .7rem; border-radius: 5px; background: #eef5ee; color: #345d40; line-height: 1.5; overflow-wrap: anywhere; }
.model-feedback p { margin: 0; }
.model-feedback.failure { color: #963f33; background: #fff0ec; }
.model-feedback button { margin-top: .5rem; }
.model-settings-heading h4 { font: 600 1.23rem/1.45 var(--sans, sans-serif); color: var(--ink); }
.model-settings-heading p { margin: .22rem 0 0; }
.model-default { display: flex !important; align-items: center; flex-wrap: wrap; gap: 10px !important; padding: .65rem .85rem; background: var(--surface-soft, #f6f3ed); }
.model-default select { flex: 1 1 12rem; width: auto; max-width: 100%; }
.model-default > span { font-weight: 600; color: var(--ink); }
.provider-grid { display: grid; grid-template-columns: repeat(3,minmax(0,1fr)); gap: .55rem; }
.model-settings .provider-choice { justify-content: flex-start; min-height: 68px; gap: 9px; padding: 10px; text-align: left; border-radius: 9px; background: var(--canvas,white); }
.provider-choice:hover, .model-settings .add-provider:hover { border-color: var(--brand-accent,#a9500e); }
.provider-mark { display: grid; flex: 0 0 32px; height: 32px; place-items: center; border-radius: 8px; color: #395e52; background: #e4ece7; }
.provider-mark[data-provider=deepseek] { color: #385caa; background: #e9efff; }
.provider-mark[data-provider=anthropic] { color: #87513d; background: #f4e5db; }
.provider-mark[data-provider=gemini] { color: #685398; background: #eee9f8; }
.provider-mark[data-provider=ollama], .provider-mark[data-provider=custom] { color: #555e65; background: #eaf0f0; }
.provider-label, .model-service-list button > span { display: grid; gap: 3px; min-width: 0; }
.provider-label strong, .model-service-list strong { font-size: .85rem; overflow-wrap: anywhere; }
.provider-label small { font-size: .67rem; overflow-wrap: anywhere; }
.provider-choice > svg:last-child { margin-left: auto; flex: none; color: var(--steel); }
.model-layout { display: grid; grid-template-columns: minmax(130px, 27%) minmax(0,1fr); gap: 1rem; margin: 1rem 0; align-items: start; }
.connections { min-width: 0; padding: .2rem .65rem .65rem 0; border-inline-end: 1px solid var(--line); }
.connections .model-section-heading { margin: .15rem 0 .7rem; }
.model-service-list { display: grid; gap: 4px; }
.model-service-list button { justify-content: space-between; border-radius: 6px; padding: 8px; text-align: left; }
.model-service-list strong { overflow: hidden; text-overflow: ellipsis; }
.model-settings .model-service-list [aria-pressed=true] { background: var(--surface-soft,#f5f1e9); color: var(--brand-accent-deep,#87400b); border-color: var(--brand-accent,#b45a17); }
.add-provider { width: 100%; justify-content: center; }
.model-editor { padding: .9rem; border-radius: 8px; }
.model-editor legend { font-size: .85rem; font-weight: 650; color: var(--ink); }
.connection-title, .connection-title h5, .field-heading { display: flex; align-items: center; justify-content: space-between; gap: .5rem; }
.connection-title { margin-bottom: .7rem; }
.connection-title h5 { justify-content: flex-start; }
.connection-title h5 svg { color: var(--steel); }
.connection-fields { grid-template-columns: minmax(0,1fr); }
.connection-title button, .field-heading button { min-height: 30px; font-size: .72rem; }
.catalog-field { min-width: 0; display: grid; gap: .35rem; }
.field-heading { flex-wrap: wrap; }
.model-settings .model-primary { background: var(--brand-accent,#ae5512); border-color: var(--brand-accent,#ae5512); color: white; }
@container (max-width: 640px) {
  .provider-grid { grid-template-columns: repeat(2,minmax(0,1fr)); }
  .model-layout { display: block; }
  .connections { padding: 0 0 .7rem; border: 0; }
  .model-service-list { display: flex; flex-wrap: wrap; }
  .add-provider { width: auto; }
}
@container (max-width: 400px) {
  .provider-choice > svg:last-child { display: none; }
  .provider-grid { gap: .4rem; }
  .model-settings .provider-choice { gap: 6px; padding: 8px; }
  .provider-mark { flex-basis: 24px; height: 26px; }
  .model-fields { grid-template-columns: minmax(0,1fr); }
  .model-save-row { flex-wrap: wrap; }
}
@media (max-width: 480px) {
  .model-fields { grid-template-columns: minmax(0, 1fr); }
  .model-settings-heading, .model-save-row { flex-wrap: wrap; }
  .model-settings-status { max-width: 100%; }
  .model-save-row button { width: 100%; }
}
</style>
