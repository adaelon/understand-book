import { t as uiText } from './i18n';
import { spendFailureMessage } from './account-allowance';
import type { AgentAnswerView } from "./generated/AgentAnswerView";
import type { RunActivity } from "./generated/RunActivity";
import type { AgentEffect, ReaderState, AgentChatTurn } from "./api";
import type { BoardDelivery } from './generated/BoardDelivery';
import { boardDeliveryKey, mergeBoardDeliveries } from './board-workspace';
export type { RunActivity };

export interface RunDescriptor { workspace_id?: string; workspace_generation?: number; published_book_ref?: { book_id: string; publication_id: string }; book_id: string; session_id: string; turn_id: string }
export interface AnswerDraft { message_id: number; revision: number; operation: string; view: AgentAnswerView | null }
export interface RunSnapshot {
  board_commits?: BoardDelivery[];
  music_command?: import('./music-api').MusicCommand | null;
  presentation_command?: import("./generated/PresentationCommand").PresentationCommand | null;
  observation_epoch?: string;
  reader_state?: ReaderState | null;
  effects?: { effect_id: string; effect: AgentEffect }[];
  draft?: AnswerDraft | null;
  descriptor: RunDescriptor;
  last_seq: number;
  execution_state: "queued" | "waiting_model" | "running" | "cancelling" | "finalizing" | "completed" | "failed" | "cancelled" | "ended" | "interrupted";
  persistence_state: "pending" | "saved" | "failed";
  activities: RunActivity[];
  final_view: AgentChatTurn | null;
  error: import('./account-allowance').SpendError | null;
}
export interface RunEvent { observation_epoch?: string; live_buffer_reset?: boolean; turn_id: string; seq: number; elapsed_ms: number; type: string; payload: unknown }
export function initialRun(descriptor: RunDescriptor): RunSnapshot {
  return { descriptor, last_seq: -1, execution_state: "running", persistence_state: "pending", activities: [], board_commits: [], final_view: null, error: null };
}
export function interruptRun(
  state: RunSnapshot,
  error: NonNullable<RunSnapshot["error"]>,
): RunSnapshot {
  return {
    ...state,
    execution_state: "interrupted",
    persistence_state: "failed",
    error,
  };
}
export function reduceRun(state: RunSnapshot, event: RunEvent): RunSnapshot {
  if (event.turn_id !== state.descriptor.turn_id) return state;
  const isSnapshot = event.type === 'run.snapshot' || ['run.completed', 'run.failed', 'run.cancelled', 'run.persistence_failed'].includes(event.type);
  if (isSnapshot) {
    const snapshot = event.payload as RunSnapshot;
    if (snapshot.descriptor.book_id !== state.descriptor.book_id || snapshot.descriptor.session_id !== state.descriptor.session_id || snapshot.descriptor.turn_id !== state.descriptor.turn_id) return state;
    // Durable recovery can discover a saved part without advancing the live event cursor.
    const commits = mergeBoardDeliveries(state.board_commits, snapshot.board_commits, snapshot.final_view?.board_commits);
    if (commits.length !== (state.board_commits?.length ?? 0) || commits.some((part, index) => boardDeliveryKey(part) !== boardDeliveryKey(state.board_commits![index]))) state = { ...state, board_commits: commits };
  }
  const reset = event.type === "run.snapshot" && event.live_buffer_reset === true && !!event.observation_epoch;
  if (!reset && ((state.observation_epoch && event.observation_epoch !== state.observation_epoch) || event.seq <= state.last_seq)) return state;
  if (isSnapshot) {
    const snapshot = event.payload as RunSnapshot;
    return { ...snapshot, board_commits: mergeBoardDeliveries(state.board_commits, snapshot.board_commits, snapshot.final_view?.board_commits), descriptor: { ...state.descriptor, ...snapshot.descriptor }, last_seq: event.seq, observation_epoch: event.observation_epoch ?? state.observation_epoch };
  }
  const next = { ...state, last_seq: event.seq };
  if (event.type === 'board.committed') next.board_commits = mergeBoardDeliveries(state.board_commits, [(event.payload as { delivery: BoardDelivery }).delivery]);
  if (event.type === "run.started") next.execution_state = "running";
  if (event.type === "run.resource") next.execution_state = (event.payload as { execution_state: RunSnapshot['execution_state'] }).execution_state;
  if (event.type === "presentation.command") next.presentation_command = event.payload as RunSnapshot["presentation_command"];
  if (event.type === 'music.command') next.music_command = event.payload as RunSnapshot['music_command'];
  if (event.type === "reader.changed") {
    const reader = event.payload as ReaderState;
    if (reader.book_id === state.descriptor.book_id && reader.revision >= (state.reader_state?.revision ?? -1)) next.reader_state = reader;
  }
  if (event.type === "effect.created") {
    const effect = event.payload as { effect_id: string; effect: AgentEffect };
    next.effects = [...(state.effects ?? [])];
    const index = next.effects.findIndex(old => old.effect_id === effect.effect_id);
    if (index < 0) next.effects.push(effect); else next.effects[index] = effect;
  }
  if (event.type === "answer.patch") {
    const patch = event.payload as AnswerDraft;
    const previous = state.draft;
    if (!previous || patch.message_id > previous.message_id || (patch.message_id === previous.message_id && patch.revision >= previous.revision)) {
      if (patch.operation === "append") {
        if (previous?.message_id === patch.message_id && previous.revision === patch.revision && previous.view && patch.view) {
          const parts = previous.view.parts.map(part => ({ ...part }));
          for (const part of patch.view.parts) {
            const last = parts.at(-1);
            if (last?.kind === "markdown" && part.kind === "markdown") last.text += part.text;
            else parts.push({ ...part });
          }
          const sources = [...previous.view.sources];
          for (const source of patch.view.sources) if (!sources.some(old => old.source_ref_id === source.source_ref_id)) sources.push(source);
          next.draft = { ...patch, operation: "replace", view: { parts, sources } };
        }
      } else next.draft = patch;
    }
  }
  if (event.type === "run.cancelling") next.execution_state = "cancelling";
  if (event.type === "run.finalizing") next.execution_state = "finalizing";
  if (/^(model|tool|commentary)\.(started|finished)$/.test(event.type)) {
    const activity = event.payload as RunActivity;
    next.activities = [...state.activities];
    const index = next.activities.findIndex((old) => old.step_id === activity.step_id);
    if (index < 0) next.activities.push(activity); else next.activities[index] = activity;
    next.activities.sort((a, b) => a.step_id - b.step_id);
  }
  return next;
}
export function runStatusText(snapshot: RunSnapshot, connection: string): string {
  if (snapshot.error?.error_code === "SENSITIVE_CONFIRMATION_EXPIRED") return uiText('原确认已过期，需要重新确认');
  if (snapshot.error?.error_code === "INTERRUPTED") return uiText('运行已中断；已发生的操作保留，问题不会自动重提');
  if (snapshot.execution_state === "queued") return uiText('正在排队');
  if (snapshot.execution_state === "waiting_model") return uiText('正在等待模型资源');
  if (snapshot.error?.error_code === "AGENT_RUN_NOT_FOUND") return uiText('无法核对原运行；问题不会自动重提');
  if (snapshot.error?.error_code === "OBJECT_NOT_FOUND" || snapshot.error?.error_code === "REQUEST_KEY_CLOSED") return uiText('原运行或对话已不可用；问题不会自动重提');
  if (connection === "authentication") return uiText('认证已失效；请通过现有受保护入口恢复登录');
  if (connection === "offline") return uiText('暂时无法核对运行；问题不会自动重提');
  if (snapshot.persistence_state === "failed") return uiText('运行已结束，但结果未保存');
  if (snapshot.execution_state === "cancelled") return uiText('已停止');
  if (snapshot.execution_state === "failed") return uiText('运行失败');
  if (snapshot.execution_state === "completed") return uiText('运行已结束');
  if (snapshot.execution_state === "cancelling") return uiText('正在停止，等待当前请求退出');
  if (snapshot.execution_state === "finalizing") return uiText('正在保存结果');
  if (connection === "reconnecting") return uiText('连接中断，正在恢复活动');
  const current = [...snapshot.activities].reverse().find(activity => activity.status === 'running' && activity.kind !== 'commentary' && activity.name !== 'agent.progress');
  if (current?.label) return uiText(current.label);
  return uiText('正在运行');
}

