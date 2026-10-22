import { readFileSync } from "node:fs";
import path from "node:path";
import { describe, expect, it } from "vitest";
import { resolveContentProfile } from "../src/content-profile";
import { markdownToBlocks } from "../src/md-adapter";
import { renderProfileSidecarModelInput } from "../src/model-input-renderer";
import { analyzeProfileSidecarSemanticUnits, buildProfileSidecarSemanticArtifact } from "../src/profile-sidecar-router";
import { segment } from "../src/segment";
import { splitWindows } from "../src/window";

const source = readFileSync(path.join(__dirname, "fixtures/optimization-context.md"), "utf8");

function fixture(maxLeavesSoft?: number) {
  const nodes = segment(markdownToBlocks(source));
  const byLid = new Map(nodes.map((node) => [node.lid, node]));
  const analysis = analyzeProfileSidecarSemanticUnits({
    source, byLid, windows: splitWindows(nodes, source, maxLeavesSoft === undefined ? undefined : { maxInputTokens: 12000, maxLeavesSoft }),
    content_profile: resolveContentProfile("technical_learning"),
  });
  const packets = Object.values(analysis.packets);
  const raw = (lid: string) => { const node = byLid.get(lid)!; return source.slice(node.span.start, node.span.end); };
  const formula = (text: string) => packets.find((packet) => packet.formula_lids.some((lid) => raw(lid).includes(text)));
  return { analysis, packets, raw, formula };
}

describe("optimization textbook context in technical_learning", () => {
  it.each([undefined, 2])("keeps theorem assumptions and inline symbols with the conclusion across windows (%s)", (limit) => {
    const input = fixture(limit);
    const packet = input.formula("\\nabla f(x^{*})=0.")!;
    expect(packet).toBeDefined();
    for (const text of ["定理 5.3", "$R^n$", "可微. 如果", "$x^*$", "是一个局部极小点, 那么"]) {
      expect(packet.text).toContain(text);
    }
    const statement = source.slice(source.indexOf("定理 5.3"), source.indexOf("\n\n$$"));
    expect(packet.text.replace(/\[\d+(?:\.\d+)*\] /g, "")).toContain(statement);
    expect(packet.text).not.toContain("证明. 任取");
    const discourse = input.packets.find((item) => item.unit_kind === "profile_sidecar_discourse" && item.text.includes("定理 5.3"))!;
    expect(discourse.text).toContain("可微. 如果");
    expect(discourse.text).toContain("\\nabla f(x^{*})=0.");
    expect(discourse.text).not.toContain("证明. 任取");
  });

  it("keeps short exercise instructions and the exercise role without anchor or next-exercise contamination", () => {
    const input = fixture();
    const exercise = input.packets.find((packet) => packet.unit_kind === "profile_sidecar_discourse" && packet.text.includes("利用定理6.5证明推论6.2."));
    expect(exercise).toBeDefined();
    expect(exercise!.text).toContain("习题 6.3");
    const packet = input.formula("\\sigma_{1}(A)")!;
    expect(packet.text).toContain("习题 2.2");
    expect(packet.text).toContain("证明：矩阵 A 的 2 范数等于其最大奇异值，即");
    expect(packet.text).not.toContain("<a ");
    expect(packet.text).not.toContain("习题 2.3");
    expect(packet.text).not.toContain("证明如下");
  });

  it("routes mathematical constraints and algorithm updates containing text commands", () => {
    const input = fixture();
    expect(input.formula("\\text{s.t.}")?.text).toContain("也可以消掉变量 s");
    expect(input.formula("\\text{令}")?.text).toContain("算法6.2");
  });

  it("keeps the proof's complete opening sentence with its Taylor expansion", () => {
    const input = fixture(2);
    const packet = input.formula("\\boldsymbol{o}(t)")!;
    for (const text of ["证明. 任取", "$v \\in \\mathbb{R}^n$", "，考虑", "$f$", "在点", "$x = x^*$", "处的泰勒展开", "整理得"]) {
      expect(packet.text).toContain(text);
    }
    const targets = input.packets.flatMap((item) => item.discourse_lids);
    expect(targets.some((lid) => input.raw(lid) === "整理得")).toBe(true);
  });

  it("classifies each original paragraph once while retaining formula and short-fragment evidence", () => {
    const input = fixture(2);
    const packets = input.packets.filter((packet) => packet.unit_kind === "profile_sidecar_discourse");
    const targets = packets.flatMap((packet) => packet.discourse_lids);
    expect(new Set(targets).size).toBe(targets.length);
    expect(input.analysis.accounting.discourse_eligible_lids).toBe(targets.length);
    const packet = packets.find((item) => item.text.includes("定理 5.3"))!;
    const contextOnly = packet.visible_lids.find((lid) => input.raw(lid).includes("可微. 如果"))!;
    expect(packet.discourse_lids).not.toContain(contextOnly);
    expect(renderProfileSidecarModelInput(packet)).toContain(`discourse_lids: ${JSON.stringify(packet.discourse_lids)}`);
    expect(() => buildProfileSidecarSemanticArtifact(packet, {
      discourse_items: [{ lid: contextOnly, mode: "informative", relations: [] }],
    })).toThrow("out-of-scope lid");
  });
});
