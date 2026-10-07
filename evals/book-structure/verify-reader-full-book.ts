import { spawn } from "node:child_process";
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { isDeepStrictEqual } from "node:util";

/** Exercise the actual installed/packaged Book MCP process and Reader's shared projections. */
export async function verifyFullBookReader(workspace: string, executable: string, output: string, options: { active_reader?: boolean } = {}) {
  const dir = path.resolve(output), book = path.resolve(workspace), binary = path.resolve(executable);
  const structure = JSON.parse(readFileSync(path.join(book, "book_structure.json"), "utf8"));
  mkdirSync(dir, { recursive: true });
  const env: NodeJS.ProcessEnv = { ...process.env, UNDERSTAND_BOOK_PRIVATE_DIR: path.join(dir, "private") };
  if (options.active_reader) {
    // Exercise the installed default resolver against Reader's actual saved selection.
    delete env.UNDERSTAND_BOOK_DIR;
    delete env.UNDERSTAND_BOOK_MEMORY_DIR;
  } else env.UNDERSTAND_BOOK_MEMORY_DIR = path.join(dir, "memory");
  const child = spawn(binary, options.active_reader ? [] : [book], { windowsHide: true,
    stdio: ["pipe", "pipe", "pipe"], env });
  child.stdout.setEncoding("utf8");
  child.stderr.setEncoding("utf8");
  let buffer = "", stderr = "", nextId = 0;
  const waiting = new Map<number, { resolve: (value: any) => void; reject: (error: Error) => void; timer: ReturnType<typeof setTimeout> }>();
  const rejectAll = (error: Error) => { for (const w of waiting.values()) { clearTimeout(w.timer); w.reject(error); } waiting.clear(); };
  child.stderr.on("data", chunk => { stderr += chunk; });
  child.on("error", error => rejectAll(error));
  child.on("close", code => rejectAll(Error(`Book MCP closed (${code}): ${stderr}`)));
  child.stdout.on("data", chunk => {
    buffer += chunk;
    for (;;) {
      const end = buffer.indexOf("\n"); if (end < 0) break;
      const line = buffer.slice(0, end).trim(); buffer = buffer.slice(end + 1);
      if (!line) continue;
      try {
        const message = JSON.parse(line), w = waiting.get(message.id);
        if (!w) continue;
        clearTimeout(w.timer); waiting.delete(message.id);
        if (message.error || message.result?.isError) w.reject(Error(`Book MCP rejected: ${line}`));
        else w.resolve(message.result);
      } catch (error) { rejectAll(error instanceof Error ? error : Error(String(error))); }
    }
  });
  const rpc = (method: string, params?: unknown) => new Promise<any>((resolve, reject) => {
    const id = ++nextId;
    const timer = setTimeout(() => { waiting.delete(id); reject(Error(`Book MCP request timed out: ${method}; ${stderr}`)); }, 30000);
    waiting.set(id, { resolve, reject, timer });
    child.stdin.write(JSON.stringify({ jsonrpc: "2.0", id, method, ...(params ? { params } : {}) }) + "\n");
  });
  const call = async (name: string, arguments_: unknown) => {
    const r = await rpc("tools/call", { name, arguments: arguments_ });
    return r.structuredContent ?? JSON.parse(r.content.find((c: any) => c.type === "text").text);
  };
  try {
    const tools = (await rpc("tools/list")).tools.map((t: any) => t.name);
    for (const name of ["book_structure", "book_guide_path"]) if (!tools.includes(name)) throw Error(`Installed MCP missing ${name}`);
    const guide = await call("book_guide_path", {});
    if (!guide.available || guide.segments.length !== structure.spine.length) throw Error("Reader guide lost full-book units");
    const chapters = [];
    for (const [i, unit] of structure.spine.entries()) {
      const segment = guide.segments[i], projection = await call("book_structure", { at: unit.lid });
      writeFileSync(path.join(dir, `structure-${unit.lid}.json`), JSON.stringify(projection, null, 2) + "\n");
      if (!isDeepStrictEqual(segment.spine_unit, unit) || !projection.available || projection.spine_index !== i
        || !isDeepStrictEqual(projection.spine_unit, unit)) throw Error(`Reader chapter identity/order/content changed: ${unit.lid}`);
      if (JSON.stringify(segment.key_stops.map((s: any) => s.id)) !== JSON.stringify(unit.key_stop_ids)
        || unit.key_stop_ids.some((id: string) => !projection.key_stops.some((s: any) => s.id === id)))
        throw Error(`Reader lost accepted macro stops: ${unit.lid}`);
      const accepted = structure.key_stops.filter((s: any) => s.lid === unit.lid || s.lid.startsWith(unit.lid + "."));
      if (accepted.some((s: any) => !projection.key_stops.some((p: any) => p.id === s.id && p.lid === s.lid && p.type === s.type
        && isDeepStrictEqual(p.reason, s.reason) && (s.title === undefined || p.title === s.title))))
        throw Error(`Reader lost chapter stops or anchored content: ${unit.lid}`);
      chapters.push({ lid: unit.lid, key_stops: projection.key_stops.length, macro_stops: segment.key_stops.length });
    }
    const whole = await call("book_structure", {});
    if (!isDeepStrictEqual(whole.throughlines, structure.throughlines)) throw Error("Reader changed full-book throughlines");
    writeFileSync(path.join(dir, "guide-path.json"), JSON.stringify(guide, null, 2) + "\n");
    const receipt = { status: "passed", workspace: book, executable: binary, chapters,
      throughlines: whole.throughlines.length, tools: ["book_structure", "book_guide_path"],
      book_resolution: options.active_reader ? "Reader actual saved current book" : "explicit workspace argument",
      path: "Actual Book MCP process → shared Reader read-tools projections", model_calls: 0 };
    writeFileSync(path.join(dir, "reader-verification.json"), JSON.stringify(receipt, null, 2) + "\n");
    return receipt;
  } finally { child.stdin.end(); child.kill(); }
}
if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url))
  console.log(JSON.stringify(await verifyFullBookReader(process.argv[2], process.argv[3], process.argv[4],
    { active_reader: process.argv.includes("--active-reader") }), null, 2));
