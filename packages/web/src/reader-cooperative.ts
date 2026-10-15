/** A task boundary (not a microtask), so input and painting can run between batches. */
export function yieldReaderTask(): Promise<void> {
  return new Promise(resolve => {
    const channel = new MessageChannel();
    channel.port1.onmessage = () => {
      channel.port1.close(); channel.port2.close(); resolve();
    };
    channel.port2.postMessage(null);
  });
}

export async function runReaderWork<T>(work: Generator<void, T>, yieldTask = yieldReaderTask): Promise<T> {
  let deadline = performance.now() + 4;
  for (;;) {
    const next = work.next();
    if (next.done) return next.value;
    if (performance.now() >= deadline) { await yieldTask(); deadline = performance.now() + 4; }
  }
}
