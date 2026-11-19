import { expect, it } from 'vitest';
import { fileURLToPath } from 'node:url';
import { missingTranslations } from './fixtures/i18n-catalog';
import english from '../../i18n/src/en.json';

it('covers every localized source key and preserves interpolation parameters', () => {
  const catalog: Record<string, string> = english;
  const root = fileURLToPath(new URL('./', import.meta.url));
  expect(missingTranslations(root, catalog)).toEqual([]);
  const placeholders = (text: string) => [...text.matchAll(/\{\d+\}/g)].map(match => match[0]).sort();
  for (const [source, translation] of Object.entries(catalog)) {
    expect(translation.trim(), source).not.toBe('');
    expect(translation, source).not.toMatch(/[\u3400-\u9fff]/);
    expect(placeholders(translation), source).toEqual(placeholders(source));
  }
});
