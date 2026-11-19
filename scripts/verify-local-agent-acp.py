"""Exercise an installed ACP agent through the local Reader using an isolated loopback model.

Prepare the original quickstart book with pnpm demo:prepare. Run --help for binary paths.
The fixture checks transport, evidence, citation navigation and cancellation, not teaching quality.
"""
import argparse
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import threading
import time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from urllib.error import HTTPError
from urllib.request import ProxyHandler, Request, build_opener
from uuid import uuid4


QUOTE = "真正支撑回答的是取回的原文"
QUESTION = "为什么找到位置还要读原文？"
ANSWER = "原文提供作答的依据。"
HTTP = build_opener(ProxyHandler({}))


def json_text(value):
    return json.dumps(value, ensure_ascii=False)


def write_json(path, value):
    path.write_text(json.dumps(value, ensure_ascii=False, indent=2), encoding="utf-8")


def call(url, body=None):
    data = None if body is None else json_text(body).encode()
    request = Request(url, data=data, headers={"Content-Type": "application/json"})
    try:
        with HTTP.open(request, timeout=100) as response:
            return json.load(response)
    except HTTPError as exc:
        raise RuntimeError(f"{url} returned {exc.code}: {exc.read().decode()}") from exc


def functions(body):
    """The Responses API can group MCP functions into namespaces."""
    for tool in body.get("tools", []):
        if tool.get("type") == "namespace":
            for item in tool.get("tools", []):
                yield tool["name"], item["name"]
        elif tool.get("function") or tool.get("name"):
            yield "", tool.get("function", tool)["name"]


class FixtureHandler(BaseHTTPRequestHandler):
    def log_message(self, *args):
        pass

    def do_GET(self):
        self.send_response(200)
        self.end_headers()
        self.wfile.write(b'{"object":"list","data":[{"id":"gpt-5.4","object":"model","owned_by":"local"}]}')

    def do_POST(self):
        body = json.loads(self.rfile.read(int(self.headers["Content-Length"])))
        reqs = self.server.requests
        messages = body.get("input", body.get("messages", []))
        conversation = json_text(messages)
        latest = next((m.get("content", "") for m in reversed(messages) if m.get("role") == "user"), "")
        if isinstance(latest, list):
            latest = "\n".join(part.get("text", "") for part in latest if isinstance(part, dict))
        names = list(functions(body))
        # Read only tool output. A matching string in the host's prompt is insufficient.
        outputs = [m for m in messages if m.get("role") == "tool" or m.get("type") == "function_call_output"]
        observed = json_text(outputs)
        marker = re.search(r"\[\[source:[^\]]+\]\]", observed)
        if latest.rstrip().endswith("CANCEL_GATE"):
            self.server.cancel_gate.set()
            self.server.release.wait(15)
        handoff = bool(names and latest.rstrip().endswith("请接续这段讨论。"))
        next_name = "source_present" if QUOTE in observed else "book_text"
        if not names or marker or handoff:
            action = None
        else:
            action = next((pair for pair in names if pair[1].endswith(next_name)), None)
            if not action:
                action = next((pair for pair in names if pair[1] in ("tool_search", "tool_search_tool", "search_tools")), None)
        if len(outputs) >= 12:
            # Fail the assertions below if an upstream repeats an unavailable tool.
            action = None
        args = {"quote": QUOTE} if action and action[1].endswith("source_present") else {"lid": "1.3.4"}
        if action and action[1] in ("tool_search", "tool_search_tool", "search_tools"):
            args = {"query": next_name}
        reqs.append({"endpoint": self.path, "tool": action,
                     "tool_count": len(names),
                     "tool_names": [f"{ns}.{name}" if ns else name for ns, name in names],
                     "reader_tools": [name for ns, name in names if "understand" in (name + ns)],
                     "observed_text": QUOTE in observed,
                     "observed_marker": marker.group() if marker else None})
        answer = ANSWER + (marker.group() if marker else "")
        if handoff:
            self.server.handoff_seen = QUESTION in conversation and ANSWER in conversation
            answer = "上一个回答已经区分了候选位置和原文证据。"
        self.send_response(200)
        self.send_header("Content-Type", "text/event-stream")
        self.send_header("Cache-Control", "no-cache")
        self.end_headers()
        try:
            if self.path.endswith("/responses"):
                self.responses_stream(body["model"], action, args, answer)
            else:
                self.completions(body["model"], action, args, answer)
        except (BrokenPipeError, ConnectionResetError, ConnectionAbortedError):
            pass

    def event(self, payload):
        self.wfile.write(("data: " + json_text(payload) + "\n\n").encode())
        self.wfile.flush()

    def responses_stream(self, model, action, args, answer):
        uid = uuid4().hex
        if action:
            item = {"type": "function_call", "id": "fc_" + uid, "call_id": "call_" + uid,
                    "name": action[1], "arguments": json_text(args), "status": "completed"}
            if action[0]:
                item["namespace"] = action[0]
        else:
            item = {"type": "message", "role": "assistant", "id": "msg_" + uid, "status": "completed",
                    "content": [{"type": "output_text", "text": answer, "annotations": []}]}
        response = {"id": "resp_" + uid, "object": "response", "created_at": int(time.time()),
                    "status": "in_progress", "model": model, "output": []}
        self.event({"type": "response.created", "response": response})
        partial = {"arguments": ""} if action else {"content": []}
        self.event({"type": "response.output_item.added", "output_index": 0,
                    "item": {**item, **partial, "status": "in_progress"}})
        if action:
            self.event({"type": "response.function_call_arguments.delta", "output_index": 0,
                        "item_id": item["id"], "delta": item["arguments"]})
        else:
            self.event({"type": "response.output_text.delta", "output_index": 0, "content_index": 0,
                        "item_id": item["id"], "delta": answer})
        self.event({"type": "response.output_item.done", "output_index": 0, "item": item})
        self.event({"type": "response.completed", "response": {**response, "status": "completed",
                    "output": [item], "usage": {"input_tokens": 30, "output_tokens": 10, "total_tokens": 40}}})

    def completions(self, model, action, args, answer):
        uid = uuid4().hex
        base = {"id": "chatcmpl-" + uid, "object": "chat.completion.chunk", "created": int(time.time()), "model": model}
        delta = {"role": "assistant", "content": answer}
        if action:
            delta = {"role": "assistant", "tool_calls": [{"index": 0, "id": "call_" + uid, "type": "function",
                     "function": {"name": action[1], "arguments": json_text(args)}}]}
        self.event({**base, "choices": [{"index": 0, "delta": delta, "finish_reason": None}]})
        self.event({**base, "choices": [{"index": 0, "delta": {}, "finish_reason": "tool_calls" if action else "stop"}]})
        self.wfile.write(b"data: [DONE]\n\n")


