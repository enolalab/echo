import { describe, it, expect } from 'vitest';
import { parseGlossaryCsv, mergeGlossary } from './csv';

describe('parseGlossaryCsv', () => {
  it('parses rows, skips header and bad rows', () => {
    const r = parseGlossaryCsv('source,target\nrelu,\n"gradient, descent","hạ ""gradient"""\n,orphan\na,b,c\n\nsoftmax');
    expect(r.entries).toEqual([
      { source: 'relu', target: '' },
      { source: 'gradient, descent', target: 'hạ "gradient"' },
      { source: 'softmax', target: '' },
    ]);
    expect(r.skipped).toBe(2);
  });

  it('detects semicolon delimiter and Vietnamese header', () => {
    const r = parseGlossaryCsv('Gốc;Dịch\r\nđạo hàm;derivative\r\n');
    expect(r.entries).toEqual([{ source: 'đạo hàm', target: 'derivative' }]);
    expect(r.skipped).toBe(0);
  });

  it('keeps quoted newlines inside a cell', () => {
    const r = parseGlossaryCsv('"a\nb",c');
    expect(r.entries).toEqual([{ source: 'a\nb', target: 'c' }]);
  });
});

describe('mergeGlossary', () => {
  it('adds new and overwrites case-insensitively', () => {
    const r = mergeGlossary(
      [{ source: 'ReLU', target: 'x' }, { source: 'loss', target: 'mất mát' }],
      [{ source: 'relu', target: '' }, { source: 'epoch', target: '' }, { source: 'EPOCH', target: 'kỷ' }]
    );
    expect(r.glossary).toEqual([
      { source: 'relu', target: '' },
      { source: 'loss', target: 'mất mát' },
      { source: 'EPOCH', target: 'kỷ' },
    ]);
    expect(r.added).toBe(1);
    expect(r.updated).toBe(2);
  });
});
