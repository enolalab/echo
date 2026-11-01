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
  video_path: string;
  subtitle_path?: string | null;
  whisper_model: string;
  use_cloud_stt: boolean;
  stt_api_key?: string | null;
  target_lang: string;
  skip_translate: boolean;
  use_cloud_llm: boolean;
  llm_model: string;
  llm_api_base?: string | null;
  llm_api_key?: string | null;
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
  stage: 'extract_audio' | 'stt' | 'translate' | 'ready';
  message: string;
  percentage: number;
  current_step?: number;
  total_steps?: number;
  elapsed_secs: number;
}
