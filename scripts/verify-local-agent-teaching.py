"""Observe one real OpenCode teaching sample on the original quickstart book.

Use the course trial's OPENCODE_BASE_URL, OPENCODE_API_KEY and FLUID_LLM_MODEL.
Write the independent answer to learner-answer.txt in --out after reading turn-3.json.
"""
import argparse
import json
import os
import runpy
import shutil
import subprocess
import threading
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from urllib.error import HTTPError
from urllib.request import Request, urlopen
from uuid import uuid4


shared = runpy.run_path(str(Path(__file__).with_name("verify-local-agent-acp.py")))
call, wait, save = shared["call"], shared["await_value"], shared["write_json"]


class TrialProvider(BaseHTTPRequestHandler):
    """Only the recording proxy knows the real API credential."""
    def log_message(self, *args):
        pass

    def do_POST(self):
        if self.path != "/chat/completions":
            return self.send_error(404)
        data = json.loads(self.rfile.read(int(self.headers["Content-Length"])))
        with self.server.lock:
            spent = sum(r.get("usage", {}).get("total_tokens", 0) for r in self.server.sends)
            if (len(self.server.sends) >= 24 or spent >= 300000 or len(json.dumps(data).encode()) > 90000
                    or data.get("model") != os.environ["FLUID_LLM_MODEL"]):
                return self.send_error(402, "trial limit")
            receipt = {"ordinal": len(self.server.sends) + 1, "model": data["model"]}
            self.server.sends.append(receipt)
        data["max_tokens"] = min(data.get("max_tokens") or 2048, 2048)
        data["stream_options"] = {"include_usage": True}
        upstream = os.environ["OPENCODE_BASE_URL"].rstrip("/") + "/chat/completions"
        request = Request(upstream, data=json.dumps(data).encode(), headers={"Content-Type":"application/json",
            "Authorization":"Bearer " + os.environ["OPENCODE_API_KEY"]})
        try:
            with urlopen(request, timeout=75) as response:
                payload, status, media = response.read(), response.status, response.headers["Content-Type"]
            for line in payload.splitlines():
                if line.startswith(b"data: ") and line[6:].strip() != b"[DONE]":
                    usage = json.loads(line[6:]).get("usage")
                    if usage:
                        receipt["usage"] = usage
        except HTTPError as error:
            status, media, payload = error.code, "application/json", error.read()
        except (OSError, ValueError) as error:
            status, media = 502, "application/json"
            payload = json.dumps({"error":{"message":type(error).__name__}}).encode()
        receipt["status"] = status
        self.send_response(status)
        self.send_header("Content-Type", media)
        self.send_header("Content-Length", str(len(payload)))
        self.end_headers()
        self.wfile.write(payload)


