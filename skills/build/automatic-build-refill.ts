import { z } from "zod";
import type { AutomaticBuildStepRequestV1, AutomaticBuildStepResponseV1 } from "./automatic-build-driver";

const ref = z.string().min(1).max(1024);
const owned = z.object({ child: ref, opaque_handoff_ref: ref }).strict();
const control = z.object({
  version: z.literal("automatic_build_refill_request.v1"),
  invocation_ref: ref,
  capacity_limit: z.number().int().min(0).max(3),
  live_by_slot: z.record(z.string(), owned),
  completed_refs: z.array(ref),
  terminal_children: z.array(ref),
}).passthrough(); // Remaining fields use the existing strict build.step validator.

export interface AutomaticBuildRefillResponseV1 {
  version: "automatic_build_refill.v1";
  live_by_slot: Record<string, { child: string; opaque_handoff_ref: string }>;
  completed_refs: string[];
  available_agent_slots: number;
  step: AutomaticBuildStepResponseV1;
  ready_executors: Array<{ opaque_handoff_ref: string; dispatch_slot_ref?: string }>;
}

export class AutomaticBuildRefillRequestError extends Error {
  constructor(cause: unknown) {
    super("automatic build refill request is invalid", { cause });
    this.name = "AutomaticBuildRefillRequestError";
  }
}

/** One control operation: consume terminals, release ownership, count capacity, then step. */
interface RefillEngine {
  validateStep: (request: unknown) => AutomaticBuildStepRequestV1;
  maxParallel: (invocationRef: string) => number;
  step: (request: AutomaticBuildStepRequestV1, liveHandoffRefs?: string[], completedHandoffRefs?: string[]) => AutomaticBuildStepResponseV1 | Promise<AutomaticBuildStepResponseV1>;
}
export function automaticBuildRefill(value: unknown, engine: RefillEngine & { step: (...args: Parameters<RefillEngine["step"]>) => AutomaticBuildStepResponseV1 }): AutomaticBuildRefillResponseV1;
export function automaticBuildRefill(value: unknown, engine: RefillEngine): AutomaticBuildRefillResponseV1 | Promise<AutomaticBuildRefillResponseV1>;
export function automaticBuildRefill(value: unknown, engine: RefillEngine): AutomaticBuildRefillResponseV1 | Promise<AutomaticBuildRefillResponseV1> {
  let parsed: z.infer<typeof control>;
  let request: AutomaticBuildStepRequestV1;
  try {
    parsed = control.parse(value);
    const { version, capacity_limit, live_by_slot, completed_refs, terminal_children, ...fields } = parsed;
    request = engine.validateStep({ ...fields, version: "automatic_build_step_request.v1", available_agent_slots: 0 });
  } catch (error) {
    throw new AutomaticBuildRefillRequestError(error);
  }
  const { capacity_limit, live_by_slot, completed_refs, terminal_children } = parsed;
  const completed = new Set(completed_refs);
  const terminal = new Set(terminal_children);
  for (const [slot, child] of Object.entries(live_by_slot)) {
    if (!terminal.has(child.child)) continue;
    completed.add(child.opaque_handoff_ref);
    delete live_by_slot[slot];
  }
  const capacity = Math.min(capacity_limit, engine.maxParallel(request.invocation_ref));
  const available = Math.max(0, capacity - Object.keys(live_by_slot).length) as 0 | 1 | 2 | 3;
  const result = engine.step({ ...request, available_agent_slots: available },
    Object.values(live_by_slot).map(child => child.opaque_handoff_ref), [...completed]);
  const finish = (step: AutomaticBuildStepResponseV1): AutomaticBuildRefillResponseV1 => {
  const ready: AutomaticBuildRefillResponseV1["ready_executors"] = [];
  const selected = new Set<string>();
  if (step.action.kind === "SPAWN_EXECUTORS") {
    for (const executor of step.action.executors) {
      const slot = executor.dispatch_slot_ref ?? executor.opaque_handoff_ref;
      if (live_by_slot[slot] || completed.has(executor.opaque_handoff_ref) || selected.has(slot)) continue;
      if (ready.length >= available) break;
      selected.add(slot);
      ready.push(executor);
    }
  }
  return { version: "automatic_build_refill.v1", live_by_slot, completed_refs: [...completed],
    available_agent_slots: available, step, ready_executors: ready };
  };
  return result instanceof Promise ? result.then(finish) : finish(result);
}
