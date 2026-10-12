import { createHash, randomUUID } from "node:crypto";
import { existsSync, mkdirSync, readFileSync, renameSync, unlinkSync, writeFileSync } from "node:fs";
import path from "node:path";
import type { TeachingGenerationTask } from "./teaching-build";

interface StoredTask {
  storage: "teaching_task_storage.v1";
  task: Omit<TeachingGenerationTask, "source" | "structure">;
  shared: Record<string, string>;
}

/** The digest replaces repeated multi-megabyte writes; these are content-addressed values, not audit sidecars. */
export function storeTeachingTask(workspace: string, task: TeachingGenerationTask): TeachingGenerationTask | StoredTask {
  if (!task.alignment) return task;
  const directory = path.join(workspace, ".build", "teaching", "shared");
  mkdirSync(directory, { recursive: true });
  const shared: Record<string, string> = {};
  const blob = (value: unknown) => {
    const bytes = JSON.stringify(value);
    const name = createHash("sha256").update(bytes).digest("hex") + ".json";
    const file = path.join(directory, name);
    if (!existsSync(file)) {
      const temporary = file + "." + randomUUID() + ".tmp";
      try { writeFileSync(temporary, bytes); renameSync(temporary, file); }
      finally { if (existsSync(temporary)) unlinkSync(temporary); }
    }
    return name;
  };
  const put = (key: string, value: unknown) => { shared[key] = blob(value); };
  const { source, structure, previous, alignment, ...rest } = task;
  put("source", source); put("structure", structure);
  if (previous) put("previous", previous);
  const { objects, ...relations } = alignment.proposal;
  // Resolving moves a record to the end of the logical order. Stable storage order
  // prevents that move from rewriting every block; the manifest retains model order.
  const sorted = [...objects].sort((a, b) => a.key < b.key ? -1 : a.key > b.key ? 1 : 0);
  const object_blocks: string[] = [];
  for (let start = 0; start < sorted.length; start += 64) object_blocks.push(blob(sorted.slice(start, start + 64)));
  put("proposal", { storage: "teaching_proposal_storage.v1", object_blocks,
    object_order: objects.map(object => object.key),
    relations: Object.fromEntries(Object.entries(relations).map(([key, value]) => [key, blob(value)])) });
  put("source_ranges", alignment.source_ranges);
  put("evidence_lids", task.descriptor.evidence_lids);
  const inputBasis = task.descriptor.input_basis;
  if (inputBasis.kind !== "source_slices") put("parent_lids", inputBasis.parent_lids);
  const { proposal, source_ranges, ...cursor } = alignment;
  return { storage: "teaching_task_storage.v1", shared,
    task: { ...rest, descriptor: { ...task.descriptor, evidence_lids: [],
      input_basis: inputBasis.kind === "source_slices" ? inputBasis : { ...inputBasis, parent_lids: [] } },
      alignment: cursor as TeachingGenerationTask["alignment"] } };
}

export function restoreTeachingTask(workspace: string, saved: TeachingGenerationTask | StoredTask): TeachingGenerationTask {
  if (!("storage" in saved)) return saved;
  const blob = (name: string) => JSON.parse(readFileSync(path.join(workspace, ".build", "teaching", "shared", name), "utf8"));
  const get = (key: string) => blob(saved.shared[key]);
  const proposal = get("proposal");
  const restoredProposal = proposal.storage === "teaching_proposal_storage.v1"
    ? { objects: proposal.object_blocks.flatMap((name: string) => blob(name)),
      ...Object.fromEntries(Object.entries(proposal.relations).map(([key, name]) => [key, blob(name as string)])) } : proposal;
  if (proposal.object_order) {
    const records = new Map(restoredProposal.objects.map((object: { key: string }) => [object.key, object]));
    restoredProposal.objects = proposal.object_order.map((key: string) => records.get(key));
  }
  const descriptor = { ...saved.task.descriptor };
  if (saved.shared.evidence_lids) descriptor.evidence_lids = get("evidence_lids");
  if (saved.shared.parent_lids && descriptor.input_basis.kind !== "source_slices")
    descriptor.input_basis = { ...descriptor.input_basis, parent_lids: get("parent_lids") };
  return { ...saved.task, descriptor, source: get("source"), structure: get("structure"),
    ...(saved.shared.previous ? { previous: get("previous") } : {}),
    alignment: { ...saved.task.alignment!, proposal: restoredProposal, source_ranges: get("source_ranges") } };
}
