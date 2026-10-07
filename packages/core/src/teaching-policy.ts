export const TEACHING_STAGES = ["formal_objects", "cognitive_materials", "teaching_publish"] as const;
export type TeachingBuildStage = typeof TEACHING_STAGES[number];
export const isTeachingStage = (stage: string): stage is TeachingBuildStage => TEACHING_STAGES.some(s => s === stage);
export const TEACHING_EXTRACTORS = {
  formal_objects: "formal-objects-extractor", cognitive_materials: "cognitive-materials-extractor", teaching_publish: "teaching-source-reviewer",
} as const;
export const TEACHING_GENERATIONS = {
  formal_objects: "formal_objects.full.v3", cognitive_materials: "cognitive_materials.full.v1", teaching_publish: "teaching_publish.full.v1",
} as const;
// These identities are required by the existing executor prompt/proof contract.
const PROMPT_SHA256 = {
  formal_objects: "fbd4112005dc4691ce20523947636ca858a93ed8a15e7470f30e6a63674edd41",
  cognitive_materials: "074e4ed0345f6551aaf5ed4ba47c00a50e08d5b525c812d3d0f1ec21f18a7c94",
  teaching_publish: "f5c9567876837038e30eaef9bbc435411b1afdd785a8e6175b077894cd5d5499",
} as const;
export const TEACHING_POLICIES = Object.fromEntries(TEACHING_STAGES.map(stage => [stage, {
  stage_policy_version: `${stage}.v${stage === "formal_objects" ? 3 : 1}`,
  prompt_sha256: PROMPT_SHA256[stage],
  schema_version: `${stage}.v${stage === "formal_objects" ? 2 : 1}`,
}])) as Record<TeachingBuildStage, { stage_policy_version: string; prompt_sha256: string; schema_version: string }>;
