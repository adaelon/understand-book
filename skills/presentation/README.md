# Presentation guidance assets

`SKILL.md` is the only common explanation body (ex13.v1). `phases/global.md`, `local.md` and `review.md` add the current responsibility. `engineering.md` keeps source, version, resource, actual-preview and delivery contracts. `capabilities.md` is the short directory. `references/` contains selected technical contracts; `examples/` is not automatically injected.

EX13.3 separates user requirements, revisable Goal work items and explanation design. Version ex13.v1 keeps progress in `working.items`, design in `framework`, and checks the actual work against requirements and source material before delivery. Small revisions may use a short plan; completed items never establish delivery eligibility or content completeness.

Runtime compiles these assets into `agent_prompt::presentation` and uses `policy_modules_for_tools_with_presentation` to assemble actual instruction modules. A global context receives the directory without technical references. Local/review contexts receive only selected references and their dependencies, in stable order without duplicates. `continuous_scene` includes `state`; `manim` includes both. `konva` alone adds no timeline or saved state.

EX12.2 connects Resident sampling to this selector. First author-tool exposure uses global guidance. `prepare` replaces the run-local phase/framework/focus/needs and must be called alone; the next sampling projects the current framework/focus as task data and selects the matching guidance. Delivery and cancellation clear the record, and new runs do not restore it from history. Equivalent repeated prepares do not count as progress. Ordinary answers without the author tool load none of these assets. These assets do not grant preview or delivery eligibility.

EX12.3 adds document `scroll {y}` to preview. The actual position, viewport and action accompany DOM, selected readings and image captions; long DOM projection preserves observation positions. This is the shared observation base for the EX12.4 comparison.

EX12.4 real runs exposed global contexts writing without a local handoff. Version ex12.v5 states the global → local and complete work → review calls explicitly, and ties optional interactions and explanation scope to the current question. These remain Agent responsibilities; runtime does not reject ordinary writes for missing stages.

The v4 reruns entered local but still wrote whole pages first. Version ex12.v5 distinguishes a runnable local prototype from the complete work, and makes the explicit source_ref_ids attachment visible in engineering guidance. The shared tool schema/error also explains how to repair an omitted source list. Original failures and targeted reruns are retained in docs/performance/presentation-staged-authoring-ex12-4.md.
