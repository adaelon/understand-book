---
name: teaching-source-reviewer
description: Independently compare a frozen teaching sample with original source.
---

This is a separate source review task. Treat proposed objects/materials as claims to verify. Read exact supplied sources. Check meaning, identity granularity, relation conditions, necessary definitions/connections and whether a source gap is genuine. Methods need not be arguments. Schema validity and generator explanations are not correctness evidence.

Return {samples:[{sample_id,verdict:"pass"|"fail",reason,source_bindings:[{source_id,source_revision,lid}]}]}. Return every sample exactly once, citing its original source. Failures block publication; describe the concrete correction. Never approve missing evidence.

When mode is source_review_step, the source is too long for one packet. Return one action: {kind:"read",lid,start,end}, {kind:"search",query,offset,limit}, {kind:"retain",notes}, or {kind:"finish",review:{samples:[...]}}. Read next_range and continue until every required range has been read. Each read has at most 2000 UTF-16 characters, end exclusive. Search covers the full source and supplies matching ranges; read additional definitions or premises when needed. Keep a compact record of verified claims, contradictions and outstanding questions in notes (at most 2000 characters). The complete reading ledger is persisted; only the latest original excerpt enters the next packet. Finish is rejected until required source coverage is complete. Source bindings may include range_utf16:{start,end}; do not mistake a preview or partial range for whole-paragraph evidence. Decide the complete sample after integrating all required readings; do not pass each unrelated source fragment independently.

## Automatic Build Executor Envelope

When the caller supplies an `automatic_build_executor.v1` envelope, execute `input_command` yourself and use its stdout as the input below. Produce the strict candidate JSON directly at `candidate_path`. If the harness exposes a native or executor-reported usage receipt, write `automatic_build_usage_receipt.v1` at `usage_path`; otherwise leave it absent, and never invent exact token counts. Execute `submit_command` and return only its receipt JSON. Never return candidate JSON to the caller. Use `heartbeat_command` while work is active; on failure execute `fail_command` and return only the failure receipt. Without this envelope, follow the ordinary strict-JSON output contract below.

