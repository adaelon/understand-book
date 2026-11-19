import { readFileSync, readdirSync, writeFileSync } from "node:fs";
import path from "node:path";
import { describe, expect, it } from "vitest";
import { buildAutomaticBuildSnapshot } from "../src/build-orchestrator";
import { readAutomaticBuildAttemptSnapshot } from "../src/automatic-build-task-store";
import { readBookStructureGenerationTask } from "../src/book-structure-generation";
import { readAutomaticBuildWorker } from "../src/automatic-build-worker";
import { BUILD_EXECUTOR_BOOTSTRAP_CONTRACT_V3 } from "../src/build-executor-connection-capability";
import type { AutomaticBuildExecutorSessionResponse } from "../src/automatic-build-executor-session";
import { automaticBuildWorkerStep, createAutomaticBuildInvocation } from "../../../skills/build/automatic-build-driver";
import { createBuildExecutorMcpSession } from "../../../skills/build/build-executor-mcp";
import { runAutomaticBuildCloseStage } from "../../../skills/build/automatic-build";
import { confirmedStandardBuildPlan } from "./helpers/confirmed-build-plan";
import { structureProductionFixture, structureProductionResponse } from "./helpers/book-structure-production";

type Action = AutomaticBuildExecutorSessionResponse["action"];

