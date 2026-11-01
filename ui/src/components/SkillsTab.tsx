import React, { useEffect, useState } from 'react';
import type { GlossaryEntry, SettingsView, Skill, TrySkillResult } from '../types';
import * as ipc from '../lib/ipc';
import { allSkills, emptySkill, isBuiltinSkill } from '../lib/skills';
import { mergeGlossary, parseGlossaryCsv } from '../lib/csv';

export interface SkillsApi {
  saveSkill: (s: Skill) => Promise<SettingsView>;
  deleteSkill: (id: string) => Promise<SettingsView>;
  duplicateSkill: (id: string) => Promise<SettingsView>;
  setDefaultSkill: (id: string | null) => Promise<SettingsView>;
  importSkill: () => Promise<SettingsView>;
  exportSkill: (id: string) => Promise<string | null>;
  trySkill: (s: Skill, profileId: string | null, sample: string, sourceLang: string, targetLang: string) => Promise<TrySkillResult>;
}

const defaultApi: SkillsApi = {
  saveSkill: ipc.saveSkill,
  deleteSkill: ipc.deleteSkill,
  duplicateSkill: ipc.duplicateSkill,
  setDefaultSkill: ipc.setDefaultSkill,
  importSkill: ipc.importSkill,
  exportSkill: ipc.exportSkill,
  trySkill: ipc.trySkill,
};

interface SkillsTabProps {
  settings: SettingsView;
  onSettingsChange: (v: SettingsView) => void;
  targetLang: string;
  api?: SkillsApi;
}

const inputCls =
  'w-full bg-slate-900 border border-slate-700 rounded px-2 py-1.5 text-xs text-slate-100 focus:outline-none focus:border-blue-500 read-only:opacity-70';
const btnCls = 'px-3 py-1.5 rounded bg-slate-700 hover:bg-slate-600';

const trimmed = (s: Skill): Skill => ({
  ...s,
  name: s.name.trim(),
  description: s.description.trim(),
  system_prompt: s.system_prompt.trim(),
  glossary: s.glossary.map((g) => ({ source: g.source.trim(), target: g.target.trim() })),
});