/** Explain the failed delivery without exposing the model's internal tool contract. */
export function agentFailureMessage(error: AgentChatTurn['error'], summary?: AgentChatTurn['run_summary']): string | undefined {
  if (!error) return;
  if (error.error_code === 'AGENT_RUN_CANCELLED') return;
  if (error.error_code === 'VIDEO_DRAFT_INVALID') return uiText('问题未提交：视频提问内容无效，请检查所选画面或附件后重新发送。');
  if (error.error_code === 'HTTP_413' || error.error_code === 'REQUEST_TOO_LARGE') return uiText('图片或附件过大，请缩小后重新发送。');
  if (error.error_code === 'SUBMISSION_STORAGE_FAILED') return uiText('问题尚未发送，浏览器暂时无法保存重试信息。请保留内容后重试。');
  const trace = summary?.trace ?? [];
  const reviewFailed = error.error_code === 'TEACHING_REVIEW_FAILED'
    || (error.error_code === 'TEACHING_INVALID' && summary?.activities?.some(activity => activity.name === 'tutor_delivery_review' && activity.status === 'failed'));
  const pageSaved = trace.some(step => {
    if (step.tool !== 'presentation.author' || step.result_digest.includes('"error_code"')) return false;
    try { return JSON.parse(step.args).operation === 'deliver'; } catch { return false; }
  });
  if (reviewFailed || (error.error_code === 'TEACHING_INVALID' && pageSaved)) {
    return pageSaved
      ? uiText('演示页已生成，但回答检查未完成。请重试。')
      : uiText('回答检查未完成。请重试。');
  }
  const lastTool = trace.at(-1)?.tool;
  if (error.error_code.startsWith('PRESENTATION_')
    || (error.error_code === 'TEACHING_INVALID' && lastTool === 'presentation.author')) {
    return uiText('演示页制作未完成。请重试。');
  }
  if (error.error_code === 'TEACHING_INVALID' || error.error_code === 'TEACHING_DESIGN_REQUIRES_NEXT_SAMPLING') {
    return uiText('这次讲解尚未准备完成，请重试。');
  }
  const spend = spendFailureMessage(error.error_code);
  if (spend) return spend;
  if (error.error_code === 'TURN_UNSAVED' || error.category === 'storage') return uiText('结果保存失败，请重试保存。');
  if (error.error_code === 'INTERRUPTED' || error.error_code === 'AGENT_RUN_NOT_FOUND') return uiText('这次回答已中断，请重新提交。');
  if (error.category === 'provider' || error.category === 'protocol' || error.category === 'capacity') return uiText('服务暂时不可用，请稍后重试。');
  return uiText('这次回答未能完成，请重试。');
}
