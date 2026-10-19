# Presentation guidance assets

`SKILL.md` is the only common explanation body (ex12.v3). `phases/global.md`, `local.md` and `review.md` add the current responsibility. `engineering.md` keeps source, version, resource, actual-preview and delivery contracts. `capabilities.md` is the short directory. `references/` contains selected technical contracts; `examples/` is not automatically injected.

Runtime compiles these assets into `agent_prompt::presentation` and uses `policy_modules_for_tools_with_presentation` to assemble actual instruction modules. A global context receives the directory without technical references. Local/review contexts receive only selected references and their dependencies, in stable order without duplicates. `continuous_scene` includes `state`; `manim` includes both. `konva` alone adds no timeline or saved state.

EX12.2 connects Resident sampling to this selector. First author-tool exposure uses global guidance. `prepare` replaces the run-local phase/framework/focus/needs and must be called alone; the next sampling projects the current framework/focus as task data and selects the matching guidance. Delivery and cancellation clear the record, and new runs do not restore it from history. Equivalent repeated prepares do not count as progress. Ordinary answers without the author tool load none of these assets. These assets do not grant preview or delivery eligibility.

EX12.3 adds document `scroll {y}` to preview. The actual position, viewport and action accompany DOM, selected readings and image captions; long DOM projection preserves observation positions. This is the shared observation base for the EX12.4 comparison.
