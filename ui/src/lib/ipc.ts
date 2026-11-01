import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import type {
  SystemInfo,
  PipelineConfig,
  ExportRequest,
  SubtitleSegment,
  PipelineProgressEvent,
} from '../types';

export function isTauri(): boolean {
  if (typeof window === 'undefined') return false;
  // Running inside Tauri webview
  if ('__TAURI_INTERNALS__' in window) return true;
  // Running in Vitest test environment
  if ((import.meta as any).env?.MODE === 'test') return true;
  return false;
}

export async function getSystemInfo(): Promise<SystemInfo> {
  if (!isTauri()) {
    console.info('[Echo Studio] Browser mode detected - using mock SystemInfo');
    return {
      has_cuda: false,
      ollama_running: true,
      ollama_models: ['qwen2.5:7b', 'llama3:8b'],
      whisper_models: [
        { name: 'tiny', downloaded: true, size_mb: 75 },
        { name: 'base', downloaded: true, size_mb: 142 },
        { name: 'small', downloaded: true, size_mb: 466 },
        { name: 'medium', downloaded: false, size_mb: 1460 },
        { name: 'large-v3', downloaded: false, size_mb: 2950 },
      ],
    };
  }
  return await invoke<SystemInfo>('get_system_info');
}

export async function downloadWhisperModel(modelName: string): Promise<void> {
  if (!isTauri()) {
    console.info(`[Echo Studio] Simulating download for model: ${modelName}`);
    await new Promise((resolve) => setTimeout(resolve, 800));
    return;
  }
  return await invoke('download_whisper_model', { modelName });
}

export async function startPipeline(config: PipelineConfig): Promise<SubtitleSegment[]> {
  if (!isTauri()) {
    console.info('[Echo Studio] Simulating pipeline processing in browser mode');
    const emitSimulated = (event: PipelineProgressEvent) => {
      window.dispatchEvent(new CustomEvent('echo:pipeline-progress', { detail: event }));
    };

    emitSimulated({ stage: 'extract_audio', message: 'Đang trích xuất audio...', percentage: 5, elapsed_secs: 0 });
    await new Promise((r) => setTimeout(r, 200));
    emitSimulated({ stage: 'extract_audio', message: 'Trích xuất audio hoàn tất.', percentage: 10, elapsed_secs: 0 });
    await new Promise((r) => setTimeout(r, 200));
    emitSimulated({ stage: 'stt', message: 'Đang nhận dạng Whisper...', percentage: 35, elapsed_secs: 1 });
    await new Promise((r) => setTimeout(r, 300));
    emitSimulated({ stage: 'stt', message: 'Nhận dạng hoàn tất: 3 đoạn thoại.', percentage: 60, current_step: 3, total_steps: 3, elapsed_secs: 1 });
    await new Promise((r) => setTimeout(r, 200));
    emitSimulated({ stage: 'translate', message: 'Đang dịch 2/3 đoạn...', percentage: 85, current_step: 2, total_steps: 3, elapsed_secs: 2 });
    await new Promise((r) => setTimeout(r, 300));
    emitSimulated({ stage: 'ready', message: 'Hoàn tất! Đã tạo 3 đoạn phụ đề.', percentage: 100, current_step: 3, total_steps: 3, elapsed_secs: 2 });

    return [
      {
        id: 1,
        start_ms: 1000,
        end_ms: 4500,
        original_text: 'Welcome to Echo Studio automated subtitle generator.',
        translated_text: 'Chào mừng bạn đến với trình tạo phụ đề tự động Echo Studio.',
      },
      {
        id: 2,
        start_ms: 5000,
        end_ms: 9200,
        original_text: 'This preview is running with full editing and styling support.',
        translated_text: 'Bản xem trước này đang chạy với hỗ trợ chỉnh sửa và định kiểu đầy đủ.',
      },
      {
        id: 3,
        start_ms: 9800,
        end_ms: 14500,
        original_text: 'You can edit text, adjust timestamps, and export hardsub or softsub.',
        translated_text: 'Bạn có thể sửa văn bản, chỉnh mốc thời gian và xuất hardsub hoặc softsub.',
      },
    ];
  }
  return await invoke<SubtitleSegment[]>('start_pipeline', { config });
}

export async function listenPipelineProgress(
  handler: (event: PipelineProgressEvent) => void
): Promise<() => void> {
  if (isTauri()) {
    return await listen<PipelineProgressEvent>('pipeline-progress', (e) => handler(e.payload));
  } else {
    const listener = (e: Event) => {
      handler((e as CustomEvent<PipelineProgressEvent>).detail);
    };
    window.addEventListener('echo:pipeline-progress', listener);
    return () => window.removeEventListener('echo:pipeline-progress', listener);
  }
}

export async function cancelPipeline(): Promise<void> {
  if (!isTauri()) {
    console.info('[Echo Studio] Pipeline cancelled (browser mode)');
    return;
  }
  return await invoke('cancel_pipeline');
}

export async function exportSubtitles(request: ExportRequest): Promise<string> {
  if (!isTauri()) {
    console.info('[Echo Studio] Simulating subtitle export (browser mode):', request);
    await new Promise((resolve) => setTimeout(resolve, 500));
    return request.output_path || '/mock/output/subtitles_exported.srt';
  }
  return await invoke<string>('export_subtitles', { request });
}

export async function openInFileManager(path: string): Promise<void> {
  if (!isTauri()) {
    console.info(`[Echo Studio] File manager request for: ${path} (browser mode)`);
    return;
  }
  return await invoke('open_in_file_manager', { path });
}

export async function pickVideoFile(): Promise<string | null> {
  if (!isTauri()) {
    return null;
  }
  return await invoke<string | null>('pick_video_file');
}

export async function pickSaveFile(defaultName: string, extension: string): Promise<string | null> {
  if (!isTauri()) {
    return null;
  }
  return await invoke<string | null>('pick_save_file', { defaultName, extension });
}
