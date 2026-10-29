/** Parallel model workers share one process for durable task and publication writes. */
import { createServer } from 'node:http';
import { createInterface } from 'node:readline';
import type { AutomaticBuildExecutorSessionResponse } from '../../packages/core/src/automatic-build-executor-session';
import { BUILD_EXECUTOR_BOOTSTRAP_CONTRACT_V3 as bootstrap } from '../../packages/core/src/build-executor-connection-capability';
import { createBuildExecutorMcpSession } from './build-executor-mcp';
import type { VideoExecutorControl } from './video-executor-runtime';

type Handler = (request: unknown) => AutomaticBuildExecutorSessionResponse;

export async function createVideoExecutorCoordinator(privateRoot: string) {
  let pending: Promise<unknown> = Promise.resolve();
  function runExclusive<T>(operation: () => T | Promise<T>): Promise<T> {
    const next = pending.then(operation);
    pending = next.catch(() => undefined);
    return next;
  }
  const sessions = new Map<string, { session: ReturnType<typeof createBuildExecutorMcpSession>;
    onExchange?: (request: unknown, response: unknown) => void }>();
  const server = createServer(async (request, response) => {
    try {
      if (request.method !== 'POST') { response.writeHead(405).end(); return; }
      const chunks: Buffer[] = []; let bytes = 0;
      for await (const chunk of request) {
        bytes += chunk.length;
        // Same request carrier limit as the existing stdio server.
        if (bytes > 131_072) { response.writeHead(413).end(); return; }
        chunks.push(chunk);
      }
      const message = JSON.parse(Buffer.concat(chunks).toString('utf8'));
      await runExclusive(async () => {
        const entry = sessions.get(request.url ?? '');
        if (!entry) { response.writeHead(404).end(); return; }
        const result = await entry.session.handle_message(message);
        entry.onExchange?.(message, result);
        if (result === undefined) { response.writeHead(204).end(); return; }
        response.writeHead(200, { 'content-type': 'application/json' }).end(JSON.stringify(result));
      });
    } catch (error) {
      response.writeHead(500, { 'content-type': 'text/plain' }).end(error instanceof Error ? error.message : String(error));
    }
  });
  await new Promise<void>((resolve, reject) => {
    server.once('error', reject);
    server.listen(0, '127.0.0.1', () => { server.off('error', reject); resolve(); });
  });
  const address = server.address();
  if (!address || typeof address === 'string') throw new Error('Video coordinator did not bind its local port');
  return {
    register(handoff: string, handler: Handler, onExchange?: (request: unknown, response: unknown) => void): string {
      const route = '/' + handoff;
      if (sessions.has(route)) throw new Error('Video handoff already has a worker');
      sessions.set(route, { session: createBuildExecutorMcpSession({ bootstrap_version: bootstrap.version,
        protocol_generation: bootstrap.session_protocol, session_private_root: privateRoot, execute_request: handler }), onExchange });
      return `http://127.0.0.1:${address.port}${route}`;
    },
    unregister(handoff: string) { sessions.delete('/' + handoff); },
    runExclusive,
    async close() {
      await pending;
      sessions.clear();
      await new Promise<void>((resolve, reject) => server.close(error => error ? reject(error) : resolve()));
    },
  };
}

/** The Codex-owned MCP subprocess relays protocol bytes; it never writes task state. */
export async function proxyVideoExecutor(endpoint: string) {
  for await (const line of createInterface({ input: process.stdin })) {
    if (!line.trim()) continue;
    const result = await fetch(endpoint, { method: 'POST', headers: { 'content-type': 'application/json' }, body: line });
    if (!result.ok) throw new Error(`Video coordinator ${result.status}: ${await result.text()}`);
    if (result.status === 204) continue;
    const body = await result.text();
    await new Promise<void>((resolve, reject) => process.stdout.write(body + '\n', error => error ? reject(error) : resolve()));
  }
}

/** Stop refilling on the first failure and await every running worker's cost settlement. */
export async function runVideoExecutorPool<T>(items: T[], concurrency: number, control: VideoExecutorControl,
  run: (item: T, control: VideoExecutorControl) => Promise<void>) {
  if (concurrency !== 1 && concurrency !== 2) throw new Error('Video execution supports one or two slots');
  control.signal?.throwIfAborted();
  const stopped = new AbortController();
  const abort = () => stopped.abort(control.signal?.reason);
  control.signal?.addEventListener('abort', abort, { once: true });
  let ordinal = 0, failure: unknown;
  try {
    await Promise.all(Array.from({ length: Math.min(concurrency, items.length) }, async () => {
      while (!stopped.signal.aborted && ordinal < items.length) {
        const item = items[ordinal++];
        try { await run(item, { ...control, signal: stopped.signal }); }
        catch (error) { if (!stopped.signal.aborted) { failure = error; stopped.abort(error); } break; }
      }
    }));
    if (failure !== undefined) throw failure;
    control.signal?.throwIfAborted();
  } finally { control.signal?.removeEventListener('abort', abort); }
}
