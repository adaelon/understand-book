// Shared by the Reader and the server's real-browser presentation preview.
export function sourceChipLabel(sources, id) {
  const index = sources.findIndex(source => source.source_ref_id === id);
  if (index < 0) return '来源不可用';
  const label = sources[index].label;
  const title = (label.includes(' · ') ? label.slice(label.indexOf(' · ') + 3) : label).split(' / ').at(-1).trim();
  const chars = Array.from(title);
  return `${chars.length > 14 ? chars.slice(0, 14).join('') + '…' : title} [${index + 1}]`;
}
