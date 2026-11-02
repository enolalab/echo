import { describe, it, expect, vi } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import React from 'react';
import { HeaderBar } from './HeaderBar';

describe('HeaderBar', () => {
  it('renders Start button disabled when no video is selected', () => {
    render(
      <HeaderBar
        videoPath=""
        onSelectVideo={vi.fn()}
        whisperModel="base"
        onSelectWhisperModel={vi.fn()}
        profiles={[]}
        defaultProfileId={null}
        onSelectProfile={vi.fn()}
        onOpenSettings={vi.fn()}
        presets={[]}
        userSkills={[]}
        defaultSkillId={null}
        onSelectSkill={vi.fn()}
        targetLang="vi"
        onSelectTargetLang={vi.fn()}
        bilingual={true}
        onToggleBilingual={vi.fn()}
        stylePreset="youtube_yellow"
        onSelectPreset={vi.fn()}
        isRunning={false}
        onStart={vi.fn()}
        onCancel={vi.fn()}
        systemInfo={null}
        preferGpu={true}
        onToggleGpu={vi.fn()}
        onDownloadModel={vi.fn()}
        downloadingModel={null}
      />
    );
    const startBtn = screen.getByRole('button', { name: /Bắt Đầu/i }) as HTMLButtonElement;
    expect(startBtn.disabled).toBe(true);
  });

  it('renders Start button enabled when video is selected', () => {
    render(
      <HeaderBar
        videoPath="/path/to/movie.mp4"
        onSelectVideo={vi.fn()}
        whisperModel="base"
        onSelectWhisperModel={vi.fn()}
        profiles={[]}
        defaultProfileId={null}
        onSelectProfile={vi.fn()}
        onOpenSettings={vi.fn()}
        presets={[]}
        userSkills={[]}
        defaultSkillId={null}
        onSelectSkill={vi.fn()}
        targetLang="vi"
        onSelectTargetLang={vi.fn()}
        bilingual={true}
        onToggleBilingual={vi.fn()}
        stylePreset="youtube_yellow"
        onSelectPreset={vi.fn()}
        isRunning={false}
        onStart={vi.fn()}
        onCancel={vi.fn()}
        systemInfo={null}
        preferGpu={true}
        onToggleGpu={vi.fn()}
        onDownloadModel={vi.fn()}
        downloadingModel={null}
      />
    );
    const startBtn = screen.getByRole('button', { name: /Bắt Đầu/i }) as HTMLButtonElement;
    expect(startBtn.disabled).toBe(false);
    expect(screen.getByText('movie.mp4')).toBeDefined();
  });

  it('renders profiles, marks default, and reports selection and settings click', () => {
    const onSelectProfile = vi.fn();
    const onOpenSettings = vi.fn();
    render(
      <HeaderBar
        videoPath=""
        onSelectVideo={vi.fn()}
        whisperModel="base"
        onSelectWhisperModel={vi.fn()}
        profiles={[
          { id: 'o', name: 'Ollama local', kind: 'ollama', base_url: null, model: 'qwen2.5:7b', has_key: false },
          { id: 'd', name: 'DeepSeek', kind: 'deepseek', base_url: null, model: 'deepseek-chat', has_key: true },
        ]}
        defaultProfileId="o"
        onSelectProfile={onSelectProfile}
        onOpenSettings={onOpenSettings}
        presets={[]}
        userSkills={[]}
        defaultSkillId={null}
        onSelectSkill={vi.fn()}
        targetLang="vi"
        onSelectTargetLang={vi.fn()}
        bilingual={true}
        onToggleBilingual={vi.fn()}
        stylePreset="youtube_yellow"
        onSelectPreset={vi.fn()}
        isRunning={false}
        onStart={vi.fn()}
        onCancel={vi.fn()}
        systemInfo={null}
        preferGpu={true}
        onToggleGpu={vi.fn()}
        onDownloadModel={vi.fn()}
        downloadingModel={null}
      />
    );
    const select = screen.getByLabelText('Provider profile') as HTMLSelectElement;
    expect(select.value).toBe('o');
    expect(screen.getByRole('option', { name: /DeepSeek · deepseek-chat/ })).toBeDefined();
    fireEvent.change(select, { target: { value: 'd' } });
    expect(onSelectProfile).toHaveBeenCalledWith('d');
    fireEvent.click(screen.getByRole('button', { name: 'Cài đặt' }));
    expect(onOpenSettings).toHaveBeenCalled();
  });

  it('skill select lists none, presets and user skills', () => {
    const onSelectSkill = vi.fn();
    render(
      <HeaderBar
        videoPath=""
        onSelectVideo={vi.fn()}
        whisperModel="base"
        onSelectWhisperModel={vi.fn()}
        profiles={[]}
        defaultProfileId={null}
        onSelectProfile={vi.fn()}
        onOpenSettings={vi.fn()}
        presets={[{ id: 'builtin:math', name: 'Toán học', description: '', system_prompt: 'p', glossary: [] }]}
        userSkills={[{ id: 'u1', name: 'Của tôi', description: '', system_prompt: 'p', glossary: [] }]}
        defaultSkillId={null}
        onSelectSkill={onSelectSkill}
        targetLang="vi"
        onSelectTargetLang={vi.fn()}
        bilingual={true}
        onToggleBilingual={vi.fn()}
        stylePreset="youtube_yellow"
        onSelectPreset={vi.fn()}
        isRunning={false}
        onStart={vi.fn()}
        onCancel={vi.fn()}
        systemInfo={null}
        preferGpu={true}
        onToggleGpu={vi.fn()}
        onDownloadModel={vi.fn()}
        downloadingModel={null}
      />
    );
    const select = screen.getByLabelText('Skill') as HTMLSelectElement;
    expect(select.value).toBe('none');
    expect(Array.from(select.options).map((o) => o.value)).toEqual(['none', 'builtin:math', 'u1']);
    fireEvent.change(select, { target: { value: 'u1' } });
    expect(onSelectSkill).toHaveBeenCalledWith('u1');
    fireEvent.change(select, { target: { value: 'none' } });
    expect(onSelectSkill).toHaveBeenCalledWith(null);
  });

  it('renders YouTube import button and triggers onOpenYoutubeModal', () => {
    const onOpenYoutubeModal = vi.fn();
    render(
      <HeaderBar
        videoPath=""
        onSelectVideo={vi.fn()}
        onOpenYoutubeModal={onOpenYoutubeModal}
        whisperModel="base"
        onSelectWhisperModel={vi.fn()}
        profiles={[]}
        defaultProfileId={null}
        onSelectProfile={vi.fn()}
        onOpenSettings={vi.fn()}
        presets={[]}
        userSkills={[]}
        defaultSkillId={null}
        onSelectSkill={vi.fn()}
        targetLang="vi"
        onSelectTargetLang={vi.fn()}
        bilingual={true}
        onToggleBilingual={vi.fn()}
        stylePreset="youtube_yellow"
        onSelectPreset={vi.fn()}
        isRunning={false}
        onStart={vi.fn()}
        onCancel={vi.fn()}
        systemInfo={null}
        preferGpu={true}
        onToggleGpu={vi.fn()}
        onDownloadModel={vi.fn()}
        downloadingModel={null}
      />
    );
    const btn = screen.getByRole('button', { name: /nhập từ youtube/i });
    expect(btn).toBeTruthy();
    fireEvent.click(btn);
    expect(onOpenYoutubeModal).toHaveBeenCalled();
  });
});
