import type { LidNode } from "./generated/LidNode";
import { parseMarkdownSourceBlocks } from "./md-adapter";
import type { Span } from "./segment";

interface SourceGroup {
  lids: string[];
  span: Span;
  kind: "prose" | "formula" | "heading" | "marker" | "barrier";
  targetLid?: string;
  headingLid?: string;
  scope: number;
}

const LOCAL_CONTEXT_DISTANCE = 1_500;
const STATEMENT_START = /^(?:\*{1,2})?(?:(?:定理|引理|推论|命题|定义|假设|习题|练习|例题|算法)\s*\d|证明\s*[.．:：]|(?:theorem|lemma|corollary|proposition|definition|assumption|exercise|problem|algorithm|proof)\b)/iu;
const ROLE_HEADING = /(?:习题|练习|算法|定理|引理|推论|命题|定义|假设|证明|exercise|problem|algorithm|theorem|lemma|corollary|proposition|definition|assumption|proof)/iu;
const PROOF_START = /^(?:\*{1,2})?(?:证明(?:\s*[.．:：]|$)|proof\b)/iu;

function isMarker(text: string): boolean {
  return !text.trim() || /^(?:<!--[^]*?-->\s*|<a\b[^>]*>\s*<\/a>\s*)+$/iu.test(text.trim());
}