def environment(root, model_url):
    permitted = {"PATH", "PATHEXT", "SYSTEMROOT", "WINDIR", "COMSPEC", "PROCESSOR_ARCHITECTURE",
                 "NUMBER_OF_PROCESSORS", "PROGRAMFILES", "PROGRAMFILES(X86)", "SYSTEMDRIVE", "PROGRAMDATA"}
    env = {key: value for key, value in os.environ.items() if key.upper() in permitted}
    for key in ("HOME", "USERPROFILE", "APPDATA", "LOCALAPPDATA", "CODEX_HOME", "XDG_STATE_HOME",
                "XDG_CACHE_HOME", "XDG_CONFIG_HOME", "XDG_DATA_HOME", "TEMP", "TMP", "UNDERSTAND_BOOK_LOCAL_DATA_DIR"):
        path = root / ("home" if key in ("HOME", "USERPROFILE") else key.lower())
        path.mkdir(exist_ok=True)
        env[key] = str(path)
    env.update({"NO_BROWSER": "1", "OPENAI_API_KEY": "local-fixture", "CODEX_API_KEY": "local-fixture",
                "NO_PROXY": "127.0.0.1,localhost", "OPENCODE_DISABLE_AUTOUPDATE": "1", "OPENCODE_DISABLE_MODELS_FETCH": "1",
                "OPENCODE_DISABLE_SHARE": "1", "OPENCODE_DISABLE_LSP_DOWNLOAD": "1", "GIT_CEILING_DIRECTORIES": str(root),
                "HTTP_PROXY": "http://127.0.0.1:1", "HTTPS_PROXY": "http://127.0.0.1:1",
                "ALL_PROXY": "http://127.0.0.1:1", "OPENAI_BASE_URL": model_url, "MODEL_PROVIDER": "fixture"})
    provider = {"name": "Loopback", "base_url": model_url, "env_key": "OPENAI_API_KEY", "wire_api": "responses",
                "supports_websockets": False}
    env["CODEX_CONFIG"] = json_text({"model_provider": "fixture", "model": "gpt-5.4", "model_providers": {"fixture": provider}})
    codex_home = Path(env["CODEX_HOME"])
    (codex_home / "config.toml").write_text(
        f'model = "gpt-5.4"\nmodel_provider = "fixture"\n[model_providers.fixture]\n'
        f'name = "Loopback"\nbase_url = "{model_url}"\nwire_api = "responses"\n'
        'env_key = "OPENAI_API_KEY"\nsupports_websockets = false\n', encoding="utf-8")
    write_json(codex_home / "auth.json", {"OPENAI_API_KEY": "local-fixture"})
    env["OPENCODE_CONFIG_CONTENT"] = json_text({"model": "fixture/local", "small_model": "fixture/local", "snapshot": False,
        "provider": {"fixture": {"npm": "@ai-sdk/openai-compatible", "name": "Fixture",
        "options": {"baseURL": model_url, "apiKey": "local-fixture"}, "models": {"local": {"name": "Local"}}}},
        "permission": {"*": "allow"}})
    return env


