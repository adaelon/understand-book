import { describe, expect, it } from "vitest";
import { automaticBuildCandidateCorrection, createAutomaticBuildFailureDiagnosticV3, isAutomaticBuildCorrectableCandidateFailure } from "../src/extractor-contract";
import { automaticBuildRetryBoundaryRequiredRecovery } from "../src/automatic-build-attempt-recovery";

describe("Tutor candidate recovery", () => {
  it.each([
    { category: "schema", code: "semantic_output_invalid", phase: "artifact_writer", expected: "composite object requires components" },
    { category: "schema", code: "semantic_output_invalid", phase: "artifact_writer", expected: "learnable relation requires participants and roles" },
    { category: "transport", code: "candidate_request_too_large", phase: "generation" },
  ] as const)("recovers the persisted $code / $expected failure without changing its receipt", input => {
    const diagnostic = createAutomaticBuildFailureDiagnosticV3(input), original = JSON.stringify(diagnostic);
    expect(isAutomaticBuildCorrectableCandidateFailure(diagnostic)).toBe(true);
    expect(automaticBuildRetryBoundaryRequiredRecovery(diagnostic)).toBe("authorize_candidate_retry");
    expect(JSON.stringify(diagnostic)).toBe(original);
  });
  it("keeps an unrelated semantic failure behind the policy recovery boundary", () => {
    const diagnostic = createAutomaticBuildFailureDiagnosticV3({ category: "schema", code: "semantic_output_invalid", phase: "artifact_writer", expected: "unknown semantic failure" });
    expect(isAutomaticBuildCorrectableCandidateFailure(diagnostic)).toBe(false);
    expect(automaticBuildRetryBoundaryRequiredRecovery(diagnostic)).toBe("publish_new_policy");
  });
  it("recovers the persisted alignment read boundary with the actual paragraph length", () => {
    const diagnostic = createAutomaticBuildFailureDiagnosticV3({ category: "schema", code: "semantic_output_invalid", phase: "artifact_writer",
      expected: "alignment source range invalid or too large: lid=3.2; source_length_utf16=138; require 0 <= start < end <= source_length" });
    const original = JSON.stringify(diagnostic);
    expect(automaticBuildCandidateCorrection(diagnostic)).toMatchObject({ json_pointer: "/end", expected: expect.stringContaining("source_length_utf16=138") });
    expect(automaticBuildRetryBoundaryRequiredRecovery(diagnostic)).toBe("authorize_candidate_retry");
    expect(JSON.stringify(diagnostic)).toBe(original);
  });
});
