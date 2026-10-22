import { closeSync, mkdirSync, openSync, readFileSync, readSync, readdirSync, renameSync, statSync, writeFileSync, writeSync } from "node:fs";
import path from "node:path";
import { randomUUID } from "node:crypto";
import { StringDecoder } from "node:string_decoder";
import {
  AutomaticBuildSnapshotRecoverySignal, automaticBuildIncrementalPolicies, prepareAutomaticBuildSnapshot, refreshAutomaticBuildProfileSidecar,
  routeAutomaticBuildSnapshot, type AutomaticBuildSnapshot, type AutomaticBuildSnapshotOptions,
  type AutomaticBuildTarget,
  type AutomaticBuildGenerationTaskV1,
} from "./build-orchestrator";
import { canonicalBuildJson } from "./build-intent";
import { automaticBuildPolicyGenerationPath } from "./automatic-build-policy-generation";
import { automaticBuildGenerationArtifactPath } from "./semantic-artifact";
import type { AutomaticBuildTaskPolicyBinding } from "./semantic-artifact";
import { workUnitPlanDigest, type WorkUnitDescriptor } from "./stage-work-unit";

type Observation = {
  files: Record<string, string | null>;
  artifacts: Record<string, string[]>;
};
interface IncrementalState {
  version: "automatic_build_incremental_profile.v1";
  identity: string;
  observed: Observation;
  pending_ids: string[];
  preparation_parents: string[];
}

function missing(error: unknown) { return (error as NodeJS.ErrnoException).code === "ENOENT"; }
function names(directory: string): string[] {
  try { return readdirSync(directory).sort(); } catch (error) { if (missing(error)) return []; throw error; }
}
function stamp(file: string): string | null {
  try {
    const stat = statSync(file);
    return `${stat.size}:${stat.mtimeMs}:${stat.ctimeMs}`;
  } catch (error) { if (missing(error)) return null; throw error; }
}
function readSaved<T>(file: string): T | undefined {
  try { return JSON.parse(readFileSync(file, "utf8")) as T; }
  catch (error) { if (missing(error) || error instanceof SyntaxError) return undefined; throw error; }
}
function save(file: string, value: unknown) {
  mkdirSync(path.dirname(file), { recursive: true });
  const temporary = `${file}.${randomUUID()}.tmp`;
  writeFileSync(temporary, JSON.stringify(value), "utf8");
  renameSync(temporary, file);
}

/** Published task artifacts and migration receipts are create-only. Enumerating their directories
 * detects additions/removals without reopening every unchanged task. Mutable publication and policy
 * files have explicit stamps; leases and attempts are deliberately absent from this cache. */
function observe(target: AutomaticBuildTarget, policies: ReturnType<typeof automaticBuildIncrementalPolicies>): Observation {
  const root = path.join(target.workspace_dir, ".build", "automatic-build");
  const files: Observation["files"] = {};
  const artifacts: Observation["artifacts"] = {};
  for (const file of [target.source_path, path.join(target.workspace_dir, "source_manifest.json"),
    ...["base.json", "profile_metadata.json", "discourse_index.json", "formula_semantics.json", "math_structure.json"]
      .map(file => path.join(target.workspace_dir, file))]) files[file] = stamp(file);
  for (const policy of policies) {
    const quality = path.join(root, "v2", "quality", `${policy.stage}.json`);
    files[quality] = stamp(quality);
    const stageLock = path.join(root, "v2", "policies", `${policy.stage}.json`);
    files[stageLock] = stamp(stageLock);
    // An explicit reset/migration invalidates the prepared catalog; ordinary child commits do not.
    for (const directory of [path.join(root, "v2", "policies", policy.stage),
      path.join(root, "v4", "migrations", policy.stage)]) {
      files[directory] = stamp(directory);
      for (const child of names(directory)) {
        const file = path.join(directory, child);
        files[file] = stamp(file);
      }
    }
    for (const member of policy.members) {
      const file = automaticBuildPolicyGenerationPath(target, policy.stage, member.policy_generation_id);
      files[file] = stamp(file);
      const directory = path.dirname(automaticBuildGenerationArtifactPath(target, policy.stage,
        member.policy_generation_id, "directory-observation"));
      artifacts[directory] = names(directory);
    }
  }
  return { files, artifacts };
}

function supported(snapshot: AutomaticBuildSnapshot): boolean {
  return snapshot.target.profile_id === "technical_learning"
    && snapshot.stages.length === 2 && snapshot.stages[0].stage === "pass1" && snapshot.stages[0].closed
    && snapshot.stages[1].stage === "profile_sidecar" && !snapshot.stages[1].closed
    && !snapshot.stages.some(stage => stage.preparation_required);
}

function compact(snapshot: AutomaticBuildSnapshot): AutomaticBuildSnapshot {
  return { ...snapshot, stages: snapshot.stages.map(stage => ({
    ...stage, pending_work_units: undefined,
    // A closed Pass1 is a dependency; its generation inputs are never needed to refill sidecars.
    ...(stage.stage === "pass1" ? { generation_tasks: undefined } : {}),
  })) };
}

