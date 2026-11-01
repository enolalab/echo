import { describe, it, expect } from 'vitest';
import { jobSkillChoice, skillName, allSkills, isBuiltinSkill, emptySkill } from './skills';
import type { BatchJob, SettingsView } from '../types';

const view = {
  presets: [{ id: 'builtin:math', name: 'Toán học', description: '', system_prompt: 'p', glossary: [] }],
  skills: [{ id: 'u1', name: 'Của tôi', description: '', system_prompt: 'p', glossary: [] }],
  default_skill_id: 'builtin:math',
} as unknown as SettingsView;

const job = (skillId?: string) =>
  ({ id: 'j', videoPath: 'v', status: 'queued', progress: null, segments: [], outputVideoPath: null, error: null, skillId }) as BatchJob;

describe('skill helpers', () => {
  it('maps job override to skill_id and label', () => {
    expect(jobSkillChoice(job(), view)).toEqual({ skill_id: null, label: 'Toán học' });
    expect(jobSkillChoice(job('none'), view)).toEqual({ skill_id: 'none', label: 'Không dùng skill' });
    expect(jobSkillChoice(job('u1'), view)).toEqual({ skill_id: 'u1', label: 'Của tôi' });
    expect(jobSkillChoice(job(), { ...view, default_skill_id: null })).toEqual({ skill_id: null, label: 'Không dùng skill' });
    expect(jobSkillChoice(job(), null)).toEqual({ skill_id: null, label: 'Không dùng skill' });
  });

  it('lookups', () => {
    expect(allSkills(view).map((s) => s.id)).toEqual(['builtin:math', 'u1']);
    expect(skillName(view, 'u1')).toBe('Của tôi');
    expect(skillName(view, 'gone')).toBeNull();
    expect(isBuiltinSkill(view.presets[0])).toBe(true);
    expect(emptySkill()).toEqual({ id: '', name: '', description: '', system_prompt: '', glossary: [] });
  });
});
