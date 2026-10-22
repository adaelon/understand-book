import assert from "node:assert/strict";
import { readFileSync, writeFileSync } from "node:fs";
import path from "node:path";
import { prepareDocumentInput } from "../packages/core/src/document-import";
import { resolveAutomaticBuildTarget } from "../packages/core/src/build-orchestrator";
import { runAutomaticBuildDriverCommandWithPreparation } from "../skills/build/automatic-build-driver";

const root = path.resolve(process.argv[2] ?? "tmp/cloud-document-acceptance");
const results = [];
for (const extension of ["pptx", "ppt"]) {
  const input = path.join(root, "fixtures", `original-exam.${extension}`);
  const workspace = await prepareDocumentInput(input, path.join(root, `prepared-${extension}`));
  const target = resolveAutomaticBuildTarget(input, path.join(root, `prepared-${extension}`));
  const metadata = JSON.parse(readFileSync(path.join(workspace, "document_import.json"), "utf8"));
  const source = readFileSync(target.source_path, "utf8");
  const map = JSON.parse(readFileSync(path.join(workspace, "pdf_source_map.json"), "utf8"));
  assert.equal(metadata.page_count, 3, "Hidden slide was lost during conversion");
  assert.equal(metadata.page_unit, "slide");
  assert.equal(metadata.source_format, extension);
  assert.match(source, /Function and values/u);
  assert.match(source, /f\(x\) = x² \+ 1/u);
  assert.match(source, /Hidden review slide/u);
  assert.deepEqual([...new Set(map.entries.flatMap((entry: any) => entry.regions.map((region: any) => region.pageIndex)))].sort(), [0, 1, 2]);
  assert.equal(await prepareDocumentInput(input, path.join(root, `prepared-${extension}`)), workspace);
  assert.deepEqual(readFileSync(path.join(workspace, metadata.source_snapshot)), readFileSync(input));
  const plan: any = await runAutomaticBuildDriverCommandWithPreparation({
    version: "dsh_build_prepare.v2", target_input: input, root_dir: path.join(root, `prepared-${extension}`),
    max_parallel: 1, pass2: "disabled", build_tutor: false,
  });
  assert.equal(plan.version, "dsh_build_prepared.v2");
  assert.equal(plan.workspace_dir, workspace);
  assert.ok(plan.plan.public_stage_closure.includes("pass1"));
  results.push({ format: extension, workspace, pages: metadata.page_count, hidden_slide: true, formula: "f(x) = x² + 1", original_retained: true, resumed: true, cloud_model_plan: true });
}
writeFileSync(path.join(root, "conversion-result.json"), JSON.stringify(results, null, 2));
console.log(JSON.stringify(results, null, 2));
