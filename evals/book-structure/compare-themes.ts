import { readFileSync, writeFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { isDeepStrictEqual } from "node:util";
import { projectStructureRetrieval } from "../../packages/core/src/book-structure-retrieval";
import { retrieveHybrid } from "../../packages/core/src/semantic-retrieval";
import { estimateTokens } from "../../packages/core/src/window";
import { themeExperimentReport, replayThemeExperiment, type ThemeExperimentPlan, type ThemeExperimentRun } from "./themes";

/** Post-run structural mapping only; the source-review answers never enter generation. */
export function compareThemeExperiments(dir: string) {
  const read = <T = any>(file: string): T => JSON.parse(readFileSync(file, "utf8"));
  const plan = read<ThemeExperimentPlan>(path.join(dir, "plan.json"));
  const answers = read(fileURLToPath(new URL("../../packages/core/testdata/book-structure/acceptance.json", import.meta.url))).theme;
  const index = projectStructureRetrieval(plan.context.catalog, plan.context.chapters);
  const fixedRef = plan.directory!.works.find(w => w.plan.question === plan.fixed_question)!.plan.ref;
  const groups = ["B", "C"].map(group => {
    const run = read<ThemeExperimentRun>(path.join(dir, group, "session.json")), report = themeExperimentReport(plan, run);
    if (!report.complete) throw new Error(`group ${group} incomplete; content metrics unavailable`);
    const fixed = report.themes.find(t => t?.ref === (run.directory.redirects[fixedRef] ?? fixedRef))!;
    const requirements = answers.required_comparisons.map((r: any) => ({ mechanism: r.mechanism,
      stages: fixed.stages.filter(s => s.unit_lid === r.unit_lid && s.development.evidence_lids.some(lid => r.evidence.includes(lid))).map(s => s.development.text),
      selected_candidates: fixed.stages.flatMap(s => s.member_refs).filter(ref => index.locations[ref].evidence_lids.some(lid => r.evidence.includes(lid))) }));
    const first = run.steps.find(s => s.theme_ref === fixedRef)!;
    const sequence = first.preparation?.sequence ?? retrieveHybrid(index.records, { kind: "search", query: plan.fixed_question });
    const ranking = [6, 12].map(k => ({ k, hits: sequence.hits.slice(0, k),
      source_groups_reached: answers.required_comparisons.filter((r: any) => sequence.hits.slice(0, k).some(h => index.locations[h.key].evidence_lids.some(lid => r.evidence.includes(lid)))).map((r: any) => r.mechanism) }));
    const negative = answers.negative_dependency;
    const lastRevision = run.revisions?.at(-1);
    const before = lastRevision ? read<ThemeExperimentRun>(path.join(dir, group, `before-revision-${run.revisions!.length - 1}.json`)) : undefined;
    return { group, report, verification: replayThemeExperiment(plan, run), fixed_theme_ref: fixed.ref, fixed_theme_source_mapping: requirements,
      targeted_revision_preserved_other_themes: before ? isDeepStrictEqual(before.directory.works.filter(w => w.plan.ref !== lastRevision!.ref),
        run.directory.works.filter(w => w.plan.ref !== lastRevision!.ref)) : null,
      unsupported_fixed_dependency_absent: !fixed.dependencies.some(d => d.unit_lid === negative.unit_lid && d.depends_on === negative.depends_on),
      initial_question_ranking: ranking };
  });
  return { version: "book_structure_theme_comparison.v1", groups,
    shared_planning: { accepted_semantic_submissions: 1, input_tokens_estimate: estimateTokens(plan.prompt + readFileSync(path.join(dir, "planning-input.json"), "utf8")),
      output_tokens_estimate: estimateTokens(JSON.stringify(plan.planning_response)), actual_model_calls: null, actual_input_tokens: null, actual_output_tokens: null },
    interpretation: "Source mappings and initial ranking locate evidence; they do not judge semantic completeness, dependencies, or causal cost improvement. Review actual themes against source separately." };
}
if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const dir = path.resolve(process.argv[2]); const result = compareThemeExperiments(dir);
  writeFileSync(path.join(dir, "comparison.json"), JSON.stringify(result, null, 2) + "\n");
  for (const group of result.groups) writeFileSync(path.join(dir, group.group, "verification.json"), JSON.stringify(group.verification, null, 2) + "\n");
  console.log(JSON.stringify(result.groups.map(g => ({ group: g.group, complete: g.report.complete,
    verification: g.verification, fixed_source_groups: g.fixed_theme_source_mapping.length, unsupported_fixed_dependency_absent: g.unsupported_fixed_dependency_absent })), null, 2));
}
