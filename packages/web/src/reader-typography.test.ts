// @vitest-environment happy-dom
import { afterEach, describe, expect, it, vi } from 'vitest';
import { effectScope, ref } from 'vue';
import { defaultTypography, normalizeTypography, readTypography, saveTypography, typographyHtml, typographyStorageKey } from './reader-typography';
import { useReaderTypography } from './useReaderTypography';
import { createMarkdownDomSourceMap } from './markdown-source-map';
import { renderInlineMarkdown } from './md';

afterEach(() => { localStorage.clear(); vi.restoreAllMocks(); });
describe('reader typography', () => {
  it('uses defaults for damaged storage, bounds parameters and preserves actual custom values', () => {
    localStorage.setItem(typographyStorageKey('a'), '{broken');
    expect(readTypography('a')).toEqual(defaultTypography);
    expect(normalizeTypography({ fontSizePx: 99, lineHeight: NaN, measure: 'unknown' })).toMatchObject({ fontSizePx: 22, lineHeight: 1.85, measure: 'standard' });
    expect(normalizeTypography({ languageMode: 'latin' })).toMatchObject({ fontSizePx: 18, lineHeight: 1.7 });
    saveTypography('a', { ...defaultTypography, fontSizePx: 21 });
    expect(readTypography('a').fontSizePx).toBe(21);
    expect(readTypography('b')).toEqual(defaultTypography);
    expect(readTypography(null)).toEqual(defaultTypography);
  });
  it('clears preview and error on identity change; failed saving still updates the page', () => {
    const scope = effectScope(), owner = ref<string | null>('a');
    const state = scope.run(() => useReaderTypography(owner))!;
    state.update({ ...defaultTypography, font: 'sans' });
    state.open.value = true;
    owner.value = 'b';
    expect(state.preferences.value.font).toBe('serif');
    expect(state.open.value).toBe(false);
    vi.spyOn(localStorage, 'setItem').mockImplementation(() => { throw new Error('quota'); });
    state.update({ ...defaultTypography, fontSizePx: 22 });
    expect(state.preferences.value.fontSizePx).toBe(22);
    expect(state.saveError.value).toContain('未能');
    owner.value = null;
    expect(state.preferences.value).toEqual(defaultTypography);
    expect(state.saveError.value).toBe('');
    scope.stop();
  });
  it('keeps mixed text, inline code and formula source ranges unchanged', () => {
    const source = '中文 **English** 字符 `code中文` 与 $x+1$，emoji 😀。';
    const before = document.createElement('div'), after = document.createElement('div');
    before.innerHTML = renderInlineMarkdown(source);
    after.innerHTML = typographyHtml(before.innerHTML);
    expect(after.textContent).toBe(before.textContent);
    expect(after.querySelector('code .reader-cjk')).toBeNull();
    expect(after.querySelector('.katex .reader-cjk')).toBeNull();
    const oldMap = createMarkdownDomSourceMap(source, before), newMap = createMarkdownDomSourceMap(source, after);
    expect(newMap.semanticText).toBe(oldMap.semanticText);
    for (let start = 0; start < oldMap.semanticText.length; start++) {
      expect(newMap.sourceRangesForSemanticRange(start, start + 1)).toEqual(oldMap.sourceRangesForSemanticRange(start, start + 1));
    }
  });
});
