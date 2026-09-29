import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";
import { parseObservation } from "../src/contract.js";

const fixturePath = fileURLToPath(
  new URL("../../../fixtures/observability/ub_observation.v1.json", import.meta.url),
);
const fixtures = JSON.parse(readFileSync(fixturePath, "utf8")) as {
  valid: unknown[];
  invalid: unknown[];
};

describe("ub_observation.v1", () => {
  it("accepts the shared metadata-only fixtures", () => {
    expect(fixtures.valid.map(parseObservation)).toHaveLength(fixtures.valid.length);
  });

  it("rejects unknown content fields and dishonest usage values", () => {
    for (const fixture of fixtures.invalid) {
      expect(() => parseObservation(fixture)).toThrow();
    }
  });
});

it("preserves optional reasoning output without adding it to billed totals", () => {
  const observation = structuredClone(parseObservation(fixtures.valid[0]));
  observation.usage = { input_tokens: 100, output_tokens: 30, reasoning_output_tokens: 13,
    cached_input_tokens: 80, cache_creation_input_tokens: null, total_tokens: 130,
    source: "provider_reported", completeness: "complete" };
  const parsed = parseObservation(observation);
  expect(parsed.usage.reasoning_output_tokens).toBe(13);
  expect(parsed.usage.total_tokens).toBe(130);
  expect(() => parseObservation({ ...observation, usage: { ...observation.usage, reasoning_output_tokens: -1 } })).toThrow();
});
