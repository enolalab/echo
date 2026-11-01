import React from 'react';
import { FileVideo, Loader2, CheckCircle2, XCircle, Clock, X, Square, FolderOpen } from 'lucide-react';
import type { BatchJob } from '../types';

interface JobListProps {
  jobs: BatchJob[];
  selectedJobId: string | null;
  onSelect: (id: string) => void;
  onRemove: (id: string) => void;
  onCancelJob: (id: string) => void;
  onOpenFolder: (path: string) => void;
  skillOptions: { id: string; name: string }[];
  defaultSkillName: string | null;
  onChangeJobSkill: (jobId: string, skillId: string | undefined) => void;
}

function fileName(path: string): string {
  return path.split(/[/\\]/).pop() || path;
}

const EDITABLE: BatchJob['status'][] = ['queued', 'error', 'cancelled'];

const STATUS_LABEL: Record<BatchJob['status'], string> = {
  queued: 'Chờ xử lý',
  running: 'Đang chạy',
  done: 'Hoàn tất',
  error: 'Lỗi',
  cancelled: 'Đã hủy',
};

function StatusBadge({ job }: { job: BatchJob }) {
  switch (job.status) {
    case 'running':
      return (
        <span className="flex items-center gap-1 text-blue-400">
          <Loader2 className="w-3 h-3 animate-spin" />
          <span>{Math.round(job.progress?.percentage ?? 0)}%</span>
        </span>
      );
    case 'done':
      return (
        <span className="flex items-center gap-1 text-emerald-400">
          <CheckCircle2 className="w-3 h-3" />
          <span>{STATUS_LABEL.done}</span>
        </span>
      );
    case 'error':
      return (
        <span className="flex items-center gap-1 text-rose-400">
          <XCircle className="w-3 h-3" />
          <span>{STATUS_LABEL.error}</span>
        </span>
      );
    case 'cancelled':
      return (
        <span className="flex items-center gap-1 text-amber-400">
          <Square className="w-3 h-3" />
          <span>{STATUS_LABEL.cancelled}</span>
        </span>
      );
    default:
      return (
        <span className="flex items-center gap-1 text-slate-400">
          <Clock className="w-3 h-3" />
          <span>{STATUS_LABEL.queued}</span>
        </span>
      );
  }
}

export const JobList: React.FC<JobListProps> = ({
  jobs,
  selectedJobId,
  onSelect,
  onRemove,
  onCancelJob,
  onOpenFolder,
  skillOptions,
  defaultSkillName,
  onChangeJobSkill,
}) => {
  if (jobs.length === 0) {
    return (
      <div className="h-full flex flex-col items-center justify-center gap-2 text-slate-600 text-[11px]">
        <FileVideo className="w-8 h-8 opacity-30" />
        <span className="italic">Chưa có file nào trong danh sách. Kéo thả hoặc chọn video để bắt đầu.</span>
      </div>
    );
  }

  return (
    <div className="h-full overflow-y-auto flex flex-col gap-1 p-1.5">
      {jobs.map((job) => {
        const isSelected = job.id === selectedJobId;
        const pct = Math.round(job.progress?.percentage ?? 0);
        return (
          <div
            key={job.id}
            onClick={() => onSelect(job.id)}
            className={`group relative flex flex-col gap-1 px-3 py-2 rounded-md border cursor-pointer transition text-[11px] ${
              isSelected
                ? 'bg-blue-950/50 border-blue-600/70'
                : 'bg-slate-800/40 border-slate-700/50 hover:bg-slate-800/70'
            }`}
          >
            <div className="flex items-center gap-2 min-w-0">
              <FileVideo
                className={`w-3.5 h-3.5 shrink-0 ${isSelected ? 'text-blue-400' : 'text-slate-500'}`}
              />
              <span className={`truncate font-medium ${isSelected ? 'text-white' : 'text-slate-300'}`}>
                {fileName(job.videoPath)}
              </span>
              <span className="ml-auto shrink-0">
                <StatusBadge job={job} />
              </span>
              {job.status === 'running' ? (
                <button
                  onClick={(e) => {
                    e.stopPropagation();
                    onCancelJob(job.id);
                  }}
                  className="shrink-0 p-0.5 text-slate-500 hover:text-rose-400 transition"
                  title="Hủy file này"
                >
                  <Square className="w-3 h-3" />
                </button>
              ) : (
                <button
                  onClick={(e) => {
                    e.stopPropagation();
                    onRemove(job.id);
                  }}
                  className="shrink-0 p-0.5 text-slate-600 hover:text-rose-400 opacity-0 group-hover:opacity-100 transition"
                  title="Xóa khỏi danh sách"
                >
                  <X className="w-3 h-3" />
                </button>
              )}
            </div>

            {EDITABLE.includes(job.status) ? (
              <select
                aria-label="Skill cho file"
                value={job.skillId ?? ''}
                onClick={(e) => e.stopPropagation()}
                onChange={(e) => onChangeJobSkill(job.id, e.target.value === '' ? undefined : e.target.value)}
                className="self-start bg-slate-900/70 border border-slate-700 rounded px-1.5 py-0.5 text-[10px] text-slate-300 focus:outline-none"
              >
                <option value="">Mặc định ({defaultSkillName ?? 'không dùng'})</option>
                <option value="none">Không dùng skill</option>
                {skillOptions.map((s) => (
                  <option key={s.id} value={s.id}>{s.name}</option>
                ))}
              </select>
            ) : (
              job.skillUsed && <span className="text-slate-500 text-[10px]">🎓 {job.skillUsed}</span>
            )}

            {/* Progress bar for running job */}
            {job.status === 'running' && (
              <>
                <div className="w-full h-1 bg-slate-700/70 rounded-full overflow-hidden">
                  <div
                    className="h-full bg-blue-500 transition-all duration-300"
                    style={{ width: `${pct}%` }}
                  />
                </div>
                <span className="text-slate-500 truncate">{job.progress?.message}</span>
              </>
            )}

            {job.status === 'error' && job.error && (
              <span className="text-rose-400/80 truncate" title={job.error}>
                {job.error}
              </span>
            )}

            {/* Output result row */}
            {job.outputVideoPath && (
              <div className="flex items-center gap-1.5 text-emerald-400/90 min-w-0">
                <span className="truncate font-mono" title={job.outputVideoPath}>
                  → {fileName(job.outputVideoPath)}
                </span>
                <button
                  onClick={(e) => {
                    e.stopPropagation();
                    onOpenFolder(job.outputVideoPath!);
                  }}
                  className="shrink-0 p-0.5 text-slate-500 hover:text-emerald-300 transition"
                  title="Mở thư mục chứa file"
                >
                  <FolderOpen className="w-3 h-3" />
                </button>
              </div>
            )}

            {job.status === 'done' && !job.outputVideoPath && (
              <span className="text-slate-500">
                {job.segments.length} đoạn phụ đề — sẵn sàng xuất
              </span>
            )}
          </div>
        );
      })}
    </div>
  );
};
