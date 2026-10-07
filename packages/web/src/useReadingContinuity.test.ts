import { describe, expect, it } from "vitest";
import { createReadingContinuity } from "./useReadingContinuity";

describe("reading continuity", () => {
  it("returns to the original annotation surface and record in the same scene", () => {
    const continuity = createReadingContinuity();
    const point = { contextKey: 'A:workspace:2:publication', turnId: '', origin: 'notes' as const, memId: 'note-new', anchor: null };
    continuity.push(point);
    expect(continuity.peek(point.contextKey)).toEqual(point);
    expect(continuity.pop(point.contextKey)).toEqual(point);
    continuity.push(point);
    continuity.invalidateContext('B:workspace:2:publication');
    expect(continuity.peek(point.contextKey)).toBeNull();
  });
  it("keeps a bounded LIFO stack per current context", () => {
    const continuity = createReadingContinuity(2);
    continuity.push({ contextKey: "a", turnId: "1", anchor: null });
    continuity.push({ contextKey: "a", turnId: "2", anchor: { surface: "markdown", lid: "L2", top: 20 } });
    continuity.push({ contextKey: "a", turnId: "3", anchor: { surface: "markdown", lid: "L3", top: 30 } });
    expect(continuity.pop("a")?.turnId).toBe("3");
    expect(continuity.pop("a")?.turnId).toBe("2");
    expect(continuity.pop("a")).toBeNull();
  });

  it("invalidates foreign contexts and stale restore work", () => {
    const continuity = createReadingContinuity();
    continuity.push({ contextKey: "old", turnId: "1", anchor: null });
    continuity.push({ contextKey: "current", turnId: "2", anchor: null });
    const stale = continuity.beginRestore("current");
    continuity.invalidateContext("current");
    expect(continuity.has("old")).toBe(false);
    expect(continuity.has("current")).toBe(true);
    expect(continuity.isCurrent(stale)).toBe(false);
  });

  it("lets explicit user interaction cancel an in-flight restore", () => {
    const continuity = createReadingContinuity();
    const token = continuity.beginRestore("current");
    continuity.cancelRestore();
    expect(continuity.isCurrent(token)).toBe(false);
  });
});
