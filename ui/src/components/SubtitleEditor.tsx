import React, { useState, useRef, useMemo } from 'react';
import { Search, Plus, Replace, FileText } from 'lucide-react';
import type { SubtitleSegment } from '../types';
import { SubtitleRow } from './SubtitleRow';
import { useSubtitleSync } from '../hooks/useSubtitleSync';

interface SubtitleEditorProps {
  segments: SubtitleSegment[];
  currentTimeMs: number;
  onSeek: (ms: number) => void;
  onUpdateSegment: (seg: SubtitleSegment) => void;
  onDeleteSegment: (id: number) => void;
  onSplitSegment: (id: number) => void;
  onMergeSegment: (id: number) => void;
  onAddSegment: () => void;
  onBatchReplace: (find: string, replaceWith: string) => void;
}

export const SubtitleEditor: React.FC<SubtitleEditorProps> = ({
  segments,
  currentTimeMs,
  onSeek,
  onUpdateSegment,
  onDeleteSegment,
  onSplitSegment,
  onMergeSegment,
  onAddSegment,
  onBatchReplace,
}) => {
  const containerRef = useRef<HTMLDivElement | null>(null);
  const { activeSegmentId } = useSubtitleSync(segments, currentTimeMs, containerRef);

  const [searchQuery, setSearchQuery] = useState('');
  const [showReplace, setShowReplace] = useState(false);
  const [replaceQuery, setReplaceQuery] = useState('');

  const filteredSegments = useMemo(() => {
    if (!searchQuery.trim()) return segments;
    const q = searchQuery.toLowerCase();
    return segments.filter(
      (s) =>
        s.original_text.toLowerCase().includes(q) ||
        (s.translated_text && s.translated_text.toLowerCase().includes(q))
    );
  }, [segments, searchQuery]);

  const handleApplyReplace = () => {
    if (!searchQuery.trim()) return;
    onBatchReplace(searchQuery, replaceQuery);
    setSearchQuery('');
    setReplaceQuery('');
    setShowReplace(false);
  };

  return (
    <div className="flex flex-col h-full bg-slate-900/90 border border-slate-800 rounded-lg overflow-hidden shadow-xl">
      {/* Search & Action Bar */}
      <div className="p-3 bg-slate-800/80 border-b border-slate-700/80 flex flex-col gap-2">
        <div className="flex items-center justify-between gap-2">
          {/* Search box */}
          <div className="relative flex-1">
            <Search className="w-3.5 h-3.5 absolute left-2.5 top-1/2 -translate-y-1/2 text-slate-400" />
            <input
              type="text"
              value={searchQuery}
              onChange={(e) => setSearchQuery(e.target.value)}
              placeholder="Tìm kiếm phụ đề..."
              className="w-full bg-slate-900 border border-slate-700 text-xs rounded-md pl-8 pr-2.5 py-1.5 text-slate-200 placeholder-slate-500 focus:outline-none focus:border-blue-500"
            />
          </div>

          <button
            onClick={() => setShowReplace(!showReplace)}
            className={`p-1.5 rounded-md border text-xs transition flex items-center gap-1 ${
              showReplace
                ? 'bg-blue-600/30 text-blue-300 border-blue-500/60'
                : 'bg-slate-700/60 text-slate-300 border-slate-600 hover:bg-slate-700'
            }`}
            title="Mở thanh Thay thế hàng loạt"
          >
            <Replace className="w-3.5 h-3.5" />
          </button>

          <button
            onClick={onAddSegment}
            className="flex items-center gap-1 bg-blue-600 hover:bg-blue-500 text-white font-medium text-xs px-2.5 py-1.5 rounded-md shadow transition"
            title="Thêm một dòng phụ đề mới"
          >
            <Plus className="w-3.5 h-3.5" />
            <span>Thêm</span>
          </button>
        </div>

        {/* Expandable Replace Bar */}
        {showReplace && (
          <div className="flex items-center gap-2 pt-1 border-t border-slate-700/60">
            <input
              type="text"
              value={replaceQuery}
              onChange={(e) => setReplaceQuery(e.target.value)}
              placeholder="Thay thế bằng..."
              className="flex-1 bg-slate-900 border border-slate-700 text-xs rounded-md px-2.5 py-1 text-slate-200 placeholder-slate-500 focus:outline-none focus:border-amber-400"
            />
            <button
              onClick={handleApplyReplace}
              disabled={!searchQuery.trim()}
              className="bg-amber-600 hover:bg-amber-500 disabled:opacity-50 text-white text-xs font-semibold px-3 py-1 rounded transition"
            >
              Thay tất cả
            </button>
          </div>
        )}
      </div>

      {/* Subtitle List Area */}
      <div
        ref={containerRef}
        data-testid="subtitle-list"
        className="flex-1 overflow-y-auto p-2.5 flex flex-col gap-2"
      >
        {filteredSegments.length > 0 ? (
          filteredSegments.map((seg, idx) => (
            <SubtitleRow
              key={seg.id}
              segment={seg}
              isActive={seg.id === activeSegmentId}
              onSeek={onSeek}
              onUpdate={onUpdateSegment}
              onDelete={onDeleteSegment}
              onSplit={onSplitSegment}
              onMergeWithNext={onMergeSegment}
              isLast={idx === filteredSegments.length - 1}
            />
          ))
        ) : (
          <div className="flex-1 flex flex-col items-center justify-center text-slate-500 text-xs py-16 gap-2">
            <FileText className="w-8 h-8 opacity-30" />
            <span>{segments.length === 0 ? 'Chưa có phụ đề. Hãy bấm "Bắt Đầu" để sinh tự động.' : 'Không tìm thấy dòng phụ đề phù hợp.'}</span>
          </div>
        )}
      </div>

      {/* Footer Status */}
      <div className="h-7 bg-slate-800/60 border-t border-slate-800 px-3 flex items-center justify-between text-[11px] text-slate-400">
        <span>Tổng cộng: {segments.length} đoạn</span>
        {activeSegmentId && <span>Đang phát dòng #{activeSegmentId}</span>}
      </div>
    </div>
  );
};
