import { existsSync, mkdirSync, mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { spawnSync } from "node:child_process";
import { tmpdir } from "node:os";
import path from "node:path";
import { describe, expect, it } from "vitest";
import { resolveContentProfile } from "../src/content-profile";
import { automaticBuildFailureDiagnosticFromWriterError, automaticBuildCandidateCorrection, parseExtractorContractErrorFromStderr, parseExtractorCandidate, ExtractorContractError } from "../src/extractor-contract";
import { buildMathStructureIndex, queryMathStructure, restoreMathStructureSourceQuotes, validateMathStructureCandidates, type MathStructureCandidate } from "../src/math-structure";
import { markdownToBlocks } from "../src/md-adapter";
import { buildReproducibleProfileArtifactHeader } from "../src/profile-artifact";
import { analyzeProfileSidecarSemanticUnits, buildProfileSidecarSemanticArtifact } from "../src/profile-sidecar-router";
import { segment } from "../src/segment";
import { splitWindows } from "../src/window";

function optimizationMathFixture(maxLeavesSoft = 2) {
  const source = readFileSync(path.join(__dirname, "fixtures/optimization-context.md"), "utf8");
  const nodes = segment(markdownToBlocks(source));
  const byLid = new Map(nodes.map((node) => [node.lid, node]));
  const windows = splitWindows(nodes, source, { maxInputTokens: 12000, maxLeavesSoft });
  const analysis = analyzeProfileSidecarSemanticUnits({ source, byLid, windows, content_profile: resolveContentProfile("technical_learning") });
  const raw = (lid: string) => { const node = byLid.get(lid)!; return source.slice(node.span.start, node.span.end); };
  const packet = (prefix: string) => Object.values(analysis.packets).find((unit) => unit.math_anchor_lids.some((lid) => raw(lid).includes(prefix)))!;
  const item = (prefix: string, kind: MathStructureCandidate["kind"], label?: string): MathStructureCandidate => {
    const unit = packet(prefix);
    const anchor = unit.math_anchor_lids.find((lid) => raw(lid).includes(prefix))!;
    return { anchor_lid: anchor, kind, ...(label ? { label } : {}), evidence_lids: unit.visible_lids, assumptions: [], references: [] };
  };
  const theorem = item("定理 5.3", "theorem", "定理 5.3");
  theorem.assumptions = [
    { quote: "假设 f 在全空间  $R^n$ 可微.", evidence_lids: packet("定理 5.3").visible_lids.filter((lid) => /假设|R\^n|可微/.test(raw(lid))) },
    { quote: "如果  $x^*$ 是一个局部极小点", evidence_lids: packet("定理 5.3").visible_lids.filter((lid) => /如果|\$x\^\*\$|局部极小点/.test(raw(lid))) },
  ];
  const proof = item("证明. 任取", "proof");
  proof.references = [{ relation: "proves", target_kind: "theorem", target_label: "定理 5.3", scope: "local", quote: "证明. 任取", evidence_lids: [proof.anchor_lid, theorem.anchor_lid] }];
  const exercise = item("习题 6.3", "exercise", "习题 6.3");
  const instruction = packet("习题 6.3").visible_lids.filter((lid) => raw(lid).includes("利用定理6.5证明推论6.2."));
  exercise.references = [
    { relation: "uses", target_kind: "theorem", target_label: "定理6.5", scope: "local", quote: "利用定理6.5证明推论6.2.", evidence_lids: instruction },
    { relation: "asks_to_prove", target_kind: "corollary", target_label: "推论6.2", scope: "local", quote: "利用定理6.5证明推论6.2.", evidence_lids: instruction },
  ];
  const candidates = [theorem, proof, exercise, item("定理 6.5", "theorem", "定理 6.5"), item("推论 6.2", "corollary", "推论 6.2")];
  return { source, nodes, byLid, windows, analysis, packet, raw, candidates, theorem, proof, exercise };
}

const header = buildReproducibleProfileArtifactHeader({ book_id: "optimization-context", content_profile: "technical_learning" });

describe("lightweight mathematical structure", () => {
  it("restores unique cited source quotes before strict writer validation without changing the candidate", () => {
    const input = optimizationMathFixture();
    const candidate = structuredClone(input.theorem);
    candidate.label = candidate.label!.replace(/\s/gu, "");
    for (const assumption of candidate.assumptions) assumption.quote = assumption.quote.replace(/\s/gu, "");
    const original = structuredClone(candidate);
    const artifact = buildProfileSidecarSemanticArtifact(input.packet("定理 5.3"), { math_items: [candidate] }, input);
    expect(candidate).toEqual(original);
    expect(artifact.math_items).toEqual([input.theorem]);
    expect(() => validateMathStructureCandidates(artifact.math_items!, {
      ...input, anchor_lids: [input.theorem.anchor_lid], visible_lids: input.packet("定理 5.3").visible_lids,
    })).not.toThrow();
  });

  it.each([
    ["x <= 4", "x  <= 3"],
    ["x >= 3", "x  <= 3"],
    ["f 连续", "f 不连续"],
    ["x <= 3", "x  <=3; x <=  3"],
    ["x<=3", "x  <= 3", true],
  ] as const)("keeps content changes, ambiguity and evidence boundaries strict: %s", (quote: string, source: string, outside: boolean = false) => {
    const input = { source, byLid: new Map([[
      "1", { span: { start: 0, end: outside ? 1 : source.length } },
    ]]) };
    const candidate: MathStructureCandidate = { kind: "assumption", anchor_lid: "1", evidence_lids: ["1"],
      assumptions: [{ quote, evidence_lids: ["1"] }], references: [] };
    const restored = restoreMathStructureSourceQuotes([candidate], input);
    expect(restored[0].assumptions[0].quote).toBe(quote);
    expect(() => validateMathStructureCandidates(restored, { ...input, anchor_lids: ["1"], visible_lids: ["1"] }))
      .toThrow("assumption is not quoted");
  });

  it("transports a rejected math quotation as bounded correctable evidence feedback", () => {
    const input = optimizationMathFixture();
    const candidate = { ...input.theorem, assumptions: [{ quote: "invented assumption", evidence_lids: input.theorem.evidence_lids }] };
    let failure: unknown;
    try { buildProfileSidecarSemanticArtifact(input.packet("定理 5.3"), { math_items: [candidate] }, input); }
    catch (error) { failure = error; }
    expect(failure).toBeInstanceOf(ExtractorContractError);
    const transported = parseExtractorContractErrorFromStderr((failure as Error).stack);
    expect(transported).toBeInstanceOf(ExtractorContractError);
    const diagnostic = automaticBuildFailureDiagnosticFromWriterError(transported, { writer_started: true });
    expect(diagnostic).toMatchObject({ category: "evidence", code: "quote_not_in_evidence", phase: "artifact_writer",
      json_pointer: "/math_items/0/assumptions/0/quote" });
    expect(automaticBuildCandidateCorrection(diagnostic)?.expected).toContain("exact contiguous quotation");
    expect(JSON.stringify(diagnostic)).not.toContain("invented assumption");
    expect(automaticBuildFailureDiagnosticFromWriterError(new Error("disk failure"), {writer_started:true}))
      .toMatchObject({category:"internal",code:"writer_failed"});
  });
  it("routes unique mathematical anchors and gives proofs their visible theorem without adding a model pass", () => {
    const input = optimizationMathFixture();
    const anchors = Object.values(input.analysis.packets).flatMap((unit) => unit.math_anchor_lids);
    expect(new Set(anchors).size).toBe(anchors.length);
    expect(input.packet("证明. 任取").text).toContain("定理 5.3");
    expect(input.packet("证明. 任取").math_anchor_lids).not.toContain(input.theorem.anchor_lid);
    expect(input.packet("习题 6.3").math_anchor_lids).toEqual([input.exercise.anchor_lid]);
    const paper = analyzeProfileSidecarSemanticUnits({ ...input, content_profile: resolveContentProfile("paper") });
    expect(Object.values(paper.packets).every((unit) => unit.math_anchor_lids.length === 0)).toBe(true);
  });

  it("accepts exact assumptions across inline math leaves and resolves theorem/proof/exercise references", () => {
    const input = optimizationMathFixture();
    for (const candidate of input.candidates) {
      const packet = Object.values(input.analysis.packets).find((unit) => unit.math_anchor_lids.includes(candidate.anchor_lid))!;
      const output = parseExtractorCandidate("profile_sidecar", { math_items: [candidate] }, { allowed_evidence_lids: packet.visible_lids, formula_lids: [] });
      expect(buildProfileSidecarSemanticArtifact(packet, output, input).math_items).toEqual([candidate]);
    }
    const index = buildMathStructureIndex(header, input.candidates, input);
    expect(queryMathStructure(index, "定理5.3")[0].assumptions).toHaveLength(2);
    expect(queryMathStructure(index, input.proof.anchor_lid)[0].references[0]).toMatchObject({ relation: "proves", resolution: "resolved", target_lid: input.theorem.anchor_lid });
    expect(queryMathStructure(index, "习题6.3")[0].references).toMatchObject([
      { relation: "uses", resolution: "resolved", target_lid: input.candidates[3].anchor_lid },
      { relation: "asks_to_prove", resolution: "resolved", target_lid: input.candidates[4].anchor_lid },
    ]);
  });

  it("keeps labelled headings as anchors and supplies the theorem to a proof heading", () => {
    const source = "# Theorem 1\n\nAssume x is positive. Then x is nonzero.\n\n# 证明\n\n由正数的定义可知结论成立。\n\n# Algorithm 1\n\nReturn x.\n\n练习 1 证明定理1。";
    const nodes = segment(markdownToBlocks(source));
    const byLid = new Map(nodes.map((node) => [node.lid, node]));
    const analysis = analyzeProfileSidecarSemanticUnits({ source, byLid, windows: splitWindows(nodes, source), content_profile: resolveContentProfile("technical_learning") });
    const raw = (lid: string) => { const node = byLid.get(lid)!; return source.slice(node.span.start, node.span.end); };
    const anchors = Object.values(analysis.packets).flatMap((packet) => packet.math_anchor_lids);
    expect(anchors.map(raw)).toEqual(["# Theorem 1", "# 证明", "# Algorithm 1", "练习 1 证明定理1。"]);
    const proof = Object.values(analysis.packets).find((packet) => packet.math_anchor_lids.some((lid) => raw(lid) === "# 证明"))!;
    expect(proof.text).toContain("# Theorem 1");
    expect(proof.text).toContain("Assume x is positive.");
    expect(proof.math_anchor_lids.map(raw)).toEqual(["# 证明"]);
  });

  it("rejects invented conditions, context-only anchors and treating exercise requests as source proofs", () => {
    const input = optimizationMathFixture();
    const invented = structuredClone(input.theorem);
    invented.assumptions[0].quote = "假设 f 在全空间二阶连续可微";
    expect(() => buildMathStructureIndex(header, [invented], input)).toThrow("assumption is not quoted");
    expect(() => buildProfileSidecarSemanticArtifact(input.packet("证明. 任取"), { math_items: [input.theorem] }, input)).toThrow("anchor is out of scope");
    const inventedReference = structuredClone(input.exercise);
    inventedReference.references[0].target_label = "定理99.9";
    expect(() => buildMathStructureIndex(header, [inventedReference], input)).toThrow("target label is not quoted");
    const exercise = structuredClone(input.exercise);
    exercise.references[1].relation = "proves";
    expect(() => buildMathStructureIndex(header, [exercise], input)).toThrow("only a source proof");
  });

  it("preserves missing and external references without inventing local targets", () => {
    const input = optimizationMathFixture();
    const exercise = structuredClone(input.exercise);
    exercise.references[0].scope = "external";
    const index = buildMathStructureIndex(header, [exercise, input.candidates[3]], input);
    expect(index.items.find((item) => item.kind === "exercise")!.references).toMatchObject([
      { resolution: "external" }, { resolution: "missing" },
    ]);
    expect(index.items.flatMap((item) => item.references).every((ref) => !ref.target_lid)).toBe(true);
  });

  it("keeps duplicate printed labels ambiguous and disallows duplicate source anchors", () => {
    const source = "# A\n\n定理 1 设 f 可微。\n\n# B\n\n定理 1 设 g 可微。\n\n习题 1 利用定理1。";
    const nodes = segment(markdownToBlocks(source));
    const byLid = new Map(nodes.map((node) => [node.lid, node]));
    const leaves = nodes.filter((node) => !node.children.length && /^(定理|习题)/u.test(source.slice(node.span.start, node.span.end)));
    const items: MathStructureCandidate[] = leaves.map((node, index) => ({ anchor_lid: node.lid, kind: index === 2 ? "exercise" : "theorem", label: index === 2 ? "习题 1" : "定理 1", evidence_lids: [node.lid], assumptions: [], references: [] }));
    items[2].references = [{ relation: "uses", target_kind: "theorem", target_label: "定理1", scope: "local", quote: "利用定理1", evidence_lids: [leaves[2].lid] }];
    expect(buildMathStructureIndex(header, items, { source, byLid }).items[2].references[0]).toMatchObject({ resolution: "ambiguous" });
    expect(() => buildMathStructureIndex(header, [items[0], items[0]], { source, byLid })).toThrow("duplicated");
  });

  it("publishes the accepted textbook index with the sidecars and queries it by printed number", () => {
    const input = optimizationMathFixture(80);
    const root = mkdtempSync(path.join(tmpdir(), "ub-math-structure-"));
    const workspace = path.join(root, ".understand-book/optimization-context");
    const buildDir = path.join(workspace, ".build/profile-sidecar");
    mkdirSync(buildDir, { recursive: true });
    for (const packet of Object.values(input.analysis.packets)) {
      const candidates = input.candidates.filter((item) => packet.math_anchor_lids.includes(item.anchor_lid));
      if (!candidates.length) continue;
      const artifact = buildProfileSidecarSemanticArtifact(packet, {
        discourse_items: packet.discourse_lids.map((lid) => ({ lid, mode: "informative", relations: [] })),
        math_items: candidates,
      }, input);
      writeFileSync(path.join(buildDir, `${packet.work_unit_id}.json`), JSON.stringify(artifact));
    }
    const repo = path.resolve(__dirname, "../../..");
    const tsx = path.join(repo, "node_modules/tsx/dist/cli.mjs");
    const run = (script: string, args: string[], cwd = root) => spawnSync(process.execPath, [tsx, path.join(repo, "skills/build", script), ...args], { cwd, encoding: "utf8" });
    const sourcePath = path.join(__dirname, "fixtures/optimization-context.md");
    const batch = run("profile-sidecar-batch.ts", [sourcePath, "--allow-partial"]);
    expect(batch.status, batch.stderr).toBe(0);
    const query = run("math-structure-query.ts", [workspace, "习题6.3"]);
    expect(query.status, query.stderr).toBe(0);
    expect(JSON.parse(query.stdout).items[0].references).toMatchObject([
      { relation: "uses", resolution: "resolved" }, { relation: "asks_to_prove", resolution: "resolved" },
    ]);
    const result = JSON.parse(readFileSync(path.join(workspace, "math_structure.json"), "utf8"));
    expect(result.items).toHaveLength(5);
    const paperRoot = path.join(root, "paper");
    mkdirSync(paperRoot);
    const paper = run("profile-sidecar-batch.ts", [sourcePath, "--allow-partial", "--content-profile", "paper"], paperRoot);
    expect(paper.status, paper.stderr).toBe(0);
    expect(existsSync(path.join(paperRoot, ".understand-book/optimization-context/math_structure.json"))).toBe(false);
  }, 30_000);
});
