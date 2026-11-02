import React, { useEffect, useState } from 'react';
import { X, RefreshCw, AlertTriangle } from 'lucide-react';
import type {
  ConnectionTest,
  GeneralSettings,
  ProfileView,
  ProviderProfile,
  SettingsView,
  StylePreset,
  WhisperModelInfo,
} from '../types';
import * as ipc from '../lib/ipc';
import { SkillsTab, type SkillsApi } from './SkillsTab';
import {
  KIND_ICONS,
  KIND_LABELS,
  PROVIDER_KINDS,
  defaultBaseUrl,
  emptyProfile,
  requiresApiKey,
} from '../lib/providers';

export interface SettingsApi {
  saveProfile: (p: ProviderProfile, apiKey: string | null) => Promise<SettingsView>;
  deleteProfile: (id: string) => Promise<SettingsView>;
  setDefaultProfile: (id: string) => Promise<SettingsView>;
  testProfile: (p: ProviderProfile, apiKey: string | null) => Promise<ConnectionTest>;
  listProfileModels: (p: ProviderProfile, apiKey: string | null) => Promise<string[]>;
  setSttKey: (key: string) => Promise<SettingsView>;
}

const defaultApi: SettingsApi = {
  saveProfile: ipc.saveProfile,
  deleteProfile: ipc.deleteProfile,
  setDefaultProfile: ipc.setDefaultProfile,
  testProfile: ipc.testProfile,
  listProfileModels: ipc.listProfileModels,
  setSttKey: ipc.setSttKey,
};

interface SettingsModalProps {
  isOpen: boolean;
  onClose: () => void;
  settings: SettingsView;
  onSettingsChange: (s: SettingsView) => void;
  general: GeneralSettings;
  onGeneralChange: (g: GeneralSettings) => void;
  whisperModels: WhisperModelInfo[];
  api?: SettingsApi;
  skillsApi?: SkillsApi;
}

const toProfile = ({ has_key: _hasKey, ...p }: ProfileView): ProviderProfile => p;

const inputCls =
  'w-full bg-slate-900 border border-slate-700 rounded px-2 py-1.5 text-xs text-slate-100 focus:outline-none focus:border-blue-500';

