// Evaluation-only comparison of the final provider request. Keep the previous
// request in memory only; the result contains positions, lengths and tool names,
// never message, argument, schema-description or reasoning text.
const publicFields = new Set([
  'role', 'content', 'reasoning_content', 'name', 'tool_calls', 'tool_call_id',
  'id', 'type', 'function', 'arguments', 'description', 'parameters', 'properties',
  'required', 'additionalProperties', 'enum', 'const', 'items', 'anyOf', 'oneOf',
  'allOf', 'minimum', 'maximum', 'minItems', 'maxItems', 'minLength', 'maxLength',
  'strict', 'text', 'image_url', 'url', 'detail',
]);

function publicPath(parts) {
  return parts.reduce((path, part) => typeof part === 'number'
    ? `${path}[${part}]` : `${path}.${publicFields.has(part) ? part : '<field>'}`, '');
}

// Walk in serialization order. A changed key order matters to a literal
// provider prefix even if it describes the same JSON object.
function firstDifference(previous, current, parts = []) {
  if (Object.is(previous, current)) return null;
  if (previous === null || current === null || typeof previous !== 'object'
      || typeof current !== 'object' || Array.isArray(previous) !== Array.isArray(current)) return parts;
  const previousKeys = Object.keys(previous), currentKeys = Object.keys(current);
  for (let i = 0; i < Math.max(previousKeys.length, currentKeys.length); i++) {
    if (previousKeys[i] !== currentKeys[i]) {
      const key = !Object.hasOwn(current, previousKeys[i]) ? previousKeys[i] :
        !Object.hasOwn(previous, currentKeys[i]) ? currentKeys[i] : '<key_order>';
      return [...parts, Array.isArray(previous) ? i : key ?? currentKeys[i]];
    }
    const key = previousKeys[i];
    const difference = firstDifference(previous[key], current[key], [...parts, Array.isArray(previous) ? Number(key) : key]);
    if (difference) return difference;
  }
  return null;
}

function differenceCategory(parts, previous, current) {
  if (parts.includes('reasoning_content')) return 'reasoning';
  if (parts.at(-1) === 'name') return 'tool_name';
  if ([previous?.role, current?.role].some(role => role === 'system' || role === 'developer')) return 'system';
  if (parts.includes('content') || parts.includes('arguments')) return 'body';
  if (parts.includes('role')) return 'message_role';
  if (parts.includes('tool_call_id') || parts.includes('id')) return 'tool_call_id';
  return 'message_structure';
}

function commonSerializedPrefix(previous, current) {
  const a = JSON.stringify(previous), b = JSON.stringify(current);
  let end = 0;
  while (end < a.length && end < b.length && a[end] === b[end]) end++;
  // Do not count half of a Unicode character when the first mismatch is in a
  // surrogate pair. These are serialized characters/bytes, not model tokens.
  if (end && end < a.length && /[\uD800-\uDBFF]/.test(a[end - 1])) end--;
  const prefix = a.slice(0, end);
  return { common_prefix_serialized_chars: [...prefix].length,
    common_prefix_serialized_utf8_bytes: Buffer.byteLength(prefix, 'utf8') };
}

function toolNames(request) {
  return [...new Set((request.tools ?? []).map(tool => tool.function?.name).filter(name => typeof name === 'string'))].sort();
}

export function compareProviderRequests(previous, current) {
  const before = previous.messages ?? [], after = current.messages ?? [];
  let common = 0;
  while (common < before.length && common < after.length
      && JSON.stringify(before[common]) === JSON.stringify(after[common])) common++;
  let firstOldDifference = null;
  if (common < before.length) {
    const parts = after[common] === undefined ? [] : firstDifference(before[common], after[common]);
    firstOldDifference = { index: common, path: `messages[${common}]${publicPath(parts ?? [])}`,
      category: after[common] === undefined ? 'message_removed' : differenceCategory(parts ?? [], before[common], after[common]),
      previous_role: before[common]?.role ?? null, current_role: after[common]?.role ?? null };
  }
  const previousNames = toolNames(previous), currentNames = toolNames(current);
  const toolsDifference = firstDifference(previous.tools, current.tools);
  const firstToolIndex = typeof toolsDifference?.[0] === 'number' ? toolsDifference[0] : null;
  return {
    tools_changed: JSON.stringify(previous.tools) !== JSON.stringify(current.tools),
    tool_names_added: currentNames.filter(name => !previousNames.includes(name)),
    tool_names_removed: previousNames.filter(name => !currentNames.includes(name)),
    first_tool_difference: toolsDifference ? `tools${publicPath(toolsDifference)}` : null,
    previous_tool_name: firstToolIndex === null ? null : previous.tools?.[firstToolIndex]?.function?.name ?? null,
    current_tool_name: firstToolIndex === null ? null : current.tools?.[firstToolIndex]?.function?.name ?? null,
    other_request_fields_changed: [...new Set([...Object.keys(previous), ...Object.keys(current)])]
      .filter(key => key !== 'messages' && key !== 'tools' && JSON.stringify(previous[key]) !== JSON.stringify(current[key])),
    messages: {
      previous_count: before.length, current_count: after.length,
      only_appended: common === before.length && after.length > before.length,
      identical: common === before.length && common === after.length,
      common_prefix_messages: common,
      ...commonSerializedPrefix(before, after),
      first_old_difference: firstOldDifference,
    },
  };
}

export function createRequestDiffTracker() {
  const previousByPurpose = new Map();
  return {
    observe(request, purpose, ordinal) {
      const previous = previousByPurpose.get(purpose);
      const sequence = (previous?.sequence ?? 0) + 1;
      const result = { previous_ordinal: previous?.ordinal ?? null, purpose_sequence: sequence,
        ...(previous ? compareProviderRequests(previous.request, request) : {}) };
      previousByPurpose.set(purpose, { request, ordinal, sequence });
      return result;
    },
    clear() { previousByPurpose.clear(); },
  };
}
