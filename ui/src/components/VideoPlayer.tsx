import React, { useRef, useState, useEffect, useCallback } from 'react';
import {
  Play,
  Pause,
  RotateCcw,
  Volume2,
  VolumeX,
  FastForward,
  Rewind,
  Repeat,
  Film,
} from 'lucide-react';
import { convertFileSrc } from '@tauri-apps/api/core';
import { isTauri } from '../lib/ipc';
import type { SubtitleSegment, StylePreset } from '../types';
import { SubtitleOverlay } from './SubtitleOverlay';
import { SeekBar } from './SeekBar';

interface VideoPlayerProps {
  videoPath: string;
  outputPath?: string | null;
  segments: SubtitleSegment[];
  preset: StylePreset;
  isBilingual: boolean;
  currentTimeMs: number;
  onTimeUpdate: (ms: number) => void;
  seekTimeMs: number | null;
  onSeekComplete: () => void;
}

function formatTime(ms: number): string {
  const totalSeconds = Math.floor(ms / 1000);
  const m = Math.floor(totalSeconds / 60);
  const s = totalSeconds % 60;
  return `${m.toString().padStart(2, '0')}:${s.toString().padStart(2, '0')}`;
}

export const VideoPlayer: React.FC<VideoPlayerProps> = ({
  videoPath,
  outputPath,
  segments,
  preset,
  isBilingual,
  currentTimeMs,
  onTimeUpdate,
  seekTimeMs,
  onSeekComplete,
}) => {
  const videoRef = useRef<HTMLVideoElement | null>(null);
  const containerRef = useRef<HTMLDivElement | null>(null);

  const [isPlaying, setIsPlaying] = useState(false);
  const [durationMs, setDurationMs] = useState(0);
  const [isMuted, setIsMuted] = useState(false);
  const [playbackRate, setPlaybackRate] = useState(1);
  const [showOutput, setShowOutput] = useState(false);
  const [loadError, setLoadError] = useState<string | null>(null);

  // Auto-switch to the exported output once a new export finishes
  useEffect(() => {
    if (outputPath) setShowOutput(true);
  }, [outputPath]);

  const activePath = showOutput && outputPath ? outputPath : videoPath;
  const outputFileName = outputPath ? outputPath.split(/[\\/]/).pop() : null;

  // Convert local filesystem path to Tauri Asset URL safely
  const videoSrc = React.useMemo(() => {
    if (!activePath) return '';
    if (isTauri()) {
      try {
        return convertFileSrc(activePath);
      } catch {
        return activePath;
      }
    }
    return activePath;
  }, [activePath]);

  // Reset error state whenever the effective source changes
  useEffect(() => {
    setLoadError(null);
  }, [videoSrc]);

  // Handle external seek request (e.g. from clicking a subtitle row)
  useEffect(() => {
    if (seekTimeMs !== null && videoRef.current) {
      videoRef.current.currentTime = seekTimeMs / 1000;
      onSeekComplete();
    }
  }, [seekTimeMs, onSeekComplete]);

  const togglePlay = useCallback(() => {
    if (!videoRef.current) return;
    if (isPlaying) {
      videoRef.current.pause();
    } else {
      videoRef.current.play().catch(console.error);
    }
  }, [isPlaying]);

  const handleTimeUpdate = () => {
    if (videoRef.current) {
      const ms = Math.floor(videoRef.current.currentTime * 1000);
      onTimeUpdate(ms);
    }
  };

  const handleLoadedMetadata = () => {
    if (videoRef.current) {
      setDurationMs(Math.floor(videoRef.current.duration * 1000));
    }
  };

  const handleSeek = (ms: number) => {
    if (videoRef.current) {
      videoRef.current.currentTime = ms / 1000;
      onTimeUpdate(ms);
    }
  };

  const skipSeconds = (sec: number) => {
    if (!videoRef.current) return;
    videoRef.current.currentTime = Math.max(0, videoRef.current.currentTime + sec);
  };

  const repeatCurrentSegment = () => {
    const active = segments.find(
      (s) => currentTimeMs >= s.start_ms && currentTimeMs <= s.end_ms
    );
    if (active && videoRef.current) {
      videoRef.current.currentTime = active.start_ms / 1000;
      if (!isPlaying) videoRef.current.play().catch(console.error);
    }
  };

  const changePlaybackRate = (rate: number) => {
    if (videoRef.current) {
      videoRef.current.playbackRate = rate;
      setPlaybackRate(rate);
    }
  };

  // Keyboard navigation
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      // Ignore if user is editing inside an input/textarea
      if (['INPUT', 'TEXTAREA'].includes((e.target as HTMLElement)?.tagName)) {
        return;
      }
      if (e.code === 'Space') {
        e.preventDefault();
        togglePlay();
      } else if (e.key === 'j' || e.key === 'J') {
        skipSeconds(-2);
      } else if (e.key === 'l' || e.key === 'L') {
        skipSeconds(2);
      } else if (e.key === 'r' || e.key === 'R') {
        repeatCurrentSegment();
      }
    };

    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [togglePlay, currentTimeMs, segments, isPlaying]);

  return (
    <div
      ref={containerRef}
      className="relative flex flex-col h-full w-full bg-black/95 rounded-lg overflow-hidden border border-slate-800 shadow-xl"
    >
      {/* Video Screen Area */}
      <div className="relative flex-1 flex items-center justify-center overflow-hidden bg-black group">
        {videoSrc ? (
          <video
            key={videoSrc}
            ref={videoRef}
            src={videoSrc}
            preload="auto"
            onTimeUpdate={handleTimeUpdate}
            onLoadedMetadata={handleLoadedMetadata}
            onPlay={() => setIsPlaying(true)}
            onPause={() => setIsPlaying(false)}
            onError={() => {
              const err = videoRef.current?.error;
              setLoadError(
                err
                  ? `Không phát được video (mã lỗi ${err.code}). Có thể thiếu codec GStreamer cho định dạng này.`
                  : 'Không phát được video.'
              );
            }}
            onClick={togglePlay}
            className="w-full h-full object-contain cursor-pointer"
          />
        ) : (
          <div className="text-slate-500 text-sm flex flex-col items-center gap-2">
            <RotateCcw className="w-8 h-8 opacity-40 animate-pulse" />
            <span>Kéo thả video vào đây hoặc chọn file trên thanh menu</span>
          </div>
        )}

        {/* Video load error banner */}
        {loadError && (
          <div className="absolute inset-x-0 top-0 z-10 bg-rose-950/90 border-b border-rose-800 px-4 py-2 text-[11px] text-rose-300 text-center">
            {loadError}
          </div>
        )}

        {/* Real-time Subtitle Overlay (hidden on exported output: subs already burned/embedded) */}
        {!showOutput && (
          <SubtitleOverlay
            currentTimeMs={currentTimeMs}
            segments={segments}
            preset={preset}
            isBilingual={isBilingual}
          />
        )}

        {/* Source / Output toggle + output path badge */}
        {outputPath && (
          <div className="absolute top-2 left-2 right-2 flex items-center justify-between gap-2 pointer-events-none">
            <div className="flex items-center bg-slate-900/85 border border-slate-700/70 rounded-full p-0.5 text-[11px] pointer-events-auto">
              <button
                onClick={() => setShowOutput(false)}
                className={`px-2.5 py-1 rounded-full transition ${
                  !showOutput ? 'bg-blue-600 text-white font-semibold' : 'text-slate-400 hover:text-white'
                }`}
              >
                Gốc
              </button>
              <button
                onClick={() => setShowOutput(true)}
                className={`px-2.5 py-1 rounded-full transition ${
                  showOutput ? 'bg-emerald-600 text-white font-semibold' : 'text-slate-400 hover:text-white'
                }`}
              >
                Kết quả
              </button>
            </div>
            {showOutput && (
              <div
                className="flex items-center gap-1.5 bg-emerald-950/85 border border-emerald-800/60 text-emerald-300 px-2.5 py-1 rounded-full text-[10px] font-mono truncate max-w-[60%]"
                title={outputPath}
              >
                <Film className="w-3 h-3 shrink-0" />
                <span className="truncate">{outputFileName}</span>
              </div>
            )}
          </div>
        )}
      </div>

      {/* Control Bar */}
      <div className="h-14 bg-slate-900/90 border-t border-slate-800 px-4 flex flex-col justify-center gap-1 text-slate-300">
        {/* Seek timeline bar with subtitle blocks */}
        <div className="flex items-center gap-2">
          <SeekBar
            durationMs={durationMs}
            segments={segments}
            currentTimeMs={currentTimeMs}
            onSeek={handleSeek}
          />
        </div>

        {/* Action Controls */}
        <div className="flex items-center justify-between text-xs">
          <div className="flex items-center gap-3">
            <button
              onClick={togglePlay}
              disabled={!videoSrc}
              className="p-1 text-white hover:text-blue-400 transition"
              title={isPlaying ? 'Tạm dừng (Space)' : 'Phát (Space)'}
            >
              {isPlaying ? <Pause className="w-4 h-4" /> : <Play className="w-4 h-4 fill-current" />}
            </button>

            <button
              onClick={() => skipSeconds(-2)}
              disabled={!videoSrc}
              className="p-1 hover:text-blue-400 transition"
              title="Tua lùi 2s (J)"
            >
              <Rewind className="w-3.5 h-3.5" />
            </button>

            <button
              onClick={() => skipSeconds(2)}
              disabled={!videoSrc}
              className="p-1 hover:text-blue-400 transition"
              title="Tua tới 2s (L)"
            >
              <FastForward className="w-3.5 h-3.5" />
            </button>

            <button
              onClick={repeatCurrentSegment}
              disabled={!videoSrc}
              className="p-1 hover:text-amber-400 transition"
              title="Lặp lại câu phụ đề đang phát (R)"
            >
              <Repeat className="w-3.5 h-3.5" />
            </button>

            <span className="font-mono text-[11px] text-slate-400">
              {formatTime(currentTimeMs)} / {formatTime(durationMs)}
            </span>
          </div>

          <div className="flex items-center gap-2">
            {/* Speed Selector */}
            <div className="flex items-center bg-slate-800 rounded px-1 text-[11px]">
              {[1, 1.25, 1.5, 2].map((rate) => (
                <button
                  key={rate}
                  onClick={() => changePlaybackRate(rate)}
                  className={`px-1.5 py-0.5 rounded transition ${
                    playbackRate === rate ? 'bg-blue-600 text-white font-bold' : 'hover:text-white'
                  }`}
                >
                  {rate}x
                </button>
              ))}
            </div>

            {/* Volume */}
            <button
              onClick={() => {
                if (videoRef.current) {
                  videoRef.current.muted = !isMuted;
                  setIsMuted(!isMuted);
                }
              }}
              className="p-1 hover:text-white"
            >
              {isMuted ? <VolumeX className="w-4 h-4" /> : <Volume2 className="w-4 h-4" />}
            </button>
          </div>
        </div>
      </div>
    </div>
  );
};