export const SettingsModal: React.FC<SettingsModalProps> = ({
  isOpen,
  onClose,
  settings,
  onSettingsChange,
  general,
  onGeneralChange,
  whisperModels,
  api = defaultApi,
  skillsApi,
}) => {
  const [tab, setTab] = useState<'providers' | 'general' | 'skills'>('providers');
  const [selectedId, setSelectedId] = useState<string | null>(settings.default_profile_id);
  const [form, setForm] = useState<ProviderProfile>(emptyProfile());
  const [apiKey, setApiKey] = useState('');
  const [models, setModels] = useState<string[]>([]);
  const [testResult, setTestResult] = useState<ConnectionTest | null>(null);
  const [testing, setTesting] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [sttKey, setSttKeyInput] = useState('');

  const selected = settings.profiles.find((p) => p.id === selectedId) ?? null;
  const isNew = selectedId === null;

  useEffect(() => {
    setForm(selected ? toProfile(selected) : emptyProfile());
    setApiKey('');
    setTestResult(null);
    setError(null);
    setModels([]);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [selectedId]);

  const refreshModels = async (profile: ProviderProfile = form) => {
    try {
      setModels(await api.listProfileModels(profile, apiKey.trim() ? apiKey : null));
    } catch {
      setModels([]);
    }
  };

  useEffect(() => {
    if (isOpen && selected) refreshModels(toProfile(selected));
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [isOpen, selectedId]);

  if (!isOpen) return null;

  const run = async (fn: () => Promise<SettingsView>) => {
    setError(null);
    try {
      onSettingsChange(await fn());
      return true;
    } catch (e) {
      setError(String(e));
      return false;
    }
  };

  const handleSave = async () => {
    const payload: ProviderProfile = {
      ...form,
      base_url: form.base_url && form.base_url.trim() ? form.base_url.trim() : null,
    };
    const ok = await run(() => api.saveProfile(payload, apiKey.trim() ? apiKey : null));
    if (ok) setApiKey('');
  };

  const handleDelete = async () => {
    if (!selected || !window.confirm(`Xóa profile "${selected.name}"?`)) return;
    if (await run(() => api.deleteProfile(selected.id))) setSelectedId(null);
  };

  const handleTest = async () => {
    setTesting(true);
    setTestResult(null);
    try {
      setTestResult(await api.testProfile(form, apiKey.trim() ? apiKey : null));
    } catch (e) {
      setTestResult({ ok: false, latency_ms: 0, error: String(e) });
    } finally {
      setTesting(false);
    }
  };

  const patchGeneral = (patch: Partial<GeneralSettings>) => onGeneralChange({ ...general, ...patch });

  return (
    <div className="fixed inset-0 z-50 bg-black/60 flex items-center justify-center p-4">
      <div className="w-full max-w-3xl max-h-[85vh] bg-slate-800 border border-slate-700 rounded-lg shadow-xl flex flex-col text-xs text-slate-200">
        <div className="flex items-center justify-between px-4 py-3 border-b border-slate-700">
          <div className="flex gap-2">
            <button
              onClick={() => setTab('providers')}
              className={`px-3 py-1 rounded ${tab === 'providers' ? 'bg-blue-600 text-white' : 'bg-slate-700'}`}
            >
              Providers
            </button>
            <button
              onClick={() => setTab('general')}
              className={`px-3 py-1 rounded ${tab === 'general' ? 'bg-blue-600 text-white' : 'bg-slate-700'}`}
            >
              Chung
            </button>
            <button
              onClick={() => setTab('skills')}
              className={`px-3 py-1 rounded ${tab === 'skills' ? 'bg-blue-600 text-white' : 'bg-slate-700'}`}
            >
              Skills
            </button>
          </div>
          <button onClick={onClose} aria-label="Đóng" className="text-slate-400 hover:text-white">
            <X className="w-4 h-4" />
          </button>
        </div>

        {settings.secrets_backend === 'file' && (
          <div className="mx-4 mt-3 flex items-center gap-2 bg-amber-900/40 border border-amber-700 text-amber-200 rounded px-3 py-2">
            <AlertTriangle className="w-4 h-4 shrink-0" />
            <span>Không tìm thấy keychain hệ thống — API key đang lưu dạng file</span>
          </div>
        )}

        {tab === 'skills' ? (
          <SkillsTab
            settings={settings}
            onSettingsChange={onSettingsChange}
            targetLang={general.target_lang}
            api={skillsApi}
          />
        ) : tab === 'providers' ? (
          <div className="flex-1 min-h-0 flex gap-3 p-4">
            <div className="w-48 shrink-0 flex flex-col gap-1 overflow-y-auto">
              {settings.profiles.map((p) => (
                <button
                  key={p.id}
                  onClick={() => setSelectedId(p.id)}
                  className={`text-left px-2 py-1.5 rounded truncate ${
                    p.id === selectedId ? 'bg-slate-700 text-white' : 'hover:bg-slate-700/50'
                  }`}
                  title={KIND_LABELS[p.kind]}
                >
                  <span className="mr-1">{KIND_ICONS[p.kind]}</span>
                  <span>{p.id === settings.default_profile_id ? `★ ${p.name}` : p.name}</span>
                </button>
              ))}
              <button
                onClick={() => setSelectedId(null)}
                className="mt-1 px-2 py-1.5 rounded border border-dashed border-slate-600 hover:bg-slate-700/50"
              >
                + Thêm
              </button>
            </div>

            <div className="flex-1 flex flex-col gap-2.5 overflow-y-auto">
              <label className="flex flex-col gap-1">
                <span>Loại</span>
                <select
                  aria-label="Loại"
                  value={form.kind}
                  onChange={(e) => setForm({ ...form, kind: e.target.value as ProviderProfile['kind'] })}
                  className={inputCls}
                >
                  {PROVIDER_KINDS.map((k) => (
                    <option key={k} value={k}>
                      {KIND_LABELS[k]}
                    </option>
                  ))}
                </select>
              </label>
              <label className="flex flex-col gap-1">
                <span>Tên</span>
                <input
                  aria-label="Tên"
                  value={form.name}
                  onChange={(e) => setForm({ ...form, name: e.target.value })}
                  className={inputCls}
                />
              </label>
              <label className="flex flex-col gap-1">
                <span>Base URL</span>
                <input
                  aria-label="Base URL"
                  value={form.base_url ?? ''}
                  placeholder={defaultBaseUrl(form.kind) ?? 'https://...'}
                  onChange={(e) => setForm({ ...form, base_url: e.target.value })}
                  className={inputCls}
                />
              </label>
              {requiresApiKey(form.kind) && (
                <div className="flex flex-col gap-1">
                  <span className="flex items-center justify-between">
                    <span>API key</span>
                    {selected?.has_key && (
                      <button
                        type="button"
                        onClick={() => run(() => api.saveProfile(form, ''))}
                        className="text-rose-400 hover:underline"
                      >
                        Xóa key
                      </button>
                    )}
                  </span>
                  <input
                    aria-label="API key"
                    type="password"
                    value={apiKey}
                    placeholder={selected?.has_key ? '••• đã lưu' : 'sk-...'}
                    onChange={(e) => setApiKey(e.target.value)}
                    className={inputCls}
                  />
                </div>
              )}
              <div className="flex flex-col gap-1">
                <span className="flex items-center justify-between">
                  <span>Model</span>
                  <button
                    type="button"
                    aria-label="Tải danh sách model"
                    onClick={() => refreshModels()}
                    className="text-slate-400 hover:text-white"
                  >
                    <RefreshCw className="w-3.5 h-3.5" />
                  </button>
                </span>
                <input
                  aria-label="Model"
                  list="model-suggestions"
                  value={form.model}
                  onChange={(e) => setForm({ ...form, model: e.target.value })}
                  className={inputCls}
                />
                <datalist id="model-suggestions">
                  {models.map((m) => (
                    <option key={m} value={m} />
                  ))}
                </datalist>
              </div>

              {testResult && (
                <div className={testResult.ok ? 'text-emerald-400' : 'text-rose-400'}>
                  {testResult.ok ? `✅ Kết nối OK (${testResult.latency_ms} ms)` : `❌ ${testResult.error}`}
                </div>
              )}
              {error && <div className="text-rose-400">{error}</div>}

              <div className="flex flex-wrap gap-2 pt-1">
                <button
                  onClick={handleTest}
                  disabled={testing}
                  className="px-3 py-1.5 rounded bg-slate-700 hover:bg-slate-600 disabled:opacity-50"
                >
                  {testing ? 'Đang test...' : 'Test kết nối'}
                </button>
                <button onClick={handleSave} className="px-3 py-1.5 rounded bg-blue-600 hover:bg-blue-500 text-white">
                  Lưu
                </button>
                {!isNew && selected && (
                  <>
                    {selected.id !== settings.default_profile_id && (
                      <button
                        onClick={() => run(() => api.setDefaultProfile(selected.id))}
                        className="px-3 py-1.5 rounded bg-slate-700 hover:bg-slate-600"
                      >
                        Đặt mặc định
                      </button>
                    )}
                    <button onClick={handleDelete} className="px-3 py-1.5 rounded bg-rose-700 hover:bg-rose-600 text-white">
                      Xóa
                    </button>
                  </>
                )}
              </div>
            </div>
          </div>
        ) : (
          <div className="flex-1 min-h-0 flex flex-col gap-3 p-4 overflow-y-auto">
            <label className="flex flex-col gap-1">
              <span>Whisper model</span>
              <select
                aria-label="Whisper model"
                value={general.whisper_model}
                onChange={(e) => patchGeneral({ whisper_model: e.target.value })}
                className={inputCls}
              >
                {whisperModels.map((m) => (
                  <option key={m.name} value={m.name}>
                    {m.name} ({m.size_mb}MB){m.downloaded ? ' ✓' : ''}
                  </option>
                ))}
              </select>
            </label>
            <label className="flex items-center gap-2">
              <input
                type="checkbox"
                checked={general.use_cloud_stt}
                onChange={(e) => patchGeneral({ use_cloud_stt: e.target.checked })}
              />
              <span>Dùng Whisper API (OpenAI) thay cho Whisper local</span>
            </label>
            {general.use_cloud_stt && (
              <div className="flex gap-2">
                <input
                  aria-label="API key Whisper"
                  type="password"
                  value={sttKey}
                  placeholder={settings.stt_has_key ? '••• đã lưu' : 'sk-...'}
                  onChange={(e) => setSttKeyInput(e.target.value)}
                  className={inputCls}
                />
                <button
                  onClick={async () => {
                    if (await run(() => api.setSttKey(sttKey))) setSttKeyInput('');
                  }}
                  className="px-3 py-1.5 rounded bg-blue-600 hover:bg-blue-500 text-white shrink-0"
                >
                  Lưu key
                </button>
              </div>
            )}
            <label className="flex flex-col gap-1">
              <span>Ngôn ngữ đích</span>
              <select
                aria-label="Ngôn ngữ đích"
                value={general.target_lang}
                onChange={(e) => patchGeneral({ target_lang: e.target.value })}
                className={inputCls}
              >
                <option value="vi">Tiếng Việt (VI)</option>
                <option value="en">English (EN)</option>
                <option value="ja">日本語 (JA)</option>
                <option value="ko">한국어 (KO)</option>
                <option value="zh">中文 (ZH)</option>
              </select>
            </label>
            <label className="flex items-center gap-2">
              <input
                type="checkbox"
                checked={general.bilingual}
                onChange={(e) => patchGeneral({ bilingual: e.target.checked })}
              />
              <span>Phụ đề song ngữ</span>
            </label>
            <label className="flex flex-col gap-1">
              <span>Thư mục lưu video tải về</span>
              <div className="flex gap-2">
                <input
                  aria-label="Thư mục lưu video tải về"
                  value={general.download_dir ?? ''}
                  placeholder="~/Videos/auto-subtitle/downloads"
                  onChange={(e) =>
                    patchGeneral({ download_dir: e.target.value.trim() || null })
                  }
                  className={inputCls}
                />
                <button
                  type="button"
                  onClick={async () => {
                    try {
                      const dir = await ipc.pickDirectory('Chọn thư mục lưu video YouTube');
                      if (dir) patchGeneral({ download_dir: dir });
                    } catch {
                      // Dialog unavailable — keep manual input
                    }
                  }}
                  className="px-3 py-1.5 rounded bg-slate-700 hover:bg-slate-600 shrink-0"
                >
                  Chọn...
                </button>
              </div>
            </label>
            <label className="flex flex-col gap-1">
              <span>Style mặc định</span>
              <select
                aria-label="Style mặc định"
                value={general.style_preset}
                onChange={(e) => patchGeneral({ style_preset: e.target.value as StylePreset })}
                className={inputCls}
              >
                <option value="youtube_yellow">YouTube Yellow</option>
                <option value="classic_white">Classic White</option>
                <option value="netflix_modern">Netflix Modern</option>
                <option value="bilingual_highlight">Bilingual Highlight</option>
              </select>
            </label>
            {error && <div className="text-rose-400">{error}</div>}
          </div>
        )}
      </div>
    </div>
  );
};
