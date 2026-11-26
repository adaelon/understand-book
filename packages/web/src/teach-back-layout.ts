import type { TeachBackBookGraph } from './generated/TeachBackBookGraph';
import type { TeachBackStudentGraph } from './generated/TeachBackStudentGraph';
import type { TeachBackRow } from './generated/TeachBackRow';

type Graph = TeachBackBookGraph | TeachBackStudentGraph;
export type GraphNode = Graph['nodes'][number];
export type GraphEdge = Graph['edges'][number];
export type GraphSide = 'book' | 'student';

export const focusNumber = (index: number) => String.fromCodePoint(0x2460 + index);
export const statusLabels = {
  consistent: '一致', divergent: '有分歧', omitted: '本题核心未提', alternative_valid: '另一种成立的解释',
  unverified_against_passage: '据本段待核对', insufficient_book_basis: '书中依据不足', clarified: '原因已澄清',
};
export const relationLabels = {
  causes: '导致', requires: '需要', contrasts: '对比', defines: '定义', rebuts: '反驳', supports: '支持', concludes: '得出结论',
};

export function graphLayout(graph: Graph, side: GraphSide, rows: TeachBackRow[], maxColumns: number) {
  const edges = graph.edges as GraphEdge[];
  const nodes = graph.nodes as GraphNode[];
  const edgesAt = (id: string) => edges.filter(edge => edge.from === id || edge.to === id);
  const nodeGroups = new Map(nodes.map(node => [node.id, rows.flatMap((row, index) => {
    const member = row[`${side}_node_ids`].includes(node.id) ||
      edgesAt(node.id).some(edge => row[`${side}_edge_ids`].includes(edge.id));
    return member ? [index] : [];
  })]));
  const edgeGroups = new Map(edges.map(edge => [edge.id, rows.flatMap((row, i) => row[`${side}_edge_ids`].includes(edge.id) ? [i] : [])]));
  type Layer = { nodes: GraphNode[]; outgoing: GraphEdge[]; inlineEdge?: GraphEdge; reversed?: boolean };
  const layers: Layer[] = [];
  const unseen = new Set(nodes.map(n => n.id));
  for (const node of nodes) {
    if (!unseen.has(node.id)) continue;
    const pending = new Set([node.id]);
    for (const id of pending) {
      unseen.delete(id);
      for (const edge of edgesAt(id)) pending.add(edge.from === id ? edge.to : edge.from);
    }
    const ranks: GraphNode[][] = [];
    while (pending.size) {
      const available = nodes.filter(n => pending.has(n.id) &&
        !edges.some(edge => edge.to === n.id && pending.has(edge.from)));
      const rank = available.length ? available : [nodes.find(n => pending.has(n.id))!];
      for (let i = 0; i < rank.length; i += maxColumns) ranks.push(rank.slice(i, i + maxColumns));
      rank.forEach(n => pending.delete(n.id));
    }

    // Put consecutive single steps on the same line. The next pair starts on
    // the side where its premise ended, so a chain reads as a continuous path.
    let previous: Layer | undefined;
    for (let i = 0; i < ranks.length; i++) {
      const group = [...ranks[i]];
      const from = group[0].id;
      const next = ranks[i + 1];
      const inlineCandidates = group.length === 1 && next?.length === 1
        ? edges.filter(e => e.from === from && e.to === next[0].id) : [];
      const inlineEdge = inlineCandidates.length === 1 ? inlineCandidates[0] : undefined;
      let reversed = false;
      if (inlineEdge && next) {
        group.push(next[0]);
        i++;
        if (previous && previous.nodes.length > 1) {
          const incoming = previous.nodes.findIndex(n => edges.some(e => e.from === n.id && e.to === from));
          reversed = incoming >= 0 && (previous.reversed ? incoming === 0 : incoming > 0);
        }
      }
      const outgoing = group.flatMap(n => edges.filter(e => e.from === n.id && e !== inlineEdge));
      const layer: Layer = { nodes: group, outgoing, inlineEdge, reversed };
      layers.push(layer);
      previous = layer;
    }
  }
  const rankOf = new Map(layers.flatMap((layer, rank) => layer.nodes.map(n => [n.id, rank] as const)));
  const inlineIds = new Set(layers.flatMap(layer => layer.inlineEdge ? [layer.inlineEdge.id] : []));
  const bypass = edges.filter(e => !inlineIds.has(e.id) && rankOf.get(e.to)! !== rankOf.get(e.from)! + 1);
  // Spread arrows in the same order as their endpoints. When several edges
  // leave one node, this keeps the labels under their respective branches.
  function column(id: string) {
    const layer = layers[rankOf.get(id)!];
    const index = layer.nodes.findIndex(n => n.id === id);
    return ((layer.reversed ? layer.nodes.length - 1 - index : index) + 0.5) / layer.nodes.length;
  }
  // Put parallel premises nearest the conclusions they feed. For a simple
  // merge they tie and retain the author's order.
  for (const layer of [...layers].reverse()) {
    if (layer.inlineEdge || layer.nodes.length < 2) continue;
    const weights = new Map(layer.nodes.map(n => {
      const outgoing = layer.outgoing.filter(e => e.from === n.id);
      const weight = outgoing.length
        ? outgoing.reduce((sum, e) => sum + column(e.to), 0) / outgoing.length : column(n.id);
      return [n.id, weight];
    }));
    layer.nodes.sort((a, b) => weights.get(a.id)! - weights.get(b.id)!);
  }
  for (const layer of layers) {
    const labelOrder = (e: GraphEdge) => bypass.includes(e) ? 2 : (column(e.from) + column(e.to)) / 2;
    layer.outgoing.sort((a, b) => labelOrder(a) - labelOrder(b));
  }
  return { layers, bypass, inlineIds, nodeGroups, edgeGroups };
}
