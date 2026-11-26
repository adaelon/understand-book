"""Capture real Reader ACP prompts on either side of the six-turn history window."""
import argparse
import json
import os
from pathlib import Path
import runpy
import shutil
import subprocess
import sys

fixture = runpy.run_path(str(Path(__file__).with_name("verify-local-agent-acp.py")))
call, wait, write_json = (fixture[name] for name in ("call", "await_value", "write_json"))

QUESTIONS = [
    "讲义先给数值、再写公式，学习率全篇用 η。例子先取 0.1。等我明确说保存再写笔记。先讨论，等我说整理时再做一页窄屏讲义。",
    "采用这个一维例子：L(w)=(w-2)^2，从 w=0 开始。先看导数。",
    "导数的符号怎样决定更新方向？",
    "刚才的示例值改为 0.01，接着看。",
    "为什么接近最低点时更新量会变小？",
    "请再讨论一步，离最低点更远时会发生什么？",
    "现在检查一下上一步推导的依据。",
    "按之前的约定整理。",
]
FACTS = {"order": "先给数值、再写公式", "notation": "学习率全篇用 η",
         "save_permission": "等我明确说保存再写笔记", "correction": "改为 0.01"}
NEUTRAL = "收到，继续讨论这个例子。"
PROBE = "请回读新聊天开头的全文。"
LONG_QUESTION = "只看这段原话。" + "例" * 1200 + "新聊天专用的尾部约定"


def probe_history(server):
    with subprocess.Popen([server["command"], *server["args"]], stdin=subprocess.PIPE,
                          stdout=subprocess.PIPE, text=True, encoding="utf-8") as bridge:
        def rpc(method, params):
            bridge.stdin.write(json.dumps({"jsonrpc":"2.0", "id":1, "method":method, "params":params}) + "\n")
            bridge.stdin.flush()
            return json.loads(bridge.stdout.readline())["result"]

        rpc("initialize", {"protocolVersion":"2025-06-18","clientInfo":{"name":"acp-test","version":"1"},
                           "capabilities":{}})
        if not any(t["name"] == "reader_history" for t in rpc("tools/list", {})["tools"]):
            raise ValueError("Missing reader_history")
        def read(args):
            return rpc("tools/call", {"name":"reader_history", "arguments":args})
        first = read({"turn":1})["structuredContent"]
        tail = read(first["next"])["structuredContent"]
        invalid = read({"turn":2})
        bridge.stdin.close()
        return {"first":first, "tail":tail, "current_turn_rejected":invalid["isError"]}


def capture(path, agent):
    sys.stdin.reconfigure(encoding="utf-8")
    sys.stdout.reconfigure(encoding="utf-8")

    def send(message):
        print(json.dumps({"jsonrpc": "2.0", **message}), flush=True)

    session = f"{agent}-{os.getpid()}"
    for line in sys.stdin:
        request = json.loads(line)
        method, key = request.get("method"), request.get("id")
        if method == "initialize":
            send({"id": key, "result": {"protocolVersion": 1, "agentInfo": {"name": agent},
                                        "agentCapabilities": {}}})
        elif method == "session/new":
            server = request["params"]["mcpServers"][0]
            send({"id": key, "result": {"sessionId": session}})
        elif method == "session/prompt":
            if request["params"]["prompt"][0]["text"].endswith(PROBE):
                write_json(path.parent / "history-tool.json", probe_history(server))
            with path.open("a", encoding="utf-8") as log:
                print(json.dumps({"agent": agent, "request": request}, ensure_ascii=False), file=log)
            send({"method": "session/update", "params": {"sessionId": session, "update": {
                "sessionUpdate": "agent_message_chunk", "content": {"type": "text", "text": NEUTRAL}}}})
            send({"id": key, "result": {"stopReason": "end_turn"}})


def saved_questions(profile, session):
    for path in (profile / "memory" / "agent-sessions").glob("*.jsonl"):
        events = [json.loads(line) for line in path.read_text(encoding="utf-8").splitlines()]
        if events and events[0]["payload"].get("session_id") == session:
            return path, [event["payload"]["turn"]["user"] for event in events
                          if event["kind"] == "turn.accepted"]
    raise RuntimeError(f"No on-disk journal for {session}")


