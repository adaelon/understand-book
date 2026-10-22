import { request } from 'playwright';
import { readFile, writeFile } from 'node:fs/promises';
import assert from 'node:assert/strict';
const root = 'tmp/local-library-20261021', web = `${root}/frontend/packages/web/dist`;
const client = await request.newContext({ userAgent: 'UnderstandBook-MCP10-Verification', extraHTTPHeaders: { 'Cache-Control': 'no-cache' }, timeout: 45000 });
const seen = new Set();
async function get(url) { const response = await client.get(url); assert(response.ok(), `${response.status()} ${url}`); return response.body(); }
try {
  for (const name of ['index.html','home.html']) {
    const body = (await get(`https://understandbook.top/${name}?release=local-library-031`)).toString('utf8');
    const clean = body.replace(/<script\b[^>]*src="https:\/\/static\.cloudflareinsights\.com\/[^\"]*"[^>]*><\/script>\s*(?=<\/body>)/g, '');
    assert.equal(clean, await readFile(`${web}/${name}`, 'utf8'));
    for (const match of body.matchAll(/(?:src|href)="(\/assets\/[^"#]+)"/g)) {
      const asset = match[1]; if (seen.has(asset)) continue;
      const remote = await get(`https://understandbook.top${asset}`);
      assert.deepEqual(remote, await readFile(`${web}${asset}`)); seen.add(asset); console.log(`Verified ${asset}`);
    }
  }
  const installer = await get('https://understandbook.top/downloads/UnderstandBookSetup.exe?v=0.3.1');
  assert.deepEqual(installer, await readFile('dist/UnderstandBookSetup.exe'));
  const report = { status: 'passed', version: '0.3.1', release: '/opt/understand-book/releases/cloud-20261021', backup: '/opt/understand-book/acceptance/local-library-20261021/backup', installer_bytes: installer.length, public_entry_assets_match: true, public_installer_matches: true, entry_assets: [...seen], server_restarted: false, database_changes: false, model_requests: 0, finished_at_utc: new Date().toISOString(), verification_note: 'CDN analytics script removed before comparing HTML; all remaining HTML, entry assets and installer match candidate bytes. Browser API fixtures and native WebView2 tests recorded separately.' };
  await writeFile(`${root}/deployment.json`, JSON.stringify(report, null, 2)); console.log(JSON.stringify(report, null, 2));
} finally { await client.dispose(); }
