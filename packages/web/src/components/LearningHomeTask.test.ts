// @vitest-environment happy-dom
import { mount } from '@vue/test-utils';
import { afterEach, beforeEach, expect, it } from 'vitest';
import { setLocale } from '../i18n';
import type { LearningTaskView } from '../learning-tasks';
import type { SharedBook } from '../shared-workspaces';
import LearningHomeTask from './LearningHomeTask.vue';

const originalPublication = { book_id: '物理与生活', publication_id: 'original-publication' };
const books: SharedBook[] = [
  { published_book_ref: { book_id: originalPublication.book_id, publication_id: 'new-publication' }, title: '物理与生活·新版' },
  { published_book_ref: originalPublication, title: '物理与生活·原版' },
];
const task = (overrides: Partial<LearningTaskView> = {}): LearningTaskView => ({
  learning_workspace_id: 'original-learning', published_book_ref: originalPublication,
  course_run_id: null, chat_session_id: 'original-chat', selected_at: 10, kind: 'conversation',
  title: '为什么这个过程仍然守恒？', status: 'active', lesson_id: null, stage: null,
  closed_book_id: null, question: '这是我自己的问题。', message: null, ...overrides,
});
const originalContext: NonNullable<LearningTaskView['course_context']> = {
  release_id: 'original-release', release_version: 2, collection_kind: 'course', collection_title: '原课程：从现象到原理',
  unit_title: '第二单元 · 我们自己的观察', lesson_number: 4, lesson_count: 12,
};
let wrapper: ReturnType<typeof mount> | undefined;
function render(tasks: LearningTaskView[]) {
  wrapper = mount(LearningHomeTask, { props: { tasks, books, loading: false, error: '', busy: false } });
  return wrapper;
}
beforeEach(() => setLocale('zh-CN'));
afterEach(() => { wrapper?.unmount(); wrapper = undefined; localStorage.clear(); setLocale('zh-CN'); });

it('shows the exact task publication title and opens its book without resuming the conversation', async () => {
  const original = task();
  const w = render([original]);
  expect(w.get('.task-collection').text()).toBe('物理与生活·原版');
  expect(w.text()).not.toContain('物理与生活·新版');
  expect(w.get('h2').text()).toBe(original.title);
  expect(w.find('.task-course-position').exists()).toBe(false);

  await w.get('.task-collection').trigger('click');
  expect(w.emitted('collection')).toEqual([[{ kind: 'book', publication: originalPublication, learningWorkspaceId: 'original-learning' }]]);
  expect(w.emitted('resume')).toBeUndefined();
  await w.get('.task-primary').trigger('click');
  expect(w.emitted('resume')).toEqual([[original]]);
});

it('keeps original course context, unit and lesson order alongside the authored current question', async () => {
  const original = task({ kind: 'course', course_run_id: 'original-run', lesson_id: 'lesson-4', stage: 'prediction', course_context: originalContext });
  const w = render([original]);
  expect(w.get('.task-collection').text()).toBe(originalContext.collection_title);
  expect(w.get('.task-course-position').findAll('span').map(span => span.text())).toEqual([
    originalContext.unit_title, '第 4 课 · 共 12 课',
  ]);
  expect(w.get('h2').text()).toBe(original.title);
  expect(w.get('.task-question').text()).toBe(original.question);
  expect(w.get('.task-primary').text()).toBe('继续回答');

  await w.get('.task-collection').trigger('click');
  expect(w.emitted('collection')).toEqual([[{
    kind: 'course', releaseId: originalContext.release_id, version: originalContext.release_version, learningWorkspaceId: 'original-learning',
  }]]);
  expect(w.emitted('resume')).toBeUndefined();
});

it('returns a book-based course to its exact book publication and does not invent course context for older tasks', async () => {
  const original = task({ kind: 'course', course_run_id: 'original-run', lesson_id: 'lesson-4', course_context: {
    ...originalContext, collection_kind: 'book', collection_title: '物理与生活·原版',
  } });
  const w = render([original]);
  await w.get('.task-collection').trigger('click');
  expect(w.emitted('collection')?.[0]).toEqual([{ kind: 'book', publication: originalPublication, learningWorkspaceId: 'original-learning' }]);

  const { course_context: _context, ...olderTask } = original;
  await w.setProps({ tasks: [olderTask] });
  expect(w.get('.task-collection').text()).toBe('物理与生活·原版');
  expect(w.find('.task-course-position').exists()).toBe(false);
  expect(w.text()).not.toContain(originalContext.unit_title);
});

it('uses a readable book fallback when the exact publication is absent instead of borrowing a newer edition title', async () => {
  const w = render([task()]);
  await w.setProps({ books: [books[0]!] });
  expect(w.get('.task-collection').text()).toBe('物理与生活');
  expect(w.text()).not.toContain('物理与生活·新版');

  await w.setProps({ tasks: [task({ learning_workspace_id: null })] });
  await w.get('.task-collection').trigger('click');
  expect(w.emitted('collection')?.[0]).toEqual([{ kind: 'book', publication: originalPublication }]);
});

it('offers books or complete courses in the Chinese empty state without creating a task', async () => {
  const w = render([]);
  expect(w.get('h2').text()).toBe('选一本书，或开始一套课程');
  expect(w.text()).toContain('沿着完整的学习路线，一次学清一个问题。');
  expect(w.get('.task-primary').text()).toBe('看看可以学什么');
  await w.get('.task-primary').trigger('click');
  expect(w.emitted('courses')).toEqual([[]]);
  expect(w.emitted('resume')).toBeUndefined();
  expect(w.emitted('collection')).toBeUndefined();
});
