/** Bind every frame message to this exact document load and its WindowProxy. */
export function acceptsPresentationMessage(event: MessageEvent, source: Window | null | undefined, channel: string): boolean {
  if (!source || event.source !== source || event.origin !== "null") return false;
  const data = event.data;
  if (!data || typeof data !== "object" || data.channel !== channel || typeof data.kind !== "string") return false;
  switch (data.kind) {
    case "static-result": return typeof data.request_id === "string"
      && data.state !== null && typeof data.state === "object"
      && typeof data.image?.svg === "string"
      && Number.isFinite(data.image.width) && data.image.width > 0
      && Number.isFinite(data.image.height) && data.image.height > 0
      && (data.image.resources === undefined || Array.isArray(data.image.resources) && data.image.resources.every((url: unknown) => typeof url === "string" && url.startsWith("data:")));
    case "static-error": return typeof data.request_id === "string" && typeof data.message === "string";
    case "state": return data.state !== null && typeof data.state === "object"
      && (data.request_id === undefined || Number.isSafeInteger(data.request_id));
    case "source": return typeof data.source_ref_id === "string";
    case "teaching-action": return typeof data.move_id === "string" && data.move_id.length <= 256;
    case "editing-focus": return typeof data.editing === "boolean" && Number.isSafeInteger(data.generation);
    case "ready": case "restore-partial": case "error": return true;
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
