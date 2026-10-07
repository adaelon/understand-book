import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";
import { parseObservation, type RequestDiagnostics } from "../src/contract.js";

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

it("accepts request differences alongside usage and rejects content in diagnostic fields", () => {
  const observation = structuredClone(parseObservation(fixtures.valid[0]));
  const diagnostics: RequestDiagnostics = {
    request_index: 2, step_id: 4, previous_step_id: 1,
    message_count: 6, previous_message_count: 4, unchanged_prefix_messages: 2,
    first_changed_message: { index: 2, role: "assistant", fields: ["reasoning_content", "tool_arguments"],
      previous_bytes: 100, current_bytes: 80, unchanged_prefix_bytes: 30 },
    messages_append_only: false, tool_count: 3, previous_tool_count: 3,
    tools_changed: false, first_changed_tool: null, settings_changed: false,
    image_count: 0, previous_image_count: 1, reasoning_message_count: 0, previous_reasoning_message_count: 1,
  };
  observation.metadata.request_diagnostics = diagnostics;
  expect(parseObservation(observation).metadata.request_diagnostics).toEqual(diagnostics);
  for (const bad of [
    { ...diagnostics, body: "private prompt" },
    { ...diagnostics, step_id: -1 },
    { ...diagnostics, first_changed_message: { ...diagnostics.first_changed_message, role: "private text" } },
    { ...diagnostics, first_changed_message: { ...diagnostics.first_changed_message, fields: ["private arguments"] } },
    { ...diagnostics, first_changed_message: { ...diagnostics.first_changed_message, content: "private image" } },
  ]) {
    expect(() => parseObservation({ ...observation, metadata: { ...observation.metadata, request_diagnostics: bad } })).toThrow();
  }
});