export const SkillsTab: React.FC<SkillsTabProps> = ({ settings, onSettingsChange, targetLang, api = defaultApi }) => {
  const all = allSkills(settings);
  const [selectedId, setSelectedId] = useState<string | null>(settings.default_skill_id ?? all[0]?.id ?? null);
  const [form, setForm] = useState<Skill>(emptySkill());
  const [error, setError] = useState<string | null>(null);
  const [info, setInfo] = useState<string | null>(null);
  const [csvOpen, setCsvOpen] = useState(false);
  const [csvText, setCsvText] = useState('');
  const [sample, setSample] = useState('');
  const [tryProfileId, setTryProfileId] = useState<string | null>(settings.default_profile_id);
  const [tryResult, setTryResult] = useState<TrySkillResult | null>(null);
  const [trying, setTrying] = useState(false);
  const [showPrompt, setShowPrompt] = useState(false);

  const selected = all.find((s) => s.id === selectedId) ?? null;
  const readOnly = selected ? isBuiltinSkill(selected) : false;

  useEffect(() => {
    setForm(selected ? JSON.parse(JSON.stringify(selected)) : emptySkill());
    setError(null);
    setInfo(null);
    setTryResult(null);
    setCsvOpen(false);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [selectedId]);

  const run = async (fn: () => Promise<SettingsView>): Promise<SettingsView | null> => {
    setError(null);
    try {
      const v = await fn();
      onSettingsChange(v);
      return v;
    } catch (e) {
      setError(String(e));
      return null;
    }
  };

  const setGlossary = (glossary: GlossaryEntry[]) => setForm((f) => ({ ...f, glossary }));

  const handleSave = async () => {
    const payload = trimmed(form);
    const v = await run(() => api.saveSkill(payload));
    if (v && !payload.id) {
      const created = v.skills.find((s) => s.name === payload.name);
      if (created) setSelectedId(created.id);
    }
  };

  const handleDuplicate = async () => {
    if (!selected) return;
    const v = await run(() => api.duplicateSkill(selected.id));
    if (v && v.skills.length > 0) setSelectedId(v.skills[v.skills.length - 1].id);
  };

  const handleDelete = async () => {
    if (!selected || !window.confirm(`Xóa skill "${selected.name}"?`)) return;
    if (await run(() => api.deleteSkill(selected.id))) setSelectedId(null);
  };

  const handleExport = async () => {
    if (!selected) return;
    try {
      const path = await api.exportSkill(selected.id);
      if (path) setInfo(`Đã xuất: ${path}`);
    } catch (e) {
      setError(String(e));
    }
  };

  const handleImport = async () => {
    const before = settings.skills.length;
    const v = await run(() => api.importSkill());
    if (v && v.skills.length > before) setSelectedId(v.skills[v.skills.length - 1].id);
  };

  const applyCsv = () => {
    const { entries, skipped } = parseGlossaryCsv(csvText);
    const { glossary, added, updated } = mergeGlossary(form.glossary, entries);
    setGlossary(glossary);
    setInfo(`Đã thêm ${added}, cập nhật ${updated}, bỏ qua ${skipped} dòng lỗi`);
    setCsvOpen(false);
    setCsvText('');
  };

  const readCsvFile = (file: File) => {
    const reader = new FileReader();
    reader.onload = () => setCsvText(String(reader.result ?? ''));
    reader.readAsText(file);
  };

  const handleTry = async () => {
    setTrying(true);
    setTryResult(null);
    try {
      setTryResult(await api.trySkill(trimmed(form), tryProfileId, sample, 'auto', targetLang));
    } catch (e) {
      setTryResult({ lines: [], system_prompt: '', error: String(e) });
    } finally {
      setTrying(false);
    }
  };

  const sampleLines = sample.split('\n').map((l) => l.trim()).filter(Boolean);

  const listButton = (s: Skill) => (
    <button
      key={s.id}
      onClick={() => setSelectedId(s.id)}
      className={`text-left px-2 py-1.5 rounded truncate ${s.id === selectedId ? 'bg-slate-700 text-white' : 'hover:bg-slate-700/50'}`}
      title={s.description}
    >
      {isBuiltinSkill(s) && <span className="mr-1">🔒</span>}
      <span>{s.id === settings.default_skill_id ? `★ ${s.name}` : s.name}</span>
    </button>
  );

  return (
    <div className="flex-1 min-h-0 flex gap-3 p-4 text-xs">
      <div className="w-52 shrink-0 flex flex-col gap-1 overflow-y-auto">
        <div className="text-[10px] uppercase text-slate-500 mt-1">Có sẵn</div>
        {settings.presets.map(listButton)}
        <div className="text-[10px] uppercase text-slate-500 mt-2">Của tôi</div>
        {settings.skills.map(listButton)}
        <button onClick={() => setSelectedId(null)} className="mt-1 px-2 py-1.5 rounded border border-dashed border-slate-600 hover:bg-slate-700/50">
          + Thêm skill
        </button>
        <button onClick={handleImport} className="px-2 py-1.5 rounded border border-slate-600 hover:bg-slate-700/50">
          Nhập file
        </button>
      </div>

      <div className="flex-1 flex flex-col gap-2.5 overflow-y-auto pr-1">
        <input aria-label="Tên skill" placeholder="Tên" value={form.name} readOnly={readOnly}
          onChange={(e) => setForm({ ...form, name: e.target.value })} className={inputCls} />
        <input aria-label="Mô tả" placeholder="Mô tả ngắn" value={form.description} readOnly={readOnly}
          onChange={(e) => setForm({ ...form, description: e.target.value })} className={inputCls} />
        <textarea aria-label="System prompt" rows={6} value={form.system_prompt} readOnly={readOnly}
          onChange={(e) => setForm({ ...form, system_prompt: e.target.value })} className={`${inputCls} font-mono`} />
        <span className="text-slate-500">Có thể dùng {'{source_lang}'} và {'{target_lang}'}</span>

        <div className="flex items-center justify-between">
          <span className="font-semibold">Glossary ({form.glossary.length})</span>
          {!readOnly && (
            <div className="flex gap-2">
              <button onClick={() => setGlossary([...form.glossary, { source: '', target: '' }])} className={btnCls}>
                + Thêm thuật ngữ
              </button>
              <button onClick={() => setCsvOpen((o) => !o)} className={btnCls}>Nhập CSV</button>
            </div>
          )}
        </div>

        {csvOpen && (
          <div className="flex flex-col gap-2 border border-slate-700 rounded p-2">
            <textarea aria-label="Nội dung CSV" rows={4} value={csvText} placeholder="gốc,dịch"
              onChange={(e) => setCsvText(e.target.value)} className={`${inputCls} font-mono`} />
            <div className="flex gap-2 items-center">
              <input type="file" accept=".csv,text/csv" aria-label="File CSV"
                onChange={(e) => e.target.files?.[0] && readCsvFile(e.target.files[0])} />
              <button onClick={applyCsv} className="px-3 py-1.5 rounded bg-blue-600 hover:bg-blue-500 text-white">Áp dụng CSV</button>
            </div>
          </div>
        )}

        <div className="flex flex-col gap-1 max-h-56 overflow-y-auto">
          {form.glossary.map((g, i) => (
            <div key={i} className="flex gap-1.5 items-center">
              <input aria-label={`Gốc ${i + 1}`} value={g.source} readOnly={readOnly} className={inputCls}
                onChange={(e) => setGlossary(form.glossary.map((x, j) => (j === i ? { ...x, source: e.target.value } : x)))} />
              <span className="text-slate-500">→</span>
              <input aria-label={`Dịch ${i + 1}`} value={g.target} readOnly={readOnly} placeholder="(giữ nguyên)" className={inputCls}
                onChange={(e) => setGlossary(form.glossary.map((x, j) => (j === i ? { ...x, target: e.target.value } : x)))} />
              {!readOnly && (
                <button aria-label={`Xóa thuật ngữ ${i + 1}`} className="text-rose-400 px-1"
                  onClick={() => setGlossary(form.glossary.filter((_, j) => j !== i))}>×</button>
              )}
            </div>
          ))}
        </div>

        {info && <div className="text-emerald-400">{info}</div>}
        {error && <div className="text-rose-400">{error}</div>}

        <div className="flex flex-wrap gap-2">
          {!readOnly && (
            <button onClick={handleSave} className="px-3 py-1.5 rounded bg-blue-600 hover:bg-blue-500 text-white">Lưu</button>
          )}
          {selected && (
            <>
              <button onClick={handleDuplicate} className={btnCls}>Nhân bản</button>
              <button onClick={handleExport} className={btnCls}>Xuất file</button>
              {selected.id !== settings.default_skill_id && (
                <button onClick={() => run(() => api.setDefaultSkill(selected.id))} className={btnCls}>Đặt mặc định</button>
              )}
              {!readOnly && (
                <button onClick={handleDelete} className="px-3 py-1.5 rounded bg-rose-700 hover:bg-rose-600 text-white">Xóa</button>
              )}
            </>
          )}
        </div>

        <div className="border-t border-slate-700 pt-2 flex flex-col gap-2">
          <span className="font-semibold">Chạy thử</span>
          <textarea aria-label="Câu mẫu" rows={3} value={sample} placeholder="Mỗi dòng một câu (tối đa 20)"
            onChange={(e) => setSample(e.target.value)} className={inputCls} />
          <div className="flex gap-2">
            <select aria-label="Profile chạy thử" value={tryProfileId ?? ''} onChange={(e) => setTryProfileId(e.target.value || null)} className={inputCls}>
              {settings.profiles.map((p) => (
                <option key={p.id} value={p.id}>{p.name}</option>
              ))}
            </select>
            <button onClick={handleTry} disabled={trying} className="px-3 py-1.5 rounded bg-blue-600 hover:bg-blue-500 text-white shrink-0 disabled:opacity-50">
              {trying ? 'Đang dịch...' : 'Dịch thử'}
            </button>
          </div>
          {tryResult?.error && <div className="text-rose-400">{tryResult.error}</div>}
          {tryResult && tryResult.lines.length > 0 && (
            <table className="w-full text-left">
              <tbody>
                {tryResult.lines.map((l, i) => (
                  <tr key={i} className="border-b border-slate-800">
                    <td className="py-1 pr-2 text-slate-400 w-1/2">{sampleLines[i]}</td>
                    <td className="py-1">{l}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          )}
          {tryResult && tryResult.system_prompt && (
            <div>
              <button onClick={() => setShowPrompt((s) => !s)} className="text-blue-400 hover:underline">Xem prompt</button>
              {showPrompt && (
                <pre className="mt-1 whitespace-pre-wrap bg-slate-900 border border-slate-700 rounded p-2 font-mono text-[11px]">
                  {tryResult.system_prompt}
                </pre>
              )}
            </div>
          )}
        </div>
      </div>
    </div>
  );
};
