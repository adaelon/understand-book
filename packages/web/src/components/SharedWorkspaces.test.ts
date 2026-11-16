// @vitest-environment happy-dom
import { mount, flushPromises } from '@vue/test-utils';
import { afterEach, expect, it, vi } from 'vitest';
import SharedWorkspaceCatalog from './SharedWorkspaceCatalog.vue';
import SharedWorkspaceInstance from './SharedWorkspaceInstance.vue';
import SharedWorkspaceManifest from './SharedWorkspaceManifest.vue';
import { installIdentity, network, type LearningWorkspace } from '../network-context';
import type { SharedInstance, SharedRelease, SharedUpdatePreview } from '../shared-workspaces';
import type { CourseProgress } from '../generated/CourseProgress';
import type { LearningTaskView } from '../learning-tasks';

const first = { book_id: 'one', publication_id: 'first' }, second = { book_id: 'two', publication_id: 'second' }, privateBook = { book_id: 'private', publication_id: 'private-first' };
const books = [{ published_book_ref: first, title: '微积分' }, { published_book_ref: second, title: '线性代数' }, { published_book_ref: privateBook, title: '我的复习笔记' }];
const space = (): LearningWorkspace => ({ learning_workspace_id: 'personal-space', goal: { text: '两周期末冲刺', constraints: ['每天一小时'], revision: 2, created_at: 1 }, goal_history: [], members: [first, privateBook].map(published_book_ref => ({ published_book_ref, readable: true, added_at: 1 })), archived: false, created_at: 1, updated_at: 1 });
function release(version = 1): SharedRelease {
  const readiness = { status: 'ready' as const, materials: [first].map(published_book_ref => ({ published_book_ref, available_capabilities: ['text', 'tutor'], missing_capabilities: [], publication_available: true })), audience: [{ user_id: 'reader', ready: true, missing_grants: [] }] };
  return { release_id: 'shared', version, latest_version: 2, archived: false, created_at: 1, manifest: { name: '数学基础', publisher: '课程组', public_goal: version === 1 ? '掌握数学基础' : '掌握数学基础与矩阵运算', applicability: '本科期末复习', prerequisites: '高中数学', materials: [first].map(published_book_ref => ({ published_book_ref, required_capabilities: ['text', 'tutor'] })), topic_links: [{ title: '导数与向量', description: '从变化理解向量', materials: [first] }], suggested_route: [{ title: version === 1 ? '先学导数' : '先学矩阵', description: '按章节逐步复习', materials: [first] }] }, readiness_at_publication: readiness, availability: readiness };
}
const instance = (version = 1): SharedInstance => ({ release_id: 'shared', version, latest_version: 2, release: release(version), history: [{ version: 1, selected_at: 1 }] });
function courseRelease(version = 1): SharedRelease {
  const value = release(version);
  value.manifest.inquiry_course = {
    language: 'zh-CN', units: [{ id: 'calculus', title: '导数' }],
    routes: [{ id: 'chapters', title: '从变化开始', question: '怎样描述变化？', structure_ref: 'chapter:1', lesson_ids: ['derivative'] }],
    lessons: [{ id: 'derivative', unit_id: 'calculus', question: '平均变化怎样接近瞬时变化？', objective: '用割线解释导数的含义',
      prerequisites: '会计算直线的斜率', estimated_minutes: 25, duration_basis: '包含观察与独立解释',
      source: { publication: first, source_revision: 'calculus-v1', start_lid: '1.1', end_lid: '1.3', label: '导数的含义', structure_refs: ['chapter:1'] } }],
  };
  return value;
}
const courseProgress = (overrides: Partial<CourseProgress> = {}): CourseProgress => ({
  run_id: 'saved-course-run', release_id: 'shared', release_version: 1, lesson_id: 'derivative', revision: 8,
  goal_revision: 2, stage: 'explanation', status: 'paused', ...overrides,
});
const savedTask = (overrides: Partial<LearningTaskView> = {}): LearningTaskView => ({
  learning_workspace_id: 'personal-space', published_book_ref: first, course_run_id: 'saved-course-run', chat_session_id: 'original-chat',
  selected_at: 1, kind: 'course', title: '平均变化怎样接近瞬时变化？', status: 'paused', lesson_id: 'derivative',
  stage: 'explanation', closed_book_id: null, question: '怎样从割线理解瞬时变化？', message: null, ...overrides,
});
const preview = (): SharedUpdatePreview => ({ instance: instance(), release: { ...release(2), manifest: { ...release(2).manifest, materials: [{ published_book_ref: second, required_capabilities: ['text'] }] } }, changes: { added_materials: [second], removed_materials: [first], public_goal_changed: true, composition_changed: true }, preserved_private_materials: [privateBook] });
function setupFetch(handler: (url: string, body: any) => Promise<Response> | Response,
  courseHandler = (_url: string, _body: any): Promise<Response> | Response => Response.json({ release_id: 'exam-course', release_version: 1, course: null, progress: [] })) {
  installIdentity({ user_id: 'reader', csrf_token: 'csrf' });
  const fetch = vi.fn(async (url: string, init: RequestInit) => (url.endsWith('/shared-release/course') ? courseHandler : handler)(url, init.body ? JSON.parse(String(init.body)) : null)); vi.stubGlobal('fetch', fetch); return fetch;
}
async function click(wrapper: ReturnType<typeof mount>, text: string) {
  const button = wrapper.findAll('button').find(item => item.text() === text); expect(button, text).toBeTruthy(); await button!.trigger('click'); await flushPromises();
}
afterEach(() => { vi.useRealTimers(); vi.unstubAllGlobals(); network.value = { ...network.value, enabled: false, identity: null, workspace: null }; });

