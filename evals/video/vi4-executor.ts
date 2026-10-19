import { appendFileSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { spawn, spawnSync } from "node:child_process";
import { createInterface } from "node:readline";
import path from "node:path";
import { fileURLToPath } from "node:url";
import assert from "node:assert/strict";
import { videoExecutorFixture } from "../../packages/core/test/helpers/video-executor-fixture";
import { freezeBuildExecutorMediaInput } from "../../packages/core/src/build-executor-media";
import { createBuildExecutorMcpSession } from "../../skills/build/build-executor-mcp";
import { BUILD_EXECUTOR_BOOTSTRAP_CONTRACT_V3 as bootstrap } from "../../packages/core/src/build-executor-connection-capability";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../..");
const script = fileURLToPath(import.meta.url);
const save = (file: string, value: unknown) => writeFileSync(file, JSON.stringify(value, null, 2));
const [mode, argument, model = "gpt-6-astra"] = process.argv.slice(2);

if (mode === "serve") {
  const dir = path.resolve(argument);
  process.env.UNDERSTAND_BOOK_AUTOMATIC_BUILD_DRIVER_ROOT = path.join(dir, "workspace/registry");
  const session = createBuildExecutorMcpSession({ bootstrap_version: bootstrap.version, protocol_generation: bootstrap.session_protocol,
    session_private_root: process.env.UNDERSTAND_BOOK_AUTOMATIC_BUILD_DRIVER_ROOT });
  let ordinal = 0;
  for await (const line of createInterface({ input: process.stdin })) {
    const request = JSON.parse(line);
    const response = session.handle_message(request);
    if (response !== undefined) {
      const n = ++ordinal;
      save(path.join(dir, `request-${n}.json`), request);
      save(path.join(dir, `response-${n}.json`), response);
      appendFileSync(path.join(dir, "calls.jsonl"), JSON.stringify({ ordinal: n, method: request.method, tool: request.params?.name }) + "\n");
      process.stdout.write(JSON.stringify(response) + "\n");
    }
  }
} else if (mode === "live") {
  assert(argument, "usage: node --import tsx evals/video/vi4-executor.ts live CODEX_EXE [MODEL]");
  const dir = path.join(root, "evals/video/generated/vi4", new Date().toISOString().replaceAll(":", "-"));
  mkdirSync(dir, { recursive: true });
  const originalDir = path.join(root, "evals/video/generated/vi2/moved-workspace");
  const originalSource = JSON.parse(readFileSync(path.join(originalDir, "video_source_manifest.json"), "utf8"));
  const originalTranscript = JSON.parse(readFileSync(path.join(originalDir, "transcript_map.json"), "utf8"));
  const segments = originalTranscript.segments.filter((s: any) => s.location.start_ms < 600000 && s.location.end_ms > 590000);
  assert(segments.length);
  const body = segments.map((s: any) => s.text).join(" ");
  const fixture = videoExecutorFixture(path.join(dir, "workspace"), body);
  const source = { ...originalSource, material: { ...originalSource.material, book_id: fixture.target.book_id } };
  // This isolated excerpt task retains the original segment text and media times;
  // its text ranges point into its own frozen semantic input.
  let offset = 0;
  const transcript = { ...originalTranscript, segments: segments.map((s: any) => {
    const mapped = { ...s, text_range: { lid: fixture.descriptor.evidence_lids.at(-1), range: { start: offset, end: offset + s.text.length } } };
    offset += s.text.length + 1;
    return mapped;
  }) };
  const inputs = JSON.parse(readFileSync(path.join(root, "evals/video/results/vi0c-20261010/inputs.json"), "utf8"));
  const selected = ["f650000", "f650000_crop"];
  const images = selected.map(id => {
    const entry = inputs.frames[id];
    const frame = { video: source.source.video, actual_pts_ms: entry.actual_pts_ms, width: source.source.width, height: source.source.height };
    return { requested_ms: entry.requested_ms, frame, region: entry.rect_normalized ? { frame, rect_normalized: entry.rect_normalized } : null,
      png: readFileSync(path.join(root, "evals/video/generated", entry.path)) };
  });
  const media = freezeBuildExecutorMediaInput({ target: fixture.target, descriptor: fixture.descriptor, source, transcript, images });
  const role = readFileSync(path.join(root, "agents/automatic-build-dispatch-executor.md"), "utf8");
  const executable = path.resolve(argument);
  const host = spawnSync(executable, ["--version"], { encoding: "utf8", windowsHide: true }).stdout.trim();
  const args = ["exec", "--ignore-user-config", "--skip-git-repo-check", "--ephemeral", "-C", dir, "-m", model,
    "-c", 'model_reasoning_effort="low"', "-c", "developer_instructions=" + JSON.stringify(role),
    "-c", "mcp_servers.understand_book_build_executor.command=" + JSON.stringify(process.execPath),
    "-c", "mcp_servers.understand_book_build_executor.args=" + JSON.stringify(["--import", "tsx", script, "serve", dir]),
    "-c", 'mcp_servers.understand_book_build_executor.default_tools_approval_mode="approve"',
    "--json", "-o", path.join(dir, "lifecycle.txt"), fixture.envelope.opaque_handoff_ref];
  save(path.join(dir, "input.json"), { original_material: originalSource.material, excerpt_task: fixture.descriptor,
    original_segments: segments, media, host, model, handoff: fixture.envelope.opaque_handoff_ref });
  const started = Date.now();
  const child = spawn(executable, args, { cwd: root, windowsHide: true, stdio: ["ignore", "pipe", "pipe"] });
  child.stdout.on("data", b => appendFileSync(path.join(dir, "events.jsonl"), b));
  child.stderr.on("data", b => appendFileSync(path.join(dir, "stderr.txt"), b));
  let timedOut = false;
  const timer = setTimeout(() => { timedOut = true; child.kill(); }, 300000);
  const exit = await new Promise<number | null>((resolve, reject) => { child.on("close", resolve); child.on("error", reject); }).finally(() => clearTimeout(timer));
  save(path.join(dir, "run.json"), { host, model, exit_code: exit, timed_out: timedOut, elapsed_ms: Date.now() - started });
  console.log(JSON.stringify({ directory: dir, exit_code: exit, elapsed_ms: Date.now() - started }));
  assert.equal(exit, 0); assert(!timedOut);
  const calls = readFileSync(path.join(dir, "calls.jsonl"), "utf8").trim().split("\n").map(s => JSON.parse(s));
  const nativeImages: any[] = [];
  const submitted: unknown[] = [];
  const envelopes: Array<{ tool: string; mcp_result_bytes: number; jsonrpc_line_bytes: number }> = [];
  let committed = false, generated = false;
  for (const call of calls) {
    const rpc = JSON.parse(readFileSync(path.join(dir, `response-${call.ordinal}.json`), "utf8"));
    const result = rpc.result;
    if (!call.tool) continue;
    assert(!result.isError);
    envelopes.push({ tool: call.tool, mcp_result_bytes: Buffer.byteLength(JSON.stringify(result)), jsonrpc_line_bytes: Buffer.byteLength(JSON.stringify(rpc) + "\n") });
    nativeImages.push(...result.content.filter((c: any) => c.type === "image"));
    const response = JSON.parse(result.content[0].text);
    if (response.action?.kind === "GENERATE") generated = true;
    if (response.action?.kind === "DONE" && response.action.status === "committed") committed = true;
    if (call.tool === "executor.submit_candidate") submitted.push(JSON.parse(readFileSync(path.join(dir, `request-${call.ordinal}.json`), "utf8")).params.arguments.candidate);
  }
  assert.equal(nativeImages.length, images.length);
  images.forEach((img, i) => assert(Buffer.from(nativeImages[i].data, "base64").equals(img.png)));
  assert(generated && committed && submitted.length > 0);
  const recognized = submitted.some((candidate: any) => candidate.nodes.some((node: any) =>
    node.name === "bfloat16" && node.occurrences.includes(fixture.descriptor.evidence_lids.at(-1))));
  assert(recognized, "joint input must resolve the machine transcript's beef float 16 using the code image");
  const lifecycle = readFileSync(path.join(dir, "lifecycle.txt"), "utf8").trim();
  assert.equal(JSON.parse(lifecycle).status, "committed");
  const events = readFileSync(path.join(dir, "events.jsonl"), "utf8").trim().split("\n").map(line => JSON.parse(line));
  const summary = { status: "passed", host, model, original_material: originalSource.material,
    work_unit: fixture.descriptor.work_unit_id, media_input: media, image_count: nativeImages.length,
    native_bytes_and_order_equal: true, generation_started: generated, candidate_accepted: committed,
    parent_result: JSON.parse(lifecycle), calls: calls.filter(c => c.tool).map(c => c.tool),
    elapsed_ms: Date.now() - started, private_results: path.relative(root, dir),
    joint_term_disambiguation: recognized, envelopes, usage: events.find(event => event.type === "turn.completed")?.usage ?? null,
    limits: "Source-mode Codex MCP; DSH and installed binary media path remain unverified. Graph media evidence schema is VI13." };
  save(path.join(dir, "verification.json"), summary);
  const archive = path.join(root, "evals/video/results/vi4-20261010"); mkdirSync(archive, { recursive: true });
  save(path.join(archive, "verification.json"), summary);
  console.log(JSON.stringify({ status: "passed", image_count: nativeImages.length, candidate_accepted: committed }));
} else throw new Error("Expected serve or live");
