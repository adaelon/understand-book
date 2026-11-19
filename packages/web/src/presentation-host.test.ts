import { describe, expect, it } from "vitest";
import { acceptsPresentationMessage, resolvePresentationEditingMessage } from "./presentation-host";

describe("MU7 frame channel", () => {
  it('OM1 accepts semantic and DOM-only snapshots on the current frame channel', () => {
    const source = {} as Window;
    const state = { values: { page: { input_speed_b_m_s: 2, applied_speed_b_m_s: 1 } },
      visible_step: 'awaiting_apply', observed_result: 'B 走 10 米', source_ref_ids: [] };
    for (const snapshot of [state, { ...state, observation: {
      summary: 'B 的新输入等待应用', state: { speed_b_m_s: 2 }, rendered: { distance_b_m: 10 },
    } }]) {
      expect(acceptsPresentationMessage({ source, origin: 'null',
        data: { channel: 'current', kind: 'state', request_id: 1, state: snapshot } } as MessageEvent, source, 'current')).toBe(true);
    }
  });
  it('accepts bridge teaching help and region results only from the current frame and channel', () => {
    const source = {} as Window;
    for (const data of [
      { kind: 'interaction-result', request_id: 3, status: 'applied', state: { values: { page: { speed: 2 } } } },
      { kind: 'interaction-result', request_id: 4, status: 'failed', reason: 'Page changed' },
      { kind: 'teaching-help', request_id: 1, move_id: 'observe-candidate', help_kind: 'hint', region_id: 'candidate-help', text: '乙仍在候选里。' },
      { kind: 'teaching-help', request_id: 2, move_id: 'observe-candidate', help_kind: 'explanation', region_id: 'candidate-help', text: '主干只是一种选择结果。' },
      { kind: 'region-located', region_id: 'candidate-help', located: false },
      { kind: 'region-activated', region_id: 'candidate-help' },
    ]) {
      const event = { source, origin: 'null', data: { channel: 'current', ...data } } as MessageEvent;
      expect(acceptsPresentationMessage(event, source, 'current')).toBe(true);
      expect(acceptsPresentationMessage(event, {} as Window, 'current')).toBe(false);
      expect(acceptsPresentationMessage(event, source, 'old')).toBe(false);
    }
  });

  it('requires the bridge help receipt and region result fields', () => {
    const source = {} as Window;
    const help = { kind: 'teaching-help', request_id: 1, move_id: 'observe-candidate', help_kind: 'hint', region_id: 'candidate-help', text: '乙仍在候选里。' };
    for (const data of [{ ...help, request_id: undefined }, { ...help, help_kind: 'answer' }, { ...help, region_id: undefined },
      { ...help, text: undefined }, { kind: 'region-located', region_id: 'candidate-help', located: 'false' },
      { kind: 'region-activated', region_id: undefined }]) {
      expect(acceptsPresentationMessage({ source, origin: 'null', data: { channel: 'current', ...data } } as MessageEvent, source, 'current')).toBe(false);
    }
  });

  it("rejects another window, an old channel, a nonopaque origin and invalid actions", () => {
    const source = {} as Window;
    const event = { source, origin: "null", data: { channel: "current", kind: "source", source_ref_id: "source-1" } } as MessageEvent;
    expect(acceptsPresentationMessage(event, source, "current")).toBe(true);
    expect(acceptsPresentationMessage({ ...event, data: { channel: "current", kind: "ready" } } as MessageEvent, source, "current")).toBe(true);
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


it('RS7 accepts static output only from the same opaque frame and current channel', () => {
  const source = {} as Window;
  for (const data of [
    { kind: 'static-result', request_id: 'export-1', state: {}, image: { svg: '<svg/>', width: 640, height: 360, resources: [] } },
    { kind: 'static-error', request_id: 'export-1', message: '资源未就绪' },
  ]) {
    const event = { source, origin: 'null', data: { ...data, channel: 'current' } } as MessageEvent;
    expect(acceptsPresentationMessage(event, source, 'current')).toBe(true);
    expect(acceptsPresentationMessage(event, source, 'previous')).toBe(false);
    expect(acceptsPresentationMessage(event, {} as Window, 'current')).toBe(false);
  }
});
