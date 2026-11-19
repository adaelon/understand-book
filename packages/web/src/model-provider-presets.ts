import type { ModelApiProtocol, ModelAuthentication } from './desktop-provider';

export interface ProviderPreset {
  id: string;
  name: string;
  address: string;
  auth: ModelAuthentication;
  protocol: ModelApiProtocol;
}

export const providerPresets: ProviderPreset[] = [
  { id: 'openai', name: 'OpenAI', address: 'https://api.openai.com/v1', auth: 'bearer', protocol: 'openai_chat_completions' },
  { id: 'deepseek', name: 'DeepSeek', address: 'https://api.deepseek.com/v1', auth: 'bearer', protocol: 'openai_chat_completions' },
  { id: 'anthropic', name: 'Anthropic', address: 'https://api.anthropic.com', auth: 'api_key', protocol: 'anthropic_messages' },
  { id: 'gemini', name: 'Google Gemini', address: 'https://generativelanguage.googleapis.com', auth: 'api_key', protocol: 'gemini_generate_content' },
  { id: 'ollama', name: 'Ollama', address: 'http://127.0.0.1:11434', auth: 'none', protocol: 'ollama_chat' },
  { id: 'custom', name: '自定义服务', address: '', auth: 'bearer', protocol: 'openai_chat_completions' },
];

export function providerForAddress(address: string) {
  const normalized = address.trim().replace(/\/+$/, '');
  return providerPresets.find(provider => provider.address && provider.address === normalized);
}

export function hasModelDirectory(protocol: ModelApiProtocol) {
  return providerPresets.some(provider => provider.protocol === protocol)
    || protocol === 'openai_responses' || protocol === 'gemini_interactions';
}
