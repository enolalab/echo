import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import React from 'react';
import { SkillsTab, type SkillsApi } from './SkillsTab';
import type { SettingsView } from '../types';

const view = (over: Partial<SettingsView> = {}): SettingsView =>
  ({
    profiles: [{ id: 'p', name: 'DeepSeek', kind: 'deepseek', base_url: null, model: 'm', has_key: true }],
    default_profile_id: 'p',
    general: { whisper_model: 'base', target_lang: 'vi', bilingual: true, style_preset: 'youtube_yellow', use_cloud_stt: false },
    stt_has_key: false,
    secrets_backend: 'keyring',
    load_warning: null,
    presets: [{ id: 'builtin:math', name: 'Toán học', description: 'd', system_prompt: 'Math', glossary: [{ source: 'limit', target: 'giới hạn' }] }],
    skills: [{ id: 'u1', name: 'Của tôi', description: '', system_prompt: 'Mine', glossary: [{ source: 'a', target: 'b' }] }],
    default_skill_id: 'builtin:math',
    ...over,
  }) as SettingsView;

let api: SkillsApi;
beforeEach(() => {
  api = {
    saveSkill: vi.fn().mockResolvedValue(view()),
    deleteSkill: vi.fn().mockResolvedValue(view()),
    duplicateSkill: vi.fn().mockResolvedValue(view()),
    setDefaultSkill: vi.fn().mockResolvedValue(view()),
    importSkill: vi.fn().mockResolvedValue(view()),
    exportSkill: vi.fn().mockResolvedValue('/tmp/x.echo-skill.json'),
    trySkill: vi.fn().mockResolvedValue({ lines: ['Xin chào'], system_prompt: 'Mine\n\nGlossary…', error: null }),
  };
});

const pick = (name: RegExp) => fireEvent.click(screen.getByRole('button', { name }));
const renderTab = () => render(<SkillsTab settings={view()} onSettingsChange={vi.fn()} targetLang="vi" api={api} />);

describe('SkillsTab', () => {
  it('preset is read-only with limited actions', () => {
    renderTab();
    pick(/Toán học/);
    expect((screen.getByLabelText('Tên skill') as HTMLInputElement).readOnly).toBe(true);
    expect(screen.getByRole('button', { name: 'Nhân bản' })).toBeDefined();
    expect(screen.getByRole('button', { name: 'Xuất file' })).toBeDefined();
    expect(screen.queryByRole('button', { name: 'Lưu' })).toBeNull();
    expect(screen.queryByRole('button', { name: 'Xóa' })).toBeNull();
  });

  it('saves a user skill with trimmed payload', async () => {
    renderTab();
    pick(/Của tôi/);
    fireEvent.change(screen.getByLabelText('Tên skill'), { target: { value: '  Mine 2 ' } });
    fireEvent.change(screen.getByLabelText('Dịch 1'), { target: { value: ' bb ' } });
    fireEvent.click(screen.getByRole('button', { name: 'Lưu' }));
    await waitFor(() =>
      expect(api.saveSkill).toHaveBeenCalledWith({
        id: 'u1',
        name: 'Mine 2',
        description: '',
        system_prompt: 'Mine',
        glossary: [{ source: 'a', target: 'bb' }],
      })
    );
  });

  it('adds and removes glossary rows', () => {
    renderTab();
    pick(/Của tôi/);
    fireEvent.click(screen.getByRole('button', { name: '+ Thêm thuật ngữ' }));
    expect(screen.getByLabelText('Gốc 2')).toBeDefined();
    fireEvent.click(screen.getByRole('button', { name: 'Xóa thuật ngữ 1' }));
    expect(screen.queryByLabelText('Gốc 2')).toBeNull();
  });

  it('imports CSV into the form and reports counts', () => {
    renderTab();
    pick(/Của tôi/);
    fireEvent.click(screen.getByRole('button', { name: 'Nhập CSV' }));
    fireEvent.change(screen.getByLabelText('Nội dung CSV'), { target: { value: 'source,target\nA,x\nnew,y\n,bad' } });
    fireEvent.click(screen.getByRole('button', { name: 'Áp dụng CSV' }));
    expect(screen.getByText('Đã thêm 1, cập nhật 1, bỏ qua 1 dòng lỗi')).toBeDefined();
    expect((screen.getByLabelText('Gốc 2') as HTMLInputElement).value).toBe('new');
  });

  it('try panel shows translated lines and prompt', async () => {
    renderTab();
    pick(/Của tôi/);
    fireEvent.change(screen.getByLabelText('Câu mẫu'), { target: { value: 'Hello' } });
    fireEvent.click(screen.getByRole('button', { name: 'Dịch thử' }));
    expect(await screen.findByText('Xin chào')).toBeDefined();
    expect(api.trySkill).toHaveBeenCalledWith(expect.objectContaining({ id: 'u1' }), 'p', 'Hello', 'auto', 'vi');
    fireEvent.click(screen.getByText('Xem prompt'));
    expect(screen.getByText(/Glossary…/)).toBeDefined();
  });

  it('marks the default skill and sets default', async () => {
    renderTab();
    expect(screen.getByRole('button', { name: /★ Toán học/ })).toBeDefined();
    pick(/Của tôi/);
    fireEvent.click(screen.getByRole('button', { name: 'Đặt mặc định' }));
    await waitFor(() => expect(api.setDefaultSkill).toHaveBeenCalledWith('u1'));
  });
});
