import type { BatchJob, SettingsView, Skill } from '../types';

export const NO_SKILL = 'none';
export const NO_SKILL_LABEL = 'Không dùng skill';

export const isBuiltinSkill = (s: Skill): boolean => s.id.startsWith('builtin:');

export const emptySkill = (): Skill => ({ id: '', name: '', description: '', system_prompt: '', glossary: [] });

export const allSkills = (v: SettingsView): Skill[] => [...v.presets, ...v.skills];

export function skillName(v: SettingsView | null, id: string | null | undefined): string | null {
  if (!v || !id) return null;
  return allSkills(v).find((s) => s.id === id)?.name ?? null;
}

export function jobSkillChoice(job: BatchJob, v: SettingsView | null): { skill_id: string | null; label: string } {
  if (job.skillId === NO_SKILL) return { skill_id: NO_SKILL, label: NO_SKILL_LABEL };
  if (job.skillId) return { skill_id: job.skillId, label: skillName(v, job.skillId) ?? job.skillId };
  return { skill_id: null, label: skillName(v, v?.default_skill_id) ?? NO_SKILL_LABEL };
}
