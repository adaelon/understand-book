export interface ReaderTypographyPreferences {
  font: 'serif' | 'sans';
  languageMode: 'cjk-mixed' | 'latin';
  fontSizePx: number;
  lineHeight: number;
  cjkLetterSpacingEm: number;
  measure: 'narrow' | 'standard' | 'wide';
}

export const typographyPresets: Record<string, ReaderTypographyPreferences> = {
  book: { font: 'serif', languageMode: 'cjk-mixed', fontSizePx: 19, lineHeight: 1.85, cjkLetterSpacingEm: 0.015, measure: 'standard' },
  technical: { font: 'sans', languageMode: 'cjk-mixed', fontSizePx: 19, lineHeight: 1.8, cjkLetterSpacingEm: 0, measure: 'wide' },
  compact: { font: 'sans', languageMode: 'cjk-mixed', fontSizePx: 17, lineHeight: 1.6, cjkLetterSpacingEm: 0, measure: 'narrow' },
};
export const defaultTypography = typographyPresets.book;
export const typographyFont = (font: ReaderTypographyPreferences['font']) => font === 'sans' ? 'Noto Sans SC Variable' : 'Noto Serif SC Variable';

export function normalizeTypography(value: unknown): ReaderTypographyPreferences {
  const v = (value && typeof value === 'object' ? value : {}) as Partial<ReaderTypographyPreferences>;
  const number = (n: unknown, min: number, max: number, fallback: number) => typeof n === 'number' && Number.isFinite(n) ? Math.max(min, Math.min(max, n)) : fallback;
  return {
    font: v.font === 'sans' ? 'sans' : 'serif',
    languageMode: v.languageMode === 'latin' ? 'latin' : 'cjk-mixed',
    fontSizePx: number(v.fontSizePx, 17, 22, v.languageMode === 'latin' ? 18 : 19),
    lineHeight: number(v.lineHeight, 1.6, 2, v.languageMode === 'latin' ? 1.7 : 1.85),
    cjkLetterSpacingEm: number(v.cjkLetterSpacingEm, 0, 0.04, 0.015),
    measure: v.measure === 'narrow' || v.measure === 'wide' ? v.measure : 'standard',
  };
}
export function typographyStorageKey(owner: string) { return `understand-book:reader-typography:v1:${encodeURIComponent(owner)}`; }
export function readTypography(owner: string | null): ReaderTypographyPreferences {
  try { return normalizeTypography(owner ? JSON.parse(localStorage.getItem(typographyStorageKey(owner)) ?? 'null') : null); }
  catch { return { ...defaultTypography }; }
}
export function saveTypography(owner: string | null, value: ReaderTypographyPreferences): boolean {
  if (!owner) return false;
  try { localStorage.setItem(typographyStorageKey(owner), JSON.stringify(normalizeTypography(value))); return true; }
  catch { return false; }
}
export function typographyStyle(p: ReaderTypographyPreferences) {
  const latin = p.languageMode === 'latin';
  return {
    '--reader-font': `"${typographyFont(p.font)}", ${p.font === 'serif' ? 'serif' : 'sans-serif'}`,
    '--reader-size': `${p.fontSizePx}px`, '--reader-leading': String(p.lineHeight),
    '--reader-cjk-spacing': latin ? '0em' : `${p.cjkLetterSpacingEm}em`,
    '--reader-measure': latin ? `${{ narrow: 60, standard: 68, wide: 75 }[p.measure]}ch` : `${{ narrow: 32, standard: 36, wide: 38 }[p.measure]}em`,
    '--reader-paragraph-gap': latin ? '1.15em' : '1.2em',
  };
}

/** Styling spans preserve source characters and are transparent to the existing DOM/source map. */
export function typographyHtml(html: string): string {
  const template = document.createElement('template');
  template.innerHTML = html;
  const walker = document.createTreeWalker(template.content, NodeFilter.SHOW_TEXT);
  const nodes: Text[] = [];
  while (walker.nextNode()) nodes.push(walker.currentNode as Text);
  for (const node of nodes) {
    if (node.parentElement?.closest('code, pre, .katex, math, [data-reader-selection-ignore]')) continue;
    const parts = node.data.split(/([\p{Script=Han}\p{Script=Hiragana}\p{Script=Katakana}\u3000-\u303f\uff00-\uffef]+)/u);
    if (parts.length === 1) continue;
    const fragment = document.createDocumentFragment();
    parts.forEach((text, index) => {
      if (index % 2) { const span = document.createElement('span'); span.className = 'reader-cjk'; span.textContent = text; fragment.append(span); }
      else fragment.append(document.createTextNode(text));
    });
    node.replaceWith(fragment);
  }
  return template.innerHTML;
}
