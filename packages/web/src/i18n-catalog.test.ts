import { expect, it } from 'vitest';
import { readFileSync, readdirSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import ts from 'typescript';
import english from '../../i18n/src/en.json';

it('covers every localized source key and preserves interpolation parameters', () => {
  const catalog: Record<string, string> = english;
  const root = fileURLToPath(new URL('./', import.meta.url));
  const missing: string[] = [];
  const walk = (dir: string) => {
    for (const entry of readdirSync(dir, { withFileTypes: true })) {
      if (['assets', 'generated'].includes(entry.name)) continue;
      const file = `${dir}/${entry.name}`;
      if (entry.isDirectory()) { walk(file); continue; }
      if (!/\.(ts|vue|js)$/.test(file) || /\.test\.|\.d\.ts$/.test(file)) continue;
      const source = readFileSync(file, 'utf8');
      // Both Vue expressions and TS call sites use single-quoted source keys.
      for (const match of source.matchAll(/\buiText\(('(?:\\.|[^'\\])*')/g)) {
        const ast = ts.createSourceFile('key.ts', `const key = ${match[1]}`, ts.ScriptTarget.Latest, true);
        const key = (ast.statements[0] as ts.VariableStatement).declarationList.declarations[0].initializer as ts.StringLiteral;
        if (!(key.text in catalog)) missing.push(`${entry.name}: ${key.text}`);
      }
    }
  };
  walk(root);
  expect(missing).toEqual([]);
  const placeholders = (text: string) => [...text.matchAll(/\{\d+\}/g)].map(match => match[0]).sort();
  for (const [source, translation] of Object.entries(catalog)) {
    expect(translation.trim(), source).not.toBe('');
    expect(translation, source).not.toMatch(/[\u3400-\u9fff]/);
    expect(placeholders(translation), source).toEqual(placeholders(source));
  }
});