/** Groups retain original paragraph/list boundaries without changing source leaves or LIDs. */
export function createProfileSidecarContext(source: string, byLid: Map<string, LidNode>) {
  const leaves = [...byLid.values()].filter((node) => !node.children.length)
    .sort((left, right) => left.span.start - right.span.start);
  const headings = new Set([...byLid.values()]
    .filter((node) => node.kind === "chapter" || node.kind === "section")
    .map((node) => node.children[0]));
  const ranges = parseMarkdownSourceBlocks(source).alignment_contexts.map((context) => context.source_span);
  const groups: SourceGroup[] = [];
  const groupForLid = new Map<string, number>();
  let rangeIndex = 0;
  let headingLid: string | undefined;
  let scope = 0;
  for (let index = 0; index < leaves.length;) {
    const first = leaves[index];
    while (rangeIndex < ranges.length && ranges[rangeIndex].end <= first.span.start) rangeIndex += 1;
    const range = ranges[rangeIndex];
    const span = range && range.start <= first.span.start && first.span.end <= range.end && !headings.has(first.lid)
      ? range : first.span;
    const members: LidNode[] = [first];
    index += 1;
    while (index < leaves.length && leaves[index].span.end <= span.end) members.push(leaves[index++]);
    const text = source.slice(span.start, span.end);
    const target = members.find((node) => node.kind === "paragraph"
      && !isMarker(source.slice(node.span.start, node.span.end))
      && /\p{L}/u.test(source.slice(node.span.start, node.span.end)));
    let kind: SourceGroup["kind"];
    if (headings.has(first.lid)) {
      kind = "heading";
      headingLid = first.lid;
      scope += 1;
    } else if (isMarker(text)) {
      kind = "marker";
    } else if (members.some((node) => node.kind !== "paragraph" && node.kind !== "formula")) {
      kind = "barrier";
      scope += 1;
    } else if (target) {
      kind = "prose";
      if (STATEMENT_START.test(text.trim())) scope += 1;
    } else {
      kind = members.some((node) => node.kind === "formula") ? "formula" : "marker";
    }
    const group: SourceGroup = {
      lids: members.map((node) => node.lid), span, kind,
      targetLid: kind === "prose" ? target!.lid : undefined,
      headingLid, scope,
    };
    for (const lid of group.lids) groupForLid.set(lid, groups.length);
    groups.push(group);
  }

  function nearby(index: number, direction: -1 | 1): number[] {
    const origin = groups[index];
    const result: number[] = [];
    for (let next = index + direction; next >= 0 && next < groups.length; next += direction) {
      const candidate = groups[next];
      if (candidate.scope !== origin.scope || candidate.kind === "heading" || candidate.kind === "barrier") break;
      const distance = direction === -1 ? origin.span.start - candidate.span.end : candidate.span.start - origin.span.end;
      if (distance > LOCAL_CONTEXT_DISTANCE) break;
      if (candidate.kind === "marker") continue;
      result.push(next);
      if (candidate.kind === "prose") break;
    }
    return result;
  }

  function withHeading(indices: number[], roleOnly: boolean): string[] {
    const lids = indices.flatMap((index) => groups[index].lids);
    const heading = groups[indices[0]].headingLid;
    if (heading && (!roleOnly || ROLE_HEADING.test(source.slice(byLid.get(heading)!.span.start, byLid.get(heading)!.span.end)))) {
      lids.push(heading);
    }
    return [...new Set(lids)].sort((a, b) => byLid.get(a)!.span.start - byLid.get(b)!.span.start);
  }

  const mathAnchors = new Map<string, string>();
  for (let index = 0; index < groups.length; index += 1) {
    const group = groups[index];
    if (!group.targetLid) continue;
    if (STATEMENT_START.test(source.slice(group.span.start, group.span.end).trim())) {
      mathAnchors.set(group.targetLid, group.targetLid);
    } else if (group.headingLid) {
      const headingIndex = groupForLid.get(group.headingLid)!;
      const heading = source.slice(groups[headingIndex].span.start, groups[headingIndex].span.end).replace(/^#{1,6}\s*/, "").trim();
      if ((STATEMENT_START.test(heading) || /^proof\b|^证明$/iu.test(heading))
        && !groups.slice(headingIndex + 1, index).some((previous) => previous.targetLid)) {
        mathAnchors.set(group.targetLid, group.headingLid);
      }
    }
  }

  return {
    discourseTargets: new Map(groups.filter((group) => group.targetLid).map((group) => [group.targetLid!, group.scope])),
    mathAnchors,
    mathContextLids(lid: string): string[] {
      const index = groupForLid.get(mathAnchors.get(lid) ?? lid)!;
      const opening = source.slice(groups[index].span.start, groups[index].span.end).replace(/^#{1,6}\s*/, "").trim();
      if (!PROOF_START.test(opening)) return [];
      const indices: number[] = [];
      for (let previous = index - 1; previous >= 0; previous -= 1) {
        const candidate = groups[previous];
        if (groups[index].span.start - candidate.span.end > LOCAL_CONTEXT_DISTANCE || candidate.kind === "barrier") break;
        if (candidate.kind === "marker") continue;
        const text = source.slice(candidate.span.start, candidate.span.end).replace(/^#{1,6}\s*/, "").trim();
        if (PROOF_START.test(text)) break;
        indices.push(previous);
        if (STATEMENT_START.test(text)) return withHeading(indices.reverse(), true);
        if (candidate.kind === "heading") break;
      }
      return [];
    },
    discourseLids(lid: string): string[] {
      const index = groupForLid.get(lid)!;
      // A statement ending in “then” / “即” needs its immediately following display equations.
      const equations = nearby(index, 1).filter((next) => groups[next].kind === "formula");
      return withHeading([index, ...equations], true);
    },
    formulaContext(lid: string): { lids: string[]; hasExplanation: boolean } {
      const index = groupForLid.get(lid)!;
      const indices = [...nearby(index, -1), index, ...nearby(index, 1)].sort((a, b) => a - b);
      return { lids: withHeading(indices, false), hasExplanation: indices.some((next) => groups[next].kind === "prose") };
    },
    render(lids: string[]): string {
      const selected = new Set(lids);
      const indices = [...new Set(lids.map((lid) => groupForLid.get(lid)!))].sort((a, b) => a - b);
      return indices.map((index) => {
        const members = groups[index].lids.filter((lid) => selected.has(lid));
        return members.map((lid, position) => {
          const node = byLid.get(lid)!;
          const gap = position ? source.slice(byLid.get(members[position - 1])!.span.end, node.span.start) : "";
          return `${gap}[${lid}] ${source.slice(node.span.start, node.span.end)}`;
        }).join("");
      }).join("\n\n");
    },
  };
}

export type ProfileSidecarContext = ReturnType<typeof createProfileSidecarContext>;
