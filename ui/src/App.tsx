import { useState, useEffect, useCallback, useMemo, useRef } from 'react';
import { listen } from '@tauri-apps/api/event';
import { FileVideo } from 'lucide-react';
import { SplitPane } from './components/SplitPane';
import { Toasts } from './components/Toasts';
import { useFileDrop } from './hooks/useFileDrop';
import { useToasts } from './hooks/useToasts';
import { HeaderBar } from './components/HeaderBar';
import { VideoPlayer } from './components/VideoPlayer';
import { SubtitleEditor } from './components/SubtitleEditor';
import { ProgressDrawer } from './components/ProgressDrawer';
import { ExportModal } from './components/ExportModal';
import { JobList } from './components/JobList';
import { SettingsModal } from './components/SettingsModal';
import { YoutubeImportModal } from './components/YoutubeImportModal';
import { useSystemInfo } from './hooks/useSystemInfo';
import {
  startPipeline,
  cancelPipeline,
  exportSubtitles,
  openInFileManager,
  pickVideoFiles,
  getOutputDir,
  getSettings,
  saveGeneral,
  setDefaultProfile,
  setDefaultSkill,
  listenPipelineProgress,
  isTauri,
} from './lib/ipc';
import { allSkills, jobSkillChoice, skillName } from './lib/skills';
import type {
  SubtitleSegment,
  GeneralSettings,
  SettingsView,
  PipelineStageEvent,
  BatchJob,
} from './types';

let jobCounter = 0;
const nextJobId = () => `job-${Date.now()}-${jobCounter++}`;

