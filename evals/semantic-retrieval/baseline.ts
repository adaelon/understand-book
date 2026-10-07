import { fixture, cases, policy } from "../../packages/core/testdata/semantic-retrieval/gold";
import { advanceObjectAlignment } from "../../packages/core/src/teaching-object-alignment";

// SR0 experiment only. This is not the SR1 projection or a production provider API.
export function prepareBaseline() {
  const { source, work, previous } = fixture();
  const catalog = [...work.proposal.objects.map(object => ({ key: object.key, object })),
    ...previous.active_refs.map(ref => ({ key: `previous/${encodeURIComponent(ref.object_id)}`,
      object: previous.objects.filter(o => o.ref.object_id === ref.object_id).at(-1)! }))];
  const records = catalog.map(({ key, object }) => {
    const summary = (target: string) => {
      const o = catalog.find(r => r.key === target)!.object;
      return [o.kind, o.meaning, ...o.aliases, ...o.conditions].join("; ");
    };
    const roles = object.participants.map(p => "object_key" in p
      ? `${p.role}: ${summary(p.object_key)}` : `${p.role}: ${summary(`previous/${encodeURIComponent(p.object_ref.object_id)}`)}`);
    const components = "component_keys" in object ? object.component_keys.map(summary)
      : object.component_refs.map(ref => summary(`previous/${encodeURIComponent(ref.object_id)}`));
    const lexical_fields = [object.kind, object.meaning, ...object.aliases, ...object.conditions, ...roles, ...components];
    return { key, meaning: object.meaning, aliases: object.aliases, lexical_fields, embedding_text: lexical_fields.join("\n") };
  });
  const queries = cases.map(c => {
    let current = work;
    const a: string[] = [];
    let offset: number | null = 0;
    do {
      current = advanceObjectAlignment({ work: current, source, previous, operation_id: "sr0-baseline",
        action: { kind: "search", query: c.query, offset } });
      a.push(...current.search!.keys); offset = current.search!.next_offset;
    } while (offset !== null);
    return { ...c, substring: a };
  });
  return { policy, source, catalog, records, queries };
}
