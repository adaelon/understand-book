import { describe, expect, it } from "vitest";
import { acceptsPresentationMessage, resolvePresentationEditingMessage } from "./presentation-host";

describe("MU7 frame channel", () => {
  it("rejects another window, an old channel, a nonopaque origin and invalid actions", () => {
    const source = {} as Window;
    const event = { source, origin: "null", data: { channel: "current", kind: "source", source_ref_id: "source-1" } } as MessageEvent;
    expect(acceptsPresentationMessage(event, source, "current")).toBe(true);
    expect(acceptsPresentationMessage(event, {} as Window, "current")).toBe(false);
    expect(acceptsPresentationMessage(event, source, "old")).toBe(false);
    expect(acceptsPresentationMessage({ ...event, origin: "https://reader.example" } as MessageEvent, source, "current")).toBe(false);
    for (const data of [{ kind: "navigate", url: "/api/me" }, { kind: "observe", revision: 1, text: "x", source_ref_ids: [42] }, { kind: "state", state: null }]) {
      expect(acceptsPresentationMessage({ ...event, data: { channel: "current", ...data } } as MessageEvent, source, "current")).toBe(false);
    }
  });
});

describe("presentation host editing focus", () => {
  it("accepts only the current visible and actually focused frame", () => {
    const message = { kind: "editing-focus", generation: 4, editing: true };
    expect(resolvePresentationEditingMessage(message, { generation: 4, frameFocused: true, visible: true })).toBe(true);
    expect(resolvePresentationEditingMessage(message, { generation: 5, frameFocused: true, visible: true })).toBeNull();
    expect(resolvePresentationEditingMessage(message, { generation: 4, frameFocused: false, visible: true })).toBe(false);
    expect(resolvePresentationEditingMessage(message, { generation: 4, frameFocused: true, visible: false })).toBe(false);
  });

  it("clears a current editing hint without requiring the frame to stay focused", () => {
    expect(resolvePresentationEditingMessage(
      { kind: "editing-focus", generation: 4, editing: false },
      { generation: 4, frameFocused: false, visible: true },
    )).toBe(false);
  });
});
