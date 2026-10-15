// @vitest-environment happy-dom
import { expect, it, vi } from 'vitest';
import { createVideoQuestion, videoTime, type VideoDraft, type DraftCommand } from './video-question';
import type { VideoQuestionContext } from '../../core/src/generated/VideoQuestionContext';
const video = { media_id: 'v', media_revision: '1' };
export const question = (id: string, pts = 492000): VideoQuestionContext => ({ draft_id: id,
  publication: { book_id: 'b', publication_id: 'original' }, material: { book_id: 'b', video,
    transcript: { transcript_id: 't', transcript_revision: '1', video } },
  frame: { video, actual_pts_ms: pts, width: 1920, height: 1080 }, region: null, range: null });
function storage() {
  const drafts = new Map<string, VideoDraft>();
  return vi.fn(async (input: DraftCommand) => {
    if (input.action === 'save') drafts.set(input.window_id, structuredClone(input.draft!));
    if (input.action === 'release') drafts.delete(input.window_id);
    return { draft: drafts.get(input.window_id) ?? null };
  });
}
it('binds 08:12 once through composition, paste and editing, then sends at 08:27', async () => {
  const command = storage(); let position = 492000;
  const capture = vi.fn(id => question(id, position));
  const scene = createVideoQuestion('a', command, capture);
  expect(capture).not.toHaveBeenCalled(); // focus does not enter the input state machine
  scene.input('n'); scene.input('你'); position = 507000;
  scene.input('你能解释这行代码吗？ pasted');
  const meta = await scene.submission();
  expect(capture).toHaveBeenCalledTimes(1);
  expect(scene.draft.value?.context.frame.actual_pts_ms).toBe(492000);
  expect(meta.video_draft_id).toBe(scene.draft.value?.context.draft_id);
  expect(scene.draft.value?.context.publication.publication_id).toBe('original');
});
it('retains draft and binding on save/send failure and restores without recapturing', async () => {
  const command = storage(), capture = vi.fn(question);
  const scene = createVideoQuestion('a', command, capture);
  command.mockRejectedValueOnce(new Error('offline'));
  scene.input('question'); await expect(scene.submission()).rejects.toThrow('offline');
  const id = scene.draft.value!.context.draft_id;
  scene.input('question retry'); await scene.submission();
  const restored = createVideoQuestion('a', command, capture); await restored.restore();
  expect(restored.draft.value?.context.draft_id).toBe(id);
  expect(restored.draft.value?.message).toBe('question retry');
  expect(capture).toHaveBeenCalledTimes(1);
  // A rejected send leaves the same state; only a confirmed acceptance releases it.
  await restored.submission(); expect(restored.draft.value?.context.draft_id).toBe(id);
  restored.accepted('different'); expect(restored.draft.value).not.toBeNull();
  restored.accepted(id); expect(restored.draft.value).toBeNull();
});
it('clearing and cancellation release a binding; selection-only drafts remain bound', async () => {
  const command = storage(), capture = vi.fn(question);
  const scene = createVideoQuestion('a', command, capture);
  scene.input('first'); const first = await scene.submission(); scene.input('');
  scene.input('second'); const second = await scene.submission();
  expect(second.video_draft_id).not.toBe(first.video_draft_id);
  scene.draft.value!.context.range = { video, start_ms: 492000, end_ms: 493000 };
  scene.input(''); await scene.submission(); expect(scene.draft.value).not.toBeNull();
  scene.clear(); await scene.restore(); expect(scene.draft.value).toBeNull();
});
it('keeps windows independent and ignores a late restore after new input', async () => {
  const command = storage();
  const a = createVideoQuestion('a', command, question), b = createVideoQuestion('b', command, id => question(id, 507000));
  a.input('a'); b.input('b'); await Promise.all([a.submission(), b.submission()]);
  await Promise.all([a.restore(), b.restore()]);
  expect(a.draft.value?.context.frame.actual_pts_ms).toBe(492000);
  expect(b.draft.value?.context.frame.actual_pts_ms).toBe(507000);
  let complete!: (value: { draft: null }) => void;
  command.mockImplementationOnce(() => new Promise(resolve => { complete = resolve; }));
  const restore = a.restore(); await Promise.resolve(); await Promise.resolve();
  a.input('new editing'); complete({ draft: null }); await restore; await a.submission();
  expect(a.draft.value?.message).toBe('new editing');
});