function twoLessons(version = 1) {
  const value = courseRelease(version), lesson = value.manifest.inquiry_course!.lessons[0]!;
  value.manifest.inquiry_course!.units.push({id:'application',title:'变化的应用'});
  value.manifest.inquiry_course!.lessons.push({...lesson,source:{...lesson.source},id:'application',unit_id:'application',question:'怎样比较两种变化？',objective:'在新情境中使用变化率'});
  return value;
}
function catalogFetch(values: SharedRelease[], options: { saved?: SharedRelease; workspace?: LearningWorkspace; progress?: CourseProgress[]; use?: (body:any)=>Response; catalog?: ()=>Response; list?: (url:string)=>Response } = {}) {
  return setupFetch((url, body) => {
    if (url.includes('?limit=')) {
      if (options.list) return options.list(url);
      const bookId = new URL(url, 'https://example.test').searchParams.get('book_id');
      const items = bookId ? values.filter(value=>value.manifest.inquiry_course?.lessons.some(lesson=>lesson.source.publication.book_id===bookId)) : values;
      return Response.json({items,total:items.length,limit:12,offset:0});
    }
    if (url.endsWith('/shared-release')) return Response.json({instance:{...instance(options.saved?.version ?? 1),release:options.saved ?? values[0]}});
    if (url.endsWith('/use')) return options.use?.(body) ?? Response.json({workspace:options.workspace ?? space(),instance:{...instance(),release:values[0]}});
    const version = Number(url.match(/\/versions\/(\d+)/)?.[1]);
    return Response.json(values.find(value=>value.version===version) ?? values[0]);
  },()=>options.catalog?.() ?? Response.json({release_id:'shared',release_version:options.saved?.version ?? 1,course:(options.saved ?? values[0])?.manifest.inquiry_course ?? null,progress:options.progress ?? []}));
}
async function openBook(wrapper: ReturnType<typeof mount>, name = '微积分') {
  const card=wrapper.findAll('.collection-card').find(card=>card.get('h3').text()===name);expect(card).toBeTruthy();
  await card!.get('button').trigger('click');await flushPromises();
}
async function startLesson(wrapper: ReturnType<typeof mount>, lesson='derivative') {
  await wrapper.get(`[data-lesson="${lesson}"] .primary`).trigger('click');await flushPromises();
}

