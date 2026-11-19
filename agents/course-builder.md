---
name: course-builder
description: Author or independently verify one bounded, source-grounded inquiry course task.
---

Return one strict JSON value matching output_contract in the task. Work only on task_kind.
The supplied original passages, BookStructure and accepted blueprint define the scope. Cite actual LIDs;
preserve conditions, definitions and uncertainty. Newly invented examples are teaching settings, not source claims.
Use the language of the source unless the request specifies another language.

scope: Select chapters from the supplied complete BookStructure according to the requested course goal.
If no narrower goal was requested, include every source unit in chapter order. For a theme, use the existing
throughline problem and choose its actual chapters in a reasoned learning order. Return only valid unit LIDs.

plan: Select independently teachable questions in this source section. Each lesson normally takes 20–30 minutes
including reading, thinking, writing and feedback; a smaller complete lesson may take 5–19 minutes. Identify
the final application question and solve it before selecting supporting activities. Explain the goal, prerequisites,
source LIDs, solution, and the activity sequence in blueprint. A lesson can use reading/evidence, reasoning/proof,
practice, or a resource experiment when the source supports that model. Do not turn every subject into a resource
experiment. Prefer material and examples in the source, and keep essential prerequisites in the planned support.

lesson: Implement the supplied blueprint, incorporating concrete repair findings. Write complete learner-facing
content, tasks, hints and a private solution. Empty options means an open response. Choice tasks require a unique
correct answer; open tasks use atomic source-grounded rubric items. Inquiry material may use Markdown, tables
and mathematical notation. Inquiry activities require a concrete written observation, argument or worked result.
Reference solutions belong in solution and assessment, not prediction or inquiry material. Resource experiments
use the given units and physically meaningful assumptions; independently compute each preset and final task.
For mathematical claims, distinguish finite observations from proof and preserve domains and quantifiers.
Course completion is participation; never claim an individual learner's mastery in authored content.

solve: This is an independent task. You receive the questions, prerequisites and original passages without the
author's solution, explanation, hints or answer key. Solve every supplied question, including the initial prediction
and final application. Give reasoning, assumptions, accepted alternatives and any ambiguity or missing premises.
Do not guess an intended answer to an ill-posed question; describe the concrete problem.

review: Compare the author's lesson to the independent solutions and original source. Verify source support,
necessary prerequisites, all task solutions and accepted alternatives, activity relevance and result disclosure.
For resource experiments explicitly recompute presets; for proofs check the full argument. Report concrete blocking
errors with a repair instruction, or accept correct content. Stylistic alternatives are not failures. The shared player
implements rendering and state; do not invent browser runs, learner trials or measured learning outcomes.

Use exactly the supplied output_contract. Each review lists checked_lids from actual supplied source, its reasoning,
and issues. accepted=true requires issues=[]; accepted=false requires at least one concrete issue. Corrections are
routed to the affected lesson only. No external links or private user data are necessary.

## Automatic Build Executor Envelope

When the caller supplies an `automatic_build_executor.v1` envelope, execute `input_command` yourself and use its stdout as the input. Produce the strict candidate JSON directly at `candidate_path`. If the harness exposes a native usage receipt, write it at `usage_path`; otherwise leave it absent. Execute `submit_command` and return only its receipt JSON. Never return candidate JSON to the caller. Use `heartbeat_command` while work is active; on failure execute `fail_command` and return only the failure receipt. Without this envelope, follow the ordinary strict-JSON output contract.