def sample(options, server):
    root = options.out.resolve()
    root.mkdir(parents=True, exist_ok=False)
    server.output = root
    private = options.resume.resolve() if options.resume else root
    book = private / "book"
    if not options.resume:
        shutil.copytree(options.book, book)
    prior = options.resume or options.replay
    if prior:
        ledger = options.ledger or prior / "provider-sends.json"
        server.sends.extend(json.loads(ledger.read_text(encoding="utf-8")))
    env = shared["environment"](private, f"http://127.0.0.1:{server.server_port}")
    cfg = json.loads(env["OPENCODE_CONFIG_CONTENT"])
    model = os.environ["FLUID_LLM_MODEL"]
    cfg["model"] = cfg["small_model"] = "fixture/" + model
    cfg["provider"]["fixture"]["models"] = {model: {"name":model}}
    env["OPENCODE_CONFIG_CONTENT"] = json.dumps(cfg)
    env["UNDERSTAND_BOOK_WEB_DIST"] = str(options.web_dist.resolve())
    record = Path(env["UNDERSTAND_BOOK_LOCAL_DATA_DIR"]) / "reader-host.json"
    with (root / "reader.stderr").open("wb") as stderr:
        process = subprocess.Popen([str(options.reader.resolve()), "--local-profile", str(book)],
                                   cwd=root, env=env, stdout=stderr, stderr=stderr)
        try:
            def ready():
                if process.poll() is not None:
                    raise RuntimeError("Reader exited before opening the local API")
                location = json.loads(record.read_text(encoding="utf-8")) if record.exists() else {}
                return location if location.get("pid") == process.pid else None
            api = wait(ready, 25)["url"] + "/api"
            call(api + "/local-agent/settings", {"action":"save","agent":{"id":"trial","name":"OpenCode",
                "command":str(options.opencode.resolve()), "args":["acp"]}})
            history = call(api + "/agent/history") if options.resume else call(api + "/agent/new", {})["history"]
            chat = history["active_session_id"]
            scope = {"chat_session_id":chat}
            def state():
                return call(api + "/tutor/closed-book", {**scope,"action":"state"})
            def turn(number, message, agent="trial"):
                accepted = call(api + "/agent/runs", {"message":message,"local_agent_id":agent})
                turn_id = accepted.get("descriptor", accepted)["turn_id"]
                def finished():
                    view = call(api + "/agent/runs/" + turn_id)
                    return view if view["persistence_state"] != "pending" else None
                view = wait(finished, 180)
                closed = state()
                result = {"learner":message, "reader":view, "practice":closed}
                save(root / f"turn-{number}.json", result)
                if view["execution_state"] != "completed":
                    raise RuntimeError(f"Turn {number} failed; see turn-{number}.json")
                print(view["final_view"]["outcome"]["answer"], flush=True)
                return closed
            if options.resume:
                variant = state()
                if variant["status"] != "active" or variant["attempt"] < 2:
                    raise RuntimeError("The saved sample is not waiting for an independent answer")
            else:
                call(api + "/tutor/closed-book", {**scope,"action":"start","revision":0,"operation_id":str(uuid4()),
                    "target":"用本书的例子，练习解释为什么检索到候选段落后还要回读原文；知识图谱的关系边能证明什么？"})
                def earlier(number, message):
                    if not options.replay:
                        return turn(number, message)
                    # The exact initial problem, learner messages and the delivered
                    # hint are frozen from the original trial, prior to the change.
                    source = json.loads((options.replay / f"turn-{number}.json").read_text(encoding="utf-8"))
                    decision = {"action":"question" if number == 1 else "help",
                        "target":source["practice"]["target"],"criteria":source["practice"]["criteria"],
                        "text":source["reader"]["final_view"]["outcome"]["answer"],"provides_help":number == 2,
                        "reason":"Recorded original trial; restoring the delivered question and hint."}
                    save(root / "recorded.json", {"context":message,"decision":decision})
                    return turn(number, source["learner"], "recorded")
                if options.replay:
                    fixture = Path(__file__).resolve().parents[1] / "crates/reader-service/src/tests/fixtures/acp_teaching.cjs"
                    call(api + "/local-agent/settings", {"action":"save","agent":{"id":"recorded","name":"Recorded trial",
                        "command":"node","args":[str(fixture), str(root / "recorded.json")]}})
                question = earlier(1, "请用本书第 1.3.4 段的要点出一道闭卷题，一次只问一件事。")
                if not question["visible_turn_ids"]:
                    raise RuntimeError("Question was not delivered")
                helped = earlier(2, "我还分不清；给我一个具体提示吧。")
                if not helped["assisted"]:
                    raise RuntimeError("Displayed hint was not attributed")
                variant = turn(3, "换一道尚未解答的变式，我要自己判断。")
            if variant["assisted"] or variant["attempt"] < 2:
                raise RuntimeError("No independent retry was opened")
            def respond(number, filename):
                answer = private / filename
                print(f"Waiting for the learner's answer in {answer}", flush=True)
                response = wait(lambda: answer.read_text(encoding="utf-8-sig").strip() if answer.exists() else None, 180)
                return turn(number, response), response
            pending_followup = options.resume and len(variant["visible_turn_ids"]) > 1
            if pending_followup:
                result, response = respond(5, "learner-followup.txt")
            else:
                result, response = respond(4, "learner-answer.txt")
                if result["status"] == "active" and not result["assisted"]:
                    result, response = respond(5, "learner-followup.txt")
            if result["status"] != "complete" or result["assisted"] or result["learner_quote"] not in response:
                raise RuntimeError("Independent performance requires review; inspect turn-4.json")
        finally:
            process.terminate()
            process.wait(timeout=10)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--reader", type=Path, required=True)
    parser.add_argument("--opencode", type=Path, required=True)
    parser.add_argument("--out", type=Path, required=True)
    source = parser.add_mutually_exclusive_group()
    source.add_argument("--resume", type=Path, help="Existing sample waiting for its final learner turn")
    source.add_argument("--replay", type=Path, help="Restore the original first question and help before retrying the variant")
    parser.add_argument("--ledger", type=Path, help="Most recent cumulative provider-sends.json for this sample")
    parser.add_argument("--book", type=Path, default=Path(".understand-book/quickstart-demo"))
    parser.add_argument("--web-dist", type=Path, default=Path("packages/web/dist"))
    args = parser.parse_args()
    for variable in ("OPENCODE_API_KEY","OPENCODE_BASE_URL","FLUID_LLM_MODEL"):
        if not os.environ.get(variable):
            parser.error("Missing course model trial configuration: " + variable)
    server = ThreadingHTTPServer(("127.0.0.1", 0), TrialProvider)
    # Retain the provider receipt even if the Agent exited during a title or
    # background request. Each forwarded request has its own timeout.
    server.daemon_threads = False
    server.lock, server.sends, server.output = threading.Lock(), [], None
    threading.Thread(target=server.serve_forever, daemon=True).start()
    try:
        sample(args, server)
    finally:
        server.shutdown()
        server.server_close()
        if server.output:
            save(server.output / "provider-sends.json", server.sends)