describe("continuous worker BookStructure integration", () => {
  it("keeps one connection across newly created dependent dispatches, then returns the unchanged full-quality stage gate to Root", async () => {
    const previousRegistry = process.env.UNDERSTAND_BOOK_AUTOMATIC_BUILD_DRIVER_ROOT;
    const fixture = structureProductionFixture(1);
    const registry = path.join(fixture.root, "registry");
    process.env.UNDERSTAND_BOOK_AUTOMATIC_BUILD_DRIVER_ROOT = registry;
    try {
      const planFile = path.join(fixture.root, "plan.json");
      writeFileSync(planFile, JSON.stringify(confirmedStandardBuildPlan(fixture.source, fixture.root)));
      const invocation = createAutomaticBuildInvocation({ version: "automatic_build_invocation_create.v1",
        target_input: fixture.source, root_dir: fixture.root, build_plan_path: planFile,
        quality_profile: "full", max_parallel: 8, created_at: new Date().toISOString() });
      // Only one dependency is ready initially. Eight is the concurrency ceiling;
      // dependency order is not relaxed in order to fill the other seven slots.
      const initial = await automaticBuildWorkerStep({ version: "automatic_build_step_request.v1",
        invocation_ref: invocation.invocation_ref, available_agent_slots: 8 });
      expect(initial.action.kind).toBe("SPAWN_EXECUTORS");
      if (initial.action.kind !== "SPAWN_EXECUTORS") throw new Error(JSON.stringify(initial));
      expect(initial.action.executors).toHaveLength(1);
      const rootRef = initial.action.executors[0].opaque_handoff_ref;
      expect(readAutomaticBuildWorker(rootRef)?.stage).toBe("book_structure");
      const connection = createBuildExecutorMcpSession({
        bootstrap_version: BUILD_EXECUTOR_BOOTSTRAP_CONTRACT_V3.version,
        protocol_generation: BUILD_EXECUTOR_BOOTSTRAP_CONTRACT_V3.session_protocol,
        session_private_root: path.join(fixture.root, "connection"),
      });
      let sequence = 0;
      const call = async (name: string, args: unknown): Promise<Action> => {
        const rpc = await connection.handle_message(JSON.parse(JSON.stringify({ jsonrpc: "2.0", id: ++sequence,
          method: "tools/call", params: { name, arguments: args } }))) as {
            result: { isError: boolean; content: { text: string }[] };
          };
        expect(rpc.result?.isError, rpc.result?.content?.[0]?.text).toBe(false);
        return (JSON.parse(rpc.result.content[0].text) as AutomaticBuildExecutorSessionResponse).action;
      };
      const frozenTask = (sessionRef: string) => {
        const sessions = path.join(registry, "executor-task-sessions");
        const session = readdirSync(sessions).filter(file => file.endsWith(".json"))
          .map(file => JSON.parse(readFileSync(path.join(sessions, file), "utf8")))
          .find(record => record.opaque_session_ref === sessionRef);
        if (!session) throw new Error("generation must have a persisted task session");
        const current = buildAutomaticBuildSnapshot(fixture.target, { quality_profile: "full" })
          .stages.find(stage => stage.stage === "book_structure")?.generation_tasks?.[session.work_unit_id];
        if (current?.kind !== "book_structure") throw new Error("generation must have a current BookStructure policy binding");
        const task = readBookStructureGenerationTask(fixture.target, current.task.policy_generation_id, session.work_unit_id);
        if (!task) throw new Error("generation must have a frozen BookStructure task");
        return task;
      };
      let action = await call("executor.open", { version: "automatic_build_executor_open_request.v3", opaque_handoff_ref: rootRef });
      const completedTasks: string[] = [];
      const dependentDispatches: Array<{ before: string; after: string }> = [];
      let previousTask: { id: string; knownTaskIds: Set<string>; handoffRef: string } | undefined;
      // This guard catches a stalled protocol; it does not supply a preselected
      // task list. Every task and dependency comes from the production router.
      for (let turn = 0; turn < 30 && action.kind !== "DONE"; turn++) {
        expect(action.kind, JSON.stringify(action)).toBe("DELIVER_INPUT");
        if (action.kind !== "DELIVER_INPUT") throw new Error(`unexpected worker action ${action.kind}`);
        const manifest = action.input_manifest;
        let request = action.next_request;
        for (;;) {
          const delivered = await call("executor.input.next", request);
          if (delivered.kind !== "INPUT_BATCH") throw new Error(`input: ${delivered.kind}`);
          if (delivered.batch.final_for_generation) {
            action = await call("executor.generation.start", {
              version: "automatic_build_executor_generation_start_request.v3",
              opaque_session_ref: delivered.batch.opaque_session_ref,
              generation_input_ref: delivered.batch.generation_input_ref,
              confirmed_through_ordinal: delivered.batch.last_ordinal,
            });
            break;
          }
          request = { version: "automatic_build_executor_input_next_request.v4",
            opaque_session_ref: delivered.batch.opaque_session_ref,
            generation_input_ref: delivered.batch.generation_input_ref,
            ack_through_ordinal: delivered.batch.last_ordinal };
        }
        if (action.kind !== "GENERATE") throw new Error(`start: ${action.kind}`);
        expect(connection.is_terminal()).toBe(false);
        const task = frozenTask(action.opaque_session_ref);
        const worker = readAutomaticBuildWorker(rootRef)!;
        expect(worker).toMatchObject({ root_handoff_ref: rootRef, stage: "book_structure", status: "active" });
        if (previousTask && previousTask.handoffRef !== worker.current_handoff_ref
          && !previousTask.knownTaskIds.has(task.descriptor.work_unit_id)) {
          dependentDispatches.push({ before: previousTask.id, after: task.descriptor.work_unit_id });
        }
        const before = buildAutomaticBuildSnapshot(fixture.target, { quality_profile: "full" })
          .stages.find(stage => stage.stage === "book_structure")!;
        expect(before.pending_tasks).toContain(task.descriptor.work_unit_id);
        previousTask = { id: task.descriptor.work_unit_id, knownTaskIds: new Set(Object.keys(before.generation_tasks ?? {})),
          handoffRef: worker.current_handoff_ref };
        action = await call("executor.submit_candidate", {
          version: "automatic_build_executor_candidate_submit.v3", opaque_session_ref: action.opaque_session_ref,
          candidate_sink_ref: action.candidate_sink_ref, candidate: structureProductionResponse(task),
        });
        expect(action.kind, `task ${task.descriptor.work_unit_id}; input ${manifest.generation_input_ref}`).not.toBe("GENERATE");
        completedTasks.push(task.descriptor.work_unit_id);
        const receipt = readAutomaticBuildAttemptSnapshot(fixture.target).stages.book_structure?.[task.descriptor.work_unit_id];
        expect(receipt).toMatchObject({ semantic_attempt: 1, failures: 0, submit_revision: 1 });
      }
      expect(action).toEqual({ kind: "DONE", status: "committed" });
      expect(connection.is_terminal()).toBe(true);
      expect(dependentDispatches.length).toBeGreaterThanOrEqual(3);
      expect(new Set(completedTasks).size).toBe(completedTasks.length);
      const worker = readAutomaticBuildWorker(rootRef)!;
      expect(worker).toMatchObject({ root_handoff_ref: rootRef, stage: "book_structure", status: "done", terminal_reason: "stage_complete" });
      expect(worker.assignments.length).toBeGreaterThanOrEqual(4);
      expect(worker.completed_assignments).toEqual(worker.assignments);
      const finished = buildAutomaticBuildSnapshot(fixture.target, { quality_profile: "full" });
      expect(finished.stages.find(stage => stage.stage === "book_structure")?.pending_tasks).toEqual([]);
      // The worker stops before Root's stage close; the existing full policy is
      // still responsible for accepting the complete structure artifact.
      expect(runAutomaticBuildCloseStage(fixture.source, fixture.root, "book_structure", { quality_profile: "full" }))
        .toMatchObject({ status: "closed" });
    } finally {
      if (previousRegistry === undefined) delete process.env.UNDERSTAND_BOOK_AUTOMATIC_BUILD_DRIVER_ROOT;
      else process.env.UNDERSTAND_BOOK_AUTOMATIC_BUILD_DRIVER_ROOT = previousRegistry;
    }
  }, 180_000);

  it("reopens a waiting connection to claim a chapter task created by another worker's last discovery commit", async () => {
    const previousRegistry = process.env.UNDERSTAND_BOOK_AUTOMATIC_BUILD_DRIVER_ROOT;
    const fixture = structureProductionFixture(2);
    const registry = path.join(fixture.root, "registry");
    process.env.UNDERSTAND_BOOK_AUTOMATIC_BUILD_DRIVER_ROOT = registry;
    try {
      // Complete the real outline so the router exposes one discovery fragment
      // per chapter. Chapter organization needs both fragment commits.
      const outlineStage = fixture.get();
      expect(outlineStage.pending_tasks).toHaveLength(1);
      const outline = outlineStage.generation_tasks![outlineStage.pending_tasks[0]];
      if (outline.kind !== "book_structure" || !("phase" in outline.task.input)
        || outline.task.input.phase !== "outline") throw new Error("expected provisional outline");
      fixture.submit(outline.task);
      const discoveryStage = fixture.get();
      expect(discoveryStage.pending_tasks).toHaveLength(2);
      for (const id of discoveryStage.pending_tasks) {
        const generation = discoveryStage.generation_tasks![id];
        expect(generation.kind === "book_structure" && generation.task.output_role).toBe("unit_observation");
      }

      const planFile = path.join(fixture.root, "plan.json");
      writeFileSync(planFile, JSON.stringify(confirmedStandardBuildPlan(fixture.source, fixture.root)));
      const invocation = createAutomaticBuildInvocation({ version: "automatic_build_invocation_create.v1",
        target_input: fixture.source, root_dir: fixture.root, build_plan_path: planFile,
        quality_profile: "full", max_parallel: 8, created_at: new Date().toISOString() });
      const initial = await automaticBuildWorkerStep({ version: "automatic_build_step_request.v1",
        invocation_ref: invocation.invocation_ref, available_agent_slots: 8 });
      expect(initial.action.kind).toBe("SPAWN_EXECUTORS");
      if (initial.action.kind !== "SPAWN_EXECUTORS") throw new Error(JSON.stringify(initial));
      expect(initial.action.executors).toHaveLength(2);

      const connect = (rootRef: string, ordinal: number) => {
        const connection = createBuildExecutorMcpSession({
          bootstrap_version: BUILD_EXECUTOR_BOOTSTRAP_CONTRACT_V3.version,
          protocol_generation: BUILD_EXECUTOR_BOOTSTRAP_CONTRACT_V3.session_protocol,
          session_private_root: path.join(fixture.root, `waiting-connection-${ordinal}`),
        });
        let sequence = 0;
        const call = async (name: string, args: unknown): Promise<Action> => {
          const rpc = await connection.handle_message(JSON.parse(JSON.stringify({ jsonrpc: "2.0", id: ++sequence,
            method: "tools/call", params: { name, arguments: args } }))) as {
              result: { isError: boolean; content: { text: string }[] };
            };
          expect(rpc.result?.isError, rpc.result?.content?.[0]?.text).toBe(false);
          return (JSON.parse(rpc.result.content[0].text) as AutomaticBuildExecutorSessionResponse).action;
        };
        const open = () => call("executor.open", {
          version: "automatic_build_executor_open_request.v3", opaque_handoff_ref: rootRef,
        });
        const start = async (action: Action) => {
          if (action.kind !== "DELIVER_INPUT") throw new Error(`expected delivered input, got ${action.kind}`);
          let request = action.next_request;
          for (;;) {
            const delivered = await call("executor.input.next", request);
            if (delivered.kind !== "INPUT_BATCH") throw new Error(`input: ${delivered.kind}`);
            if (delivered.batch.final_for_generation) {
              const generation = await call("executor.generation.start", {
                version: "automatic_build_executor_generation_start_request.v3",
                opaque_session_ref: delivered.batch.opaque_session_ref,
                generation_input_ref: delivered.batch.generation_input_ref,
                confirmed_through_ordinal: delivered.batch.last_ordinal,
              });
              if (generation.kind !== "GENERATE") throw new Error(`start: ${generation.kind}`);
              const sessions = path.join(registry, "executor-task-sessions");
              const session = readdirSync(sessions).filter(file => file.endsWith(".json"))
                .map(file => JSON.parse(readFileSync(path.join(sessions, file), "utf8")))
                .find(record => record.opaque_session_ref === generation.opaque_session_ref);
              if (!session) throw new Error("generation must have a persisted task session");
              const current = fixture.get().generation_tasks?.[session.work_unit_id];
              if (current?.kind !== "book_structure") throw new Error("expected current BookStructure task");
              const task = readBookStructureGenerationTask(fixture.target, current.task.policy_generation_id, session.work_unit_id);
              if (!task) throw new Error("expected frozen BookStructure task");
              return { generation, task };
            }
            request = { version: "automatic_build_executor_input_next_request.v4",
              opaque_session_ref: delivered.batch.opaque_session_ref,
              generation_input_ref: delivered.batch.generation_input_ref,
              ack_through_ordinal: delivered.batch.last_ordinal };
          }
        };
        const submit = ({ generation, task }: Awaited<ReturnType<typeof start>>) => call("executor.submit_candidate", {
          version: "automatic_build_executor_candidate_submit.v3", opaque_session_ref: generation.opaque_session_ref,
          candidate_sink_ref: generation.candidate_sink_ref, candidate: structureProductionResponse(task),
        });
        return { rootRef, connection, open, start, submit };
      };
      const waiting = connect(initial.action.executors[0].opaque_handoff_ref, 0);
      const predecessor = connect(initial.action.executors[1].opaque_handoff_ref, 1);
      const first = await waiting.start(await waiting.open());
      const last = await predecessor.start(await predecessor.open());
      expect(first.task.output_role).toBe("unit_observation");
      expect(last.task.output_role).toBe("unit_observation");
      expect(first.task.parent_unit_lid).not.toBe(last.task.parent_unit_lid);
      const firstAssignment = readAutomaticBuildWorker(waiting.rootRef)!.current_handoff_ref;

      const idle = await waiting.submit(first);
      expect(idle.kind, JSON.stringify(idle)).toBe("WAIT");
      expect(waiting.connection.is_terminal()).toBe(false);
      expect(readAutomaticBuildWorker(waiting.rootRef)).toMatchObject({
        root_handoff_ref: waiting.rootRef, status: "waiting", assignments: [firstAssignment],
        completed_assignments: [firstAssignment],
      });
      expect(readAutomaticBuildWorker(predecessor.rootRef)?.status).toBe("active");
      const beforeLastCommit = fixture.get();
      expect(beforeLastCommit.pending_tasks).toEqual([last.task.descriptor.work_unit_id]);
      const knownBeforeLastCommit = new Set(Object.keys(beforeLastCommit.generation_tasks ?? {}));

      // Reopen the original connection while its peer can still make progress.
      // The first submit deliberately waited out the bounded server wait above;
      // this second wait must not hold up the commit which creates both chapters.
      const [resumedInput, nextForPredecessor] = await Promise.all([
        waiting.open(), predecessor.submit(last),
      ]);
      expect(resumedInput.kind, JSON.stringify(resumedInput)).toBe("DELIVER_INPUT");
      expect(nextForPredecessor.kind, JSON.stringify(nextForPredecessor)).toBe("DELIVER_INPUT");
      const afterLastCommit = fixture.get();
      expect(afterLastCommit.pending_tasks).toHaveLength(2);
      for (const id of afterLastCommit.pending_tasks) {
        expect(knownBeforeLastCommit.has(id)).toBe(false);
        const task = afterLastCommit.generation_tasks![id];
        expect(task.kind === "book_structure" && "phase" in task.task.input && task.task.input.phase).toBe("chapter");
      }
      const resumed = await waiting.start(resumedInput);
      expect(knownBeforeLastCommit.has(resumed.task.descriptor.work_unit_id)).toBe(false);
      expect("phase" in resumed.task.input && resumed.task.input.phase).toBe("chapter");
      const resumedWorker = readAutomaticBuildWorker(waiting.rootRef)!;
      expect(resumedWorker).toMatchObject({ root_handoff_ref: waiting.rootRef, stage: "book_structure", status: "active" });
      expect(resumedWorker.current_handoff_ref).not.toBe(firstAssignment);
      expect(resumedWorker.current_handoff_ref).not.toBe(readAutomaticBuildWorker(predecessor.rootRef)!.current_handoff_ref);
      expect(resumedWorker.assignments).toHaveLength(2);
      expect(waiting.connection.is_terminal()).toBe(false);
      for (const completed of [first.task, last.task]) {
        expect(readAutomaticBuildAttemptSnapshot(fixture.target).stages.book_structure?.[completed.descriptor.work_unit_id])
          .toMatchObject({ semantic_attempt: 1, failures: 0, submit_revision: 1 });
      }
    } finally {
      if (previousRegistry === undefined) delete process.env.UNDERSTAND_BOOK_AUTOMATIC_BUILD_DRIVER_ROOT;
      else process.env.UNDERSTAND_BOOK_AUTOMATIC_BUILD_DRIVER_ROOT = previousRegistry;
    }
  }, 180_000);
});

