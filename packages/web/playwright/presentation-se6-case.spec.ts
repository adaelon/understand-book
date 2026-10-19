import { test, expect } from '@playwright/test';
import fs from 'node:fs';
import path from 'node:path';

// Opt-in replay of the selected historical case, not a generated user signal.
test('SE6 reopens the original and repaired disk versions from the private case', async ({ browser }) => {
  test.skip(!process.env.SE6_CASE, 'Set SE6_CASE to the collected private case.json');
  test.setTimeout(60000);
  const casePath = path.resolve(process.env.SE6_CASE!);
  const evidence = JSON.parse(fs.readFileSync(casePath, 'utf8'));
  const versions = ['original', 'repaired'].map(role => evidence.materials[role].data);
  expect(versions.map(v => v.reference.revision)).toEqual([2, 3]);
  const records: any[] = [];
  for (const viewport of [{ width: 960, height: 720 }, { width: 320, height: 420 }, { width: 640, height: 240 }]) {
    const context = await browser.newContext({ viewport, hasTouch: viewport.width !== 960 });
    try {
      const page = await context.newPage();
      await page.route('**/se6-case', route => route.fulfill({ contentType: 'text/html', body: `<!doctype html><html><body>
        <iframe sandbox="allow-scripts" style="width:100%;height:95vh;border:0"></iframe>
        <script type="module">
        import { presentationDocument } from '/src/presentation-document.ts';
        let channel;
        window.caseMessages = [];
        addEventListener('message', event => { if(event.data?.channel === channel) window.caseMessages.push(event.data); });
        window.openCase = (version, restored) => {
          channel = crypto.randomUUID(); window.caseMessages = [];
          const view = {...version.content, reference:version.reference, sources:[], restored_state:restored,
            restored_state_revision:restored ? 1 : null, readable_view:{parts:[],sources:[]}};
          document.querySelector('iframe').srcdoc = presentationDocument(view,channel);
        };
        window.captureCase = () => document.querySelector('iframe').contentWindow.postMessage({channel,kind:'snapshot',request_id:42},'*');
        </script></body></html>` }));
      await page.goto('/se6-case');
      await page.waitForFunction(() => typeof (window as any).openCase === 'function');
      for (const version of versions) {
        const open = async (restored: unknown = null) => {
          await page.evaluate(({ version, restored }) => (window as any).openCase(version, restored), { version, restored });
          await expect.poll(() => page.evaluate(() => (window as any).caseMessages.some((m: any) => m.kind === 'ready'))).toBe(true);
        };
        await open();
        const frame = page.frameLocator('iframe');
        await frame.locator('body').evaluate(() => (window as any).presentationScene.seek({ semantic_state: 7, transition_progress: .999 }));
        const before = await frame.locator('body').evaluate(() => (window as any).presentationScene.snapshot());
        await page.evaluate(() => (window as any).captureCase());
        await expect.poll(() => page.evaluate(() => (window as any).caseMessages.some((m: any) => m.kind === 'state' && m.request_id === 42))).toBe(true);
        const saved = await page.evaluate(() => (window as any).caseMessages.find((m: any) => m.kind === 'state' && m.request_id === 42).state);
        expect(saved.values.page.theta).toBeCloseTo(2 * Math.PI * 7.999 / 8, 12);
        await open(saved);
        const restored = await frame.locator('body').evaluate(() => (window as any).presentationScene.snapshot());
        records.push({ viewport, reference: version.reference, before, saved, restored });
        if (version.reference.revision === 2) {
          expect(restored.theta).toBe(2 * Math.PI); expect(restored.semantic_state).toBe(8);
        } else {
          expect(restored.theta).toBe(before.theta); expect(restored.semantic_state).toBe(7);
          expect(restored.transition_progress).toBeCloseTo(.999, 12);
        }
        expect(restored.playing).toBe(false);
      }
    } finally { await context.close(); }
  }
  // Full scene data remains alongside the private case, never in the public index.
  fs.writeFileSync(path.join(path.dirname(casePath), 'replay.json'), JSON.stringify(records, null, 2) + '\n', { flag: 'wx' });
});
