# BookStructure source discovery

## Automatic Build Executor Envelope

When the caller supplies an automatic_build_executor.v1 envelope, execute input_command and use its stdout as the input below. Write the strict candidate JSON at candidate_path, execute submit_command, and return only its receipt. If a native usage receipt is available, save it at usage_path; never invent exact token counts. Use heartbeat_command while active; on failure execute fail_command and return the failure receipt. Without this envelope, follow the strict JSON contract below.

Discover grounded candidates from this natural section's complete core range.
The provisional outline is orientation only; revise its assumptions when the body disagrees.
Read every core paragraph, preserving mechanisms, conditions, counterexamples and teaching value.
Emit book_structure_fragment_observation.v1 with parent_unit_lid, summary_fragments,
candidate_key_stops, role_hints, dependency_hints (empty), and evidence_lids.
AnchoredText = {"text":string,"evidence_lids":string[]}; keep each text under 600 characters.
Candidate = {"id":string,"lid":string,"type":"definition"|"formula"|"claim"|"example"|"turning_point"|"warning"|"summary",
"meaning":string,"conditions":string[],"reason":AnchoredText,"title"?:string,"aliases"?:string[]}.
Use distinct local IDs for different meanings even at the same LID. State the mechanism in meaning,
its applicable conditions explicitly (empty if unconditional), and why it is worth teaching in reason.
Do not impose a fixed number of points per section. Empty candidates are valid for orientation-only material.
Produce a concise grounded overview in summary_fragments; summaries do not replace any candidates.
Only reference_scope.evidence_by_unit[parent_unit_lid] grants evidence. Titles, outline and graph IDs
are navigation, never proof. Preserve the provenance and conditions of supplied discourse/formula results.
Do not cite a heading or manufacture evidence. Heading-only core packets return empty summaries and candidates.
role_hints uses setup|foundation|method|application|case|synthesis. Output strict JSON only.
