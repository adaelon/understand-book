// @vitest-environment happy-dom
import { mount, flushPromises } from '@vue/test-utils';
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import { installIdentity } from './network-context';
import { localMaterials, syncLocalMaterial, type LocalMaterial } from './local-materials';
import LocalMaterials from './components/LocalMaterials.vue';
import PersonalMaterials from './components/PersonalMaterials.vue';
const native = vi.hoisted(() => ({ invoke: vi.fn(), open: vi.fn(), stop: vi.fn() }));
vi.mock('@tauri-apps/api/core', () => ({ invoke: native.invoke, isTauri: () => true, convertFileSrc: (id: string) => `http://local-material.localhost/${id}` }));
vi.mock('@tauri-apps/plugin-dialog', () => ({ open: native.open }));
vi.mock('@tauri-apps/api/webviewWindow', () => ({ getCurrentWebviewWindow: () => ({ onDragDropEvent: async () => native.stop }) }));
const scope = { account_id: 'student', workspace_id: 'exam' };
const item: LocalMaterial = { material_id: 'local-file', filename: '考纲.txt', bytes: 6, source_path: 'C:\\library\\source.txt', scope, workspace_name: '考试冲刺', imported_at: 1, cloud_copy: null };
beforeEach(() => { vi.clearAllMocks(); installIdentity({ user_id: 'student', csrf_token: 'csrf' }); window.__UNDERSTAND_BOOK_DESKTOP__ = { localMaterials: 1 }; });
afterEach(() => { vi.unstubAllGlobals(); delete window.__UNDERSTAND_BOOK_DESKTOP__; });

it('imports and reopens native copies with no storage, subscription, or upload request', async () => {
  const saved: LocalMaterial[] = [];
  const fetch = vi.fn(() => { throw new Error('Local import must not call the server'); }); vi.stubGlobal('fetch', fetch);
  native.open.mockResolvedValue('C:\\chosen\\考纲.txt');
  native.invoke.mockImplementation(async (command: string) => {
    if (command === 'desktop_local_materials') return [...saved];
    if (command === 'desktop_import_local_material') { saved.push(item); return item; }
    throw new Error(command);
  });
  const options = { props: { scope, workspaceName: '考试冲刺' }, global: { stubs: { LocalMaterialReader: { props: ['material'], template: '<div data-reader>{{ material.filename }}</div>' } } } };
  let wrapper = mount(LocalMaterials, options);
  await flushPromises();
  await wrapper.findAll('button').find(b => b.text() === '导入到本机')!.trigger('click'); await flushPromises();
  expect(wrapper.text()).toContain('已保存在这台电脑，不占用云端空间');
  expect(fetch).not.toHaveBeenCalled();
  wrapper.unmount();
  wrapper = mount(LocalMaterials, options); await flushPromises();
  await wrapper.findAll('button').find(b => b.text() === '打开资料')!.trigger('click');
  expect(wrapper.get('[data-reader]').text()).toBe('考纲.txt');
  expect(fetch).not.toHaveBeenCalled();
  wrapper.unmount();
});

it('checks cloud entitlement only after explicit sync and leaves local reading available on refusal', async () => {
  native.invoke.mockResolvedValue([item]);
  const fetch = vi.fn(async (_url: string) => Response.json({ can_upload: false, available_bytes: 0, items: [] })); vi.stubGlobal('fetch', fetch);
  const wrapper = mount(LocalMaterials, { props: { scope }, global: { stubs: { LocalMaterialReader: true } } }); await flushPromises();
  expect(fetch).not.toHaveBeenCalled();
  await wrapper.findAll('button').find(b => b.text() === '同步到云端')!.trigger('click'); await flushPromises();
  expect(fetch).toHaveBeenCalledTimes(1); expect(fetch.mock.calls[0][0]).toBe('/api/account/storage');
  expect(wrapper.text()).toContain('本机资料仍可正常打开');
  expect(wrapper.findAll('button').find(b => b.text() === '打开资料')!.attributes('disabled')).toBeUndefined();
  expect(native.invoke.mock.calls.every(([command]) => command === 'desktop_local_materials')).toBe(true);
  wrapper.unmount();
});

