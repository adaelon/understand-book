import { expect, it } from "vitest";
import { money, moneyInput, timestamp, dateInput } from "./format";
it("keeps fen and micro amounts exact, including negative adjustments and unknown costs", () => {
  expect(moneyInput("1.000001")).toBe(1000001);
  expect(moneyInput("-0.29", 2)).toBe(-29);
  expect(money(1000001)).toBe("1.000001");
  expect(money(-29, 2)).toBe("-0.29");
  expect(money(null)).toBe("未知");
  expect(() => moneyInput("1.001", 2)).toThrow();
  expect(() => moneyInput("1e3")).toThrow();
  expect(() => moneyInput("9007199255")).toThrow();
});
it("uses Hong Kong time regardless of the browser time zone", () => {
  expect(timestamp("2026-10-08T10:30")).toBe(
    Date.parse("2026-10-08T02:30:00Z") / 1000,
  );
  expect(dateInput(timestamp("2026-10-08T10:30"))).toBe("2026-10-08T10:30");
});