type ProfileTask = Extract<AutomaticBuildGenerationTaskV1,
  { kind: "profile_sidecar_discourse" | "profile_sidecar_fast_path" }>;

function* catalogLines(file: string): Generator<string> {
  const fd = openSync(file, "r");
  const buffer = Buffer.allocUnsafe(64 * 1024);
  const decoder = new StringDecoder("utf8");
  let pending = "";
  try {
    for (;;) {
      const size = readSync(fd, buffer, 0, buffer.length, null);
      pending += size ? decoder.write(buffer.subarray(0, size)) : decoder.end();
      let start = 0;
      for (let end = pending.indexOf("\n"); end >= 0; end = pending.indexOf("\n", start)) {
        yield pending.slice(start, end);
        start = end + 1;
      }
      pending = pending.slice(start);
      if (!size) {
        if (pending) yield pending;
        break;
      }
    }
  } finally { closeSync(fd); }
}

type ProfileCatalog = { identity: string; snapshot: AutomaticBuildSnapshot; descriptor_plan_digest?: string };

function readCatalog(file: string): ProfileCatalog | undefined {
  try {
    let catalog: ProfileCatalog | undefined;
    const units = new Map<string, WorkUnitDescriptor>();
    for (const line of catalogLines(file)) {
      if (!catalog) { catalog = JSON.parse(line); continue; }
      const [stageIndex, kind, id, value] = JSON.parse(line) as [number, string, string, unknown];
      const stage = catalog.snapshot.stages[stageIndex];
      if (kind === "unit") {
        const unit = value as WorkUnitDescriptor;
        stage.work_units!.push(unit);
        units.set(`${stageIndex}:${id}`, unit);
      } else if (kind === "binding") {
        stage.task_bindings![id] = value as AutomaticBuildTaskPolicyBinding;
      } else {
        const saved = value as ProfileTask;
        stage.generation_tasks![id] = { ...saved, task: { ...saved.task,
          descriptor: units.get(`${stageIndex}:${id}`),
        } } as ProfileTask;
      }
    }
    if (catalog?.snapshot.stages[1]) catalog.snapshot.stages[1].descriptor_plan_digest = catalog.descriptor_plan_digest;
    return catalog;
  } catch (error) { if (missing(error) || error instanceof SyntaxError) return undefined; throw error; }
}

function saveCatalog(file: string, identity: string, snapshot: AutomaticBuildSnapshot): void {
  // Store the existing canonical digest atomically with the descriptors it summarizes.
  // Pending/lease changes do not alter it; refreshed descriptors clear it before this write.
  const sidecar = snapshot.stages[1];
  sidecar.descriptor_plan_digest ??= workUnitPlanDigest(sidecar.work_units!);
  mkdirSync(path.dirname(file), { recursive: true });
  const temporary = `${file}.${randomUUID()}.tmp`;
  const fd = openSync(temporary, "w");
  const write = (value: unknown) => { writeSync(fd, `${JSON.stringify(value)}\n`, undefined, "utf8"); };
  const saved = compact(snapshot);
  try {
    // Older engines ignore this header hint and drop it when rewriting the catalog.
    // Keeping it out of persisted stage data prevents an old writer carrying a stale hint forward.
    write({ identity, descriptor_plan_digest: sidecar.descriptor_plan_digest,
      snapshot: { ...saved, stages: saved.stages.map(({ descriptor_plan_digest, ...stage }) => ({ ...stage,
      work_units: stage.work_units ? [] : undefined,
      task_bindings: stage.task_bindings ? {} : undefined,
      generation_tasks: stage.generation_tasks ? {} : undefined,
    })) } });
    for (const [index, stage] of saved.stages.entries()) {
      for (const unit of stage.work_units ?? []) write([index, "unit", unit.work_unit_id, unit]);
      for (const [id, binding] of Object.entries(stage.task_bindings ?? {})) write([index, "binding", id, binding]);
      for (const [id, task] of Object.entries(stage.generation_tasks ?? {})) {
        if (task.kind !== "profile_sidecar_discourse" && task.kind !== "profile_sidecar_fast_path") {
          throw new Error("incremental profile catalog contains an unsupported task kind");
        }
        // Descriptors already belong to work_units; restore that shared reference when reading.
        // Keep the descriptor key's position: frozen task receipts compare JSON bytes.
        write([index, "task", id, { kind: task.kind, task: { ...task.task, descriptor: null } }]);
      }
    }
  } finally { closeSync(fd); }
  renameSync(temporary, file);
}

/** Command-owned routing index. Full read-only inspection remains available through the ordinary
 * snapshot API. This persists only derived routing; durable artifacts remain completion authority. */
export function routeIncrementalAutomaticBuildSnapshot(target: AutomaticBuildTarget,
  options: AutomaticBuildSnapshotOptions, prepareStage?: "profile_sidecar", owner = "inspection") {
  try { return routeIncrementalSnapshot(target, options, prepareStage, owner); }
  catch (error) {
    if (error instanceof AutomaticBuildSnapshotRecoverySignal) return { status: "blocked" as const, recovery: error.recovery };
    throw error;
  }
}

