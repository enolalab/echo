import { useState, useEffect } from 'react';
import { listen } from '@tauri-apps/api/event';
import { HeaderBar } from './components/HeaderBar';
import { VideoPlayer } from './components/VideoPlayer';
import { SubtitleEditor } from './components/SubtitleEditor';
import { ProgressDrawer } from './components/ProgressDrawer';
import { ExportModal } from './components/ExportModal';
import { useSystemInfo } from './hooks/useSystemInfo';
import {
  startPipeline,
  cancelPipeline,
  exportSubtitles,
  openInFileManager,
  isTauri,
} from './lib/ipc';
import type {
  SubtitleSegment,
  StylePreset,
  PipelineStageEvent,
} from './types';

export default function App() {
  const { systemInfo, downloadModel, downloadingModel } = useSystemInfo();

  // App settings state
  const [videoPath, setVideoPath] = useState('');
  const [whisperModel, setWhisperModel] = useState('base');
  const [ollamaModel, setOllamaModel] = useState('qwen2.5:7b');
  const [targetLang, setTargetLang] = useState('vi');
  const [bilingual, setBilingual] = useState(true);
  const [stylePreset, setStylePreset] = useState<StylePreset>('youtube_yellow');

  // Subtitle & Playback state
  const [segments, setSegments] = useState<SubtitleSegment[]>([]);
  const [currentTimeMs, setCurrentTimeMs] = useState(0);
  const [seekTimeMs, setSeekTimeMs] = useState<number | null>(null);

  // Pipeline execution state
  const [isRunning, setIsRunning] = useState(false);
  const [currentStage, setCurrentStage] = useState<PipelineStageEvent | null>(null);
  const [logs, setLogs] = useState<string[]>([]);
  const [isExportOpen, setIsExportOpen] = useState(false);

  // Auto-select first available Ollama model
  useEffect(() => {
    if (systemInfo?.ollama_models && systemInfo.ollama_models.length > 0) {
      if (!systemInfo.ollama_models.includes(ollamaModel)) {
        setOllamaModel(systemInfo.ollama_models[0]);
      }
    }
  }, [systemInfo, ollamaModel]);

  // Setup Tauri event listeners
  useEffect(() => {
    if (!isTauri()) {
      return;
    }

    let unlistenStage: (() => void) | undefined;
    let unlistenLog: (() => void) | undefined;

    const setupListeners = async () => {
      try {
        unlistenStage = await listen<PipelineStageEvent>('pipeline-stage', (event) => {
          setCurrentStage(event.payload);
        });

        unlistenLog = await listen<string>('pipeline-log', (event) => {
          setLogs((prev) => [...prev, event.payload]);
        });
      } catch (err) {
        // Fallback for non-tauri or browser test environment
        console.warn('Tauri event listeners not registered:', err);
      }
    };

    setupListeners();

    return () => {
      if (unlistenStage) unlistenStage();
      if (unlistenLog) unlistenLog();
    };
  }, []);

  const handleSelectVideo = () => {
    // Hidden file input trigger
    const input = document.createElement('input');
    input.type = 'file';
    input.accept = 'video/*,audio/*';
    input.onchange = (e) => {
      const file = (e.target as HTMLInputElement).files?.[0];
      if (file) {
        // In Tauri webview: file.path exists; in browser: create object URL
        const fullPath = (file as any).path || URL.createObjectURL(file);
        setVideoPath(fullPath);
      }
    };
    input.click();
  };

  const handleStart = async () => {
    if (!videoPath || isRunning) return;

    setIsRunning(true);
    setLogs([]);
    setCurrentStage({ stage: 'extract_audio', message: 'Bắt đầu quy trình tạo phụ đề...' });

    try {
      const result = await startPipeline({
        video_path: videoPath,
        whisper_model: whisperModel,
        use_cloud_stt: false,
        target_lang: targetLang,
        skip_translate: false,
        use_cloud_llm: false,
        llm_model: ollamaModel,
      });
      setSegments(result);
      setCurrentStage({ stage: 'ready', message: `Đã sinh thành công ${result.length} đoạn phụ đề!` });
    } catch (err) {
      setLogs((prev) => [...prev, `[LỖI] ${String(err)}`]);
      setCurrentStage({ stage: 'ready', message: 'Tác vụ gặp lỗi hoặc đã bị dừng.' });
    } finally {
      setIsRunning(false);
    }
  };

  const handleCancel = async () => {
    try {
      await cancelPipeline();
    } catch (err) {
      console.error('Cancel failed:', err);
    }
  };

  // Subtitle editor operations
  const handleUpdateSegment = (updated: SubtitleSegment) => {
    setSegments((prev) => prev.map((s) => (s.id === updated.id ? updated : s)));
  };

  const handleDeleteSegment = (id: number) => {
    setSegments((prev) =>
      prev
        .filter((s) => s.id !== id)
        .map((s, idx) => ({ ...s, id: idx + 1 }))
    );
  };

  const handleSplitSegment = (id: number) => {
    setSegments((prev) => {
      const idx = prev.findIndex((s) => s.id === id);
      if (idx === -1) return prev;
      const target = prev[idx];
      const midMs = Math.floor((target.start_ms + target.end_ms) / 2);

      const part1: SubtitleSegment = {
        ...target,
        end_ms: midMs,
      };
      const part2: SubtitleSegment = {
        id: target.id + 1,
        start_ms: midMs,
        end_ms: target.end_ms,
        original_text: target.original_text,
        translated_text: target.translated_text,
      };

      const copy = [...prev];
      copy.splice(idx, 1, part1, part2);
      return copy.map((s, i) => ({ ...s, id: i + 1 }));
    });
  };

  const handleMergeSegment = (id: number) => {
    setSegments((prev) => {
      const idx = prev.findIndex((s) => s.id === id);
      if (idx === -1 || idx >= prev.length - 1) return prev;
      const cur = prev[idx];
      const next = prev[idx + 1];

      const merged: SubtitleSegment = {
        id: cur.id,
        start_ms: cur.start_ms,
        end_ms: next.end_ms,
        original_text: `${cur.original_text} ${next.original_text}`,
        translated_text:
          cur.translated_text && next.translated_text
            ? `${cur.translated_text} ${next.translated_text}`
            : cur.translated_text || next.translated_text,
      };

      const copy = [...prev];
      copy.splice(idx, 2, merged);
      return copy.map((s, i) => ({ ...s, id: i + 1 }));
    });
  };

  const handleAddSegment = () => {
    setSegments((prev) => {
      const last = prev[prev.length - 1];
      const start_ms = last ? last.end_ms + 100 : 0;
      const end_ms = start_ms + 3000;
      const newSeg: SubtitleSegment = {
        id: prev.length + 1,
        start_ms,
        end_ms,
        original_text: 'Dòng phụ đề mới',
        translated_text: 'Dòng phụ đề mới',
      };
      return [...prev, newSeg];
    });
  };

  const handleBatchReplace = (find: string, replaceWith: string) => {
    setSegments((prev) =>
      prev.map((s) => ({
        ...s,
        original_text: s.original_text.replaceAll(find, replaceWith),
        translated_text: s.translated_text
          ? s.translated_text.replaceAll(find, replaceWith)
          : null,
      }))
    );
  };

  const defaultOutputPath = videoPath
    ? videoPath.replace(/\.[^/.]+$/, '') + '_subtitled.mp4'
    : 'video_subtitled.mp4';

  return (
    <div className="flex flex-col h-screen w-screen bg-slate-950 text-slate-100 overflow-hidden font-sans">
      {/* 1. Header Bar */}
      <HeaderBar
        videoPath={videoPath}
        onSelectVideo={handleSelectVideo}
        whisperModel={whisperModel}
        onSelectWhisperModel={setWhisperModel}
        ollamaModel={ollamaModel}
        onSelectOllamaModel={setOllamaModel}
        targetLang={targetLang}
        onSelectTargetLang={setTargetLang}
        bilingual={bilingual}
        onToggleBilingual={() => setBilingual(!bilingual)}
        stylePreset={stylePreset}
        onSelectPreset={setStylePreset}
        isRunning={isRunning}
        onStart={handleStart}
        onCancel={handleCancel}
        systemInfo={systemInfo}
        onDownloadModel={downloadModel}
        downloadingModel={downloadingModel}
      />

      {/* 2. Main Studio Workspace: Two columns */}
      <main className="flex-1 min-h-0 grid grid-cols-1 md:grid-cols-2 gap-3 p-3 overflow-hidden">
        {/* Left Column: Video Player */}
        <section className="h-full min-h-0">
          <VideoPlayer
            videoPath={videoPath}
            segments={segments}
            preset={stylePreset}
            isBilingual={bilingual}
            currentTimeMs={currentTimeMs}
            onTimeUpdate={setCurrentTimeMs}
            seekTimeMs={seekTimeMs}
            onSeekComplete={() => setSeekTimeMs(null)}
          />
        </section>

        {/* Right Column: Subtitle Editor */}
        <section className="h-full min-h-0">
          <SubtitleEditor
            segments={segments}
            currentTimeMs={currentTimeMs}
            onSeek={(ms) => setSeekTimeMs(ms)}
            onUpdateSegment={handleUpdateSegment}
            onDeleteSegment={handleDeleteSegment}
            onSplitSegment={handleSplitSegment}
            onMergeSegment={handleMergeSegment}
            onAddSegment={handleAddSegment}
            onBatchReplace={handleBatchReplace}
          />
        </section>
      </main>

      {/* 3. Bottom Progress Drawer */}
      <ProgressDrawer
        currentStage={currentStage}
        logs={logs}
        isRunning={isRunning}
        onOpenExport={() => setIsExportOpen(true)}
        canExport={segments.length > 0}
      />

      {/* 4. Export Modal */}
      <ExportModal
        isOpen={isExportOpen}
        onClose={() => setIsExportOpen(false)}
        videoPath={videoPath}
        defaultOutputPath={defaultOutputPath}
        segments={segments}
        initialPreset={stylePreset}
        initialBilingual={bilingual}
        onExport={exportSubtitles}
        onOpenFolder={openInFileManager}
      />
    </div>
  );
}
