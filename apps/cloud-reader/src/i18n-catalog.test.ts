import { expect, it } from 'vitest';
import { fileURLToPath } from 'node:url';
import { missingTranslations } from '@understand-book/web/reader/fixtures/i18n-catalog';
import english from '../../../packages/i18n/src/en.json';

it('covers localized keys from the cloud edition in the shared catalog', () => {
  expect(missingTranslations(fileURLToPath(new URL('./', import.meta.url)), english)).toEqual([]);
});
