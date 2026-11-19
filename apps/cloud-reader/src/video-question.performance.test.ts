// @vitest-environment happy-dom
import { writeFileSync } from 'node:fs';
import { afterEach, expect, it, vi } from 'vitest';
import { api } from '@understand-book/web/reader/api';
import { createVideoQuestion } from '@understand-book/web/reader/video-question';
import { installIdentity, installWorkspace, network, type NetworkWorkspace } from './network-context';

afterEach(() => { vi.useRealTimers(); vi.unstubAllGlobals(); network.value = { ...network.value, enabled: false, identity: null, workspace: null }; });

it('bounds the submission wait after typing over a slow connection', async () => {
  vi.useFakeTimers(); vi.setSystemTime(0);
  sessionStorage.clear();
  const video = { media_id: 'v', media_revision: '1' };
  const publication = { book_id: 'b', publication_id: 'p' };
  let workspace = { workspace_id: 'w', generation: 1, revision: 1, selected_chat: 'chat',
    published_book_ref: publication, reader: {} } as NetworkWorkspace;
  installIdentity({ user_id: 'submit-benchmark', csrf_token: 'test' }); installWorkspace(workspace);
  const requests: { method: string; url: string; at_ms: number }[] = [];
  let savedMessage = '';
  vi.stubGlobal('fetch', vi.fn(async (url: string, options: RequestInit) => {
    requests.push({ method: options.method!, url, at_ms: Date.now() });
    await new Promise(resolve => setTimeout(resolve, 500));
    const input = options.body ? JSON.parse(String(options.body)) : {};
    if (url.endsWith('/reader/video_draft')) {
      savedMessage = input.draft.message;
      workspace = { ...workspace, revision: workspace.revision + 1 };
      return new Response(JSON.stringify({ ...workspace, result: { draft: input.draft } }));
    }
    if (url.endsWith('/agent/runs')) return new Response(JSON.stringify({ book_id: 'b', session_id: 'chat', turn_id: 't', dispatch_state: 'queued' }));
    return new Response(JSON.stringify(workspace));
  }));
  const scene = createVideoQuestion('window', input => api.videoDraft(input), draft_id => ({ draft_id, publication,
    material: { book_id: 'b', video, transcript: { transcript_id: 't', transcript_revision: '1', video } },
    frame: { video, actual_pts_ms: 274000, width: 1920, height: 1080 }, region: null, range: null }));
  const message = '这里提到的资源管理和ai infra这本书的思想是不是差不多呢';
  for (let i = 1; i <= message.length; i++) { scene.input(message.slice(0, i)); await vi.advanceTimersByTimeAsync(80); }
  const clickedAt = Date.now();
  const submitted = scene.submission().then(meta => api.agentRunCreate(message, meta));
  await vi.runAllTimersAsync(); await submitted;
  const report = { scenario: '500 ms per HTTP request, 80 ms between input events; simulated network, no model',
    input_events: message.length, clicked_at_ms: clickedAt, submitted_at_ms: requests.find(r => r.url.endsWith('/agent/runs'))!.at_ms,
    click_to_submit_ms: requests.find(r => r.url.endsWith('/agent/runs'))!.at_ms - clickedAt,
    click_to_accepted_ms: Date.now() - clickedAt, http_requests: requests.length,
    draft_saves: requests.filter(r => r.url.endsWith('/reader/video_draft')).length, final_message_matches: savedMessage === message, requests };
  if (process.env.QUESTION_SUBMIT_REPORT) writeFileSync(process.env.QUESTION_SUBMIT_REPORT, JSON.stringify(report, null, 2) + '\n');
  expect(savedMessage).toBe(message);
  expect(report.click_to_submit_ms).toBeLessThanOrEqual(1500);
});
