import type { GlossaryEntry } from '../types';

function detectDelimiter(text: string): ',' | ';' {
  const first = text.split(/\r?\n/).find((l) => l.trim() !== '') ?? '';
  const count = (ch: string) => first.split(ch).length - 1;
  return count(';') > count(',') ? ';' : ',';
}

function parseRows(text: string, delim: string): string[][] {
  const rows: string[][] = [];
  let row: string[] = [];
  let cell = '';
  let inQuotes = false;
  for (let i = 0; i < text.length; i++) {
    const ch = text[i];
    if (inQuotes) {
      if (ch === '"' && text[i + 1] === '"') {
        cell += '"';
        i++;
      } else if (ch === '"') {
        inQuotes = false;
      } else {
        cell += ch;
      }
    } else if (ch === '"') {
      inQuotes = true;
    } else if (ch === delim) {
      row.push(cell);
      cell = '';
    } else if (ch === '\n' || ch === '\r') {
      if (ch === '\r' && text[i + 1] === '\n') i++;
      row.push(cell);
      rows.push(row);
      row = [];
      cell = '';
    } else {
      cell += ch;
    }
  }
  row.push(cell);
  rows.push(row);
  return rows;
}

const HEADERS = [
  ['source', 'target'],
  ['gốc', 'dịch'],
];

export function parseGlossaryCsv(text: string): { entries: GlossaryEntry[]; skipped: number } {
  const rows = parseRows(text, detectDelimiter(text))
    .map((r) => r.map((c) => c.trim()))
    .filter((r) => r.some((c) => c !== ''));
  if (rows.length > 0) {
    const head = rows[0].map((c) => c.toLowerCase());
    if (HEADERS.some((h) => h.length === head.length && h.every((v, i) => v === head[i]))) rows.shift();
  }
  const entries: GlossaryEntry[] = [];
  let skipped = 0;
  for (const r of rows) {
    if (r.length > 2 || !r[0]) {
      skipped++;
      continue;
    }
    entries.push({ source: r[0], target: r[1] ?? '' });
  }
  return { entries, skipped };
}

export function mergeGlossary(
  existing: GlossaryEntry[],
  incoming: GlossaryEntry[]
): { glossary: GlossaryEntry[]; added: number; updated: number } {
  const glossary = existing.map((e) => ({ ...e }));
  let added = 0;
  let updated = 0;
  for (const inc of incoming) {
    const idx = glossary.findIndex((e) => e.source.toLowerCase() === inc.source.toLowerCase());
    if (idx >= 0) {
      glossary[idx] = { ...inc };
      updated++;
    } else {
      glossary.push({ ...inc });
      added++;
    }
  }
  return { glossary, added, updated };
}
