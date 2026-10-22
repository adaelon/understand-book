import { chromium } from 'playwright';
import assert from 'node:assert/strict';
import { writeFile } from 'node:fs/promises';
const browser = await chromium.launch({ headless: true });
const results = [];
try {
  for (const mode of ['website','desktop-030','desktop-031']) {
    const context = await browser.newContext({ viewport: { width: 1440, height: 1000 } });
    await context.addInitScript(mode => {
      if (mode === 'website') return;
      window.isTauri = true;
      if (mode === 'desktop-031') window.__UNDERSTAND_BOOK_DESKTOP__ = { localMaterials: 1 };
      const items = [];
      window.__TAURI_INTERNALS__ = {
        metadata: { currentWindow: { label: 'main' }, currentWebview: { label: 'main' } },
        transformCallback: () => 1, unregisterCallback: () => {},
        convertFileSrc: id => `http://local-material.localhost/${id}`,
        invoke: async (command, args) => {
          if (command === 'desktop_local_materials') return [...items];
          if (command === 'plugin:dialog|open') return 'C:\\fixture\\期末考纲.txt';
          if (command === 'desktop_import_local_material') { const item = { material_id: 'fixture', filename: '期末考纲.txt', bytes: 103, source_path: 'C:\\fixture', scope: args.scope, workspace_name: args.workspaceName, cloud_copy: null, imported_at: 1 }; items.push(item); return item; }
          if (command.startsWith('plugin:event|')) return 1;
          throw new Error(command);
        },
      };
    }, mode);
    const apiRequests = [], errors = [];
    const workspace = { learning_workspace_id: 'exam', goal: { text: '期末考试冲刺', revision: 1, constraints: [], created_at: 1 }, goal_history: [], members: [], archived: false, created_at: 1, updated_at: 1 };
    await context.route('**/api/**', async route => {
      const request = route.request(), pathname = new URL(request.url()).pathname; apiRequests.push({ path: pathname, method: request.method() });
      const json = pathname === '/api/auth/me' ? { user_id: 'fixture-student', csrf_token: 'fixture-only', email: '本地导入验收', capabilities: { presentation: { authoring: false, reason: '' } } }
        : pathname === '/api/library' ? { books: [] }
        : pathname === '/api/videos' ? { videos: [] }
        : pathname === '/api/learning-workspaces' ? { workspaces: [workspace] }
        : pathname.endsWith('/exam-sprint') ? { sprint: null }
        : pathname.endsWith('/shared-release') ? { instance: null }
        : pathname.endsWith('/preparations') ? { items: [], supported_formats: ['txt','pdf','epub','mp4'] }
        : pathname === '/api/account/storage' ? { can_upload: false, used_bytes: 0, reserved_bytes: 0, total_bytes: 0, limit_bytes: null, max_file_bytes: null, available_bytes: 0, items: [], retained_until: null }
        : { instance: null, workspaces: [], items: [] };
      assert.equal(request.method(), 'GET', `Unexpected mutation ${pathname}`);
      await route.fulfill({ json });
    });
    const page = await context.newPage(); page.on('pageerror', error => errors.push(error.message));
    await page.goto('https://understandbook.top/index.html?release=local-library-031');
    await page.getByRole('button', { name: '管理工作区', exact: true }).click();
    if (mode === 'website') {
      await page.getByRole('heading', { name: '云端资料与准备', exact: true }).waitFor();
      assert.equal(await page.getByRole('button', { name: '导入到本机', exact: true }).count(), 0);
      assert(apiRequests.some(request => request.path === '/api/account/storage'));
    } else if (mode === 'desktop-030') {
      await page.getByRole('link', { name: '下载新版 Windows 客户端', exact: true }).waitFor();
      assert(!apiRequests.some(request => request.path === '/api/account/storage'));
    } else {
      await page.getByRole('button', { name: '导入到本机', exact: true }).click();
      await page.getByText('已保存在这台电脑，不占用云端空间。', { exact: true }).waitFor();
      assert(!apiRequests.some(request => request.path === '/api/account/storage' || request.path.endsWith('/preparations')));
      await page.locator('.local-materials').screenshot({ path: 'tmp/local-library-20261021/cloud-workspace-local.png' });
      await page.getByRole('button', { name: '同步到云端', exact: true }).click();
      await page.getByText('本机资料仍可正常打开。', { exact: false }).waitFor();
      assert(apiRequests.some(request => request.path === '/api/account/storage'));
      assert(!apiRequests.some(request => request.path.endsWith('/preparations')));
      assert(await page.getByRole('button', { name: '打开资料', exact: true }).isEnabled());
    }
    assert.deepEqual(errors, []);
    results.push({ mode, status: 'passed', api_fixture: true, production_mutations: 0, errors });
    await context.close();
  }
  await writeFile('tmp/local-library-20261021/public-ui.json', JSON.stringify({ status: 'passed', results }, null, 2));
  console.log(JSON.stringify(results, null, 2));
} finally { await browser.close(); }