it('ends a stalled catalog request and lets the reader retry', async () => {
  vi.useFakeTimers();installIdentity({user_id:'reader',csrf_token:'csrf'});let signal:AbortSignal|undefined;
  const fetch=vi.fn((_url:string,init:RequestInit)=>new Promise<Response>((_resolve,reject)=>{signal=init.signal??undefined;signal?.addEventListener('abort',()=>reject(signal!.reason),{once:true});}));vi.stubGlobal('fetch',fetch);
  const wrapper=mount(SharedWorkspaceCatalog,{props:{books}});
  try {
    await flushPromises();expect(wrapper.text()).toContain('正在读取学习目录');
    await vi.advanceTimersByTimeAsync(15_000);await flushPromises();expect(signal?.aborted).toBe(true);expect(wrapper.get('[role=alert]').text()).toContain('读取超时');
    expect(wrapper.find('[role=status]').exists()).toBe(false);
    fetch.mockImplementationOnce(async()=>Response.json({items:[release()],total:1,limit:12,offset:0}));
    await click(wrapper,'刷新目录');expect(wrapper.text()).toContain('数学基础');expect(wrapper.find('[role=alert]').exists()).toBe(false);
  } finally {wrapper.unmount();}
});
it('cancels catalog reads when the account changes or the catalog closes', async () => {
  installIdentity({user_id:'reader',csrf_token:'csrf'});const signals:AbortSignal[]=[];
  vi.stubGlobal('fetch',vi.fn((_url:string,init:RequestInit)=>new Promise<Response>((_resolve,reject)=>{if(init.signal){const signal=init.signal;signals.push(signal);signal.addEventListener('abort',()=>reject(signal.reason),{once:true});}})));
  const wrapper=mount(SharedWorkspaceCatalog,{props:{books}});
  try {await flushPromises();installIdentity({user_id:'other',csrf_token:'other-csrf'});await flushPromises();expect(signals).toHaveLength(2);expect(signals[0].aborted).toBe(true);expect(signals[1].aborted).toBe(false);expect(wrapper.find('[role=alert]').exists()).toBe(false);}
  finally {wrapper.unmount();}expect(signals[1].aborted).toBe(true);
});
it('shows course external resources as outbound guides without claiming they are prepared sources', () => {
  const value=release();value.manifest.external_resources=[{title:'课程原站',url:'https://example.org/course',description:'按章节前往原站观看'}];
  const wrapper=mount(SharedWorkspaceManifest,{props:{release:value,books}});
  try {const section=wrapper.get('[aria-label="外部链接导览"]');expect(section.text()).toContain('未作为已解析的工作区资料');expect(section.get('a').attributes()).toMatchObject({href:'https://example.org/course',target:'_blank',rel:'noopener noreferrer'});expect(value.manifest.materials).toHaveLength(1);}
  finally {wrapper.unmount();}
});
it('groups routes and publication versions under one book without reading private instances while browsing the home page', async () => {
  const firstRoute=courseRelease(),otherRoute=courseRelease();otherRoute.release_id='another-route';otherRoute.manifest.name='导数应用';
  otherRoute.manifest.inquiry_course!.lessons[0]!.source.publication={...first,publication_id:'second-edition'};
  const workspace={...space(),shared_release:{release_id:'shared',version:1}};
  const fetch=catalogFetch([firstRoute,otherRoute]);
  const wrapper=mount(SharedWorkspaceCatalog,{props:{books:[...books,{published_book_ref:{...first,publication_id:'second-edition'},title:'微积分'}],workspaces:[workspace]}});
  try {await flushPromises();expect(wrapper.findAll('.collection-card').filter(card=>card.get('h3').text()==='微积分')).toHaveLength(1);expect(wrapper.text()).toContain('已提供 2 条课程路线 · 2 节课');expect(fetch.mock.calls.some(([url])=>url.includes('/learning-workspaces/'))).toBe(false);expect(fetch.mock.calls.some(([url])=>url.endsWith('/use'))).toBe(false);}
  finally {wrapper.unmount();}
});
it('opens a book home from its exact URL target and reads a book with no course without creating a workspace', async () => {
  const fetch=catalogFetch([]);const wrapper=mount(SharedWorkspaceCatalog,{props:{books,target:{kind:'book',publication:first}}});
  try {await flushPromises();expect(wrapper.get('h2').text()).toBe('微积分');expect(wrapper.text()).toContain('当前页尚未提供本书的单元课');expect(fetch.mock.calls[0][0]).toContain('book_id=one');await click(wrapper,'阅读原文');expect(wrapper.emitted('reading')?.[0]).toEqual([first,undefined]);expect(wrapper.emitted('started')).toBeUndefined();expect(fetch.mock.calls.some(([url])=>url.endsWith('/use'))).toBe(false);}
  finally {wrapper.unmount();}
});
it('shows the actual units and starts the selected second lesson only after accepting its visible route goal', async () => {
  const value=twoLessons();const calls:any[]=[];const created={...space(),goal:{...space().goal,text:value.manifest.public_goal,revision:1}};
  catalogFetch([value],{use:body=>{calls.push(body);return Response.json({workspace:created,instance:{...instance(),release:value}});}});
  const wrapper=mount(SharedWorkspaceCatalog,{props:{books}});
  try {await flushPromises();await openBook(wrapper);expect(calls).toHaveLength(0);expect(wrapper.emitted('update:target')?.[0]).toEqual([{kind:'book',publication:first}]);expect(wrapper.findAll('.course-unit')).toHaveLength(2);expect(wrapper.get('.accepted-goal').text()).toBe(value.manifest.public_goal);expect(wrapper.text()).toContain('会计算直线的斜率');expect(wrapper.text()).toContain('约 25 分钟');await startLesson(wrapper,'application');expect(calls).toHaveLength(1);expect(calls[0].goal).toEqual({text:value.manifest.public_goal,constraints:[]});expect(wrapper.emitted('started')?.[0]).toEqual([{...created,shared_release:{release_id:'shared',version:1}},undefined,true,{publication:first,lessonId:'application'}]);}
  finally {wrapper.unmount();}
});
it.each(['book', 'course'] as const)('jumps from the %s home to the first unit without creating or opening a learning record', async kind => {
  const value=twoLessons();const fetch=catalogFetch([value]);
  const target=kind==='book'?{kind,publication:first}:{kind,releaseId:'shared',version:1};
  const wrapper=mount(SharedWorkspaceCatalog,{props:{books,target},attachTo:document.body});
  try {
    await flushPromises();const unit=wrapper.get('.course-unit');const scroll=vi.fn();Object.defineProperty(unit.element,'scrollIntoView',{value:scroll,configurable:true});
    const browse=wrapper.get('.browse-lessons');expect(browse.text()).toContain(kind==='book'?'跟着课程学':'选择一课开始');
    if(kind==='book')expect(wrapper.get('.book-home-actions button:nth-child(2)').classes()).not.toContain('primary');
    else expect(wrapper.findAll('.release-heading h3')).toHaveLength(0);
    await browse.trigger('click');expect(scroll).toHaveBeenCalledWith({block:'start'});expect(document.activeElement).toBe(unit.get('h4').element);
    expect(wrapper.emitted('started')).toBeUndefined();expect(wrapper.emitted('reading')).toBeUndefined();expect(fetch.mock.calls.some(([url])=>url.endsWith('/use'))).toBe(false);
  } finally {wrapper.unmount();}
});
it('keeps a failed start operation and edited goal when returning through the book home', async () => {
  const value=twoLessons();const calls:any[]=[];
  catalogFetch([value],{use:body=>{calls.push(body);return calls.length===1?Response.json({message:'开课连接中断'},{status:503}):Response.json({workspace:space(),instance:{...instance(),release:value}});}});
  const wrapper=mount(SharedWorkspaceCatalog,{props:{books}});
  try {await flushPromises();await openBook(wrapper);await wrapper.get('textarea').setValue(' ');expect(wrapper.get('[data-lesson=application] .primary').attributes('disabled')).toBeDefined();await wrapper.get('textarea').setValue('两周期末冲刺');await startLesson(wrapper,'application');expect(wrapper.get('[role=alert]').text()).toContain('开课连接中断');await click(wrapper,'返回学习首页');await openBook(wrapper);expect(wrapper.get('.accepted-goal').text()).toBe('两周期末冲刺');await startLesson(wrapper,'application');expect(calls).toHaveLength(2);expect(calls[1]).toEqual(calls[0]);expect(wrapper.emitted('started')).toHaveLength(1);}
  finally {wrapper.unmount();}
});
it('preserves exam settings and explicit first question on the selected lesson', async () => {
  const value=twoLessons();let submitted:any;catalogFetch([value],{use:body=>{submitted=body;return Response.json({workspace:space(),instance:{...instance(),release:value}});}});
  const wrapper=mount(SharedWorkspaceCatalog,{props:{books}});
  try {await flushPromises();await openBook(wrapper);await wrapper.get('input[type=checkbox]').setValue(true);expect(wrapper.get('[data-lesson=application] .primary').attributes('disabled')).toBeDefined();expect(wrapper.text()).toContain('请先填写考试日期。');await wrapper.get('input[type=date]').setValue('2026-12-23');await wrapper.findAll('textarea')[1].setValue('老师要求前三章');await wrapper.get('input[value=question]').setValue(true);await wrapper.findAll('textarea')[2].setValue('如何判断这个极限？');await startLesson(wrapper,'application');expect(submitted.exam_sprint).toMatchObject({exam_date:'2026-12-23',syllabus:'老师要求前三章',entry_mode:'question'});expect(wrapper.emitted('started')?.[0].slice(1)).toEqual(['如何判断这个极限？',true,{publication:first,lessonId:'application'}]);}
  finally {wrapper.unmount();}
});
it('loads the existing frozen course and restores the selected lesson original run and chat instead of a newer created run', async () => {
  const current=twoLessons(2),saved=twoLessons(1);current.manifest.name='新的公开课程';current.manifest.inquiry_course!.lessons[1]!.question='新版题目';
  const workspace={...space(),shared_release:{release_id:'shared',version:1}};
  const progress=[courseProgress({lesson_id:'derivative',status:'completed',stage:'complete',run_id:'finished'}),courseProgress({lesson_id:'application',run_id:'newer-run',status:'active'}),courseProgress({lesson_id:'application'})];
  const tasks=[savedTask({course_run_id:'newer-run',lesson_id:'application',selected_at:2,chat_session_id:'newer-chat'}),savedTask({lesson_id:'application',selected_at:4,kind:'closed_book',closed_book_id:'saved-practice'})];
  const fetch=catalogFetch([current],{saved,workspace,progress});const wrapper=mount(SharedWorkspaceCatalog,{props:{books,workspaces:[workspace],learningTasks:tasks}});
  try {await flushPromises();await openBook(wrapper);expect(wrapper.text()).not.toContain('新的公开课程');expect(wrapper.text()).not.toContain('新版题目');expect(wrapper.text()).toContain('第 1 版');expect(wrapper.text()).toContain('已完成 1 / 2 节');expect(wrapper.get('.accepted-goal').text()).toBe(workspace.goal.text);await startLesson(wrapper,'application');expect(wrapper.emitted('started')?.[0]).toEqual([workspace,undefined,true,{publication:first,lessonId:'application',runId:'saved-course-run',chatSessionId:'original-chat'}]);expect(fetch.mock.calls.some(([url])=>url.endsWith('/use'))).toBe(false);}
  finally {wrapper.unmount();}
});
it('uses an explicit historical course version after its private instance has updated and blocks unstarted historical lessons', async () => {
  const old=twoLessons(2),current=twoLessons(5);old.manifest.name='原来的第二版';current.manifest.name='现在的第五版';
  const workspace={...space(),shared_release:{release_id:'shared',version:5}};
  const fetch=catalogFetch([old,current],{saved:current,workspace,progress:[courseProgress({release_version:2})]});
  const wrapper=mount(SharedWorkspaceCatalog,{props:{books,workspaces:[workspace],learningTasks:[savedTask()],target:{kind:'course',releaseId:'shared',version:2,learningWorkspaceId:workspace.learning_workspace_id}}});
  try {await flushPromises();expect(wrapper.get('h2').text()).toBe('原来的第二版');expect(wrapper.text()).not.toContain('现在的第五版');expect(wrapper.get('[data-lesson=application] .primary').attributes('disabled')).toBeDefined();expect(wrapper.text()).toContain('旧版本的这节课尚未留下学习记录。');await startLesson(wrapper);expect(wrapper.emitted('started')?.[0][3]).toEqual({publication:first,lessonId:'derivative',runId:'saved-course-run',chatSessionId:'original-chat'});expect(fetch.mock.calls.some(([url])=>url.endsWith('/use'))).toBe(false);await click(wrapper,'查看当前学习版本');expect(wrapper.get('h2').text()).toBe('现在的第五版');}
  finally {wrapper.unmount();}
});
it('does not resume an old goal run or claim its completion for the current goal', async () => {
  const value=twoLessons(),workspace={...space(),shared_release:{release_id:'shared',version:1}};
  catalogFetch([value],{saved:value,workspace,progress:[courseProgress({goal_revision:1,status:'completed',stage:'complete'})]});
  const wrapper=mount(SharedWorkspaceCatalog,{props:{books,workspaces:[workspace],learningTasks:[savedTask()]}});
  try {await flushPromises();await openBook(wrapper);expect(wrapper.text()).toContain('已完成 0 / 2 节');await startLesson(wrapper);expect(wrapper.emitted('started')?.[0][3]).toEqual({publication:first,lessonId:'derivative'});}
  finally {wrapper.unmount();}
});
it('retries a failed private course read without creating a new instance', async () => {
  const value=twoLessons(),workspace={...space(),shared_release:{release_id:'shared',version:1}};let reads=0;
  const fetch=catalogFetch([value],{saved:value,workspace,catalog:()=>++reads===1?Response.json({message:'原课程读取失败'},{status:503}):Response.json({release_id:'shared',release_version:1,course:value.manifest.inquiry_course,progress:[courseProgress()]})});
  const wrapper=mount(SharedWorkspaceCatalog,{props:{books,workspaces:[workspace]}});
  try {await flushPromises();await openBook(wrapper);expect(wrapper.get('[role=alert]').text()).toContain('原课程读取失败');await click(wrapper,'重试');await startLesson(wrapper);expect(wrapper.emitted('started')?.[0][3]).toMatchObject({runId:'saved-course-run'});expect(fetch.mock.calls.some(([url])=>url.endsWith('/use'))).toBe(false);}
  finally {wrapper.unmount();}
});
it('reuses a just-created instance if the parent could not open the selected lesson', async () => {
  const value=twoLessons();let uses=0;catalogFetch([value],{use:()=>{uses++;return Response.json({workspace:space(),instance:{...instance(),release:value}});}});
  const wrapper=mount(SharedWorkspaceCatalog,{props:{books}});
  try {await flushPromises();await openBook(wrapper);await startLesson(wrapper,'application');await startLesson(wrapper,'application');expect(uses).toBe(1);expect(wrapper.emitted('started')).toHaveLength(2);expect(wrapper.emitted('started')?.[1][3]).toEqual({publication:first,lessonId:'application'});}
  finally {wrapper.unmount();}
});
it('keeps course browsing available when access is missing but disables starting the lesson', async () => {
  const value=courseRelease();value.availability={...value.availability,status:'incomplete',audience:[{user_id:'reader',ready:false,missing_grants:[first]}]};const fetch=catalogFetch([value]);
  const wrapper=mount(SharedWorkspaceCatalog,{props:{books}});
  try {await flushPromises();await openBook(wrapper);expect(wrapper.text()).toContain('微积分：尚未开通访问');expect(wrapper.get('[data-lesson=derivative] .primary').attributes('disabled')).toBeDefined();await startLesson(wrapper);expect(fetch.mock.calls.some(([url])=>url.endsWith('/use'))).toBe(false);value.availability.status='ready';value.archived=true;await click(wrapper,'刷新目录');expect(wrapper.text()).toContain('已归档');expect(wrapper.get('[data-lesson=derivative] .primary').attributes('disabled')).toBeDefined();}
  finally {wrapper.unmount();}
});
it.each([false,true])('keeps material-only releases as materials and opens their use only explicitly (empty course: %s)', async emptyCourse => {
  const value=release();if(emptyCourse)value.manifest.inquiry_course={language:'zh-CN',units:[],routes:[],lessons:[]};const fetch=catalogFetch([value]);
  const wrapper=mount(SharedWorkspaceCatalog,{props:{books}});
  try {await flushPromises();await click(wrapper,'查看资料与路线');expect(wrapper.findAll('.lesson-row')).toHaveLength(0);expect(fetch.mock.calls.some(([url])=>url.endsWith('/use'))).toBe(false);await click(wrapper,'使用这些资料');expect(wrapper.emitted('started')?.[0].slice(1)).toEqual([undefined,false,undefined]);}
  finally {wrapper.unmount();}
});
it('keeps cross-book courses as related collections and paginates the real book-filtered directory', async () => {
  const own=twoLessons(),cross=twoLessons();cross.release_id='cross-book';cross.manifest.name='跨书专题';cross.manifest.inquiry_course!.lessons[1]!.source.publication=second;
  const fetch=catalogFetch([own,cross],{list:url=>{const query=new URL(url,'https://example.test').searchParams;return Response.json({items:query.get('offset')==='12'?[own]:[cross],total:13,limit:12,offset:Number(query.get('offset'))});}});
  const wrapper=mount(SharedWorkspaceCatalog,{props:{books,target:{kind:'book',publication:first}}});
  try {await flushPromises();expect(wrapper.findAll('.course-unit')).toHaveLength(0);expect(wrapper.get('.related-courses').text()).toContain('跨书专题');expect(wrapper.text()).toContain('1–1 / 13');await click(wrapper,'下一页');expect(fetch.mock.calls.some(([url])=>url.includes('offset=12')&&url.includes('book_id=one'))).toBe(true);expect(wrapper.findAll('.course-unit')).toHaveLength(2);expect(wrapper.text()).toContain('13–13 / 13');expect(fetch.mock.calls.some(([url])=>url.endsWith('/use'))).toBe(false);}
  finally {wrapper.unmount();}
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
    await flushPromises(); expect(wrapper.text()).toContain('我的学习路线'); expect(wrapper.text()).toContain('先学导数'); expect(mutations).toHaveLength(0);
    await click(wrapper, '查看新版变化');
    expect(wrapper.text()).toContain('新增资料：线性代数'); expect(wrapper.text()).toContain('移出公共资料：微积分'); expect(wrapper.text()).toContain('你的学习目标继续为：两周期末冲刺'); expect(wrapper.text()).toContain('保留私人追加资料：我的复习笔记');
    await click(wrapper, '保留当前版本'); expect(mutations).toHaveLength(0);
    await click(wrapper, '查看新版变化'); await click(wrapper, '选择更新到此版本');
    expect(mutations).toHaveLength(1); expect(mutations[0]).toMatchObject({ expected_version: 1, version: 2 }); expect(mutations[0]).not.toHaveProperty('goal');
    expect(wrapper.emitted('updated')?.[0][0]).toEqual(updatedSpace); expect(wrapper.text()).toContain('当前使用第 2 版'); expect(wrapper.text()).toContain('已更新资料与学习路线');
  } finally { wrapper.unmount(); }
});

it('keeps an unavailable newer release as a preview and allows existing archived shared content to remain visible', async () => {
  const saved = instance(); saved.release.archived = true;
  const next = preview(); next.release.availability.status = 'incomplete';
  const fetch = setupFetch(url => Response.json(url.includes('/preview?') ? next : { instance: saved }));
  const wrapper = mount(SharedWorkspaceInstance, { props: { workspace: space(), books } });
  try {
    await flushPromises(); expect(wrapper.text()).toContain('此课程或资料已归档'); expect(wrapper.text()).toContain('先学导数'); await click(wrapper, '查看新版变化');
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
    finish(Response.json({ instance: instance() })); await flushPromises(); expect(wrapper.find('[aria-label="我的课程与学习路线"]').exists()).toBe(false);
  } finally { wrapper.unmount(); }
});
