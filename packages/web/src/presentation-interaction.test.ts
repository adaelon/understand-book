// @vitest-environment happy-dom
import { afterEach, expect, it, vi } from 'vitest';
import { flushPromises } from '@vue/test-utils';
import { api } from './api';
import { initialRun, reduceRun } from './agent-run-state';
import { presentationDispatcher, registerPresentationHost, type PageOperationResult } from './presentation-interaction';
import type { PresentationCommand } from './generated/PresentationCommand';

const command: PresentationCommand = { command_id: 'c', run_turn_id: 'run', session_id: 's', delivery_turn_id: 'delivery',
  request: { operation: 'observe', reference: { presentation_id: 'p', revision: 1 } }, page_id: null, expected_values: {}, expected_edits: {}, available_actions: [] };
const running = () => ({ ...initialRun({ book_id: 'book', session_id: 's', turn_id: 'run' }), presentation_command: command });
afterEach(() => vi.restoreAllMocks());

it('claims once and does not replay a command from a reconnected snapshot', async () => {
  const request = vi.spyOn(api, 'presentationInteraction').mockResolvedValue({ claimed: true, accepted: true });
  const execute = vi.fn().mockResolvedValue({ status: 'observed', receipt: { saved_state_ref: 'saved' } });
  const remove = registerPresentationHost({ accepts: () => true, pageId: () => 'page', execute });
  const dispatch = presentationDispatcher();
  try {
    dispatch.update(running()); await flushPromises();
    dispatch.stop(); dispatch.update(running()); await flushPromises();
    expect(execute).toHaveBeenCalledTimes(1);
    expect(request.mock.calls.map(([input]) => input.operation)).toEqual(['claim', 'complete']);
  } finally { dispatch.stop(); remove(); }
});

it('cancels while awaiting a result and never submits a delayed success', async () => {
  const request = vi.spyOn(api, 'presentationInteraction').mockResolvedValue({ claimed: true });
  let finish!: (value: PageOperationResult) => void;
  let signal!: AbortSignal;
  const remove = registerPresentationHost({ accepts: () => true, pageId: () => 'page',
    execute: (_command, current) => { signal = current; return new Promise(resolve => { finish = resolve; }); } });
  const dispatch = presentationDispatcher();
  try {
    dispatch.update(running()); await flushPromises();
    dispatch.update({ ...running(), execution_state: 'cancelling' });
    expect(signal.aborted).toBe(true);
    finish({ status: 'applied', receipt: { session_id: 's', turn_id: 'delivery',
      reference: command.request.reference, state_revision: 1, saved_state_ref: 'late' } }); await flushPromises();
    expect(request).toHaveBeenCalledTimes(1);
  } finally { dispatch.stop(); remove(); }
});

it('a restored frame cannot claim an action bound to the previous page', async () => {
  const request = vi.spyOn(api, 'presentationInteraction').mockResolvedValue({ claimed: false });
  const execute = vi.fn();
  const remove = registerPresentationHost({ accepts: () => true, pageId: () => 'new-page', execute });
  const dispatch = presentationDispatcher();
  try {
    dispatch.update({ ...running(), presentation_command: { ...command, page_id: 'old-page' } });
    await flushPromises();
    expect(request.mock.calls[0][0].page_id).toBeNull();
    expect(execute).not.toHaveBeenCalled();
  } finally { dispatch.stop(); remove(); }
});

it('the run reducer retains live commands and clears them at the saved completion boundary', () => {
  const first = reduceRun(initialRun(running().descriptor), { turn_id: 'run', seq: 1, elapsed_ms: 0, type: 'presentation.command', payload: command });
  expect(first.presentation_command).toEqual(command);
  expect(reduceRun(first, { turn_id: 'run', seq: 2, elapsed_ms: 0, type: 'presentation.command', payload: null }).presentation_command).toBeNull();
  expect(reduceRun(first, { turn_id: 'other', seq: 2, elapsed_ms: 0, type: 'presentation.command', payload: null })).toBe(first);
});