def await_value(callback, timeout=150):
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        value = callback()
        if value:
            return value
        time.sleep(.2)
    raise TimeoutError("Timed out waiting for " + callback.__name__)


def run_sample(args, fixture):
    root = args.out.resolve()
    book = root / "book"
    shutil.copytree(args.book.resolve(), book)
    env = environment(root, f"http://127.0.0.1:{fixture.server_port}/v1")
    env["UNDERSTAND_BOOK_WEB_DIST"] = str(args.web_dist.resolve())
    record = Path(env["UNDERSTAND_BOOK_LOCAL_DATA_DIR"]) / "reader-host.json"
    summary = {"agent": args.agent, "local_model_requests": fixture.requests}
    with (root / "reader.stderr").open("wb") as stderr:
        process = subprocess.Popen([str(args.reader.resolve()), "--local-profile", str(book)],
                                   cwd=root, env=env, stdout=stderr, stderr=stderr)
        try:
            def ready():
                if process.poll() is not None:
                    raise RuntimeError("Reader exited; inspect reader.stderr")
                return json.loads(record.read_text(encoding="utf-8")) if record.exists() else None
            api = await_value(ready, 25)["url"] + "/api"
            executable = str(args.command.resolve())
            cli_args = args.command_arg or []
            saved = call(api + "/local-agent/settings", {"action": "save", "agent": {
                "id": "fixture", "name": {"opencode": "OpenCode", "codex": "Codex"}[args.agent],
                "command": executable, "args": cli_args}})
            summary["configured"] = any(a["id"] == "fixture" for a in saved["agents"])
            selected = call(api + "/local-agent/settings", {"action": "select", "id": "fixture"})
            if selected.get("selected_agent_id") != "fixture":
                raise RuntimeError("Reader did not persist the selected agent")
            summary["probe"] = call(api + "/local-agent/settings", {"action": "probe", "id": "fixture"})["probe"]
            def start(message, agent_id="fixture", **params):
                request = {"local_agent_id": agent_id, "message": message, **params}
                result = call(api + "/agent/runs", request)
                return result.get("descriptor", result)["turn_id"]
            def finish(turn):
                def terminal():
                    result = call(api + "/agent/runs/" + turn)
                    return result if result["persistence_state"] in ("saved", "unsaved", "failed") else None
                return await_value(terminal)
            turn = start(QUESTION, question_quote={"lid": "1.4.3", "quote": "候选位置"})
            result = finish(turn)
            write_json(root / "turn.json", result)
            if result["execution_state"] != "completed":
                raise RuntimeError(f"Reader turn failed: {result['final_view'].get('error') or result.get('error')}")
            domain = result["final_view"]["domain"]
            summary["turn_agent"] = domain.get("local_agent")
            if (summary["turn_agent"] or {}).get("id") != "fixture" or domain.get("provider_binding"):
                raise RuntimeError("The saved answer was attributed to the wrong provider")
            marker = next((r["observed_marker"] for r in fixture.requests if r["observed_marker"]), None)
            if not marker or not any(r["observed_text"] for r in fixture.requests):
                raise RuntimeError("The adapter did not relay source_present and book_text results to the model")
            ref_id = marker[len("[[source:"):-2]
            view = result["final_view"]["outcome"]["answer_view"]
            if not any(ref_id in part.get("source_ref_ids", []) for part in view["parts"]):
                raise RuntimeError("The delivered answer lacks a clickable source part")
            popup = call(api + "/agent/source.resolve", {"turn_id": turn, "source_ref_id": ref_id})
            if popup.get("highlighted_quote") != QUOTE or not popup.get("can_open_in_reader"):
                raise RuntimeError(f"Citation did not resolve to the observed passage: {popup}")
            summary["citation"] = popup
            opened = call(api + "/agent/source.open", {"turn_id": turn, "source_ref_id": ref_id})
            reader_state = call(api + "/reader/state", {})
            if (not opened["opened"] or opened.get("evidence_range", {}).get("start_lid") != "1.3.4"
                    or reader_state["selection"] != "1.3.4" or "1.3.4" not in reader_state["viewport"]["visible_lids"]):
                raise RuntimeError(f"Reader did not navigate to the cited passage: {opened}")
            summary["opened_lid"] = reader_state["selection"]
            # The second prompt's model request blocks until Reader has accepted its cancellation.
            cancel_turn = start("CANCEL_GATE")
            if not fixture.cancel_gate.wait(120):
                raise RuntimeError("Second prompt did not reach the local model")
            call(api + "/agent/runs/" + cancel_turn + "/cancel", {})
            fixture.release.set()
            cancelled = finish(cancel_turn)
            summary["cancelled"] = cancelled["execution_state"]
            if cancelled["execution_state"] != "cancelled":
                raise RuntimeError(f"Unexpected terminal state {cancelled['execution_state']}")
            if args.handoff_command:
                other = "Codex" if args.agent == "opencode" else "OpenCode"
                call(api + "/local-agent/settings", {"action": "save", "agent": {
                    "id": "alternate", "name": other, "command": str(args.handoff_command.resolve()),
                    "args": args.handoff_arg or []}})
                call(api + "/local-agent/settings", {"action": "select", "id": "alternate"})
                continued = finish(start("请接续这段讨论。", agent_id="alternate"))
                previous = result["descriptor"]["session_id"]
                if (not fixture.handoff_seen or continued["execution_state"] != "completed"
                        or continued["descriptor"]["session_id"] != previous
                        or continued["final_view"]["domain"]["local_agent"]["id"] != "alternate"):
                    raise RuntimeError("The second adapter did not receive the original question and delivered answer")
                summary["handoff"] = {"agent": other, "original_question_and_answer_seen": True,
                                      "session_id": previous, "state": continued["execution_state"]}
                call(api + "/local-agent/settings", {"action": "select", "id": "fixture"})
            if args.preview_seconds:
                write_json(root / "result.json", summary)
                write_json(root / "preview-ready.json", {"url": api[:-4], "agent_id": "fixture"})
                print(f"Preview at {api[:-4]} for {args.preview_seconds} seconds", flush=True)
                deadline = time.monotonic() + args.preview_seconds
                while time.monotonic() < deadline and not (root / "preview-done").exists():
                    time.sleep(1)
        finally:
            fixture.release.set()
            write_json(root / "result.json", summary)
            process.terminate()
            process.wait(timeout=10)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--reader", type=Path, required=True, help="Reader binary with --local-profile support")
    parser.add_argument("--book", type=Path, default=Path(".understand-book/quickstart-demo"))
    parser.add_argument("--web-dist", type=Path, default=Path("packages/web/dist"))
    parser.add_argument("--agent", choices=("opencode", "codex"), required=True)
    parser.add_argument("--command", type=Path, required=True, help="Actual adapter executable (or node for a JS entry)")
    parser.add_argument("--command-arg", action="append", help="For OpenCode pass acp; give Node the absolute adapter JS path")
    parser.add_argument("--handoff-command", type=Path, help="Continue the same chat with the other adapter")
    parser.add_argument("--handoff-arg", action="append", help="Arguments for the other adapter")
    parser.add_argument("--out", type=Path, required=True, help="A new directory for isolated profiles and results")
    parser.add_argument("--preview-seconds", type=int, default=0,
                        help="Keep the verified Reader running for a visual check; create preview-done in --out to finish early")
    args = parser.parse_args()
    args.out.mkdir(parents=True, exist_ok=False)
    with ThreadingHTTPServer(("127.0.0.1", 0), FixtureHandler) as server:
        server.cancel_gate, server.release = threading.Event(), threading.Event()
        server.requests = []
        server.handoff_seen = False
        threading.Thread(target=server.serve_forever, daemon=True).start()
        try:
            run_sample(args, server)
        finally:
            server.shutdown()
    print(args.out.resolve() / "result.json")


if __name__ == "__main__":
    main()
