// @vitest-environment happy-dom
import { beforeEach, expect, it, vi } from 'vitest';
import source from './presentation-actions.js?raw';
const create = new Function(`return (${source})`)();
beforeEach(() => { document.body.innerHTML = '<input data-presentation-parameter="time" type="number" min="0" max="10"><button data-presentation-action="apply">Apply</button>'; });

it('OM4 compares human edits of targeted parameters without rejecting natural playback progress', async () => {
  const values = { time: 4 };
  const edits = { time: 0 };
  const handler = vi.fn();
  const actions = create([{ kind: 'set_parameters', parameters: ['time'] }, { kind: 'invoke', action_id: 'apply', label: 'Apply' }],
    () => ({ values }), () => handler, () => edits);
  const request = { operation: 'act', action: { kind: 'invoke', action_id: 'apply' } };
  expect(await actions.execute(request, { time: 1 }, undefined, { time: 0 })).toBe('applied');
  expect(handler).toHaveBeenCalledTimes(1);
  edits.time++;
  expect(await actions.execute(request, { time: 1 }, undefined, { time: 0 })).toBe('conflict');
  expect(handler).toHaveBeenCalledTimes(1);
});

it('OM4 waits for an asynchronous handler, rejects missing bindings and invalid parameter values', async () => {
  const values = { time: 1 };
  let finish!: () => void;
  const handler = vi.fn(() => new Promise<void>(resolve => { finish = () => { values.time = 2; resolve(); }; }));
  const contract = [{ kind: 'set_parameters', parameters: ['time'] }];
  const actions = create(contract, () => ({ values }), () => handler);
  await expect(actions.execute({ operation: 'act', action: { kind: 'set_parameters', values: { time: 11 } } }, {})).rejects.toThrow('range');
  expect(handler).not.toHaveBeenCalled();
  let done = false;
  const task = actions.execute({ operation: 'act', action: { kind: 'set_parameters', values: { time: 2 } } }, {}).then((status: string) => { done = true; return status; });
  await Promise.resolve();
  expect(done).toBe(false);
  finish(); expect(await task).toBe('applied');
  document.querySelector('input')!.remove();
  expect(actions.inspect()).toContain('Parameter time needs a bound control and reader value.');
  expect(create(contract, () => ({ values }), () => undefined).inspect()).toContain('Register registerActionHandler for declared actions.');
});