def run_arm(args, root, switch):
    root.mkdir()
    env = fixture["environment"](root, "http://127.0.0.1:1/v1")
    env["UNDERSTAND_BOOK_WEB_DIST"] = str(args.web_dist.resolve())
    profile = Path(env["UNDERSTAND_BOOK_LOCAL_DATA_DIR"])
    book = root / "book"
    shutil.copytree(args.book, book)
    log = root / "acp-prompts.jsonl"
    with (root / "reader.stderr").open("wb") as err:
        reader = subprocess.Popen([str(args.reader.resolve()), "--local-profile", str(book)],
                                  cwd=root, env=env, stdout=err, stderr=err)
        try:
            def ready():
                if reader.poll() is not None:
                    raise RuntimeError(f"Reader exited, see {err.name}")
                record = profile / "reader-host.json"
                return json.loads(record.read_text(encoding="utf-8")) if record.exists() else None

            api = wait(ready, 25)["url"] + "/api"
            chat = call(api + "/agent/new", {})["history"]["active_session_id"]
            for agent in ("initial", "alternate"):
                call(api + "/local-agent/settings", {"action": "save", "agent": {
                    "id": agent, "name": agent, "command": sys.executable,
                    "args": [str(Path(__file__).resolve()), "--capture", str(log), agent]}})
            def run_turn(session, agent, question, filename):
                started = call(api + "/agent/runs", {"message": question,
                    "chat_session_id": session, "local_agent_id": agent})
                descriptor = started.get("descriptor", started)

                def terminal():
                    result = call(api + "/agent/runs/" + descriptor["turn_id"])
                    return result if result["persistence_state"] in ("saved", "failed", "unsaved") else None

                result = wait(terminal, 30)
                write_json(root / filename, result)
                view = result["final_view"]
                if (result["persistence_state"] != "saved" or result["execution_state"] != "completed"
                        or descriptor["session_id"] != session or view["domain"]["local_agent"]["id"] != agent
                        or view["outcome"]["answer"] != NEUTRAL):
                    raise RuntimeError(f"Unexpected turn state: see {root / filename}")

            for turn, question in enumerate(QUESTIONS, 1):
                agent = "alternate" if switch and turn == 8 else "initial"
                if turn == 1 or (turn == 8 and switch):
                    call(api + "/local-agent/settings", {"action": "select", "id": agent})
                run_turn(chat, agent, question, f"turn-{turn}.json")
            probe = args.probe_history_tool and not switch
            if probe:
                other = call(api + "/agent/new", {})["history"]["active_session_id"]
                if other == chat:
                    raise RuntimeError("Reader did not create a separate conversation")
                run_turn(other, "initial", LONG_QUESTION, "separate-chat-1.json")
                run_turn(other, "initial", PROBE, "separate-chat-2.json")
        finally:
            reader.terminate()
            reader.wait(timeout=15)

    journal, persisted = saved_questions(profile, chat)
    if persisted != QUESTIONS:
        raise RuntimeError(f"Durable questions differ from the submitted history: {journal}")
    frames = [json.loads(line) for line in log.read_text(encoding="utf-8").splitlines()]
    if len(frames) != len(QUESTIONS) + (2 if probe else 0):
        raise RuntimeError(f"Unexpected number of captured ACP requests: {len(frames)}")
    if probe:
        sample = json.loads((root / "history-tool.json").read_text(encoding="utf-8"))
        p = frames[-1]["request"]["params"]["prompt"][0]["text"]
        if (sample["first"]["reader"] + sample["tail"]["reader"] != LONG_QUESTION
                or sample["tail"]["next"] is not None or not sample["current_turn_rejected"]
                or any(text in p for text in FACTS.values()) or "新聊天专用的尾部约定" in p
                or "reader_history" not in p):
            raise RuntimeError("Separate chat, truncated page or history cursor was not preserved")
    checkpoints = []
    for turn in (7, 8):
        frame = frames[turn - 1]
        parts = frame["request"]["params"]["prompt"]
        prompt = "\n".join(p["text"] for p in parts if p["type"] == "text")
        expected_agent = "alternate" if switch and turn == 8 else "initial"
        if frame["agent"] != expected_agent or not prompt.endswith(QUESTIONS[turn - 1]):
            raise RuntimeError(f"Wrong ACP recipient or question at turn {turn}")
        checks = {name: text in prompt for name, text in FACTS.items()}
        write_json(root / f"prompt-{turn}.json", frame)
        checkpoints.append({"turn": turn, "agent": frame["agent"], "facts_received": checks,
                            "earlier_questions_seen": [i for i, q in enumerate(QUESTIONS[:turn-1], 1)
                                                       if q in prompt]})
    return {"session": chat, "saved_questions": persisted, "journal": str(journal.relative_to(root)),
            "history_tool_verified":probe,
            "requests": checkpoints, "all_facts_received": all(
                all(t["facts_received"].values()) for t in checkpoints)}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--reader", type=Path, required=True)
    parser.add_argument("--book", type=Path, default=Path(".understand-book/quickstart-demo"))
    parser.add_argument("--web-dist", type=Path, default=Path("packages/web/dist"))
    parser.add_argument("--out", type=Path, required=True, help="Fresh evidence directory")
    parser.add_argument("--probe-history-tool", action="store_true",
                        help="Read a truncated quote through the turn's real MCP tool in a separate chat")
    args = parser.parse_args()
    root = args.out.resolve()
    root.mkdir(parents=True, exist_ok=False)
    report = {"reader": str(args.reader.resolve()), "model_calls": 0}
    for name in ("same-agent", "switch-agent"):
        report[name] = run_arm(args, root / name, name == "switch-agent")
        write_json(root / "result.json", report)
    print(root / "result.json")
    if not all(report[name]["all_facts_received"] for name in ("same-agent", "switch-agent")):
        print("Reader omitted an earlier user requirement; see captured prompts and saved questions.")
        return 2
    return 0


if __name__ == "__main__":
    if len(sys.argv) == 4 and sys.argv[1] == "--capture":
        capture(Path(sys.argv[2]), sys.argv[3])
    else:
        sys.exit(main())
