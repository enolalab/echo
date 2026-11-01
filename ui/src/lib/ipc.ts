import { invoke } from '@tauri-apps/api/core';
import type {
  SystemInfo,
  PipelineConfig,
  ExportRequest,
  SubtitleSegment,
} from '../types';

export async function getSystemInfo(): Promise<SystemInfo> {
  return await invoke<SystemInfo>('get_system_info');
}

export async function downloadWhisperModel(modelName: string): Promise<void> {
  return await invoke('download_whisper_model', { modelName });
}

export async function startPipeline(config: PipelineConfig): Promise<SubtitleSegment[]> {
  return await invoke<SubtitleSegment[]>('start_pipeline', { config });
}

export async function cancelPipeline(): Promise<void> {
  return await invoke('cancel_pipeline');
}

export async function exportSubtitles(request: ExportRequest): Promise<string> {
  return await invoke<string>('export_subtitles', { request });
}

export async function openInFileManager(path: string): Promise<void> {
  return await invoke('open_in_file_manager', { path });
}
