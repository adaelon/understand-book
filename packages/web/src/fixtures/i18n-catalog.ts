import { readFileSync, readdirSync } from 'node:fs';
import ts from 'typescript';

/** Both editions keep source keys in the same translation catalog. */
export function missingTranslations(root: string, catalog: Record<string, string>): string[] {
  const missing: string[] = [];
  const walk = (dir: string) => {
    for (const entry of readdirSync(dir, { withFileTypes: true })) {
      if (['assets', 'generated'].includes(entry.name)) continue;
      const file = `${dir}/${entry.name}`;
      if (entry.isDirectory()) { walk(file); continue; }
      if (!/\.(ts|vue|js)$/.test(file) || /\.test\.|\.d\.ts$/.test(file)) continue;
      const source = readFileSync(file, 'utf8');
      for (const match of source.matchAll(/\buiText\(('(?:\\.|[^'\\])*')/g)) {
        const ast = ts.createSourceFile('key.ts', `const key = ${match[1]}`, ts.ScriptTarget.Latest, true);
        const key = (ast.statements[0] as ts.VariableStatement).declarationList.declarations[0].initializer as ts.StringLiteral;
        if (!(key.text in catalog)) missing.push(`${entry.name}: ${key.text}`);
      }
    }
  };
  walk(root);
  return missing;
}
