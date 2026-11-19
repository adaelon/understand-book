---
name: understand-book-build
description: Operator maintenance tool to build or resume a local book workspace with explicitly provisioned build executables.
---

# $understand-book-build

Choose the request's planning route. An open-ended course commission starts with one playable trial.
For automatic builds, obtain an explicitly confirmed `BuildPlan`, and then let the
deterministic Build Engine drive execution until it returns `DONE` or a real user boundary. The
root Codex owns semantic planning, user conversation, live agent-slot observation, and harness
lifecycle only. It does not interpret the internal build state machine.

After plan confirmation, the root owns progress to completion within the confirmed scope. Repair
its own malformed control requests and continue the same invocation; do not turn a correctable
caller error into a user decision or a separate maintenance task.

## Authority and privacy boundary

The following ownership applies when running the deterministic Build Engine protocol.

- Reader/Core owns the current source, content profile, Blueprint validation, plan identity, and
  reader-private store.
- `build.step` owns protocol compatibility, preflight, budgets, dispatch state, leases, retries,
  stage close/publication, private artifact attempts, and completion.
- A dedicated executor owns one opaque semantic session. It sees semantic input and writes a
  candidate through the code-owned session sink.
- The root never reads, receives, summarizes, caches, or forwards semantic input or candidate JSON.
- The root must not call, probe, enumerate, or use `executor.open` to diagnose a handoff.
- The root must not call, probe, enumerate, or use `executor.input.next` to diagnose a handoff.
- The root must not call, probe, enumerate, or use `executor.generation.start` to diagnose a handoff.
- The root must not call, probe, enumerate, or use `executor.submit_candidate` to diagnose a handoff.
- Conversation memory is not build state. Re-enter through the same confirmed plan and invocation;
  code rereads durable state on every step.
  During stable profile-sidecar work, the Engine reuses completed Pass1 and prepared task routes,
  reads changed task results, and keeps claims, retries and budgets live. The first indexed step
  and source/policy/publication changes can require a full route; continue the same invocation.

## Choose the planning route first

Ordinary prebuild is the default. A request to build, prebuild, finish, or resume a book/paper
workspace uses the explicit `standard_deep` route unless the user also states a concrete reading
goal and asks the build to produce goal-specific results. A title, subject description, workspace
path, or request to “build this paper” is not by itself a natural-language reading goal.

- **Ordinary prebuild or resume:** use **Explicit standard deep build** below. When an existing
  confirmed `build_plan_path` or invocation can be recovered, resume it instead of creating a new
  plan. Do not call `planning.context`, launch `UnderstandBook.exe --codex-build-intent`, or open or
  inspect a Reader-private build plan for this route.
- **Explicit reading goal:** only when the user asks to plan the build around a stated reading goal,
  use **Natural-language reading goal** below. The goal route may call the Desktop controller and
  produce reader-private artifacts.
- **Open-ended course commission:** a title, a book, or a wish such as “make me a course” starts
  with **Teach one trial lesson** below. The authoring Agent chooses a provisional question and
  audience from the book. Invite any context the commissioner wants to offer. Use the trial to
  discover a useful course direction; leave the wider route open until they choose it.
- **Direct course build:** when the commissioner specifies the course to produce and asks to proceed,
  use the standard route with `--course`; pass their explicit topic or chapter scope with
  `--course-goal`. An explicitly requested whole-book course can follow BookStructure in order.
  After a trial use `--course-route` with the accepted route file. Include the course stages and
  their budget in the exact plan projection. The course brief governs shared learning content.

Do this classification before either plan-authoring surface is called. Never reinterpret an
ordinary prebuild request as a reading goal merely because the workspace contains a paper.

## Resolve the tools for the selected route

1. Resolve `understand-book-build.exe` from `UNDERSTAND_BOOK_BUILD_EXE`, which must name an existing
   operator-provisioned executable. Cloud Windows Setup does not install the local build tools.
