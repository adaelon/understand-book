// @vitest-environment happy-dom
import { mount, flushPromises } from '@vue/test-utils';
import { afterEach, expect, it, vi } from 'vitest';
import SharedWorkspaceCatalog from './SharedWorkspaceCatalog.vue';
import SharedWorkspaceInstance from './SharedWorkspaceInstance.vue';
import SharedWorkspaceManifest from './SharedWorkspaceManifest.vue';
import { installIdentity, network, type LearningWorkspace } from '../network-context';
import type { SharedInstance, SharedRelease, SharedUpdatePreview } from '../shared-workspaces';

const first = { book_id: 'one', publication_id: 'first' }, second = { book_id: 'two', publication_id: 'second' }, privateBook = { book_id: 'private', publication_id: 'private-first' };
const books = [{ published_book_ref: first, title: '微积分' }, { published_book_ref: second, title: '线性代数' }, { published_book_ref: privateBook, title: '我的复习笔记' }];
const space = (): LearningWorkspace => ({ learning_workspace_id: 'personal-space', goal: { text: '两周期末冲刺', constraints: ['每天一小时'], revision: 2, created_at: 1 }, goal_history: [], members: [first, privateBook].map(published_book_ref => ({ published_book_ref, readable: true, added_at: 1 })), archived: false, created_at: 1, updated_at: 1 });
function release(version = 1): SharedRelease {
  const readiness = { status: 'ready' as const, materials: [first].map(published_book_ref => ({ published_book_ref, available_capabilities: ['text', 'tutor'], missing_capabilities: [], publication_available: true })), audience: [{ user_id: 'reader', ready: true, missing_grants: [] }] };
  return { release_id: 'shared', version, latest_version: 2, archived: false, created_at: 1, manifest: { name: '数学基础', publisher: '课程组', public_goal: version === 1 ? '掌握数学基础' : '掌握数学基础与矩阵运算', applicability: '本科期末复习', prerequisites: '高中数学', materials: [first].map(published_book_ref => ({ published_book_ref, required_capabilities: ['text', 'tutor'] })), topic_links: [{ title: '导数与向量', description: '从变化理解向量', materials: [first] }], suggested_route: [{ title: version === 1 ? '先学导数' : '先学矩阵', description: '按章节逐步复习', materials: [first] }] }, readiness_at_publication: readiness, availability: readiness };
}
const instance = (version = 1): SharedInstance => ({ release_id: 'shared', version, latest_version: 2, release: release(version), history: [{ version: 1, selected_at: 1 }] });
const preview = (): SharedUpdatePreview => ({ instance: instance(), release: { ...release(2), manifest: { ...release(2).manifest, materials: [{ published_book_ref: second, required_capabilities: ['text'] }] } }, changes: { added_materials: [second], removed_materials: [first], public_goal_changed: true, composition_changed: true }, preserved_private_materials: [privateBook] });
function setupFetch(handler: (url: string, body: any) => Promise<Response> | Response) {
  installIdentity({ user_id: 'reader', csrf_token: 'csrf' });
  const fetch = vi.fn(async (url: string, init: RequestInit) => handler(url, init.body ? JSON.parse(String(init.body)) : null)); vi.stubGlobal('fetch', fetch); return fetch;
}
async function click(wrapper: ReturnType<typeof mount>, text: string) {
  const button = wrapper.findAll('button').find(item => item.text() === text); expect(button, text).toBeTruthy(); await button!.trigger('click'); await flushPromises();
}
afterEach(() => { vi.useRealTimers(); vi.unstubAllGlobals(); network.value = { ...network.value, enabled: false, identity: null, workspace: null }; });

it('ends a stalled catalog request and lets the reader retry', async () => {
  vi.useFakeTimers();
  installIdentity({ user_id: 'reader', csrf_token: 'csrf' });
  let signal: AbortSignal | undefined;
  const fetch = vi.fn((_url: string, init: RequestInit) => new Promise<Response>((_resolve, reject) => {
    signal = init.signal ?? undefined;
    signal?.addEventListener('abort', () => reject(signal!.reason), { once: true });
  }));
  vi.stubGlobal('fetch', fetch);
  const wrapper = mount(SharedWorkspaceCatalog, { props: { books } });
  try {
    await flushPromises();
    expect(wrapper.text()).toContain('正在读取共享工作区');
    await vi.advanceTimersByTimeAsync(15_000); await flushPromises();
    expect(signal?.aborted).toBe(true);
    expect(wrapper.get('[role="alert"]').text()).toContain('读取超时');
    expect(wrapper.find('[role="status"]').exists()).toBe(false);
    expect(wrapper.get('button').attributes('disabled')).toBeUndefined();
    fetch.mockImplementationOnce(async () => Response.json({ items: [release()], total: 1, limit: 12, offset: 0 }));
    await click(wrapper, '刷新共享工作区');
    expect(wrapper.text()).toContain('数学基础');
    expect(wrapper.find('[role="alert"]').exists()).toBe(false);
  } finally { wrapper.unmount(); }
});