function routeIncrementalSnapshot(target: AutomaticBuildTarget,
  options: AutomaticBuildSnapshotOptions, prepareStage: "profile_sidecar" | undefined, owner: string) {
  if (target.profile_id !== "technical_learning" || options.stage) {
    return prepareStage ? prepareAutomaticBuildSnapshot(target, prepareStage, options)
      : routeAutomaticBuildSnapshot(target, options);
  }
  const quality = options.quality_profile ?? "full";
  const policies = automaticBuildIncrementalPolicies(target, quality);
  const identity = canonicalBuildJson({ target: target.target_ref, options, policies });
  const directory = path.join(target.workspace_dir, ".build", "automatic-build", "v3", "incremental-profile", owner);
  const stateFile = path.join(directory, "state.json");
  // A derived cache is rebuilt once on a format change; plan/artifact identities stay intact.
  const catalogFile = path.join(directory, "catalog.jsonl");
  // Observe before routing so a concurrent child commit cannot be marked as seen without being read.
  const observed = observe(target, policies);
  const previous = readSaved<IncrementalState>(stateFile);
  const pass1Directories = policies[0].members.map(member => path.dirname(
    automaticBuildGenerationArtifactPath(target, "pass1", member.policy_generation_id, "directory-observation")));
  const canReuse = previous?.version === "automatic_build_incremental_profile.v1"
    && previous.identity === identity && canonicalBuildJson(previous.observed.files) === canonicalBuildJson(observed.files)
    && pass1Directories.every(dir => canonicalBuildJson(previous.observed.artifacts[dir]) === canonicalBuildJson(observed.artifacts[dir]));
  const catalog = canReuse ? readCatalog(catalogFile) : undefined;
  if (catalog?.identity === identity && catalog.snapshot.stages[1]?.stage === "profile_sidecar") {
    const snapshot = catalog.snapshot;
    snapshot.target = target;
    for (const stage of snapshot.stages) {
      if (stage.stage === "profile_sidecar") stage.pending_tasks = previous!.pending_ids;
      const pending = new Set(stage.pending_tasks);
      stage.pending_work_units = stage.work_units?.filter(unit => pending.has(unit.work_unit_id));
    }
    const stage = snapshot.stages[1];
    const byFile = new Map(Object.entries(stage.generation_tasks!).map(([id, task]) => [
      automaticBuildGenerationArtifactPath(target, "profile_sidecar", task.task.policy_generation_id, id), id,
    ]));
    const changed = new Set<string>();
    let unknownChange = false;
    for (const [dir, entries] of Object.entries(observed.artifacts)) {
      if (pass1Directories.includes(dir)) continue;
      const before = new Set(previous!.observed.artifacts[dir] ?? []);
      const after = new Set(entries);
      for (const entry of [...entries.filter(name => !before.has(name)), ...[...before].filter(name => !after.has(name))]) {
        const id = byFile.get(path.join(dir, entry));
        if (id) changed.add(id);
        else unknownChange = true;
      }
    }
    const preparedParents = prepareStage ? previous!.preparation_parents : [];
    if (changed.size && !prepareStage && previous!.preparation_parents.length) {
      for (const [id, task] of Object.entries(stage.generation_tasks!)) {
        if (task.kind === "profile_sidecar_discourse"
          && previous!.preparation_parents.includes(task.task.route.parent_lid)) changed.add(id);
      }
    }
    if (!unknownChange && (changed.size || preparedParents.length)) {
      const refreshed = refreshAutomaticBuildProfileSidecar(target, stage, [...changed], quality, preparedParents);
      snapshot.stages[1] = refreshed.stage;
      const structuralChange = preparedParents.length > 0 || [...changed].some(id =>
        stage.generation_tasks![id]?.kind === "profile_sidecar_discourse");
      if (structuralChange) saveCatalog(catalogFile, identity, snapshot);
      save(stateFile, { ...previous, observed: prepareStage
        ? { files: observe(target, policies).files, artifacts: observed.artifacts } : observed,
        pending_ids: refreshed.stage.pending_tasks, preparation_parents: refreshed.preparation_parents });
    }
    // When the last unit commits, use the normal stage-close/next-stage path once.
    if (!unknownChange && snapshot.stages[1].pending_tasks.length) {
      // Existing catalogs remain reusable: fill a missing derived value without replanning.
      if (!snapshot.stages[1].descriptor_plan_digest) saveCatalog(catalogFile, identity, snapshot);
      return { status: "ready" as const, value: snapshot };
    }
  }
  const result = prepareStage ? prepareAutomaticBuildSnapshot(target, prepareStage, options)
    : routeAutomaticBuildSnapshot(target, options);
  if (result.status === "ready" && supported(result.value)) {
    saveCatalog(catalogFile, identity, result.value);
    save(stateFile, { version: "automatic_build_incremental_profile.v1", identity,
      observed: prepareStage ? { files: observe(target, policies).files, artifacts: observed.artifacts } : observed,
      pending_ids: result.value.stages[1].pending_tasks, preparation_parents: [] } satisfies IncrementalState);
  }
  return result;
}