export default function App() {
  const { systemInfo, downloadModel, downloadingModel } = useSystemInfo();

  // App settings state
  const [general, setGeneral] = useState<GeneralSettings>({
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
  });
  const [settingsView, setSettingsView] = useState<SettingsView | null>(null);
  const settingsRef = useRef<SettingsView | null>(null);
  settingsRef.current = settingsView;
  const { toasts, pushToast, dismissToast } = useToasts();
  const [isSettingsOpen, setIsSettingsOpen] = useState(false);
  const lastSavedGeneral = useRef<string | null>(null);
  const patchGeneral = (patch: Partial<GeneralSettings>) => setGeneral((g) => ({ ...g, ...patch }));
  const { whisper_model: whisperModel, target_lang: targetLang, bilingual, style_preset: stylePreset } = general;

  // Batch jobs state
  const [jobs, setJobs] = useState<BatchJob[]>([]);
  const [selectedJobId, setSelectedJobId] = useState<string | null>(null);

  // Subtitle & Playback state
  const [currentTimeMs, setCurrentTimeMs] = useState(0);
  const [seekTimeMs, setSeekTimeMs] = useState<number | null>(null);

  // UI state
  const [logs, setLogs] = useState<string[]>([]);
  const [isExportOpen, setIsExportOpen] = useState(false);
  const [isYoutubeModalOpen, setIsYoutubeModalOpen] = useState(false);
  const [outputDir, setOutputDir] = useState<string>('');
  const [pendingAdd, setPendingAdd] = useState<{ paths: string[]; dupes: string[] } | null>(null);

  const jobsRef = useRef(jobs);
  jobsRef.current = jobs;

  // YouTube video ids already in the queue (from "[id]" filename suffix)
  const existingYoutubeIds = useMemo(() => {
    const ids = new Set<string>();
    for (const j of jobs) {
      const m = j.videoPath.match(/\[([A-Za-z0-9_-]{11})\]\.[^./\\]+$/);
      if (m) ids.add(m[1]);
    }
    return ids;
  }, [jobs]);

  const updateJob = useCallback((id: string, patch: Partial<BatchJob>) => {
    setJobs((prev) => prev.map((j) => (j.id === id ? { ...j, ...patch } : j)));
  }, []);

  const selectedJob = jobs.find((j) => j.id === selectedJobId) ?? null;
  const isRunning = jobs.some((j) => j.status === 'running');
  const videoPath = selectedJob?.videoPath ?? '';
  const segments = selectedJob?.segments ?? [];
  const outputVideoPath = selectedJob?.outputVideoPath ?? null;

  // Resolve the default output directory (~/Echo Studio/outputs) once
  useEffect(() => {
    getOutputDir()
      .then(setOutputDir)
      .catch((err) => console.warn('getOutputDir failed:', err));
  }, []);

  // Load persisted settings once
  useEffect(() => {
    getSettings()
      .then((s) => {
        lastSavedGeneral.current = JSON.stringify(s.general);
        setGeneral(s.general);
        setSettingsView(s);
        if (s.load_warning) {
          setLogs((prev) => [...prev, `[CẢNH BÁO] ${s.load_warning}`]);
          pushToast(s.load_warning, 'info');
        }
      })
      .catch((err) => console.warn('getSettings failed:', err));
  }, []);

  // Persist general settings, debounced
  useEffect(() => {
    if (lastSavedGeneral.current === null) return;
    const serialized = JSON.stringify(general);
    if (serialized === lastSavedGeneral.current) return;
    const timer = setTimeout(() => {
      lastSavedGeneral.current = serialized;
      saveGeneral(general).catch((err) => console.warn('saveGeneral failed:', err));
    }, 500);
    return () => clearTimeout(timer);
  }, [general]);

  const handleSelectProfile = async (id: string) => {
    try {
      setSettingsView(await setDefaultProfile(id));
    } catch (err) {
      setLogs((prev) => [...prev, `[LỖI] ${String(err)}`]);
      pushToast(String(err), 'error');
    }
  };

  const handleSelectSkill = async (id: string | null) => {
    try {
      setSettingsView(await setDefaultSkill(id));
    } catch (err) {
      setLogs((prev) => [...prev, `[LỖI] ${String(err)}`]);
      pushToast(String(err), 'error');
    }
  };

  const handleChangeJobSkill = (jobId: string, skillId: string | undefined) => updateJob(jobId, { skillId });

  // Setup pipeline progress listener — routes progress to the right job
  useEffect(() => {
    let unlisten: (() => void) | undefined;
    listenPipelineProgress((event) => {
      setJobs((prev) => {
        const targetId =
          event.job_id && prev.some((j) => j.id === event.job_id)
            ? event.job_id
            : prev.find((j) => j.status === 'running')?.id;
        if (!targetId) return prev;
        return prev.map((j) => (j.id === targetId ? { ...j, progress: event } : j));
      });
    }).then((fn) => {
      unlisten = fn;
    });

    return () => {
      if (unlisten) unlisten();
    };
  }, []);

  // Setup Tauri event listeners for logs
  useEffect(() => {
    if (!isTauri()) {
      return;
    }

    let unlistenLog: (() => void) | undefined;

    const setupListeners = async () => {
      try {
        unlistenLog = await listen<string>('pipeline-log', (event) => {
          setLogs((prev) => [...prev, event.payload]);
        });
      } catch (err) {
        console.warn('Tauri event listeners not registered:', err);
      }
    };

    setupListeners();

    return () => {
      if (unlistenLog) unlistenLog();
    };
  }, []);

  const appendJobs = useCallback((paths: string[]) => {
    if (paths.length === 0) return;
    const newJobs: BatchJob[] = paths.map((p) => ({
      id: nextJobId(),
      videoPath: p,
      status: 'queued' as const,
      progress: null,
      segments: [],
      outputVideoPath: null,
      error: null,
    }));
    setJobs((prev) => [...prev, ...newJobs]);
    setSelectedJobId(newJobs[0].id);
    setCurrentTimeMs(0);
  }, []);

  // Ask before re-adding files that are already in the queue
  const addJobs = useCallback(
    (paths: string[]) => {
      if (paths.length === 0) return;
      const existing = new Set(jobsRef.current.map((j) => j.videoPath));
      const dupes = paths.filter((p) => existing.has(p));
      if (dupes.length > 0) {
        setPendingAdd({ paths, dupes });
        return;
      }
      appendJobs(paths);
    },
    [appendJobs]
  );

  const resolvePendingAdd = (addAll: boolean) => {
    if (!pendingAdd) return;
    const { paths, dupes } = pendingAdd;
    setPendingAdd(null);
    const fresh = addAll ? paths : paths.filter((p) => !dupes.includes(p));
    if (fresh.length > 0) appendJobs(fresh);
    else pushToast('Tất cả file đã có trong hàng đợi', 'info');
  };

  const handleFilesDropped = useCallback(
    (paths: string[]) => {
      if (paths.length === 0) {
        pushToast('Không có file video/audio hợp lệ', 'error');
        return;
      }
      addJobs(paths);
    },
    [addJobs, pushToast]
  );
  const { isDragging } = useFileDrop(handleFilesDropped);

  const handleYoutubeVideosDownloaded = useCallback(
    (paths: string[]) => {
      const existing = new Set(jobsRef.current.map((j) => j.videoPath));
      const freshCount = paths.filter((p) => !existing.has(p)).length;
      addJobs(paths);
      if (freshCount === paths.length) {
        pushToast(`Đã thêm ${paths.length} video từ YouTube vào hàng đợi`, 'success');
      } else {
        pushToast(`${paths.length - freshCount}/${paths.length} video đã có trong hàng đợi`, 'info');
      }
    },
    [addJobs, pushToast]
  );

  const handleSelectVideos = async () => {
    if (isTauri()) {
      try {
        const selected = await pickVideoFiles();
        if (selected.length > 0) {
          addJobs(selected);
          return;
        }
      } catch (err) {
        console.error('Failed to open native file picker:', err);
      }
      return;
    }

    // Hidden file input trigger (browser fallback, multi-select)
    const input = document.createElement('input');
    input.type = 'file';
    input.accept = 'video/*,audio/*';
    input.multiple = true;
    input.onchange = (e) => {
      const files = Array.from((e.target as HTMLInputElement).files || []);
      addJobs(files.map((f) => (f as any).path || URL.createObjectURL(f)));
    };
    input.click();
  };

  const handleRemoveJob = (id: string) => {
    const remaining = jobsRef.current.filter((j) => j.id !== id);
    setJobs(remaining);
    if (selectedJobId === id) {
      setSelectedJobId(remaining[0]?.id ?? null);
      setCurrentTimeMs(0);
    }
  };

  const runJob = async (job: BatchJob) => {
    const current = jobsRef.current.find((j) => j.id === job.id) ?? job;
    const { skill_id, label } = jobSkillChoice(current, settingsRef.current);
    updateJob(job.id, {
      status: 'running',
      skillUsed: label,
      error: null,
      progress: {
        job_id: job.id,
        stage: 'extract_audio',
        message: 'Bắt đầu quy trình tạo phụ đề...',
        percentage: 0,
        elapsed_secs: 0,
      },
    });

    try {
      const result = await startPipeline({
        job_id: job.id,
        video_path: job.videoPath,
        whisper_model: whisperModel,
        use_cloud_stt: general.use_cloud_stt,
        target_lang: targetLang,
        skip_translate: false,
        prefer_gpu: general.prefer_gpu,
        profile_id: settingsRef.current?.default_profile_id ?? null,
        skill_id,
      });
      updateJob(job.id, { status: 'done', segments: result });
      pushToast(`Hoàn tất ${job.videoPath.split(/[/\\]/).pop()}`, 'success');
    } catch (err) {
      const msg = String(err);
      const isCancelled = msg.includes('hủy') || msg.includes('cancel');
      updateJob(job.id, {
        status: isCancelled ? 'cancelled' : 'error',
        error: msg,
      });
      setLogs((prev) => [...prev, `[LỖI] ${job.videoPath}: ${msg}`]);
      pushToast(msg, 'error');
    }
  };

  const stopQueueRef = useRef(false);

  // Run queued jobs sequentially — one file at a time
  const handleStart = async () => {
    stopQueueRef.current = false;
    const queued = jobsRef.current.filter(
      (j) => j.status === 'queued' || j.status === 'error' || j.status === 'cancelled'
    );
    for (const job of queued) {
      if (stopQueueRef.current) break;
      await runJob(job);
      // Stop the queue entirely when the current job was cancelled
      const finished = jobsRef.current.find((j) => j.id === job.id);
      if (!finished || finished.status === 'cancelled') break;
    }
  };

  // Cancel the running job AND mark every pending item cancelled
  const cancelQueue = async (id?: string) => {
    stopQueueRef.current = true;
    try {
      await cancelPipeline(id);
    } catch (err) {
      console.error('Cancel failed:', err);
    }
    setJobs((prev) =>
      prev.map((j) => (j.status === 'queued' ? { ...j, status: 'cancelled' } : j))
    );
  };

  const handleCancel = () => cancelQueue();
  const handleCancelJob = (id: string) => cancelQueue(id);

  // Subtitle editor operations — applied to the selected job
  const updateSelectedSegments = (fn: (segs: SubtitleSegment[]) => SubtitleSegment[]) => {
    if (!selectedJobId) return;
    setJobs((prev) =>
      prev.map((j) => (j.id === selectedJobId ? { ...j, segments: fn(j.segments) } : j))
    );
  };

  const handleUpdateSegment = (updated: SubtitleSegment) => {
    updateSelectedSegments((prev) => prev.map((s) => (s.id === updated.id ? updated : s)));
  };

  const handleDeleteSegment = (id: number) => {
    updateSelectedSegments((prev) =>
      prev.filter((s) => s.id !== id).map((s, idx) => ({ ...s, id: idx + 1 }))
    );
  };

  const handleSplitSegment = (id: number) => {
    updateSelectedSegments((prev) => {
      const idx = prev.findIndex((s) => s.id === id);
      if (idx === -1) return prev;
      const target = prev[idx];
      const midMs = Math.floor((target.start_ms + target.end_ms) / 2);

      const part1: SubtitleSegment = { ...target, end_ms: midMs };
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
    updateSelectedSegments((prev) => {
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
    updateSelectedSegments((prev) => {
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
    updateSelectedSegments((prev) =>
      prev.map((s) => ({
        ...s,
        original_text: s.original_text.replaceAll(find, replaceWith),
        translated_text: s.translated_text
          ? s.translated_text.replaceAll(find, replaceWith)
          : null,
      }))
    );
  };

  const defaultOutputPath = (() => {
    const base = videoPath
      ? (videoPath.split(/[/\\]/).pop() || 'video').replace(/\.[^/.]+$/, '')
      : 'video';
    const dir = outputDir || '~/Echo Studio/outputs';
    return `${dir}/${base}_subtitled.mp4`;
  })();

  // Progress for the drawer: prefer selected job, else first running job
  const progressJob =
    selectedJob && selectedJob.progress
      ? selectedJob
      : jobs.find((j) => j.status === 'running') ?? null;
  const drawerProgress = progressJob?.progress ?? null;
  const drawerStage: PipelineStageEvent | null = drawerProgress
    ? { stage: drawerProgress.stage, message: drawerProgress.message }
    : null;

  return (
    <div className="flex flex-col h-screen w-screen bg-slate-950 text-slate-100 overflow-hidden font-sans">
      {/* 1. Header Bar */}
      <HeaderBar
        videoPath={videoPath}
        jobCount={jobs.length}
        onSelectVideo={handleSelectVideos}
        onOpenYoutubeModal={() => setIsYoutubeModalOpen(true)}
        whisperModel={whisperModel}
        onSelectWhisperModel={(m) => patchGeneral({ whisper_model: m })}
        profiles={settingsView?.profiles ?? []}
        defaultProfileId={settingsView?.default_profile_id ?? null}
        onSelectProfile={handleSelectProfile}
        onOpenSettings={() => setIsSettingsOpen(true)}
        presets={settingsView?.presets ?? []}
        userSkills={settingsView?.skills ?? []}
        defaultSkillId={settingsView?.default_skill_id ?? null}
        onSelectSkill={handleSelectSkill}
        targetLang={targetLang}
        onSelectTargetLang={(l) => patchGeneral({ target_lang: l })}
        bilingual={bilingual}
        onToggleBilingual={() => patchGeneral({ bilingual: !bilingual })}
        stylePreset={stylePreset}
        onSelectPreset={(p) => patchGeneral({ style_preset: p })}
        isRunning={isRunning}
        onStart={handleStart}
        onCancel={handleCancel}
        systemInfo={systemInfo}
        preferGpu={general.prefer_gpu}
        onToggleGpu={() => patchGeneral({ prefer_gpu: !general.prefer_gpu })}
        onDownloadModel={downloadModel}
        downloadingModel={downloadingModel}
      />

      {/* 2. Main Studio Workspace: Two resizable columns */}
      <main className="flex-1 min-h-0 p-3 overflow-hidden">
        <SplitPane direction="horizontal" initialRatio={0.5} minA={420} minB={360} storageKey="main">
          {/* Left Column: Video Player + Job List */}
          <section className="h-full min-h-0 pr-1.5">
            <SplitPane direction="vertical" initialRatio={0.7} minA={240} minB={120} storageKey="video-list">
              <div className="h-full min-h-0 pb-1">
                <VideoPlayer
                  videoPath={videoPath}
                  outputPath={outputVideoPath}
                  segments={segments}
                  preset={stylePreset}
                  isBilingual={bilingual}
                  currentTimeMs={currentTimeMs}
                  onTimeUpdate={setCurrentTimeMs}
                  seekTimeMs={seekTimeMs}
                  onSeekComplete={() => setSeekTimeMs(null)}
                  onOpenYoutubeModal={() => setIsYoutubeModalOpen(true)}
                />
              </div>

              {/* Batch job / output list */}
              <div className="h-full min-h-0 pt-1 bg-slate-900 border border-slate-800 rounded-lg overflow-hidden flex flex-col">
                <div className="px-3 py-1.5 border-b border-slate-800 text-[11px] font-semibold text-slate-400 uppercase tracking-wide shrink-0">
                  Danh sách xử lý ({jobs.length})
                </div>
                <div className="flex-1 min-h-0">
                  <JobList
                    jobs={jobs}
                    selectedJobId={selectedJobId}
                    onSelect={setSelectedJobId}
                    onRemove={handleRemoveJob}
                    onCancelJob={handleCancelJob}
                    onOpenFolder={openInFileManager}
                    skillOptions={settingsView ? allSkills(settingsView).map((s) => ({ id: s.id, name: s.name })) : []}
                    defaultSkillName={skillName(settingsView, settingsView?.default_skill_id)}
                    onChangeJobSkill={handleChangeJobSkill}
                  />
                </div>
              </div>
            </SplitPane>
          </section>

          {/* Right Column: Subtitle Editor */}
          <section className="h-full min-h-0 pl-1.5">
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
        </SplitPane>
      </main>

      {/* 3. Bottom Progress Drawer */}
      <ProgressDrawer
        currentStage={drawerStage}
        progress={drawerProgress}
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
        dubDefaults={
          general.dub_enabled
            ? {
                engine: general.tts_engine,
                voice: general.tts_voice,
                model: general.tts_model,
                original_volume: general.dub_orig_volume,
              }
            : undefined
        }
        onExport={exportSubtitles}
        onOpenFolder={openInFileManager}
        onExported={(path) => {
          if (selectedJobId) updateJob(selectedJobId, { outputVideoPath: path });
        }}
      />

      {/* 5. YouTube Import Modal (kept mounted so background downloads still queue) */}
      <YoutubeImportModal
        isOpen={isYoutubeModalOpen}
        onClose={() => setIsYoutubeModalOpen(false)}
        onVideosDownloaded={handleYoutubeVideosDownloaded}
        defaultDownloadDir={general.download_dir ?? undefined}
        existingIds={existingYoutubeIds}
      />

      {/* 6. Settings Modal */}
      {settingsView && (
        <SettingsModal
          isOpen={isSettingsOpen}
          onClose={() => setIsSettingsOpen(false)}
          settings={settingsView}
          onSettingsChange={setSettingsView}
          general={general}
          onGeneralChange={setGeneral}
          whisperModels={systemInfo?.whisper_models ?? []}
        />
      )}

      {/* 6.5 Duplicate files prompt */}
      {pendingAdd && (
        <div className="fixed inset-0 z-[60] flex items-center justify-center bg-black/60 backdrop-blur-sm">
          <div className="w-full max-w-md bg-slate-900 border border-slate-700 rounded-xl shadow-2xl p-4">
            <div className="text-sm font-semibold text-slate-100 mb-1">
              {pendingAdd.dupes.length} file đã có trong hàng đợi
            </div>
            <div className="text-xs text-slate-400 mb-3">
              Bỏ qua các file trùng hay thêm lại vào danh sách xử lý?
            </div>
            <ul className="max-h-32 overflow-y-auto text-[11px] text-slate-500 mb-3 space-y-0.5">
              {pendingAdd.dupes.map((p) => (
                <li key={p} className="truncate" title={p}>
                  {p.split(/[/\\]/).pop()}
                </li>
              ))}
            </ul>
            <div className="flex justify-end gap-2">
              <button
                onClick={() => resolvePendingAdd(false)}
                className="h-8 px-3 text-xs font-medium rounded-lg bg-slate-800 hover:bg-slate-700 text-slate-200 transition-colors"
              >
                Bỏ qua file trùng
              </button>
              <button
                onClick={() => resolvePendingAdd(true)}
                className="h-8 px-3 text-xs font-semibold rounded-lg bg-blue-600 hover:bg-blue-500 text-white transition-colors"
              >
                Vẫn thêm
              </button>
            </div>
          </div>
        </div>
      )}

      {/* 7. Drag overlay + Toasts */}
      {isDragging && (
        <div className="fixed inset-0 z-40 bg-blue-950/60 border-2 border-dashed border-blue-400 flex items-center justify-center pointer-events-none">
          <div className="flex flex-col items-center gap-2 text-blue-200">
            <FileVideo className="w-10 h-10" />
            <span className="text-sm font-medium">Thả để thêm vào hàng đợi</span>
          </div>
        </div>
      )}
      <Toasts toasts={toasts} onDismiss={dismissToast} />
    </div>
  );
}
