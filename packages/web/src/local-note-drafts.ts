import type { NoteDraft } from './personal-notes';

/** Serialize native draft writes and retain failed changes for a close/save retry. */
export function createLocalNoteDraftWriter(
  write: (draftId: string, draft: NoteDraft | null) => Promise<void>,
  reportError: (message: string) => void,
) {
  const pending = new Map<string, NoteDraft | null>();
  let tail = Promise.resolve();
  async function persist(id: string) {
    if (!pending.has(id)) return;
    const value = pending.get(id)!;
    try {
      await write(id, value);
      if (pending.get(id) === value) pending.delete(id);
      if (!pending.size) reportError('');
    } catch (error) {
      reportError('设备草稿暂未写入本机；请保持窗口并再次编辑或重试保存。');
      throw error;
    }
  }
  function queue(id: string, value: NoteDraft | null) {
    pending.set(id, value === null ? null : JSON.parse(JSON.stringify(value)));
    const operation = tail.then(() => persist(id));
    tail = operation.catch(() => {});
  }
  async function flush() {
    await tail;
    while (pending.size) {
      const ids = [...pending.keys()];
      const operation = tail.then(async () => { for (const id of ids) await persist(id); });
      tail = operation.catch(() => {});
      await operation;
    }
  }
  return { queue, flush, hasPending: () => pending.size > 0 };
}
