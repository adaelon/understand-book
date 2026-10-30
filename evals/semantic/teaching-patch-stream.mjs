// Observe tool arguments without changing the forwarded Provider response bytes.
// Thinking and ordinary answer content are never retained in capture output.
import { StringDecoder } from 'node:string_decoder';

export function measureMessagesInput(request, wireBytes) {
  const blocks = content => typeof content === 'string' ? [{ type: 'text', text: content }] : (content ?? []);
  const text = content => blocks(content).filter(block => block.type === 'text' && typeof block.text === 'string');
  const bytes = values => values.reduce((sum, value) => sum + Buffer.byteLength(value, 'utf8'), 0);
  const systems = (request.messages ?? []).filter(message => message.role === 'system');
  const historyText = systems.flatMap(message => text(message.content));
  const updates = systems.flatMap(message => blocks(message.content)).filter(block => ['tool_addition', 'tool_removal'].includes(block.type));
  const topBytes = bytes(text(request.system).map(block => block.text));
  const historyBytes = bytes(historyText.map(block => block.text));
  return {
    ...(wireBytes == null ? {} : { request_body_bytes: wireBytes }),
    top_system_text_bytes: topBytes,
    history_system_text_bytes: historyBytes,
    all_system_text_bytes: topBytes + historyBytes,
    history_system_message_count: systems.length,
    history_system_text_snapshot_count: systems.filter(message => text(message.content).length > 0).length,
    history_system_text_block_count: historyText.length,
    history_tool_update_message_count: systems.filter(message => blocks(message.content).some(block => ['tool_addition', 'tool_removal'].includes(block.type))).length,
    history_tool_update_block_count: updates.length,
    history_tool_update_block_json_bytes: bytes(updates.map(block => JSON.stringify(block))),
  };
}

export function createToolUseCapture(contentType) {
  const decoder = new StringDecoder('utf8');
  const streaming = contentType?.includes('text/event-stream');
  const pending = new Map();
  const calls = [], errors = [];
  const usage = {};
  let providerError;
  let buffered = '';
  function accept(block) {
    if (block?.type === 'tool_use') calls.push({ id: block.id, name: block.name, arguments: block.input });
  }
  function updateUsage(value) {
    // Native Messages reports cumulative values within one response. Replace,
    // never add stream updates, then retain only numeric accounting fields.
    for (const field of ['input_tokens', 'output_tokens', 'cache_read_input_tokens', 'cache_creation_input_tokens']) {
      if (Number.isFinite(value?.[field])) usage[field] = value[field];
    }
  }
  function event(frame) {
    const data = frame.split(/\r?\n/).filter(line => line.startsWith('data:')).map(line => line.slice(5).trimStart()).join('\n');
    if (!data || data === '[DONE]') return;
    try {
      const value = JSON.parse(data);
      updateUsage(value.usage);
      updateUsage(value.message?.usage);
      if (value.type === 'content_block_start' && value.content_block?.type === 'tool_use') {
        pending.set(value.index, { ...value.content_block, json: '' });
      } else if (value.type === 'content_block_delta' && value.delta?.type === 'input_json_delta') {
        const block = pending.get(value.index);
        if (block) block.json += value.delta.partial_json;
      } else if (value.type === 'content_block_stop' && pending.has(value.index)) {
        const block = pending.get(value.index);
        pending.delete(value.index);
        accept({ ...block, input: block.json ? JSON.parse(block.json) : block.input });
      }
    } catch (error) { errors.push(error.message); }
  }
  function consume() {
    if (!streaming) return;
    for (;;) {
      const separator = /\r?\n\r?\n/.exec(buffered);
      if (!separator) return;
      const frame = buffered.slice(0, separator.index);
      buffered = buffered.slice(separator.index + separator[0].length);
      event(frame);
    }
  }
  return {
    write(chunk) { buffered += decoder.write(chunk); consume(); },
    finish() {
      buffered += decoder.end(); consume();
      if (streaming) {
        if (buffered.trim()) event(buffered);
        if (pending.size) errors.push(`${pending.size} incomplete tool blocks at response end`);
      } else if (buffered.trim()) {
        try {
          const value = JSON.parse(buffered);
          providerError = value.error;
          updateUsage(value.usage);
          for (const block of value.content ?? []) accept(block);
        }
        catch (error) { errors.push(error.message); }
      }
      buffered = '';
      return { calls, errors, ...(Object.keys(usage).length ? { usage } : {}), ...(providerError ? { providerError } : {}) };
    },
  };
}
