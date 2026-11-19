import { t as uiText } from './i18n';
import type { PresentationCommand } from './generated/PresentationCommand';
import type { PresentationFollowUp } from './generated/PresentationFollowUp';
import type { RunSnapshot } from './agent-run-state';
import { api } from './api';
import { sceneKey } from './reader-context';

export interface PageOperationResult {
  status: 'observed' | 'applied' | 'conflict' | 'failed' | 'cancelled';
  receipt?: PresentationFollowUp;
  reason?: string;
  support?: { region_id: string; text: string }[];
}
export interface PresentationHost {
  pageId: () => string;
  accepts: (command: PresentationCommand) => boolean;
  execute: (command: PresentationCommand, signal: AbortSignal) => Promise<PageOperationResult>;
}
const hosts = new Set<PresentationHost>();
export function registerPresentationHost(host: PresentationHost) {
  hosts.add(host);
  return () => { hosts.delete(host); };
}

/** Run-owned dispatch: claim once, never execute historical/reconnected commands twice. */
export function presentationDispatcher() {
  const attempted = new Set<string>();
  let active: { id: string; controller: AbortController } | undefined;
  function stop() { active?.controller.abort(); active = undefined; }
  function update(snapshot: RunSnapshot) {
    const command = snapshot.presentation_command;
    if (snapshot.persistence_state !== 'pending' || !['running', 'waiting_model'].includes(snapshot.execution_state)
      || !command || command.command_id !== active?.id) stop();
    if (!command || snapshot.persistence_state !== 'pending' || !['running', 'waiting_model'].includes(snapshot.execution_state)
      || command.run_turn_id !== snapshot.descriptor.turn_id || command.session_id !== snapshot.descriptor.session_id
      || attempted.has(command.command_id)) return;
    attempted.add(command.command_id);
    const current = { id: command.command_id, controller: new AbortController() };
    active = current;
    const scene = sceneKey();
    const candidates = [...hosts].filter(host => host.accepts(command));
    const host = command.page_id ? candidates.find(host => host.pageId() === command.page_id) : candidates.at(-1);
    const pageId = host?.pageId() ?? null;
    const live = () => active === current && !current.controller.signal.aborted && sceneKey() === scene
      && !!host && hosts.has(host) && host.pageId() === pageId && host.accepts(command);
    const canReport = () => active === current && !current.controller.signal.aborted && sceneKey() === scene;
    void (async () => {
      const claim = await api.presentationInteraction({
        operation: 'claim', command_id: command.command_id, run_turn_id: command.run_turn_id, page_id: pageId,
      });
      if (!claim.claimed || !canReport()) return;
      let result: PageOperationResult;
      try {
        if (!live()) throw new Error(uiText('演示页面已切换，请重新观察。'));
        result = await host!.execute(command, current.controller.signal);
        if (!live()) result = { status: 'failed', reason: uiText('演示页面已切换，请重新观察。') };
      }
      catch (failure) {
        result = { status: current.controller.signal.aborted ? 'cancelled' : 'failed',
          reason: failure instanceof Error ? failure.message : String(failure) };
      }
      if (!canReport()) return;
      await api.presentationInteraction({
        operation: 'complete', command_id: command.command_id, run_turn_id: command.run_turn_id, page_id: pageId, ...result,
      });
    })().catch(() => {
      // An uncertain transport result must not re-execute a side effect.
      if (active === current) stop();
    });
  }
  return { update, stop };
}
