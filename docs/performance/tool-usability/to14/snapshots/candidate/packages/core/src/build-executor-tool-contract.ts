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
  examples: Record<string, unknown>[];
}

const DEDICATED_EXECUTOR_ONLY =
  "Only the dedicated `understand_book_executor` Executor may call this tool.";
const EXAMPLE_REFS_NOTICE =
  "Examples use test-session refs; replace refs and ordinals with the exact values from your current handoff or tool results.";
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

const REQUEST_VERSION_BY_TOOL = Object.freeze({
  "executor.open": "automatic_build_executor_open_request.v3",
  "executor.input.next": "automatic_build_executor_input_next_request.v4",
  "executor.generation.start": "automatic_build_executor_generation_start_request.v3",
  "executor.submit_candidate": "automatic_build_executor_candidate_submit.v3",
} satisfies Record<BuildExecutorToolNameV1, string>);

// Issued by the real TO6 session fixtures, recorded in
// docs/performance/tool-usability/to6/before/{normal,dsh}.json. These are examples, not live grants.
const EXAMPLE_DELIVERY_REFS = {
  opaque_session_ref: "absession1_0c33c1a7068b7305252abff0b21e741d8112ac70c1b1d01e679f63e35b50bb24",
  generation_input_ref: "abinput1_1d48529092b7116aa9592bc13ee866267e442acd662e7873576cb6c84e975859",
};

function closedSchema(
  version: string,
  required: string[],
  properties: Record<string, unknown>,
  examples: Record<string, unknown>[],
): ClosedObjectSchemaV1 {
  return {
    type: "object",
    additionalProperties: false,
    required: ["version", ...required],
    properties: {
      version: { const: version },
      ...properties,
    },
    examples: examples.map(example => ({ version, ...example })),
  };
}

export const BUILD_EXECUTOR_TOOL_CONTRACTS_V3 = Object.freeze([
    {
      name: "executor.open" as const,
      description: `${DEDICATED_EXECUTOR_ONLY} Open or resume the assigned handoff. DELIVER_INPUT provides an input_manifest and a ready-to-call next_request; WAIT provides retry_after_ms; DONE is the code-owned terminal status. Reopen the same handoff on a replacement connection to resume unconfirmed input or rehydrate an active generation. A stale_generation_session/lease_expired requires Driver recovery and a new handoff on a fresh connection. ${EXAMPLE_REFS_NOTICE}`,
      input_schema: closedSchema(
        REQUEST_VERSION_BY_TOOL["executor.open"],
        ["opaque_handoff_ref"],
        { opaque_handoff_ref: { ...HANDOFF_REF_SCHEMA, description: "Copy the complete assigned opaque handoff ref; its record version does not change this request version." } },
        [{ opaque_handoff_ref: "abhandoff1_99716fc062fb38562304bd82a0734f13ad27fc92af6e4a35250d6f448c3047ad" }],
      ),
    },
    {
      name: "executor.input.next" as const,
      description: `${DEDICATED_EXECUTOR_ONLY} Read the next ordered INPUT_BATCH. Start with DELIVER_INPUT.next_request, consume every chunk in ordinal order, then acknowledge a non-final batch by its last_ordinal to request the next batch. Repeating the current request replays its batch. Delivery and acknowledgement update the session ledger. When final_for_generation=true, confirm the final last_ordinal with executor.generation.start instead of input.next. Examples show first read and non-final acknowledgement (the DSH fixture delivers one chunk per batch). ${EXAMPLE_REFS_NOTICE}`,
      input_schema: closedSchema(
        REQUEST_VERSION_BY_TOOL["executor.input.next"],
        ["opaque_session_ref", "generation_input_ref"],
        {
          opaque_session_ref: { ...SESSION_REF_SCHEMA, description: "Delivery session ref from the current DELIVER_INPUT.next_request." },
          generation_input_ref: { ...INPUT_REF_SCHEMA, description: "Input ref from that same delivery; keep it paired with its session." },
          ack_through_ordinal: { ...ORDINAL_SCHEMA, description: "Last ordinal actually consumed in the preceding non-final batch. Omit for first read; on resume copy next_request exactly. Never acknowledge the final batch here." },
        },
        [EXAMPLE_DELIVERY_REFS, {
          opaque_session_ref: "absession1_a2d8ee3844bfcaaba7cf95347f1eab04ace1e3bcfda8166cf5143e228880c6a0",
          generation_input_ref: "abinput1_4cd190bea7c5f2bfefa87d0059dd122c479549f6424fb1c5ecd71278cf605f4d",
          ack_through_ordinal: 0,
        }],
      ),
    },
    {
      name: "executor.generation.start" as const,
      description: `${DEDICATED_EXECUTOR_ONLY} After consuming all input, confirm the final batch and create or replay its semantic attempt. Only GENERATE grants candidate generation: use its output_contract, opaque_session_ref and candidate_sink_ref. The generation session can differ from the delivery session. Lost responses replay the same active attempt; stale_generation_session requires Driver recovery, not reuse of an old sink. Follow WAIT/DONE without generating. ${EXAMPLE_REFS_NOTICE}`,
      input_schema: closedSchema(
        REQUEST_VERSION_BY_TOOL["executor.generation.start"],
        ["opaque_session_ref", "generation_input_ref", "confirmed_through_ordinal"],
        {
          opaque_session_ref: { ...SESSION_REF_SCHEMA, description: "Delivery session ref whose complete input you consumed." },
          generation_input_ref: { ...INPUT_REF_SCHEMA, description: "Input ref paired with that delivery session." },
          confirmed_through_ordinal: { ...ORDINAL_SCHEMA, description: "last_ordinal of the consumed batch with final_for_generation=true; this confirms the final batch and requests the generation grant." },
        },
        [{ ...EXAMPLE_DELIVERY_REFS, confirmed_through_ordinal: 1 }],
      ),
    },
    {
      name: "executor.submit_candidate" as const,
      description: `${DEDICATED_EXECUTOR_ONLY} Submit one JSON value matching the current GENERATE.output_contract, within its candidate and serialized-request budgets. Copy both refs from that GENERATE, including after recovery. Code writes the private sink, validates and returns the next action or DONE status without echoing the candidate. Candidate submission alone is not proof of acceptance. candidate_sink_unavailable permits replay of the identical candidate on the same active attempt; lease_expired requires Driver recovery. The empty graph example is a valid synthetic Pass1 candidate, not a universal output shape. ${EXAMPLE_REFS_NOTICE}`,
      input_schema: closedSchema(
        REQUEST_VERSION_BY_TOOL["executor.submit_candidate"],
        ["opaque_session_ref", "candidate_sink_ref", "candidate"],
        {
          opaque_session_ref: { ...SESSION_REF_SCHEMA, description: "Generation session ref from the current GENERATE, not DELIVER_INPUT." },
          candidate_sink_ref: { ...SINK_REF_SCHEMA, description: "Private sink ref paired with that current generation session; a replacement generation supplies a new pair." },
          candidate: { ...JSON_VALUE_SCHEMA, description: "Structured JSON value satisfying the delivered semantic prompt and output_contract. Pass the value directly, not a filename or a JSON-encoded string." },
        },
        [{
          opaque_session_ref: "absession1_2b068c75e9cc8ab7548261169a25831a66583a5a4648e831dc694abf07b4058e",
          candidate_sink_ref: "absink1_84f02eecd2bcb471670eb801f48b0f7db02a6047102d0f044d228c6f331842dd",
          candidate: { nodes: [], edges: [] },
        }],
      ),
    },
  ] satisfies BuildExecutorToolContractV1[]);

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
