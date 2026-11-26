import { network, sceneKey } from './reader-context';
import { computed, onBeforeUnmount, onMounted, ref, shallowRef } from "vue";
import { api, agentRunEventsUrl, ApiError } from "./api";
import { initialRun, interruptRun, reduceRun, type RunDescriptor, type RunEvent, type RunSnapshot } from "./agent-run-state";

import { presentationDispatcher } from "./presentation-interaction";
import { musicDispatcher } from "./music-interaction";
import { mergeBoardDeliveries } from './board-workspace';

const events = ["board.committed", "music.command", "presentation.command", "run.resource", "reader.changed", "effect.created","answer.patch","run.snapshot", "run.started", "model.started", "model.finished", "tool.started", "tool.finished", "commentary.finished", "teach_back.started", "teach_back.finished", "run.cancelling", "run.finalizing", "run.completed", "run.failed", "run.cancelled", "run.persistence_failed"];
export function useAgentRun(onTerminal: (snapshot: RunSnapshot) => void) {
  const presentations = presentationDispatcher();
  const music = musicDispatcher();
  const snapshot = shallowRef<RunSnapshot | null>(null);
  const connection = ref("closed");
  const cancellationPending = ref(false);
  let disposed = false;
  let cursor: string | undefined;
  let observedScene = sceneKey();
  let source: EventSource | null = null;
  let queued: RunSnapshot | null = null;
  let frame: number | null = null;
  let observationGeneration = 0;
  let terminalNotified = false;
  let reconcileTask: Promise<void> | null = null;
  let reconcileTimer: number | null = null;
  let retryDelay = 1000;

  function sameDescriptor(left: RunDescriptor | null | undefined, right: RunDescriptor): boolean {
    return !!left
      && left.book_id === right.book_id
      && left.session_id === right.session_id
      && left.turn_id === right.turn_id;
  }
  function flushDraft() {
    if (frame !== null) cancelAnimationFrame(frame);
    frame = null;
    const next = queued; queued = null;
    if (next) install(next);
  }
  const active = computed(() => snapshot.value?.persistence_state === "pending");
  const stopping = computed(() => active.value && (cancellationPending.value || snapshot.value?.execution_state === "cancelling"));
  function closeTransport() {
    if (frame !== null) cancelAnimationFrame(frame);
    frame = null;
    queued = null;
    source?.close();
    source = null;
  }
  function clearReconcileTimer() {
    if (reconcileTimer !== null) window.clearTimeout(reconcileTimer);
    reconcileTimer = null;
  }
  function close() {
    presentations.stop(); music.stop();
    observationGeneration += 1;
    cancellationPending.value = false;
    clearReconcileTimer();
    retryDelay = 1000;
    reconcileTask = null;
    closeTransport();
    connection.value = "closed";
  }
  function install(next: RunSnapshot) {
    const previous = snapshot.value;
    next = { ...next, board_commits: mergeBoardDeliveries(sameDescriptor(previous?.descriptor, next.descriptor) ? previous?.board_commits : [], next.board_commits, next.final_view?.board_commits) };
    snapshot.value = next;
    presentations.update(next); music.update(next);
    if (next.persistence_state !== "pending") {
      clearReconcileTimer();
      closeTransport();
      connection.value = next.error?.error_code === "AGENT_RUN_NOT_FOUND" ? "interrupted" : "closed";
      if (!terminalNotified && previous?.persistence_state === "pending") {
        terminalNotified = true;
        onTerminal(next);
      }
    }
  }

  function openSource(descriptor: RunDescriptor, generation: number) {
    if (source || generation !== observationGeneration || !active.value) return;
    const current = new EventSource(agentRunEventsUrl(descriptor.turn_id, cursor));
    source = current;
    current.onopen = () => {
      if (source !== current) return;
      connection.value = "connected";
      retryDelay = 1000;
      clearReconcileTimer();
    };
    current.onerror = () => {
      if (source !== current) return;
      connection.value = "reconnecting";
      presentations.stop(); music.stop();
      // HTTP errors such as 503 close EventSource permanently. A non-null
      // closed source must not prevent subscribing to this same run again.
      if (current.readyState === 2) { flushDraft(); closeTransport(); }
      scheduleReconcile(retryDelay);
    };
    for (const name of events) current.addEventListener(name, (message) => {
      if (source !== current || !snapshot.value || (network.value.enabled && observedScene !== sceneKey())) return;
      cursor = (message as MessageEvent).lastEventId || cursor;
      const event = JSON.parse((message as MessageEvent).data) as RunEvent;
      if (event.type === "answer.patch") {
        queued = reduceRun(queued ?? snapshot.value, event);
        if (frame === null) frame = requestAnimationFrame(flushDraft);
      } else {
        flushDraft();
        install(reduceRun(snapshot.value!, event));
      }
    });
  }

  async function reconcile(): Promise<void> {
    if (reconcileTask) return reconcileTask;
    const current = snapshot.value;
    if (!current || current.persistence_state !== "pending") return;
    const descriptor = current.descriptor;
    const generation = observationGeneration;
    connection.value = connection.value === "connected" ? "connected" : "checking";
    reconcileTask = (async () => {
      try {
        const recovered = await api.agentRun(descriptor.turn_id);
        if (generation !== observationGeneration || !sameDescriptor(snapshot.value?.descriptor, descriptor)) return;
        flushDraft();
        const installed = snapshot.value;
        if (!installed) return;
        if ((!network.value.enabled && recovered.last_seq < installed.last_seq) || (installed.persistence_state !== 'pending' && recovered.persistence_state === 'pending')) {
          snapshot.value = { ...installed, board_commits: mergeBoardDeliveries(installed.board_commits, recovered.board_commits, recovered.final_view?.board_commits) }; return;
        }
        if (network.value.enabled && recovered.persistence_state === "pending") { snapshot.value = { ...installed, descriptor: recovered.descriptor, board_commits: mergeBoardDeliveries(installed.board_commits, recovered.board_commits, recovered.final_view?.board_commits) }; openSource(descriptor, generation); return; }
        install(recovered);
        if (recovered.persistence_state === "pending") openSource(descriptor, generation);
      } catch (error) {
        if (generation !== observationGeneration || !sameDescriptor(snapshot.value?.descriptor, descriptor)) return;
        if (error instanceof ApiError && (error.status === 401 || error.status === 403)) {
          closeTransport();
          connection.value = "authentication";
          return;
        }
        if (error instanceof ApiError && error.status === 404) {
          const running = snapshot.value;
          if (running?.persistence_state === "pending") {
            install(interruptRun(running, {
              error_code: error.errorCode,
              category: error.category,
              message: error.message,
            }));
          }
          return;
        }
        connection.value = navigator.onLine === false ? "offline" : "reconnecting";
      } finally {
        if (generation === observationGeneration) {
          reconcileTask = null;
          if (active.value && connection.value !== "authentication" && source?.readyState !== 1) {
            connection.value = navigator.onLine === false ? "offline" : "reconnecting";
            scheduleReconcile(retryDelay);
            retryDelay = Math.min(retryDelay * 2, 30_000);
          }
        }
      }
    })();
    return reconcileTask;
  }

  function scheduleReconcile(delay = 0) {
    if (disposed || !active.value || reconcileTask) return;
    if (reconcileTimer !== null) {
      if (delay) return;
      clearReconcileTimer();
    }
    reconcileTimer = window.setTimeout(() => {
      reconcileTimer = null;
      void reconcile();
    }, delay);
  }

  function observe(descriptor: RunDescriptor) {
    if (disposed) return;
    if (sameDescriptor(snapshot.value?.descriptor, descriptor) && snapshot.value?.persistence_state === "pending") {
      openSource(descriptor, observationGeneration);
      scheduleReconcile();
      return;
    }
    close();
    observationGeneration += 1;
    terminalNotified = false;
    cursor = undefined; observedScene = sceneKey();
    snapshot.value = initialRun(descriptor);
    connection.value = "checking";
    const generation = observationGeneration;
    void reconcile();
    openSource(descriptor, generation);
  }
  async function cancel() {
    const current = snapshot.value;
    if (!current || !active.value || stopping.value) return;
    const generation = observationGeneration;
    cancellationPending.value = true;
    try {
      const next = await api.agentRunCancel(current.descriptor.turn_id);
      if (generation !== observationGeneration || !sameDescriptor(snapshot.value?.descriptor, current.descriptor)) return;
      flushDraft();
      const installed = snapshot.value!;
      if (installed.persistence_state !== "pending" && next.persistence_state === "pending") {
        snapshot.value = { ...installed, board_commits: mergeBoardDeliveries(installed.board_commits, next.board_commits, next.final_view?.board_commits) }; return;
      }
      if (network.value.enabled && next.persistence_state === "pending") {
        // HTTP recovery omits the live event buffer; preserve the observed work.
        snapshot.value = { ...installed, execution_state: next.execution_state, descriptor: next.descriptor, board_commits: mergeBoardDeliveries(installed.board_commits, next.board_commits, next.final_view?.board_commits) };
      } else if (network.value.enabled || next.last_seq >= installed.last_seq) install(next);
    } finally {
      if (generation === observationGeneration) cancellationPending.value = false;
    }
  }
  function forget() { close(); snapshot.value = null; terminalNotified = false; }
  const recoverVisible = () => {
    if (document.visibilityState === "hidden") return;
    scheduleReconcile();
  };
  onMounted(() => {
    document.addEventListener("visibilitychange", recoverVisible);
    window.addEventListener("pageshow", recoverVisible);
    window.addEventListener("online", recoverVisible);
  });
  onBeforeUnmount(() => {
    disposed = true;
    document.removeEventListener("visibilitychange", recoverVisible);
    window.removeEventListener("pageshow", recoverVisible);
    window.removeEventListener("online", recoverVisible);
    close();
  });
  return { snapshot, connection, active, stopping, observe, reconcile, cancel, close, forget };
}
