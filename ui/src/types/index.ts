export interface SubtitleSegment {
  id: number;
  start_ms: number;
  end_ms: number;
  original_text: string;
  translated_text: string | null;
}

export interface WhisperModelInfo {
  name: string;
  downloaded: boolean;
  size_mb: number;
}

export interface SystemInfo {
  has_cuda: boolean;
  ollama_running: boolean;
  ollama_models: string[];
  whisper_models: WhisperModelInfo[];
}

export interface PipelineConfig {
  job_id?: string;
  video_path: string;
  subtitle_path?: string | null;
  whisper_model: string;
  use_cloud_stt: boolean;
  target_lang: string;
  skip_translate: boolean;
  profile_id?: string | null;
  skill_id?: string | null;
}

export type ExportMode = 'hardsub' | 'softsub' | 'subs_only';
export type StylePreset = 'classic_white' | 'youtube_yellow' | 'netflix_modern' | 'bilingual_highlight';

export interface ExportRequest {
  video_path: string;
  output_path: string;
  mode: ExportMode;
  bilingual: boolean;
  style_preset: StylePreset;
  segments: SubtitleSegment[];
}

export interface PipelineStageEvent {
  stage: 'extract_audio' | 'stt' | 'translate' | 'ready';
  message: string;
}

export interface PipelineProgressEvent {
  job_id?: string;
  stage: 'extract_audio' | 'stt' | 'translate' | 'ready';
  message: string;
  percentage: number;
  current_step?: number;
  total_steps?: number;
  elapsed_secs: number;
}

export type JobStatus = 'queued' | 'running' | 'done' | 'error' | 'cancelled';

export interface BatchJob {
  id: string;
  videoPath: string;
  status: JobStatus;
  progress: PipelineProgressEvent | null;
  segments: SubtitleSegment[];
  outputVideoPath: string | null;
  error: string | null;
  skillId?: string;
  skillUsed?: string;
}

export type ProviderKind =
  | 'openai'
  | 'openai_compatible'
  | 'anthropic'
  | 'anthropic_compatible'
  | 'deepseek'
  | 'ollama';

export interface ProviderProfile {
  id: string;
  name: string;
  kind: ProviderKind;
  base_url: string | null;
  model: string;
}

export interface ProfileView extends ProviderProfile {
  has_key: boolean;
}

export interface GeneralSettings {
  whisper_model: string;
  target_lang: string;
  bilingual: boolean;
  style_preset: StylePreset;
  use_cloud_stt: boolean;
}

export interface SettingsView {
  profiles: ProfileView[];
  default_profile_id: string | null;
  general: GeneralSettings;
  presets: Skill[];
  skills: Skill[];
  default_skill_id: string | null;
  stt_has_key: boolean;
  secrets_backend: 'keyring' | 'file';
  load_warning: string | null;
}

export interface ConnectionTest {
  ok: boolean;
  latency_ms: number;
  error: string | null;
}

export interface GlossaryEntry {
  source: string;
  target: string;
}

export interface Skill {
  id: string;
  name: string;
  description: string;
  system_prompt: string;
  glossary: GlossaryEntry[];
}

export interface TrySkillResult {
  lines: string[];
  system_prompt: string;
  error: string | null;
}
