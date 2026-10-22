import type { LearningWorkspace } from './network-context';
import { networkFetch } from './network-client';

export interface ExamSprintInput {
  exam_date: string;
  syllabus: string;
  daily_minutes: number | null;
  utc_offset_minutes: number;
  entry_mode: 'diagnostic' | 'question';
}
export interface ExamSprint {
  settings: ExamSprintInput;
  goal_revision: number;
  current_goal_revision: number;
  remaining_days: number;
  updated_at: number;
  as_of: number;
}
export const examSprints = {
  useCourse: (id: string, version: number, operationId: string, text: string, sprint?: ExamSprintInput) =>
    networkFetch<import('./shared-workspaces').SharedUseResult>('POST', `/workspace-releases/${encodeURIComponent(id)}/versions/${version}/use`,
      { operation_id: operationId, goal: { text, constraints: [] }, ...(sprint ? { exam_sprint: sprint } : {}) }),
  read: (id: string) => networkFetch<{ sprint: ExamSprint | null }>('GET', `/learning-workspaces/${encodeURIComponent(id)}/exam-sprint`),
  save: (workspace: LearningWorkspace, goalText: string, sprint: ExamSprintInput) =>
    networkFetch<{ workspace: LearningWorkspace; sprint: ExamSprint }>('POST', `/learning-workspaces/${encodeURIComponent(workspace.learning_workspace_id)}/exam-sprint`,
      { expected_goal_revision: workspace.goal.revision, goal_text: goalText, sprint }),
};

/** Keep the displayed day current even if the learning workspace stays open overnight. */
export function daysUntilExam(settings: ExamSprintInput, now = Date.now()) {
  const date = Date.parse(`${settings.exam_date}T00:00:00Z`);
  const today = Math.floor((now + settings.utc_offset_minutes * 60_000) / 86_400_000);
  return Math.round(date / 86_400_000) - today;
}
export function sprintRequest(mode: 'diagnostic' | 'question', question: string) {
  return mode === 'diagnostic'
    ? '请根据我的考试日期、已确认范围、当前目标和已有作答，先用少量问题判断今天应优先复习什么。每次只给一个问题，等我作答后再继续；不要把看过视频当作已掌握。'
    : question.trim();
}
