import type { ProviderKind, ProviderProfile } from '../types';

export const PROVIDER_KINDS: ProviderKind[] = [
  'openai',
  'openai_compatible',
  'anthropic',
  'anthropic_compatible',
  'deepseek',
  'ollama',
];

export const KIND_LABELS: Record<ProviderKind, string> = {
  openai: 'OpenAI',
  openai_compatible: 'OpenAI Compatible',
  anthropic: 'Anthropic',
  anthropic_compatible: 'Anthropic Compatible',
  deepseek: 'DeepSeek',
  ollama: 'Ollama (local)',
};

export const KIND_ICONS: Record<ProviderKind, string> = {
  openai: '◎',
  openai_compatible: '◎',
  anthropic: '✳',
  anthropic_compatible: '✳',
  deepseek: '◆',
  ollama: '🦙',
};

const DEFAULT_BASE_URLS: Record<ProviderKind, string | null> = {
  openai: 'https://api.openai.com/v1',
  openai_compatible: null,
  anthropic: 'https://api.anthropic.com',
  anthropic_compatible: null,
  deepseek: 'https://api.deepseek.com/v1',
  ollama: 'http://localhost:11434',
};

export const defaultBaseUrl = (kind: ProviderKind): string | null => DEFAULT_BASE_URLS[kind];
export const requiresBaseUrl = (kind: ProviderKind): boolean => DEFAULT_BASE_URLS[kind] === null;
export const requiresApiKey = (kind: ProviderKind): boolean => kind !== 'ollama';

export const emptyProfile = (): ProviderProfile => ({
  id: '',
  name: '',
  kind: 'openai',
  base_url: null,
  model: '',
});