it('cancels catalog reads when the account changes or the catalog closes', async () => {
  installIdentity({ user_id: 'reader', csrf_token: 'csrf' });
  const signals: AbortSignal[] = [];
  vi.stubGlobal('fetch', vi.fn((_url: string, init: RequestInit) => new Promise<Response>((_resolve, reject) => {
    if (init.signal) {
      const signal = init.signal; signals.push(signal);
      signal.addEventListener('abort', () => reject(signal.reason), { once: true });
    }
  })));
  const wrapper = mount(SharedWorkspaceCatalog, { props: { books } });
  try {
    await flushPromises();
    installIdentity({ user_id: 'other', csrf_token: 'other-csrf' }); await flushPromises();
    expect(signals).toHaveLength(2);
    expect(signals[0].aborted).toBe(true);
    expect(signals[1].aborted).toBe(false);
    expect(wrapper.find('[role="alert"]').exists()).toBe(false);
  } finally { wrapper.unmount(); }
  expect(signals[1].aborted).toBe(true);
});

it('shows course external resources as outbound guides without claiming they are prepared sources', () => {
  const value = release();
  value.manifest.external_resources = [{ title:'课程原站',url:'https://example.org/course',description:'按章节前往原站观看' }];
  const wrapper = mount(SharedWorkspaceManifest,{props:{release:value,books}});
  try {
    const section=wrapper.get('[aria-label="外部链接导览"]');
    expect(section.text()).toContain('未作为已解析的工作区资料');
    expect(section.get('a').attributes()).toMatchObject({href:'https://example.org/course',target:'_blank',rel:'noopener noreferrer'});
    expect(value.manifest.materials).toHaveLength(1);
  } finally { wrapper.unmount(); }
});

it('creates an exam instance with explicit date and scope while retaining a direct question without diagnosis', async () => {
  let submitted: any;
  setupFetch((url, body) => {
    if (url.includes('?limit=')) return Response.json({ items: [release()], total: 1, limit: 12, offset: 0 });
    if (url.endsWith('/use')) { submitted = body; return Response.json({ workspace: space(), instance: instance() }); }
    return Response.json(release());
  });
  const wrapper = mount(SharedWorkspaceCatalog, { props: { books } });
  try {
    await flushPromises(); await click(wrapper, '查看目标与路线');
    await wrapper.get('input[type=checkbox]').setValue(true);
    await wrapper.get('input[type=date]').setValue('2026-10-23');
    await wrapper.findAll('textarea')[1].setValue('老师要求前三章');
    await wrapper.get('input[value=question]').setValue(true);
    await wrapper.findAll('textarea')[2].setValue('如何判断这个极限？');
    await wrapper.get('form').trigger('submit'); await flushPromises();
    expect(submitted.exam_sprint).toMatchObject({ exam_date:'2026-10-23',syllabus:'老师要求前三章',entry_mode:'question' });
    expect(wrapper.emitted('started')?.[0][1]).toBe('如何判断这个极限？');
  } finally { wrapper.unmount(); }
});

it('requires a confirmed personal goal, preserves it on retry and reuses the original operation', async () => {
  let attempts = 0; const calls: any[] = [];
  setupFetch((url, body) => {
    if (url.includes('?limit=')) return Response.json({ items: [release()], total: 1, limit: 12, offset: 0 });
    if (url.endsWith('/use')) { calls.push(body); attempts++; return attempts === 1 ? Response.json({ message: '连接暂时中断' }, { status: 503 }) : Response.json({ workspace: space(), instance: instance() }); }
    return Response.json(release());
  });
  const wrapper = mount(SharedWorkspaceCatalog, { props: { books } });
  try {
    await flushPromises(); expect(calls).toHaveLength(0);
    await click(wrapper, '查看目标与路线');
    expect(wrapper.text()).toContain('先学导数'); expect(calls).toHaveLength(0);
    await wrapper.get('textarea').setValue(' '); expect(wrapper.get('form button').attributes('disabled')).toBeDefined();
    await wrapper.get('textarea').setValue('两周期末冲刺'); await wrapper.get('form').trigger('submit'); await flushPromises();
    expect(wrapper.text()).toContain('连接暂时中断'); expect(wrapper.emitted('started')).toBeUndefined();
    await wrapper.get('form').trigger('submit'); await flushPromises();
    expect(calls).toHaveLength(2); expect(calls[0]).toEqual(calls[1]); expect(calls[0].goal).toEqual({ text: '两周期末冲刺', constraints: [] }); expect(calls[0].operation_id).toBeTruthy();
    expect(wrapper.emitted('started')?.[0][0]).toEqual(space());
  } finally { wrapper.unmount(); }
});