it('uploads native bytes through the existing quota-controlled route only after explicit sync', async () => {
  const calls: { url: string; body: any }[] = [];
  vi.stubGlobal('fetch', vi.fn(async (url: string, init: RequestInit) => {
    const body = init.body ? JSON.parse(String(init.body)) : null; calls.push({ url, body });
    if (url.endsWith('/account/storage')) return Response.json({ can_upload: true, available_bytes: 10, max_file_bytes: 10, items: [] });
    if (body.action === 'upload') { expect(atob(body.chunk_base64)).toBe('abcdef'); return Response.json({ received: 6 }); }
    if (body.action === 'create') return Response.json({ task_id: body.task_id, state: 'draft' });
    throw new Error(url);
  }));
  native.invoke.mockImplementation(async (command: string, args: any) => {
    if (command === 'desktop_local_material_cloud_copy') return { ...item, cloud_copy: { scope, upload_id: 'upload', task_id: 'task', operation_id: 'operation', complete: args.complete } };
    if (command === 'desktop_read_local_material_chunk') return btoa('abcdef');
    throw new Error(command);
  });
  await syncLocalMaterial(item, scope, vi.fn(), vi.fn());
  expect(calls.map(call => call.body?.action ?? 'space')).toEqual(['space', 'upload', 'create']);
  expect(calls.slice(1).every(call => call.url === '/api/learning-workspaces/exam/preparations')).toBe(true);
  expect(native.invoke.mock.calls.at(-1)).toEqual(['desktop_local_material_cloud_copy', { materialId: item.material_id, scope, complete: true, restart: false }]);
});

it('keeps legacy Windows separate from website uploads instead of disguising cloud storage as local', async () => {
  delete window.__UNDERSTAND_BOOK_DESKTOP__;
  const fetch = vi.fn(); vi.stubGlobal('fetch', fetch);
  const wrapper = mount(PersonalMaterials, { props: { workspace: { learning_workspace_id: 'exam', goal: { text: 'Exam' }, archived: false } as any }, global: { stubs: { PersonalPreparations: true } } });
  await flushPromises();
  expect(wrapper.text()).toContain('本地导入不需要云端存储套餐');
  expect(wrapper.text()).toContain('下载新版 Windows 客户端');
  expect(wrapper.find('personal-preparations-stub').exists()).toBe(false);
  expect(fetch).not.toHaveBeenCalled();
  wrapper.unmount();
});

it('does not revive a cloud copy when the workspace is closed before its storage check returns', async () => {
  let release!: (value: Response) => void;
  const fetch = vi.fn(() => new Promise<Response>(resolve => { release = resolve; }));
  vi.stubGlobal('fetch', fetch);
  native.invoke.mockImplementation(async (command: string) => {
    if (command === 'desktop_local_materials') return [item];
    if (command === 'desktop_local_material_cloud_copy') return { ...item, cloud_copy: { scope, upload_id: 'upload', task_id: 'task', operation_id: 'operation', complete: false } };
    throw new Error(`Cancelled sync must not read bytes: ${command}`);
  });
  const wrapper = mount(LocalMaterials, { props: { scope }, global: { stubs: { LocalMaterialReader: true } } });
  await flushPromises();
  await wrapper.findAll('button').find(b => b.text() === '同步到云端')!.trigger('click');
  wrapper.unmount();
  release(Response.json({ can_upload: true, available_bytes: 10, max_file_bytes: 10, items: [] }));
  await flushPromises();
  expect(fetch).toHaveBeenCalledTimes(1);
  expect(native.invoke.mock.calls.some(([command]) => command === 'desktop_read_local_material_chunk')).toBe(false);
});
