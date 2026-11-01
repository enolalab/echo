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
} from 'lucide-react';
import { convertFileSrc } from '@tauri-apps/api/core';
import type { SubtitleSegment, StylePreset } from '../types';
import { SubtitleOverlay } from './SubtitleOverlay';

interface VideoPlayerProps {
  videoPath: string;
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

  // Convert local filesystem path to Tauri Asset URL safely
  const videoSrc = React.useMemo(() => {
    if (!videoPath) return '';
    try {
      return convertFileSrc(videoPath);
    } catch {
      return videoPath;
    }
  }, [videoPath]);

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

  const handleSliderSeek = (e: React.ChangeEvent<HTMLInputElement>) => {
    const newMs = Number(e.target.value);
    if (videoRef.current) {
      videoRef.current.currentTime = newMs / 1000;
      onTimeUpdate(newMs);
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
            ref={videoRef}
            src={videoSrc}
            onTimeUpdate={handleTimeUpdate}
            onLoadedMetadata={handleLoadedMetadata}
            onPlay={() => setIsPlaying(true)}
            onPause={() => setIsPlaying(false)}
            onClick={togglePlay}
            className="w-full h-full object-contain cursor-pointer"
          />
        ) : (
          <div className="text-slate-500 text-sm flex flex-col items-center gap-2">
            <RotateCcw className="w-8 h-8 opacity-40 animate-pulse" />
            <span>Kéo thả video vào đây hoặc chọn file trên thanh menu</span>
          </div>
        )}

        {/* Real-time Subtitle Overlay */}
        <SubtitleOverlay
          currentTimeMs={currentTimeMs}
          segments={segments}
          preset={preset}
          isBilingual={isBilingual}
        />
      </div>

      {/* Control Bar */}
      <div className="h-14 bg-slate-900/90 border-t border-slate-800 px-4 flex flex-col justify-center gap-1 text-slate-300">
        {/* Seek timeline bar */}
        <div className="flex items-center gap-2">
          <input
            type="range"
            min={0}
            max={durationMs || 100}
            value={currentTimeMs}
            onChange={handleSliderSeek}
            disabled={!videoSrc}
            className="w-full h-1.5 bg-slate-700 rounded-lg appearance-none cursor-pointer accent-blue-500"
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
