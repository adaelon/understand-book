/** Bind every frame message to this exact document load and its WindowProxy. */
export function acceptsPresentationMessage(event: MessageEvent, source: Window | null | undefined, channel: string): boolean {
  if (!source || event.source !== source || event.origin !== "null") return false;
  const data = event.data;
  if (!data || typeof data !== "object" || data.channel !== channel || typeof data.kind !== "string") return false;
  switch (data.kind) {
    case "observe": return Number.isSafeInteger(data.revision) && data.revision > 0
      && typeof data.text === "string" && data.text.length <= 65536
      && Array.isArray(data.source_ref_ids) && data.source_ref_ids.length <= 128
      && data.source_ref_ids.every((id: unknown) => typeof id === "string" && id.length <= 256);
    case "state": return data.state !== null && typeof data.state === "object"
      && (data.request_id === undefined || Number.isSafeInteger(data.request_id));
    case "source": return typeof data.source_ref_id === "string";
    case "teaching-action": return typeof data.move_id === "string" && data.move_id.length <= 256;
    case "editing-focus": return typeof data.editing === "boolean" && Number.isSafeInteger(data.generation);
    case "restore-partial": case "error": return true;
    default: return false;
  }
}

export function resolvePresentationEditingMessage(
  message: unknown,
  context: { generation: number; frameFocused: boolean; visible: boolean },
): boolean | null {
  if (!message || typeof message !== "object") return null;
  const candidate = message as { kind?: unknown; generation?: unknown; editing?: unknown };
  if (
    candidate.kind !== "editing-focus"
    || candidate.generation !== context.generation
    || typeof candidate.editing !== "boolean"
  ) return null;
  return candidate.editing && context.frameFocused && context.visible;
}
