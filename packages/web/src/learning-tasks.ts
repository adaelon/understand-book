import type { PublishedBookRef } from './network-context';
import { t } from './i18n';
import type { LearningCollectionTarget } from './learning-collections';

export interface LearningTaskCourseContext {
  release_id: string;
  release_version: number;
  collection_kind: 'book' | 'course';
  collection_title: string;
  unit_title: string;
  lesson_number: number;
  lesson_count: number;
}

export interface LearningTaskSelection {
  course_run_id?: string;
  chat_session_id?: string;
  continue_conversation?: boolean;
}

export interface LearningTaskView {
  learning_workspace_id: string | null;
  published_book_ref: PublishedBookRef;
  course_run_id: string | null;
  chat_session_id: string | null;
  selected_at: number;
  kind: 'course' | 'closed_book' | 'conversation';
  title: string;
  status: 'active' | 'paused' | 'unavailable';
  lesson_id: string | null;
  stage: string | null;
  closed_book_id: string | null;
  question: string | null;
  message: string | null;
  course_context?: LearningTaskCourseContext;
}

export interface CourseLaunchTarget {
  publication: PublishedBookRef;
  lessonId: string;
  runId?: string;
  chatSessionId?: string;
}

export function learningTaskKey(task: LearningTaskView): string {
  return JSON.stringify([task.learning_workspace_id, task.published_book_ref, task.course_run_id, task.chat_session_id]);
}

export function learningTaskCollection(task: LearningTaskView): LearningCollectionTarget {
  const scope = task.learning_workspace_id ? { learningWorkspaceId: task.learning_workspace_id } : {};
  const course = task.course_context;
  return course?.collection_kind === 'course'
    ? { kind: 'course', releaseId: course.release_id, version: course.release_version, ...scope }
    : { kind: 'book', publication: task.published_book_ref, ...scope };
}

export function learningTaskAction(task: LearningTaskView): string {
  if (task.kind === 'closed_book') return t(task.status === 'paused' ? '继续练习' : '继续回答');
  if (task.kind === 'conversation') return t('继续讨论');
  return t(['prediction', 'explanation', 'task', 'feedback'].includes(task.stage ?? '') ? '继续回答' : '继续本课');
}

export function learningTaskStage(task: LearningTaskView): string {
  if (task.kind === 'closed_book') return t('闭卷练习 · 当前问题与草稿已保留');
  if (task.kind === 'conversation') return t('回到原来的问题和对话');
  const labels: Record<string, string> = {
    introduction: '先认识问题和必要概念', prediction: '留下你的判断和理由',
    experiment: '继续观察、取证或实验', explanation: '把依据组织成自己的解释',
    task: '用新情境检验理解', feedback: '对照反馈，继续当前任务',
  };
  return t(labels[task.stage ?? ''] ?? '继续上次的学习任务');
}
