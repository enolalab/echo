import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import type {
  SystemInfo,
  PipelineConfig,
  ExportRequest,
  SubtitleSegment,
  PipelineProgressEvent,
  SettingsView,
  ProviderProfile,
  GeneralSettings,
  ConnectionTest,
  Skill,
  TrySkillResult,
  YtDlpStatus,
  PiperStatus,
  YoutubeDownloadRequest,
  YoutubeInspectResult,
  YoutubeProgressPayload,
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
    const jobId = config.job_id;
    const emitSimulated = (event: PipelineProgressEvent) => {
      window.dispatchEvent(
        new CustomEvent('echo:pipeline-progress', { detail: { job_id: jobId, ...event } })
      );
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

export async function cancelPipeline(jobId?: string): Promise<void> {
  if (!isTauri()) {
    console.info('[Echo Studio] Pipeline cancelled (browser mode)');
    return;
  }
  return await invoke('cancel_pipeline', { jobId });
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

export async function pickVideoFiles(): Promise<string[]> {
  if (!isTauri()) {
    return [];
  }
  return await invoke<string[]>('pick_video_files');
}

export async function getOutputDir(): Promise<string> {
  if (!isTauri()) {
    return '/mock/Echo Studio/outputs';
  }
  return await invoke<string>('get_output_dir');
}

export async function getMediaUrl(path: string): Promise<string | null> {
  if (!isTauri()) {
    return null;
  }
  return await invoke<string>('get_media_url', { path });
}

export async function pickDirectory(title?: string): Promise<string | null> {
  if (!isTauri()) {
    return null;
  }
  return await invoke<string | null>('pick_directory', { title: title ?? null });
}

export async function pickSaveFile(defaultName: string, extension: string): Promise<string | null> {
  if (!isTauri()) {
    return null;
  }
  return await invoke<string | null>('pick_save_file', { defaultName, extension });
}

let mockSettings: SettingsView = {
  profiles: [
    { id: 'mock-ollama', name: 'Ollama local', kind: 'ollama', base_url: null, model: 'qwen2.5:7b', has_key: false },
  ],
  default_profile_id: 'mock-ollama',
  general: {
    whisper_model: 'base',
    target_lang: 'vi',
    bilingual: true,
    style_preset: 'youtube_yellow',
    use_cloud_stt: false,
    prefer_gpu: true,
    dub_enabled: false,
    tts_engine: 'local',
    tts_voice: null,
    tts_model: null,
    dub_orig_volume: 0.15,
    render_speed: 'balanced',
    translate_concurrency: 4,
  },
  presets: [
    { id: 'builtin:math', name: 'Toán học', description: 'Mock preset', system_prompt: 'Math {target_lang}', glossary: [] },
    { id: 'builtin:ai-ml', name: 'AI / Machine Learning', description: 'Mock preset', system_prompt: 'ML {target_lang}', glossary: [] },
  ],
  skills: [],
  default_skill_id: null,
  stt_has_key: false,
  secrets_backend: 'keyring',
  load_warning: null,
};

const cloneMock = (): SettingsView => JSON.parse(JSON.stringify(mockSettings));

export async function getSettings(): Promise<SettingsView> {
  if (!isTauri()) return cloneMock();
  return await invoke<SettingsView>('get_settings');
}

export async function saveProfile(profile: ProviderProfile, apiKey: string | null): Promise<SettingsView> {
  if (!isTauri()) {
    const id = profile.id || `mock-${Date.now()}`;
    const existing = mockSettings.profiles.find((p) => p.id === id);
    const hasKey = apiKey === null ? existing?.has_key ?? false : apiKey.trim() !== '';
    const next = { ...profile, id, has_key: hasKey };
    mockSettings.profiles = existing
      ? mockSettings.profiles.map((p) => (p.id === id ? next : p))
      : [...mockSettings.profiles, next];
    mockSettings.default_profile_id ??= id;
    return cloneMock();
  }
  return await invoke<SettingsView>('save_profile', { profile, apiKey });
}

export async function deleteProfile(id: string): Promise<SettingsView> {
  if (!isTauri()) {
    mockSettings.profiles = mockSettings.profiles.filter((p) => p.id !== id);
    if (mockSettings.default_profile_id === id) {
      mockSettings.default_profile_id = mockSettings.profiles[0]?.id ?? null;
    }
    return cloneMock();
  }
  return await invoke<SettingsView>('delete_profile', { id });
}

export async function setDefaultProfile(id: string): Promise<SettingsView> {
  if (!isTauri()) {
    mockSettings.default_profile_id = id;
    return cloneMock();
  }
  return await invoke<SettingsView>('set_default_profile', { id });
}

export async function saveGeneral(general: GeneralSettings): Promise<SettingsView> {
  if (!isTauri()) {
    mockSettings.general = { ...general };
    return cloneMock();
  }
  return await invoke<SettingsView>('save_general', { general });
}

export async function setSttKey(key: string): Promise<SettingsView> {
  if (!isTauri()) {
    mockSettings.stt_has_key = key.trim() !== '';
    return cloneMock();
  }
  return await invoke<SettingsView>('set_stt_key', { key });
}

export async function testProfile(profile: ProviderProfile, apiKey: string | null): Promise<ConnectionTest> {
  if (!isTauri()) {
    await new Promise((r) => setTimeout(r, 300));
    return { ok: true, latency_ms: 300, error: null };
  }
  return await invoke<ConnectionTest>('test_profile', { profile, apiKey });
}

export async function listProfileModels(profile: ProviderProfile, apiKey: string | null): Promise<string[]> {
  if (!isTauri()) return ['qwen2.5:7b', 'llama3:8b'];
  return await invoke<string[]>('list_profile_models', { profile, apiKey });
}

export async function saveSkill(skill: Skill): Promise<SettingsView> {
  if (!isTauri()) {
    const id = skill.id || `mock-skill-${Date.now()}`;
    const next = { ...skill, id };
    mockSettings.skills = mockSettings.skills.some((s) => s.id === id)
      ? mockSettings.skills.map((s) => (s.id === id ? next : s))
      : [...mockSettings.skills, next];
    return cloneMock();
  }
  return await invoke<SettingsView>('save_skill', { skill });
}

export async function deleteSkill(id: string): Promise<SettingsView> {
  if (!isTauri()) {
    mockSettings.skills = mockSettings.skills.filter((s) => s.id !== id);
    if (mockSettings.default_skill_id === id) mockSettings.default_skill_id = null;
    return cloneMock();
  }
  return await invoke<SettingsView>('delete_skill', { id });
}

export async function duplicateSkill(id: string): Promise<SettingsView> {
  if (!isTauri()) {
    const src = [...mockSettings.presets, ...mockSettings.skills].find((s) => s.id === id);
    if (src) mockSettings.skills.push({ ...src, id: `mock-skill-${Date.now()}`, name: `${src.name} (bản sao)` });
    return cloneMock();
  }
  return await invoke<SettingsView>('duplicate_skill', { id });
}

export async function setDefaultSkill(id: string | null): Promise<SettingsView> {
  if (!isTauri()) {
    mockSettings.default_skill_id = id;
    return cloneMock();
  }
  return await invoke<SettingsView>('set_default_skill', { id });
}

export async function importSkill(): Promise<SettingsView> {
  if (!isTauri()) return cloneMock();
  return await invoke<SettingsView>('import_skill');
}

export async function exportSkill(id: string): Promise<string | null> {
  if (!isTauri()) return `/mock/${id}.echo-skill.json`;
  return await invoke<string | null>('export_skill', { id });
}

export async function trySkill(
  skill: Skill,
  profileId: string | null,
  sample: string,
  sourceLang: string,
  targetLang: string
): Promise<TrySkillResult> {
  if (!isTauri()) {
    const lines = sample.split('\n').map((l) => l.trim()).filter(Boolean);
    return { lines: lines.map((l) => `[mock] ${l}`), system_prompt: skill.system_prompt, error: null };
  }
  return await invoke<TrySkillResult>('try_skill', { skill, profileId, sample, sourceLang, targetLang });
}

export async function checkYtDlpStatus(): Promise<YtDlpStatus> {
  if (!isTauri()) {
    return { installed: true, path: '/mock/bin/yt-dlp', version: 'mock' };
  }
  return await invoke<YtDlpStatus>('check_yt_dlp_status');
}

export async function ensureYtDlp(): Promise<YtDlpStatus> {
  if (!isTauri()) {
    await new Promise((r) => setTimeout(r, 500));
    return { installed: true, path: '/mock/bin/yt-dlp', version: 'mock' };
  }
  return await invoke<YtDlpStatus>('ensure_yt_dlp');
}

export async function checkPiperStatus(): Promise<PiperStatus> {
  if (!isTauri()) {
    return { installed: true, path: '/mock/bin/piper/piper', version: 'mock' };
  }
  return await invoke<PiperStatus>('check_piper_status');
}

export async function ensurePiper(): Promise<PiperStatus> {
  if (!isTauri()) {
    await new Promise((r) => setTimeout(r, 500));
    return { installed: true, path: '/mock/bin/piper/piper', version: 'mock' };
  }
  return await invoke<PiperStatus>('ensure_piper');
}

export async function inspectYoutubeUrl(url: string): Promise<YoutubeInspectResult> {
  if (!isTauri()) {
    await new Promise((r) => setTimeout(r, 600));
    return {
      is_playlist: true,
      playlist_id: 'PLmock',
      title: 'Mock Playlist',
      items: [
        {
          id: 'mock1',
          title: 'Mock Video 1',
          duration_seconds: 120,
          channel_name: 'Mock Channel',
          webpage_url: 'https://youtube.com/watch?v=mock1',
        },
        {
          id: 'mock2',
          title: 'Mock Video 2',
          duration_seconds: 240,
          channel_name: 'Mock Channel',
          webpage_url: 'https://youtube.com/watch?v=mock2',
        },
      ],
    };
  }
  return await invoke<YoutubeInspectResult>('inspect_youtube_url', { url });
}

export async function downloadYoutubeItems(req: YoutubeDownloadRequest): Promise<string[]> {
  if (!isTauri()) {
    const saved = req.items.map((i, idx) => `/mock/downloads/${i.id}.mp4`);
    req.items.forEach((i, idx) => {
      window.dispatchEvent(
        new CustomEvent('echo:youtube-download-progress', {
          detail: {
            video_id: i.id,
            index: idx,
            total_items: req.items.length,
            stage: 'completed',
            percentage: 100,
            saved_path: saved[idx],
          } satisfies YoutubeProgressPayload,
        })
      );
    });
    return saved;
  }
  return await invoke<string[]>('download_youtube_items', { req });
}

export async function cancelYoutubeDownload(): Promise<void> {
  if (!isTauri()) {
    console.info('[Echo Studio] YouTube download cancelled (browser mode)');
    return;
  }
  return await invoke('cancel_youtube_download');
}

export async function listenYoutubeProgress(
  handler: (event: YoutubeProgressPayload) => void
): Promise<() => void> {
  if (isTauri()) {
    return await listen<YoutubeProgressPayload>('youtube-download-progress', (e) =>
      handler(e.payload)
    );
  }
  const listener = (e: Event) => {
    handler((e as CustomEvent<YoutubeProgressPayload>).detail);
  };
  window.addEventListener('echo:youtube-download-progress', listener);
  return () => window.removeEventListener('echo:youtube-download-progress', listener);
}
