import type { Page } from '@playwright/test';
import { readFile } from 'node:fs/promises';
import path from 'node:path';
import ts from 'typescript';

// FP7 observes the normal index.html bundle. No alternate mount or product code
// is shipped for measurement; reuse the earlier slices' read-only observers.
let observer: Promise<string> | undefined;
const observed = new WeakSet<Page>();
export async function observeProductionEntry(page: Page) {
  if (!process.env.FOCUS_ENTRY_URL) return;
  observer ??= Promise.all(['focus-turn', 'focus-pdf'].map(async name => {
    const source = (await readFile(path.resolve(`playwright/fixtures/${name}.ts`), 'utf8'))
      .replace(/^import .*;\r?\n/gm, '')
      .replace("const app = createApp(App).mount('#app');", "const app = { get $() { return (document.querySelector('#app') as any)?._vnode?.component; } };");
    return `(() => { ${ts.transpileModule(source, { compilerOptions: { target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.None } }).outputText} })();`;
  })).then(scripts => `document.addEventListener('DOMContentLoaded', () => { ${scripts.join('\n')} }, { once: true });`);
  if (!observed.has(page)) { await page.addInitScript({ content: await observer }); observed.add(page); }
  await page.route('**/fp1/**', async route => {
    const relative = new URL(route.request().url()).pathname.slice('/fp1/'.length);
    await route.fulfill({ path: path.resolve('../../tmp/focus-reading-fp1/public/fp1', relative) });
  });
}
