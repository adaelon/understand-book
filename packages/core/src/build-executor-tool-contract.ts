export const BUILD_EXECUTOR_SERVER_NAME_V1 = "understand_book_build_executor" as const;

export const BUILD_EXECUTOR_TOOL_NAMES_V1 = [
  "executor.open",
  "executor.input.next",
  "executor.generation.start",
  "executor.submit_candidate",
] as const;

export type BuildExecutorToolNameV1 = (typeof BUILD_EXECUTOR_TOOL_NAMES_V1)[number];
export class BuildExecutorInvalidArgumentsError extends Error {
  readonly diagnostic_code = "invalid_arguments" as const;
  constructor(readonly field: "arguments" | "version" | "opaque_handoff_ref") {
    super(`Build Executor invalid arguments field: ${field}`);
    this.name = "BuildExecutorInvalidArgumentsError";
  }
}

export interface ClosedObjectSchemaV1 {
  type: "object";
  additionalProperties: false;
  required: string[];
  properties: Record<string, unknown>;
}

const DEDICATED_EXECUTOR_ONLY =
  "Only the dedicated `understand_book_executor` Executor may call this tool.";
export interface BuildExecutorToolContractV1 {
  name: BuildExecutorToolNameV1;
  description: string;
  input_schema: ClosedObjectSchemaV1;
}

const HANDOFF_REF_SCHEMA = Object.freeze({
  type: "string",
  pattern: "^abhandoff1_[a-f0-9]{64}$",
  maxLength: 75,
});
const SESSION_REF_SCHEMA = Object.freeze({
  type: "string",
  pattern: "^absession1_[a-f0-9]{64}$",
  maxLength: 75,
});
const INPUT_REF_SCHEMA = Object.freeze({
  type: "string",
  pattern: "^abinput1_[a-f0-9]{64}$",
  maxLength: 73,
});
const ORDINAL_SCHEMA = Object.freeze({
  type: "integer",
  minimum: 0,
});
const SINK_REF_SCHEMA = Object.freeze({
  type: "string",
  pattern: "^absink1_[a-f0-9]{64}$",
  maxLength: 72,
});
const JSON_VALUE_SCHEMA = Object.freeze({
  oneOf: [
    { type: "null" },
    { type: "boolean" },
    { type: "number" },
    { type: "string" },
    { type: "array", items: {} },
    { type: "object", additionalProperties: true },
  ],
});

function closedSchema(
  version: string,
  required: string[],
  properties: Record<string, unknown>,
): ClosedObjectSchemaV1 {
  return {
    type: "object",
    additionalProperties: false,
    required: ["version", ...required],
    properties: {
      version: { const: version },
      ...properties,
    },
  };
}

export const BUILD_EXECUTOR_TOOL_CONTRACTS_V3 = Object.freeze([
    {
      name: "executor.open" as const,
      description: `${DEDICATED_EXECUTOR_ONLY} Open or resume one bounded executor delivery session.`,
      input_schema: closedSchema(
        "automatic_build_executor_open_request.v3",
        ["opaque_handoff_ref"],
        { opaque_handoff_ref: HANDOFF_REF_SCHEMA },
      ),
    },
    {
      name: "executor.input.next" as const,
      description: `${DEDICATED_EXECUTOR_ONLY} Read the next ordered semantic input chunk.`,
      input_schema: closedSchema(
        "automatic_build_executor_input_next_request.v4",
        ["opaque_session_ref", "generation_input_ref"],
        {
          opaque_session_ref: SESSION_REF_SCHEMA,
          generation_input_ref: INPUT_REF_SCHEMA,
          ack_through_ordinal: ORDINAL_SCHEMA,
        },
      ),
    },
    {
      name: "executor.generation.start" as const,
      description: `${DEDICATED_EXECUTOR_ONLY} Accept one generation grant and create or replay its semantic attempt.`,
      input_schema: closedSchema(
        "automatic_build_executor_generation_start_request.v3",
        ["opaque_session_ref", "generation_input_ref", "confirmed_through_ordinal"],
        {
          opaque_session_ref: SESSION_REF_SCHEMA,
          generation_input_ref: INPUT_REF_SCHEMA,
          confirmed_through_ordinal: ORDINAL_SCHEMA,
        },
      ),
    },
    {
      name: "executor.submit_candidate" as const,
      description: `${DEDICATED_EXECUTOR_ONLY} Submit one structured JSON candidate to its code-owned private sink.`,
      input_schema: closedSchema(
        "automatic_build_executor_candidate_submit.v3",
        ["opaque_session_ref", "candidate_sink_ref", "candidate"],
        {
          opaque_session_ref: SESSION_REF_SCHEMA,
          candidate_sink_ref: SINK_REF_SCHEMA,
          candidate: JSON_VALUE_SCHEMA,
        },
      ),
    },
  ] satisfies BuildExecutorToolContractV1[]);

const REQUEST_VERSION_BY_TOOL = Object.freeze({
  "executor.open": "automatic_build_executor_open_request.v3",
  "executor.input.next": "automatic_build_executor_input_next_request.v4",
  "executor.generation.start": "automatic_build_executor_generation_start_request.v3",
  "executor.submit_candidate": "automatic_build_executor_candidate_submit.v3",
} satisfies Record<BuildExecutorToolNameV1, string>);

const REQUEST_KEYS_BY_TOOL = Object.freeze({
  "executor.open": {
    required: ["version", "opaque_handoff_ref"],
    optional: [],
  },
  "executor.input.next": {
    required: ["version", "opaque_session_ref", "generation_input_ref"],
    optional: ["ack_through_ordinal"],
  },
  "executor.generation.start": {
    required: [
      "version",
      "opaque_session_ref",
      "generation_input_ref",
      "confirmed_through_ordinal",
    ],
    optional: [],
  },
  "executor.submit_candidate": {
    required: ["version", "opaque_session_ref", "candidate_sink_ref", "candidate"],
    optional: [],
  },
} satisfies Record<BuildExecutorToolNameV1, { required: string[]; optional: string[] }>);

export function validateClosedToolRequest(toolName: BuildExecutorToolNameV1, request: unknown): void {
  if (!request || typeof request !== "object" || Array.isArray(request)) {
    throw new BuildExecutorInvalidArgumentsError("arguments");
  }
  const record = request as Record<string, unknown>;
  const shape = REQUEST_KEYS_BY_TOOL[toolName];
  const allowed = new Set([...shape.required, ...shape.optional]);
  if (shape.required.some((key) => !(key in record))
    || Object.keys(record).some((key) => !allowed.has(key))) {
    throw new BuildExecutorInvalidArgumentsError("arguments");
  }
  if (record.version !== REQUEST_VERSION_BY_TOOL[toolName]) {
    throw new Error("Build Executor tool request version does not match the selected tool");
  }
  if (toolName === "executor.open"
    && (typeof record.opaque_handoff_ref !== "string"
      || !/^abhandoff1_[a-f0-9]{64}$/u.test(record.opaque_handoff_ref))) {
    throw new BuildExecutorInvalidArgumentsError("opaque_handoff_ref");
  }
}