2. Standard prebuild and course builds need only the Build Engine; do not require a maintenance
   controller for those routes. Only the explicit reading-goal route resolves
   `understand-book-maintenance.exe` from `UNDERSTAND_BOOK_DESKTOP_EXE`. Build this optional public
   CLI with `cargo build -p understand-book-maintenance --release` from `crates/maintenance-host`.
   It uses public Reader/Core services for `--codex-build-intent`. Set
   `UNDERSTAND_BOOK_LOCAL_DATA_DIR` to the local Reader data home when using its library and private
   plans; the CLI follows the existing `data-location.json` selection. Without that setting, the
   existing Windows operator settings under `%LOCALAPPDATA%/UnderstandBook` remain in use.
   Missing tools for the selected route require operator provisioning.
3. Do not install Node, Bun, Cargo, or another runtime as an implicit fallback. Do not fall back to
   old build commands when the packaged entry reports an incompatible installation.

## Establish one confirmed plan

For an explicit retrieval configuration on a standard plan, use the Engine's
`build_retrieval_configure.v1` operation through `build.step`; pass the code-issued
`build_plan_path`, `retrieval_mode`, and documents/queries/calls budget. It returns a new draft
path and `review_markdown`. Show that projection and its exact plan identity; after approval,
call `build_retrieval_confirm.v1` with that path, `plan_digest`, and
`confirmation_source: "codex_conversation"`. Use its returned plan path for the invocation.
Do not infer new embedding execution quotas from earlier samples. `lexical_only` requires no
provider; `semantic_required` currently supports an explicitly installed local MiniLM via the
host's `UNDERSTAND_BOOK_EMBEDDING_CONFIG`. The default remains lexical pending SR6 quality
acceptance. [Setup and data/usage boundaries](https://github.com/adaelon/understand-book/blob/625b4405af3b011e87d4eb256f819c8075f88613/docs/performance/semantic-retrieval-sr5-20261001.md).

Execution requires one current, code-issued `build_plan_path`. Treat that path as an opaque output
of planning: do not parse, edit, duplicate, or replace the plan in chat.

### Explicit standard deep build

New `standard_deep` plans default to Pass2 disabled. Enable long-range relation extraction only
when the user explicitly selects it; do not ask a separate Pass2 question for the default path.
Enabling Pass2 adds cross-section semantic links and model cost; disabling it still builds
BookStructure from Pass1 and profile sidecars. Record `pass2=enabled|disabled` in the plan, using
`disabled` when omitted. Ask for any budget limit that changes authorization.

For a new standard plan, call the installed code-owned surface directly:

```text
<build-exe> legacy-plan <target> --root <root> [--pass2 <enabled|disabled>] [--course] [--course-goal <goal> | --course-route <accepted-route.json>] [budget flags]
```

Consume only `explicit_legacy_build_plan.v1`. Show its exact stage closure,
reuse/create/excluded work, token and wall-clock estimate, budget, `plan_id`, and `plan_digest`.
Require explicit confirmation of that exact projection before accepting its `build_plan_path`.
Never infer Pass2 enablement, budget approval, or plan confirmation from “full build,” old conversations, or
existing artifacts. Keep the returned path opaque and reuse it for interruption recovery; never
create another standard plan merely to resume. If no unique confirmed plan path is available, stop as
`needs_user(plan_confirmation_required)`.

### Teach one trial lesson

Use the operator-provisioned Build Engine's `course-trial` commands with the prepared book's
workspace directory as BOOK. For a new source prepare its book structure through the ordinary
prebuild first. Run `course-trial state BOOK` to pick up prior revisions and route decisions.
This authoring workflow uses public source passages and the commissioner's own lesson drafts.

1. Run `course-trial catalog BOOK` and `course-trial read BOOK UNIT_LID`. Select a question
   representative of the book that a reader can investigate with a few original passages.
   Explain why it is worth trying. Use `course-trial template BOOK UNIT_LID` for a single-lesson
   artifact with its source identity. Fill the lesson, narrow the citation, and replace the example
   fields with a prediction, evidence activity, explanation and an independent task in a new setting.
2. Have an independent reviewer solve the student-facing questions using their prerequisites,
   allowed material and cited passages. Then compare the author's solution, alternative answers,
   conditions and scoring with the independent work. Repair concrete errors. Write a review JSON
   with string fields `selection_reason`, `independent_solution` and `source_review`, recording the
   actual reasoning. Keep the artifact and review inputs in the author's working area.
   Run `course-trial install BOOK REV ARTIFACT_JSON REVIEW_JSON [PREVIOUS_REV]`. Each changed
   lesson gets a new revision, for example `pilot-2`. Give the learner the returned `reader_path`
   on their running local Reader and the question the lesson explores. Wait for their experience.
3. Review the particular activity the commissioner wants to discuss. In their authorized local
   Reader session, GET `/api/book/course/BOOK_ID?trial=REV` and select the corresponding
   `progress[].run_id`. Add `&run_id=RUN_ID` to read the private view for that attempt. Use the
   delivered question, response and support to ask a focused follow-up. If there is no attempt yet,
   offer the trial link. When the commissioner shares their experience, record the course-design
   finding via `course-trial feedback BOOK REV NOTE_ID NOTE_JSON`; fields are `activity`,
   `observation` and `change`. Summarize the teaching issue and concrete change. Keep personal
   responses and help history in the private attempt. For a local difficulty, focus the revised
   trial on the affected activity, its necessary context and a fresh application. Install the
   revision linked to this one so the learner can try the change and revisit either version.
4. When there is enough experience to choose a direction, use `course-trial propose BOOK REV
   PROPOSAL_ID ROUTE_JSON`. Its fields are `title`, `question`, `audience`, `final_task`,
   ordered `unit_lids` from the catalog and one or two `example_lessons` (question strings). Show
   what these choices would mean in the next lessons. For an adjustment, save a new proposal ID;
   for “later”, run `course-trial pause BOOK REV PROPOSAL_ID`. When the commissioner selects the
   specific route, run `course-trial accept BOOK REV PROPOSAL_ID` and pass its returned
   `accepted_route_path` to the standard plan as `--course --course-route PATH`.
   The existing plan review covers the budget and exact execution scope.

### Natural-language reading goal

A natural-language goal must be explicit and planned against an absolute workspace already trusted by Reader.
The Desktop controller response is the only authority for whether foundation is required. Stop as
`needs_user(foundation_required)` only when the maintenance controller's `--codex-build-intent` returns
`error_code=CODEX_BUILD_FOUNDATION_REQUIRED`. Do not inspect `.build/input/manifest.json`, source
manifests, PDFs, optional sidecars, or workspace names to invent a profile or foundation gate.

For every controller call, put only `--codex-build-intent` in argv and send exactly one JSON object
on stdin. Never place the raw goal or candidate in argv, environment variables, shell interpolation,
public files, stdout, stderr, logs, or the displayed plan.

1. Request current planning context:

   ```json
   {"version":"codex_build_intent_command.v3","operation":"planning.context","target":{"workspace_dir":"<absolute>"},"input":{}}
   ```

   Consume one `codex_build_intent_result.v2` whose response is
   `build_planning_context.v2`. Read only its bounded scope catalog, Blueprint Registry summaries,
   candidate contract, `context_id`, and `context_revision`. If v3 is unavailable, stop as
   `needs_user(CODEX_BUILD_INTENT_V3_REQUIRED)`. Never fall back to `codex_build_intent_command.v1`
   or the old digest-based v2 command.

2. Create one strict `build_intent_planner_candidate.v2` from the explicit goal and current
   context. Select advertised Registry identities exactly or provide a complete bounded one-off
   `artifact_blueprint.v1`; never guess a truncated entry. Scope may use only shown LIDs/sections or
   `whole_book=true`. Every free-form string search field such as topic, title, question, or claim
   uses `analyzer=text`; bounded exact categories may use `analyzer=keyword`. Do not add code,
   recursion, remote schemas, raw goal text, plan identities, or Reader-private paths.

3. Send `codex_build_intent_command.v3` with `operation=draft.candidate` and
   `input={user_goal,context_id,context_revision,candidate,budget?}` over protected stdin. On
   `BUILD_PLANNING_CONTEXT_DRIFT`, discard the old context and candidate, call `planning.context`
   again, and replan. Do not silently drop a requested artifact or invoke a second planner.

4. Require `codex_build_intent_response.v1` with a `codex_build_intent_plan.v3` projection. Show the
   goal kind, scope, artifact summaries, public stage closure, reuse/create/excluded work, estimates,
   budget, `plan_id`, and `plan_revision`, then stop as `needs_user(plan_confirmation_required)`.
   After approval, re-read status; if identity drifted, show the new projection and reconfirm.
   Otherwise call `confirm` with `input={plan_id,plan_revision}`; the controller records
   `confirmation_source=codex_conversation`. Retain the returned private `build_plan_path` without
   displaying it.

## Start or recover the deterministic driver

Send exactly one bounded stdin object to `<build-exe> build.step`. For a new confirmed plan, create
the invocation:

```json
{
  "version": "automatic_build_invocation_create.v1",
  "target_input": "<target>",
  "root_dir": "<root>",
  "build_plan_path": "<confirmed opaque path>",
  "quality_profile": "full",
  "max_parallel": 8,
  "created_at": "<ISO-8601>",
  "budget": "<optional versioned budget>",
  "wall_budget": "<optional versioned wall budget>",
  "executor_provenance": "<optional proven provenance>"
}
```

Omit optional fields rather than sending placeholders. `max_parallel` is fixed in `1..8` for the
invocation; use 8 for new builds unless the user requests fewer. Actual launches must still fit
the host's currently available dedicated subagent slots. Consume only
`automatic_build_invocation_ref.v1` and retain its `invocation_ref` in the
task checkpoint. Recreating the exact same request is idempotent; changing target, plan, quality,
budget, or requested parallelism starts a different invocation and may require new confirmation.

For an initial step or a user decision without terminal observations, count currently available
dedicated subagent slots and send:

```json
{
  "version": "automatic_build_step_request.v1",
  "invocation_ref": "<opaque invocation ref>",
  "available_agent_slots": 0
}
```

`available_agent_slots` is live capacity in `0..8`; it is not remembered from a previous turn. A
fresh user choice is added only as
`decision={request_id:<returned id>,choice_id:<selected returned id>}`. Never send a digest,
receipt body, command, path inspection result, or reconstructed stage status.

When an owned public-dispatch child (a launch with `dispatch_slot_ref`) returns the exact zero-call lifecycle
`interrupted/bootstrap/bootstrap_unavailable`, add
`bootstrap_failure={"opaque_handoff_ref":"<that child's owned ref>"}` to the next step request.
Keep pending failures in a ref-keyed queue. Send at most one pending observation per step and
remove only that sent observation after a structured step response other than NEEDS_USER;
resending it is idempotent. NEEDS_USER may precede failure processing, so retain the sent item too.
Successful child finals and NEEDS_USER responses never discard other pending failures. Preserve
the queue across a user boundary and report the remaining observations when execution resumes.
The Driver checks that the ref was issued by this invocation, has never opened, and is still
current before recording a bootstrap failure. This observation cannot mark a task complete.
It advances only the bootstrap recovery epoch: the replacement ref changes, while its dispatch
slot, semantic attempt, lease, and accepted artifacts stay intact. Three consecutive bootstrap
failures produce `NEEDS_USER(reason=executor_bootstrap_failed)`; after startup is repaired,
the returned `retry_bootstrap` choice authorizes one further attempt.

When an owned public-dispatch child terminates with exact lifecycle
`interrupted/session/connection_terminal/open` or `interrupted/session/handoff_ref_mismatch/open`,
queue `executor_open_failure={"opaque_handoff_ref":"<that child's owned ref>",
"diagnostic_code":"<that exact diagnostic_code>","phase":"open"}` instead of bootstrap_failure.
These two optional fields are mutually exclusive; send one queued observation per step. The
Driver checks invocation ownership, current recovery generation, and absence of a durable open
before advancing the same bounded bootstrap epoch. Already-opened, committed, or superseded work
takes precedence. Preserve the exact diagnostic; never relabel it bootstrap_unavailable.
Historical protocol_incompatible alone is not evidence of a connection error. Diagnose the actual
owned child's open call before choosing installation recovery, using the bounded reader below.

## Diagnose an owned child's open failure

On an open error, especially protocol_incompatible or an error inconsistent with successful sibling
children, read the failed child's actual control call before retrying. Successful children do not
need history reads. Use the resolved Build Engine:
`build.diagnose-child <this-root-task-id> <owned-child-id-or-name> <original-issued-handoff-ref>`.
Pass the UUID, canonical agent path, or short name returned by the collaboration tool. The reader
resolves a name using this parent's recorded child-start events and returns the resolved `child_id`.
Keep that identity with the launch; never guess a UUID, scan recent children, or diagnose siblings
to locate a failed child. If a reused name is ambiguous, use its recorded UUID.
This reader checks parent/child ownership and streams the local Codex rollout, returning only the
last failed open request version/ref, the original structured tool diagnostic, connection state,
and the last 16 operation names. It excludes conversation, hidden reasoning, semantic input,
candidate JSON and private paths. Never use an unfiltered read_thread or dump the rollout in Root.
If local records or completed MCP facts are absent, report `evidence_missing` and its `missing`
field; do not infer a correction or recommend re-registration without evidence.

Compare `issued_handoff_ref` from the owned launch with `attempted_handoff_ref` character by
character. Keep the original `reported_diagnostic`; the separate `open_call_correction` observation
does not rewrite it. Unknown request versions remain an actual protocol boundary.

- If the child is still live and the reader says `connection_state=unbound`, send that same child
  a bounded correction: “The previous executor.open opaque_handoff_ref differs from the issued
  ref. Retry only open with this original issued ref: <issued_handoff_ref>. The semantic task is
  unchanged. Original diagnostic: <reported_diagnostic>.” Keep its live slot; do not spawn a
  second child. Do not repeat identical correction feedback; after at most three failed corrections
  end that child and use the bounded Driver recovery path.
- If the child has terminated or its connection cannot continue, queue the returned versioned
  `open_call_correction` as the sole failure observation in the next `build.step` request. It is
  mutually exclusive with bootstrap_failure and executor_open_failure. Driver verifies the issued
  ref belongs to this invocation, protects opened/leased/committed/superseded work, records the real
  diagnostic and advances only the existing bounded control generation. Preserve the failed ref
  in completed_refs; release only that child's owned live_by_slot entry. A fresh child receives
  only the Driver-issued replacement ref plus the bounded correction feedback below.
- If no correction is returned, preserve the real diagnostic and follow its existing recovery
  boundary. Never call executor.open from Root to test either ref.

For a corrected replacement add this control-only feedback to the normal spawn payload:
“Previous open failed because opaque_handoff_ref differed from the issued value. Use the attached
Driver-issued ref exactly; it supersedes the failed launch. Original diagnostic:
<reported_diagnostic>. Only the open call is being corrected; semantic work is unchanged.”
Keep feedback keyed by dispatch_slot_ref until its replacement launches, including across NEEDS_USER.
Do not send the obsolete attempted ref as a second actionable handoff. Only the next build.step
can establish durable completion; sibling slots and accepted artifacts keep their ownership.

## Repair a rejected step request

`automatic_build_request_error.v1` with `code=invalid_step_request` means the request was
rejected before build state was read or changed. Compare the last submitted control object with
`automatic_build_step_request.v1`, correct the field name or value type, and immediately resubmit
using the same `invocation_ref`. Preserve the pending observation, live slot ownership, plan,
budget, and parallelism. In particular, keep the versioned `open_call_correction` returned by
`build.diagnose-child` as a nested object under that exact field; serialize the complete step
request once for stdin. Do not stringify that object separately or put it under
`executor_open_failure`.

This response is a caller error, not a Build Engine action or a request for user input. Do not
resubmit an unchanged rejected request. If the corrected request still fails and there is no
supported correction from the request contract, report the concrete blocker without inventing a
decision. For an older engine that returns `NEEDS_USER(build_engine_failed)`, first inspect only
your own last control request for an evident field or type mistake and repair that mistake with the
same invocation. Otherwise honor the engine boundary.

## Build progress

When `build.step` returns `build_progress`, show its full ordered stage route before launching
the first executors. Report the current stage's committed/eligible work, remaining work and
planned dispatch groups. Update this summary on stage changes, meaningful count changes and
user boundaries; combine it with normal progress updates so reporting does not delay refill.

`work.scope=discovered` counts only work already materialized by Engine. `pending` includes
running work that has not committed. Missing `work` means the count is not yet available;
`awaiting_dependencies` is not zero tasks. Later reducers can increase the discovered total.
Planned dispatch groups are not native subagent launch counts. Report live executors separately
from the owned `live_by_slot`; do not infer running task counts or retry counts from these totals.

`current_stage_forecast.service_time` covers current-stage service only, excluding Root's
dispatch gaps, retries, later stages and publication. If confidence is `low`, report timing as
uncalibrated. Even a `matched` range is not a whole-book ETA. Public stages all being complete
does not finish declared private artifacts: only Engine `DONE` proves the entire plan complete.
Use only these bounded projections; never open plans, task files or semantic inputs to fill gaps.

## Four-action loop

When `build_progress.current_stage` is `profile_sidecar` or `book_structure`, first launch the returned
executors, then, while they generate, send one preparation request on stdin to
`<build-exe> build.prepare`:

```json
{"version":"automatic_build_prepare_request.v1","invocation_ref":"<saved invocation ref>","stage":"<current stage>"}
```

Use the saved invocation ref exactly. Run this once on entering this stage in the
current root chat, including after restoring a chat. If BookStructure returns
`skipped` with `reason=outline_pending`, its chapter inputs still depend on the running
outline; try preparation once more after the next `SPAWN_EXECUTORS` in that stage.
Consume the bounded
`automatic_build_prepare.v1` response (`prepared` or `skipped`) and continue the
normal completion-and-refill loop. Preparation does not acquire a slot or start a
lease. Discourse inputs and BookStructure unit/fragment routes are reused automatically by refill;
current receipts, leases, capacity and budgets still decide the actual launch.
Consume terminal observations delivered during preparation before the next refill.
BookStructure preparation binds the accepted outline and current upstream projections.
Chapter selection, reducers, themes and final assembly continue to read current accepted results.
If preparation returns a normal Driver `NEEDS_USER` failure, handle that action below.

Consume `automatic_build_step.v1` (the nested `step` when using `build.refill`). Handle its action
exactly; use the single refill operation below when a child finishes:

- `SPAWN_EXECUTORS`: treat each returned public launch as
  `opaque_handoff_ref + dispatch_slot_ref`; reader-private launches have no dispatch slot and use
  their ref as the local slot key. Exclude a launch when its slot key is already present in
  `live_by_slot` or its ref is in `completed_refs`. Launch at most one dedicated subagent per pending
  slot, filling only the currently available live slots. Store
  `live_by_slot[slot_key] = {child, opaque_handoff_ref}`. Retain the structured Driver response and
  copy the complete `opaque_handoff_ref` from that launch into the spawn message. Do not regenerate,
  shorten, or replace it with a descriptive placeholder. Consume a completed step result once;
  do not repeat build.step merely to recover an output already returned by the tool.
  Choose each ref's bootstrap provider in this
  strict order:
  1. If the spawn tool advertises `agent_type=understand_book_executor`, select that custom agent
     explicitly and give it only this payload, with the returned ref substituted exactly:
     ```text
     Process exactly this code-issued opaque handoff ref and return only bounded lifecycle state:
     <opaque_handoff_ref>
     ```
  2. Otherwise, if `$understand-book-executor` is advertised, launch a default dedicated subagent
     and give it only this fallback payload, with the returned ref substituted exactly:
     ```text
     Use $understand-book-executor for exactly this opaque handoff ref:
     <opaque_handoff_ref>
     Return only the bounded lifecycle state defined by that skill.
     Do not use $understand-book-build inside this subagent.
     ```
  3. If neither provider is advertised, do not launch an unbound generic subagent and never emulate
     the executor in root. For a public dispatch, report the returned ref through
     `bootstrap_failure` on the next `build.step`; do not silently reread an unchanged launch.
  The ref and the bounded open-correction feedback above are the only dynamic spawn data. Do not add a target path, prompt,
  task input, hash, command list, receipt, or candidate. Never pass `dispatch_slot_ref` to the child.
  Every new ref requires a newly spawned child with a fresh connection. Never use followup_task,
  send_message, or resume to give a terminal child another ref. A stable worker ref can span
  multiple public dispatches in the same stage; the Engine records the current assignment and
  preserves its task-specific recovery identity. After an accepted work unit or completed batch,
  the Engine can route newly ready work and return DELIVER_INPUT on that same live connection.
  This includes BookStructure tasks that do not exist until a preceding result is accepted.
  Keep the original worker ref and child in live_by_slot through task changes and executor WAIT.
  Release them only after the child returns DONE or a terminal error. Do not refill after each
  commit or batch; the Engine owns normal next-task selection and context-budget rotation.
  The executor reconstructs each generation separately and uses only its newly delivered evidence.
  If no provider is available, retain that ref in completed_refs and queue its bootstrap failure.
  While `live_by_slot` is non-empty, wait only
  until the first owned child becomes terminal; consume all other terminal observations already
  delivered without waiting for unfinished children. When unused executor slots remain, bound
  that wait to 30 seconds and call build.refill even with no terminal children. A live worker can
  create newly parallel ready tasks without exiting (for example, a completed structure outline).
  This refill fills only free slots and retains all live workers; do not require a child terminal
  event before filling newly available work. When all slots are occupied, no refill is needed
  solely to poll per-task progress. Pass the owned records and terminal child
  identities to `build.refill` in the next control tool call, with one pending failure if any.
  Code matches each owned child identity, moves only its owned ref to `completed_refs`, releases
  only its own slot, computes current capacity, and calls `build.step` once.
  Duplicate or late observations cannot release a replacement child's slot.
  After a refill returns SPAWN_EXECUTORS, launch its ready_executors and record their ownership
  immediately. Then consume terminal observations delivered during that call in the next refill,
  including the newly launched children in live_by_slot. Do not wait for another planning call
  before starting work already reserved for a free slot. Other actions consume terminals before waiting.
  Keep the old ref in `completed_refs`; only the Driver-issued replacement ref can launch.
  Every other live slot
  remains owned and must not be duplicated, orphaned, or forgotten. Never wait for all children
  before rereading durable state, and never treat the first child final as global completion.
  A child lifecycle final is never durable task completion authority: even if it says `committed` or
  `retryable_failure`, only the next `build.step` projection may prove that the task committed,
  advanced, remains active, or needs a replacement recovery generation.
- `WAIT`: if an owned child is live, wait for its first terminal event as above. Otherwise wait only
  `retry_after_ms`, then recompute live slots and call `build.step` again. Do not duplicate an active
  executor or lease.
- `NEEDS_USER`: show the returned `reason`, `message`, optional `projection`, and choices exactly.
  `projection.work_unit_count` counts the units in this boundary, not all remaining build work.
  `confirm_candidate_retry` authorizes another bounded correction window under the current schema;
  it does not require accepting the rejected output or changing policy.
  `build_engine_failed` has no recovery choices. After the caller-error check above, report its
  request ID and diagnostic availability when the failure remains; raw local diagnostics belong
  to a separate debugging task.
  Do not describe ordinary state reads as stage close unless the engine establishes that phase.
  Never invent or broaden choices. If choices are present, wait for the user and return the selected
  `request_id + choice_id` in the next step. If choices are empty, report the external blocker and
  do not manufacture a decision.
- `DONE`: require `live_by_slot` to be empty, report only the returned completion summary, and end.
  Drain pending failure observations through structured steps before ending; durable state decides
  whether they are obsolete.
  This covers both public stages and any declared reader-private artifacts; the root has no separate
  private-artifact loop.

Continue across ordinary executor completions, stage boundaries, retries, and internal recovery.
Stop only at `NEEDS_USER`, `DONE`, explicit user interruption, or a packaged-engine failure for
which no structured action exists. Never emulate a dedicated executor in the root.

## Single completion-and-refill operation

On a normal child final, the next control call sends one JSON object on stdin to
`<build-exe> build.refill`. Do not split completion bookkeeping, capacity calculation and
`build.step` across tool calls. Do not insert `list_agents`; use it only to recover missing or
uncertain ownership after restoring a chat. Keep the pre-release ownership map in this request:

```json
{
  "version": "automatic_build_refill_request.v1",
  "invocation_ref": "<saved invocation ref>",
  "capacity_limit": 8,
  "live_by_slot": {
    "<slot A>": {"child": "<child A>", "opaque_handoff_ref": "<ref A>"},
    "<slot B>": {"child": "<child B>", "opaque_handoff_ref": "<ref B>"},
    "<slot C>": {"child": "<child C>", "opaque_handoff_ref": "<ref C>"}
  },
  "completed_refs": [],
  "terminal_children": ["<child C>"]
}
```

Use saved values rather than placeholders; `completed_refs` retains earlier terminal refs.
`capacity_limit` is the current total capacity reserved for this build, including its live
children, capped at eight; the code also honors the invocation's `max_parallel`.
An optional `decision` or one queued failure observation uses the same fields as `build.step`.

Consume `automatic_build_refill.v1`: replace the local `live_by_slot` and `completed_refs` with
the returned records, then handle its nested `step` using the four-action loop. For a spawn action,
start fresh native subagents only for `ready_executors` and record their child identities in
the returned ownership map. These entries already exclude live slots and completed refs.
With A, B and C live and only C terminal, the internal step receives `available_agent_slots: 1`;
A and B remain owned and the next reference D is returned for launch.

If B's terminal notification arrives during that call, first start D from the returned
`ready_executors` and record its ownership. Then submit one new `build.refill` for B using
the updated records, with A and D still owned. Launch only that response's new `ready_executors`.
Never wait for A to finish. A malformed refill returns
`automatic_build_request_error.v1` with `invalid_refill_request`; correct the request and resubmit.
This operation requests work; the host's native spawn tool starts each returned subagent.

## Dedicated executor contract

Each subagent receives one `opaque_handoff_ref` and follows the
`automatic_build_executor_session.v3` protocol in the dedicated executor instructions:

```text
executor.open(ref)
  -> DELIVER_INPUT: follow next_request through executor.input.next; continue the same theme or next work unit in this bounded batch
  -> INPUT_BATCH: retain ordered chunks only in the dedicated child; send non-final `last_ordinal` as `ack_through_ordinal` to input.next, or send the final `last_ordinal` as `confirmed_through_ordinal` to generation.start
  -> GENERATE: produce one strict JSON value from this generation's input; submit it and continue on the returned action without waiting for Root
  -> WAIT: wait retry_after_ms; reopen the same ref
  -> DONE: return only committed | retryable_failure | interrupted
```

The ref is not a filesystem path and must not be decoded or inspected. The session code performs
all path, length, hash, schema, identity, mailbox, lease, receipt, writer, quality, and retry gates.
The semantic extractor prompts remain byte-stable and use their ordinary strict-JSON output branch;
the session does not supply the old command envelope. Semantic chunks may appear only in the
dedicated child tool-result context, and candidate JSON may appear only in that child's structured
`executor.submit_candidate` request and the code-owned mailbox. No candidate source file, path
submit, shell fallback, child final, or root projection is allowed.

## Hard boundaries

- The root must not call, probe, enumerate, or use `executor.open` to diagnose a handoff.
- The root must not call, probe, enumerate, or use `executor.input.next` to diagnose a handoff.
- The root must not call, probe, enumerate, or use `executor.generation.start` to diagnose a handoff.
- The root must not call, probe, enumerate, or use `executor.submit_candidate` to diagnose a handoff.
- Dedicated subagents are mandatory for semantic generation; missing capacity is represented by a
  structured driver action, never permission to synthesize empty or generic artifacts in root.
- Paper foundation remains owned by Build Workbench and Reader. This skill cannot bypass source
  reconciliation or artifact gates.
- The engine's durable plan, invocation, dispatch/task session, mailbox, receipt, and artifact state
  are the only resume truth.
- Do not expose raw stderr, stack traces, internal commands, absolute private paths, prompt bodies,
  task input, raw goals, LID allowlists, candidate bodies, or accepted private bodies to root chat.
- Do not change extractor prompt bytes as part of this orchestration protocol.
