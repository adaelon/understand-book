import { randomUUID } from "node:crypto";
import { existsSync, mkdirSync, readFileSync, renameSync, writeFileSync } from "node:fs";
import { homedir } from "node:os";
import path from "node:path";

export interface AutomaticBuildRefillTimingStart {
  invocation_ref: string;
  available_agent_slots: number;
  started_at?: string;
  released: Array<{ opaque_handoff_ref: string; dispatch_slot_ref: string; observed_at?: string }>;
}
export interface AutomaticBuildRefillTimingObserver {
  begin(input: AutomaticBuildRefillTimingStart): void;
  finish(ready: Array<{ opaque_handoff_ref: string; dispatch_slot_ref?: string }>): void;
}

type Vacancy = {
  id: string;
  source: "initial_capacity" | "terminal_refill";
  observed_at: string;
  previous_handoff_ref?: string;
  previous_dispatch_slot_ref?: string;
};
interface HandoffTiming {
  version: "automatic_build_handoff_timing.v1";
  invocation_ref: string;
  opaque_handoff_ref: string;
  dispatch_slot_ref?: string;
  vacancy?: Vacancy;
  refill_started_at: string;
  ready_at: string;
}
export interface AutomaticBuildGenerationTimingV1 {
  version: "automatic_build_generation_timing.v1";
  opaque_handoff_ref: string;
  dispatch_id: string;
  dispatch_run_id: string;
  work_unit_id: string;
  physical_attempt: number;
  semantic_attempt: number;
  lease_epoch: number;
  first_generation_for_handoff: boolean;
  opened_at: string;
  generation_started_at: string;
  handoff?: HandoffTiming;
}

function rootDirectory(configured?: string): string {
  return path.join(configured ?? process.env.UNDERSTAND_BOOK_AUTOMATIC_BUILD_DRIVER_ROOT
    ?? path.join(homedir(), ".understand-book", "automatic-build-driver-v1"), "dispatch-timing");
}
function fileAt(root: string, directory: string, ref: string): string {
  if (!/^[a-zA-Z0-9_-]{1,160}$/u.test(ref)) throw new Error("invalid dispatch timing reference");
  return path.join(root, directory, `${ref}.json`);
}
function read<T>(file: string): T | undefined {
  return existsSync(file) ? JSON.parse(readFileSync(file, "utf8")) as T : undefined;
}
function save(file: string, value: unknown): void {
  mkdirSync(path.dirname(file), { recursive: true });
  const temporary = `${file}.${randomUUID()}.tmp`;
  writeFileSync(temporary, JSON.stringify(value), "utf8");
  renameSync(temporary, file);
}

/** Observations only: failure to collect timing never changes an Engine decision. */
export function createAutomaticBuildRefillTimingObserver(options: {
  registry_root?: string; now?: () => string;
} = {}): AutomaticBuildRefillTimingObserver {
  const root = rootDirectory(options.registry_root);
  const clock = options.now ?? (() => new Date().toISOString());
  let current: { input: AutomaticBuildRefillTimingStart; started_at: string; pending: Vacancy[] } | undefined;
  return {
    begin(input) {
      current = undefined;
      try {
        const startedAt = input.started_at ?? clock();
        const stateFile = fileAt(root, "invocations", input.invocation_ref);
        const state = read<{ pending: Vacancy[] }>(stateFile);
        const releaseFile = (ref: string) => fileAt(root, `releases/${input.invocation_ref}`, ref);
        const pending = (state?.pending ?? []).filter(vacancy =>
          !read<{ assigned_to?: string }>(releaseFile(vacancy.id))?.assigned_to);
        for (const released of input.released) {
          const prior = read<{ vacancy: Vacancy; assigned_to?: string }>(releaseFile(released.opaque_handoff_ref));
          if (prior?.assigned_to || pending.some(item => item.id === released.opaque_handoff_ref)) continue;
          const vacancy: Vacancy = prior?.vacancy ?? { id: released.opaque_handoff_ref,
            source: "terminal_refill", observed_at: released.observed_at ?? startedAt,
            previous_handoff_ref: released.opaque_handoff_ref,
            previous_dispatch_slot_ref: released.dispatch_slot_ref };
          save(releaseFile(released.opaque_handoff_ref), { vacancy });
          pending.push(vacancy);
        }
        if (!state) while (pending.length < input.available_agent_slots) {
          pending.push({ id: `initial-${pending.length}`, source: "initial_capacity", observed_at: startedAt });
        }
        // A lower host capacity no longer proves that the old capacity stayed idle.
        // Drop surplus observations instead of including that interval in a sample.
        pending.splice(input.available_agent_slots);
        save(stateFile, { pending });
        current = { input, started_at: startedAt, pending };
      } catch { /* Unavailable timing is preferable to changing build behavior. */ }
    },
    finish(ready) {
      if (!current) return;
      try {
        const { input, started_at, pending } = current;
        const readyAt = clock();
        for (const launch of ready) {
          const handoffFile = fileAt(root, "handoffs", launch.opaque_handoff_ref);
          const existing = read<HandoffTiming>(handoffFile);
          if (existing) {
            if (existing.vacancy) {
              const index = pending.findIndex(item => item.id === existing.vacancy!.id);
              if (index >= 0) pending.splice(index, 1);
              save(fileAt(root, `releases/${input.invocation_ref}`, existing.vacancy.id), {
                vacancy: existing.vacancy, assigned_to: launch.opaque_handoff_ref });
            }
            continue;
          }
          const vacancy = pending.shift();
          const handoff: HandoffTiming = { version: "automatic_build_handoff_timing.v1",
            invocation_ref: input.invocation_ref, ...launch, ...(vacancy ? { vacancy } : {}),
            refill_started_at: started_at, ready_at: readyAt };
          save(handoffFile, handoff);
          if (vacancy) save(fileAt(root, `releases/${input.invocation_ref}`,
            vacancy.id), { vacancy, assigned_to: launch.opaque_handoff_ref });
        }
        save(fileAt(root, "invocations", input.invocation_ref), { pending });
      } catch { /* Missing samples are reported as unavailable by stage metrics. */ }
    },
  };
}

export function recordAutomaticBuildGenerationTiming(
  registryRoot: string,
  leaseRef: string,
  input: Omit<AutomaticBuildGenerationTimingV1, "version" | "handoff" | "first_generation_for_handoff">,
): void {
  try {
    const file = path.join(path.dirname(leaseRef), "generation-timing.json");
    // Replayed starts or a replacement child cannot replace the first ready time.
    if (existsSync(file)) return;
    const root = rootDirectory(registryRoot);
    const firstFile = fileAt(root, "first-generations", input.opaque_handoff_ref);
    const first = read<{ work_unit_id: string; physical_attempt: number }>(firstFile);
    const firstGeneration = !first || (first.work_unit_id === input.work_unit_id
      && first.physical_attempt === input.physical_attempt);
    if (!first) save(firstFile, { work_unit_id: input.work_unit_id, physical_attempt: input.physical_attempt });
    // One handoff can process several units; only its first ready generation
    // consumed host capacity. Later units must not reuse that vacancy timestamp.
    const handoff = firstGeneration ? read<HandoffTiming>(fileAt(root, "handoffs", input.opaque_handoff_ref)) : undefined;
    save(file, { version: "automatic_build_generation_timing.v1", ...input,
      first_generation_for_handoff: firstGeneration, ...(handoff ? { handoff } : {}) });
  } catch { /* Timing is not a prerequisite for an accepted generation. */ }
}

