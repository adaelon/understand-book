import { readFileSync, writeFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { isDeepStrictEqual } from "node:util";

/** Compare the matched theme phase separately from the shared discovery/chapter prefix. */
export function compareFullBookExperiments(output: string) {
  const dir = path.resolve(output), read = (file: string) => JSON.parse(readFileSync(file, "utf8"));
  const plan = read(path.join(dir, "plan.json")), bDir = plan.comparison_dir;
  if (!bDir) throw Error("Create the shared-plan lexical comparison before comparing full-book results");
  const C = read(path.join(dir, "report.json")), B = read(path.join(bDir, "report.json"));
  if (C.status !== "ready" || B.status !== "ready") throw Error("Both full-book results must be complete");
  if (C.mode !== "semantic_required" || B.mode !== "lexical_only" || B.embedding.calls !== 0)
    throw Error("Comparison requires semantic C and embedding-free lexical B");
  const chapterSpine = (r: any) => r.structure.spine.map(({ depends_on, ...unit }: any) => unit);
  if (!isDeepStrictEqual(C.candidates, B.candidates) || !isDeepStrictEqual(chapterSpine(C), chapterSpine(B)))
    throw Error("Candidate catalog or shared chapter selection differs between B/C");
  const themeCost = (root: string) => {
    const steps = read(path.join(root, "session.json")).steps.filter((s: any) =>
      s.kind === "structure_theme" || s.kind === "structure_theme_reconcile");
    const actions: Record<string, number> = {};
    for (const s of steps) {
      let action = s.kind === "structure_theme_reconcile" ? "reconcile" : "invalid_json";
      try { action = read(s.response_file)?.kind ?? action; } catch { /* rejected response retained */ }
      actions[action] = (actions[action] ?? 0) + 1;
    }
    return { submissions: steps.length, accepted: steps.filter((s: any) => s.status === "accepted").length,
      rejected: steps.filter((s: any) => s.status === "rejected").length, actions,
      input_estimated_tokens: steps.reduce((n: number, s: any) => n + s.input_estimated_tokens, 0),
      output_estimated_tokens: steps.reduce((n: number, s: any) => n + (s.output_estimated_tokens ?? 0), 0),
      first_delivery_to_last_acceptance_ms: steps.length ? Math.max(...steps.map((s: any) => Date.parse(s.finished_at)))
        - Math.min(...steps.map((s: any) => Date.parse(s.opened_at))) : 0 };
  };
  const cCost = themeCost(dir), bCost = themeCost(bDir);
  const cStops = new Map<string, any>(C.structure.key_stops.map((s: any) => [s.id, s]));
  const bStops = new Map<string, any>(B.structure.key_stops.map((s: any) => [s.id, s]));
  for (const [id, stop] of cStops) if (bStops.has(id) && !isDeepStrictEqual(stop, bStops.get(id)))
    throw Error(`Shared candidate body changed between B/C: ${id}`);
  const cLines = new Map<string, any>(C.structure.throughlines.map((s: any) => [s.id, s]));
  const bLines = new Map<string, any>(B.structure.throughlines.map((s: any) => [s.id, s]));
  const result = { version: "book_structure_full_book_comparison.v1", same_candidate_catalog: true, same_shared_chapter_spine: true,
    shared_prefix: { submissions: C.submissions.submissions - cCost.submissions,
      input_estimated_tokens: C.submissions.input_estimated_tokens - cCost.input_estimated_tokens,
      output_estimated_tokens: C.submissions.output_estimated_tokens - cCost.output_estimated_tokens },
    matched_theme_phase: { C: { ...cCost, embedding: C.embedding }, B: { ...bCost, embedding: B.embedding } },
    full_new_book: { submissions: C.submissions, wall_ms: C.wall_ms, embedding: C.embedding },
    aggregate_experiment_submissions: C.submissions.submissions + B.submissions.submissions,
    structure: { C: { chapters: C.structure.spine.length, stops: cStops.size, throughlines: cLines.size },
      B: { chapters: B.structure.spine.length, stops: bStops.size, throughlines: bLines.size },
      C_only_stop_ids: [...cStops.keys()].filter(id => !bStops.has(id)), B_only_stop_ids: [...bStops.keys()].filter(id => !cStops.has(id)),
      throughlines: [...new Set([...cLines.keys(), ...bLines.keys()])].map(id => ({ id, in_C: cLines.has(id), in_B: bLines.has(id),
        same_content: cLines.has(id) && bLines.has(id) ? isDeepStrictEqual(cLines.get(id), bLines.get(id)) : false })) },
    actual_model_calls: null, actual_input_tokens: null, actual_output_tokens: null,
    timing_scope: "Theme wall time starts at first model delivery, after initial retrieval preparation. Actual embedding call elapsed_ms is reported separately; full-new-book wall includes preparation and execution.",
    content_review: "Compare actual chapters, themes and dependencies against source; identity and membership differences alone do not establish semantic quality or cost benefit." };
  writeFileSync(path.join(dir, "comparison.json"), JSON.stringify(result, null, 2) + "\n");
  return result;
}
if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url))
  console.log(JSON.stringify(compareFullBookExperiments(process.argv[2]), null, 2));
