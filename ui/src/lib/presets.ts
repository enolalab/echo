import type { StylePreset } from '../types';

export interface PresetStyleDef {
  name: string;
  containerClass: string;
  primaryClass: string;
  secondaryClass: string;
}

export const PRESET_STYLES: Record<StylePreset, PresetStyleDef> = {
  youtube_yellow: {
    name: 'YouTube Yellow',
    containerClass: 'bottom-8 px-4 text-center',
    primaryClass:
      'font-extrabold text-[#FFDE00] drop-shadow-[0_2px_4px_rgba(0,0,0,0.95)] tracking-wide [text-shadow:_2px_2px_0_#000,_-2px_-2px_0_#000,_2px_-2px_0_#000,_-2px_2px_0_#000]',
    secondaryClass:
      'font-medium text-slate-200 text-sm mt-1 [text-shadow:_1px_1px_0_#000,_-1px_-1px_0_#000]',
  },
  classic_white: {
    name: 'Classic White',
    containerClass: 'bottom-8 px-4 text-center',
    primaryClass:
      'font-bold text-white drop-shadow-[0_2px_3px_rgba(0,0,0,0.9)] [text-shadow:_1.5px_1.5px_0_#000,_-1.5px_-1.5px_0_#000]',
    secondaryClass:
      'font-normal text-slate-300 text-sm mt-1 [text-shadow:_1px_1px_0_#000,_-1px_-1px_0_#000]',
  },
  netflix_modern: {
    name: 'Netflix Modern',
    containerClass: 'bottom-10 px-4 text-center',
    primaryClass:
      'font-semibold text-slate-100 bg-black/75 px-3 py-1 rounded inline-block shadow-lg',
    secondaryClass:
      'font-normal text-slate-300 text-sm mt-1 bg-black/60 px-2 py-0.5 rounded inline-block',
  },
  bilingual_highlight: {
    name: 'Bilingual Highlight',
    containerClass: 'bottom-8 px-4 text-center',
    primaryClass:
      'font-bold text-cyan-300 drop-shadow-[0_2px_4px_rgba(0,0,0,0.95)] [text-shadow:_2px_2px_0_#000,_-2px_-2px_0_#000]',
    secondaryClass:
      'font-semibold text-white text-sm mt-1 [text-shadow:_1px_1px_0_#000,_-1px_-1px_0_#000]',
  },
};