it('shows missing access and stops new use of incomplete or archived releases', async () => {
  const value = release(); value.availability = { ...value.availability, status: 'incomplete', audience: [{ user_id: 'reader', ready: false, missing_grants: [first] }] };
  const fetch = setupFetch(url => Response.json(url.includes('?limit=') ? { items: [value], total: 1, limit: 12, offset: 0 } : value));
  const wrapper = mount(SharedWorkspaceCatalog, { props: { books } });
  try {
    await flushPromises(); await click(wrapper, '查看目标与路线'); expect(wrapper.text()).toContain('微积分：尚未开通访问'); expect(wrapper.get('form button').attributes('disabled')).toBeDefined();
    await wrapper.get('form').trigger('submit'); expect(fetch.mock.calls.some(([url]) => url.endsWith('/use'))).toBe(false);
    await click(wrapper, '返回共享列表'); value.availability.status = 'ready'; value.archived = true; await click(wrapper, '查看目标与路线');
    expect(wrapper.text()).toContain('此共享工作区已归档'); expect(wrapper.get('form button').attributes('disabled')).toBeDefined();
  } finally { wrapper.unmount(); }
});

it('previews differences and only applies an explicitly selected update while keeping the personal goal and materials', async () => {
  const mutations: any[] = []; const updatedSpace = { ...space(), members: [second, privateBook].map(published_book_ref => ({ published_book_ref, readable: true, added_at: 1 })) };
  setupFetch((url, body) => {
    if (url.includes('/preview?')) return Response.json(preview());
    if (url.endsWith('/update')) { mutations.push(body); return Response.json({ workspace: updatedSpace, instance: instance(2) }); }
    return Response.json({ instance: instance() });
  });
  const wrapper = mount(SharedWorkspaceInstance, { props: { workspace: space(), books } });
  try {
    await flushPromises(); expect(wrapper.text()).toContain('我的个人实例'); expect(wrapper.text()).toContain('先学导数'); expect(mutations).toHaveLength(0);
    await click(wrapper, '查看新版变化');
    expect(wrapper.text()).toContain('新增资料：线性代数'); expect(wrapper.text()).toContain('移出公共资料：微积分'); expect(wrapper.text()).toContain('你的学习目标继续为：两周期末冲刺'); expect(wrapper.text()).toContain('保留私人追加资料：我的复习笔记');
    await click(wrapper, '保留当前版本'); expect(mutations).toHaveLength(0);
    await click(wrapper, '查看新版变化'); await click(wrapper, '选择更新到此版本');
    expect(mutations).toHaveLength(1); expect(mutations[0]).toMatchObject({ expected_version: 1, version: 2 }); expect(mutations[0]).not.toHaveProperty('goal');
    expect(wrapper.emitted('updated')?.[0][0]).toEqual(updatedSpace); expect(wrapper.text()).toContain('当前使用第 2 版'); expect(wrapper.text()).toContain('已更新共享资料与路线');
  } finally { wrapper.unmount(); }
});

it('keeps an unavailable newer release as a preview and allows existing archived shared content to remain visible', async () => {
  const saved = instance(); saved.release.archived = true;
  const next = preview(); next.release.availability.status = 'incomplete';
  const fetch = setupFetch(url => Response.json(url.includes('/preview?') ? next : { instance: saved }));
  const wrapper = mount(SharedWorkspaceInstance, { props: { workspace: space(), books } });
  try {
    await flushPromises(); expect(wrapper.text()).toContain('共享入口已归档'); expect(wrapper.text()).toContain('先学导数'); await click(wrapper, '查看新版变化');
    const update = wrapper.findAll('button').find(button => button.text() === '选择更新到此版本')!; expect(update.attributes('disabled')).toBeDefined();
    await update.trigger('click'); expect(fetch.mock.calls.some(([url]) => url.endsWith('/update'))).toBe(false);
  } finally { wrapper.unmount(); }
});

it('does not show the previous workspace result after switching the selected personal workspace', async () => {
  let finish!: (value: Response) => void;
  setupFetch(url => url.includes('/personal-space/') ? new Promise(resolve => { finish = resolve; }) : Response.json({ instance: null }));
  const wrapper = mount(SharedWorkspaceInstance, { props: { workspace: space(), books } });
  try {
    await flushPromises(); await wrapper.setProps({ workspace: { ...space(), learning_workspace_id: 'ordinary-workspace' } }); await flushPromises();
    finish(Response.json({ instance: instance() })); await flushPromises(); expect(wrapper.find('[aria-label="共享工作区个人实例"]').exists()).toBe(false);
  } finally { wrapper.unmount(); }
});
