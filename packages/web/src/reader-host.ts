import { ApiError } from './api-error';
import { network } from './reader-context';
import type { RunDescriptor } from './agent-run-state';
import type { PresentationRef } from './generated/PresentationRef';
import type { SharedInstance, SharedRelease } from './shared-workspaces';

export interface RunError { error_code: string; category: string; message: string; execution_error?: RunError | null }
export interface RunNotice { code: string; message: string }
export type ResultPersistence = 'saved' | 'pending' | 'failed' | 'unsubmitted';
export interface PendingSubmission { key: string; body: Record<string, unknown>; workspace: string; bodyStored?: boolean }

/** Only operations whose ownership or transport differs between the two hosts. */
export interface ReaderHost {
  request<T>(method: 'GET' | 'POST', path: string, body?: unknown, signal?: AbortSignal): Promise<T>;
  fetch<T>(method: string, path: string, body?: unknown, signal?: AbortSignal): Promise<T>;
  ownerHeaders(): Record<string, string>;
  publishedUrl(leaf: string): string;
  workspaceAction<T>(action: string, body?: Record<string, unknown>): Promise<T>;
  recoverSubmissions(): Promise<RunDescriptor[]>;
  recoverWorkspaceBinding(): Promise<void>;
  requestWorkspaceRecovery(failure: unknown): void;
  pendingSubmissions(): PendingSubmission[];
  retrySubmission(key: string): Promise<void>;
  readCourseInstance(id: string): Promise<{ instance: SharedInstance | null }>;
  readCourseRelease(id: string, version: number): Promise<SharedRelease>;
  openLinkedPresentation(sessionId: string, turnId: string, reference: PresentationRef): Promise<void>;
  runNotice(error: RunError | null | undefined, persistence: ResultPersistence): RunNotice | undefined;
  runFailureMessage(code: string): string | undefined;
}

let host: ReaderHost | undefined;
/** A hosted entry installs its adapter before mounting the common Reader. */
export function installReaderHost(value: ReaderHost) { host = value; }
function hosted(): ReaderHost {
  if (!host) throw new Error('Reader host has not been installed');
  return host;
}

export async function localRequest<T>(method: string, path: string, body?: unknown, signal?: AbortSignal): Promise<T> {
  const init: RequestInit = { method, signal };
  if (body !== undefined) {
    init.headers = { 'Content-Type': 'application/json' };
    init.body = JSON.stringify(body);
  }
  const response = await fetch(`/api${path}`, init);
  const raw = await response.text();
  const value = raw ? JSON.parse(raw) : null;
  if (!response.ok) throw new ApiError(response.status, value?.error_code ?? `HTTP_${response.status}`,
    value?.category ?? 'internal', value?.message ?? raw);
  return value as T;
}

export function readerRequest<T>(method: 'GET' | 'POST', path: string, body?: unknown, signal?: AbortSignal): Promise<T> {
  return network.value.enabled ? hosted().request<T>(method, path, body, signal) : localRequest<T>(method, path, body, signal);
}
/** Already scoped routes (courses, published assets and workspace notebooks). */
export function hostFetch<T>(method: string, path: string, body?: unknown, signal?: AbortSignal): Promise<T> {
  return network.value.enabled ? hosted().fetch<T>(method, path, body, signal) : localRequest<T>(method, path, body, signal);
}
export function ownerHeaders() { return network.value.enabled ? hosted().ownerHeaders() : {}; }
export const publishedUrl = (leaf: string) => hosted().publishedUrl(leaf);
export const workspaceAction = <T>(action: string, body?: Record<string, unknown>) => hosted().workspaceAction<T>(action, body);
export const recoverSubmissions = () => hosted().recoverSubmissions();
export const recoverWorkspaceBinding = () => hosted().recoverWorkspaceBinding();
export const requestWorkspaceRecovery = (failure: unknown) => { if (network.value.enabled) hosted().requestWorkspaceRecovery(failure); };
export const pendingSubmissions = () => hosted().pendingSubmissions();
export const retrySubmission = (key: string) => hosted().retrySubmission(key);
export const courseCollection = {
  instance: (id: string) => hosted().readCourseInstance(id),
  read: (id: string, version: number) => hosted().readCourseRelease(id, version),
};
export const openLinkedPresentation = (sessionId: string, turnId: string, reference: PresentationRef) => hosted().openLinkedPresentation(sessionId, turnId, reference);
export const runNotice = (error: RunError | null | undefined, persistence: ResultPersistence) => host?.runNotice(error, persistence);
export const runFailureMessage = (code: string) => host?.runFailureMessage(code);