it('shows the presented frame at 08:12 when its actual PTS is just before the second', () => {
  expect(videoTime(491966.688)).toBe('08:12');
});

it('creates, restores and cancels a selection-only draft; text retains its original frame', async () => {
  const command = storage(), capture = vi.fn(question);
  const scene = createVideoQuestion('a', command, capture);
  const selected = question('selection');
  selected.region = { frame: selected.frame, rect_normalized: [.1, .2, .3, .4] };
  scene.selectRegion(selected); await scene.submission();
  expect(capture).not.toHaveBeenCalled();
  scene.input(''); await scene.restore();
  expect(scene.draft.value?.context.region).toEqual(selected.region);
  scene.clearRegion(); await scene.restore(); expect(scene.draft.value).toBeNull();
  scene.input('existing text'); await scene.submission();
  const original = JSON.parse(JSON.stringify(scene.draft.value!.context));
  scene.selectRegion({ ...original, region: { frame: original.frame, rect_normalized: [.2, .3, .4, .5] } });
  await scene.submission();
  scene.clearRegion(); await scene.restore();
  expect(scene.draft.value?.message).toBe('existing text');
  expect(scene.draft.value?.context).toEqual(original);
  scene.selectRegion(selected);
  expect(scene.error.value).toContain('回到问题绑定画面');
  expect(scene.draft.value?.context).toEqual(original);
});

it('keeps a selected region after a failed save and releases it only on accepted handoff', async () => {
  const command = storage(), scene = createVideoQuestion('a', command, question);
  const selected = question('selection');
  selected.region = { frame: selected.frame, rect_normalized: [.1, .2, .3, .4] };
  command.mockRejectedValueOnce(new Error('offline'));
  scene.selectRegion(selected); await expect(scene.submission()).rejects.toThrow('offline');
  expect(scene.draft.value?.context.region).toEqual(selected.region);
  scene.input('retry'); await scene.submission();
  scene.accepted('selection'); expect(scene.draft.value).toBeNull();
});

it('edits a long range independently from region and text, restores it and releases the last content', async () => {
  const command = storage(), capture = vi.fn(question), scene = createVideoQuestion('a', command, capture);
  const selected = question('range');
  selected.range = { video, start_ms: 400000, end_ms: 510000 };
  scene.selectRange(selected); await scene.submission();
  expect(capture).not.toHaveBeenCalled();
  scene.selectRegion({ ...selected, region: { frame: selected.frame, rect_normalized: [.1, .2, .3, .4] } });
  // A range edit must not replace the region with the editor's old snapshot.
  scene.selectRange({ ...selected, range: { video, start_ms: 410000, end_ms: 500000 } });
  await scene.submission(); await scene.restore();
  expect(scene.draft.value?.context.region).not.toBeNull();
  expect(scene.draft.value?.context.range?.start_ms).toBe(410000);
  expect(scene.draft.value?.context.frame).toEqual(selected.frame);
  scene.clearRange(); await scene.submission(); expect(scene.draft.value?.context.region).not.toBeNull();
  scene.selectRange(selected); scene.clearRegion(); await scene.submission();
  expect(scene.draft.value?.context.range).toEqual(selected.range);
  scene.input('text'); scene.clearRange(); await scene.submission();
  expect(scene.draft.value?.context.draft_id).toBe('range');
  scene.selectRange(selected); scene.input(''); scene.clearRange(); await scene.restore();
  expect(scene.draft.value).toBeNull();
});

it('keeps a range after save failure, rejects changed binding and hands off only after retry', async () => {
  const command = storage(), scene = createVideoQuestion('a', command, question);
  const selected = question('range'); selected.range = { video, start_ms: 400000, end_ms: 510000 };
  command.mockRejectedValueOnce(new Error('offline'));
  scene.selectRange(selected); await expect(scene.submission()).rejects.toThrow('offline');
  expect(scene.draft.value?.context.range).toEqual(selected.range);
  scene.selectRange({ ...selected, draft_id: 'stale' });
  expect(scene.error.value).toContain('绑定已改变');
  expect(scene.draft.value?.context.draft_id).toBe('range');
  scene.selectRange(selected); await scene.submission(); scene.accepted('range');
  expect(scene.draft.value).toBeNull();
});
