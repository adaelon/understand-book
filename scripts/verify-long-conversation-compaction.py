"""Exercise Resident checkpoint installation and recovery across real Reader processes.

The HTTP fixture uses the production OpenAI-compatible adapter and isolated Reader
storage. Repeated source text supplies window pressure only. No model key is used.
"""
import argparse
from contextlib import contextmanager
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import importlib.util
import json
from pathlib import Path
import re
import shutil
import subprocess
import threading


def load_helper(filename):
    spec = importlib.util.spec_from_file_location("reader_fixture", Path(__file__).with_name(filename))
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


acp = load_helper("verify-local-agent-acp.py")
FIRST = "讲义先给数值、再写公式，学习率全篇用 η。例子先取 0.1。等我明确说保存再写笔记。我们先讨论 L(w)=(w-2)^2，从 w=0 开始。"
CHANGE = "例子的学习率改为 0.01。"
ORDER = "先给数值、再写公式"
UNSAVED = "笔记尚未保存"
FALSE_NOTE = "笔记已经保存"
WRITTEN = "梯度方向的示例"
SOURCE = "轨迹依次靠近最低点，下降方向由当前点的斜率决定。"


def compact_sources(body):
    for message in reversed(body["messages"]):
        text = message.get("content", "")
        if not isinstance(text, str):
            continue
        for line in reversed(text.splitlines()):
            if line.startswith("{") and '"eligible_items"' in line:
                data = json.loads(line)
                if "eligible_items" in data:
                    items = []
                    for item in data["eligible_items"]:
                        content = item.get("content")
                        if content is None:
                            content = body["messages"][item["ordered_message_index"]]["content"]
                        items.append((item["source_item_id"], str(content)))
                    return items
    return None


def draft(items, bad):
    source = " ".join(text for _, text in items)
    rules = []
    if ORDER in source:
        rules.extend([ORDER, "学习率用 η", "明确要求保存后才可写笔记"])
    if "0.01" in source:
        rules.append("当前学习率 0.01")
    elif "0.1" in source:
        rules.append("当前学习率 0.1")
    rules.append(FALSE_NOTE if bad else UNSAVED)
    out = {key: [] for key in ("active_goal", "progress", "decisions", "user_constraints",
           "open_obligations", "unresolved_ambiguities", "critical_facts", "critical_examples", "next_steps")}
    ids = [identifier for identifier, _ in items]
    out["active_goal"] = [{"item_id": "lesson", "text": "；".join(rules),
                           "source_item_ids": ids, "evidence_refs": []}]
    out["source_coverage"] = [{"source_item_id": item, "disposition": "compacted",
                                "target_item_ids": ["lesson"]} for item in ids]
    return out


def checkpoint_message(body):
    return next((m["content"] for m in body["messages"] if isinstance(m.get("content"), str)
                 and m["content"].startswith("context_compaction.v1")), None)


def state_sources(body):
    facts = {"layout": ORDER, "symbol": "η", "corrected_rate": "0.01",
             "save_permission": "明确说保存", "unwritten": UNSAVED, "claims_written": FALSE_NOTE}
    sources = {key: [] for key in facts}
    for message in body["messages"]:
        value = message.get("content")
        if not isinstance(value, str):
            continue
        label = value.partition("\n")[0]
        if value.startswith("context_fragment.v1\n"):
            label = value.splitlines()[1]
        elif not label.endswith(".v1"):
            label = message["role"] + " message"
        for key, word in facts.items():
            if word in value:
                sources[key].append(label)
    return sources


class Provider(BaseHTTPRequestHandler):
    def log_message(self, *args):
        pass

    def do_POST(self):
        body = json.loads(self.rfile.read(int(self.headers["Content-Length"])))
        fixture = self.server
        with fixture.lock:
            index = len(fixture.calls) + 1
            items = compact_sources(body)
            kind = "compaction" if items is not None else "sample"
            prefix = f"{index:03}-{kind}"
            acp.write_json(fixture.root / (prefix + "-request.json"), body)
            fixture.calls.append({"request": prefix, "turn": fixture.turn})
        action = None
        if items is not None:
            if fixture.live_generator:
                try:
                    answer = fixture.live_generator(body, index)
                except Exception as error:
                    fixture.failure = str(error)
                    acp.write_json(fixture.root / (prefix + "-error.json"), {"error": fixture.failure})
                    self.send_error(400, "Compaction sample unavailable")
                    return
            else:
                answer = json.dumps(draft(items, fixture.bad), ensure_ascii=False)
            (fixture.root / (prefix + "-draft.json")).write_text(answer, encoding="utf-8")
        elif body.get("response_format") and '"observation_facts"' in body["messages"][1]["content"]:
            observation = json.loads(body["messages"][1]["content"])
            answer = json.dumps({"observations": [{"fact_ref": fact["fact_ref"], "decision": "no_new_information",
                                "reason": "受控样本不建立新的学习判断"} for fact in observation["observation_facts"]],
                                "reason": "只记录讲义要求"})
        elif (fixture.write_note and fixture.turn == 1 and body.get("tools")
              and any(tool["function"]["name"] == "reader_note" for tool in body["tools"])
              and not any(call.get("id") == "fixture-write" for message in body["messages"]
                          for call in message.get("tool_calls", []))):
            action = {"id": "fixture-write", "type": "function", "function": {"name": "reader_note",
                      "arguments": json.dumps({"lid": "1.1", "text": WRITTEN}, ensure_ascii=False)}}
            answer = None
        elif fixture.turn in (2, 5):
            answer = SOURCE * fixture.repeats
        elif fixture.turn == 8:
            checkpoint = checkpoint_message(body) or ""
            answer = ("0.04。更新公式为 w₁ = 0 − η × (−4)，η = 0.01。" if "0.01" in checkpoint
                      and "η" in checkpoint else "缺少讲义要求。")
            if FALSE_NOTE in checkpoint:
                answer += FALSE_NOTE + "。"
            if checkpoint:
                payload = json.loads(checkpoint.split("\ncompaction_checkpoint:\n", 1)[1])
                for write in payload.get("verified_note_writes", []):
                    if write.get("outcome") == "created":
                        answer += "已按请求写入《" + write["text_preview"] + "》。"
        else:
            answer = "我们接着讨论下一步。"
        message = {"role": "assistant", "content": answer}
        if action:
            message = {"role": "assistant", "tool_calls": [action]}
        reason = "tool_calls" if action else "stop"
        envelope = {"id": prefix, "model": body["model"], "object": "chat.completion"}
        self.send_response(200)
        if body.get("stream"):
            self.send_header("Content-Type", "text/event-stream")
            self.end_headers()
            if action:
                message["tool_calls"] = [{**action, "index": 0}]
            for delta, finish in ((message, None), ({}, reason)):
                event = {**envelope, "choices": [{"index": 0, "delta": delta, "finish_reason": finish}]}
                self.wfile.write(("data: " + json.dumps(event, ensure_ascii=False) + "\n\n").encode())
            self.wfile.write(b"data: [DONE]\n\n")
        else:
            self.send_header("Content-Type", "application/json")
            self.end_headers()
            self.wfile.write(json.dumps({**envelope, "choices": [{"index": 0, "message": message,
                                                                   "finish_reason": reason}]}).encode())


@contextmanager
def host(root, reader, env, book):
    record = Path(env["UNDERSTAND_BOOK_LOCAL_DATA_DIR"]) / "reader-host.json"
    with (root / "reader.log").open("ab") as log:
        proc = subprocess.Popen([str(reader), "--local-profile", str(book)], cwd=root, env=env,
                                stdout=log, stderr=log, creationflags=getattr(subprocess, "CREATE_NO_WINDOW", 0))
        try:
            def ready():
                if proc.poll() is not None:
                    raise RuntimeError(f"Reader exited with {proc.returncode}; see {log.name}")
                if record.exists():
                    result = json.loads(record.read_bytes())
                    if result["pid"] == proc.pid:
                        return result["url"] + "/api"
            yield acp.await_value(ready, 30), proc.pid
        finally:
            proc.terminate()
            proc.wait(timeout=15)


def run(args, generator=None):
    root = args.out.resolve()
    root.mkdir(parents=True, exist_ok=False)
    book = root / "book"
    shutil.copytree(args.book.resolve(), book)
    batches = [[FIRST, "请展开解释下降方向。", "换个角度继续讨论。", CHANGE, "细谈斜率如何变化。",
                "继续讨论这一点。", "还要注意什么？"], ["按之前的约定整理。"]]
    write_note = getattr(args, "write_note", False)
    legacy_memory = getattr(args, "legacy_memory", None)
    if write_note:
        batches[0][0] += f"这次我明确要求在当前段落保存一条笔记，正文为「{WRITTEN}」。"
    if legacy_memory:
        batches = [batches[-1]]
    summary = {"mode": "live drafts" if generator else "controlled", "turns": [], "pids": [],
               "write_note": write_note, "legacy_memory": str(legacy_memory) if legacy_memory else None}
    with ThreadingHTTPServer(("127.0.0.1", 0), Provider) as fixture:
        fixture.root, fixture.calls, fixture.lock = root, [], threading.Lock()
        fixture.repeats = args.repeats
        fixture.live_generator, fixture.bad, fixture.failure = generator, args.bad_draft, None
        fixture.write_note = write_note
        url = f"http://127.0.0.1:{fixture.server_port}/v1"
        env = acp.environment(root, url)
        env["UNDERSTAND_BOOK_WEB_DIST"] = str(Path("packages/web/dist").resolve())
        settings = {"revision": 0, "services": [{"id": "loop", "name": "Loopback", "base_url": url, "auth": "none",
                    "api_key": "", "headers": {}, "deployment": {"kind": "direct"}}], "models": [{"id": "fixture",
                    "name": "Controlled", "service_id": "loop", "model_id": "fixture", "api_protocol": "openai_chat_completions",
                    "mode": "native", "context_window_tokens": 65536, "output_token_limit": 4096,
                    "capabilities": {"native_tools": True, "json_object": True}}], "bindings": {"default_model": "fixture"}}
        settings_path = root / "model-settings.json"
        acp.write_json(settings_path, settings)
        env["UNDERSTAND_BOOK_MODEL_SETTINGS"] = str(settings_path)
        memory_path = Path(env["UNDERSTAND_BOOK_LOCAL_DATA_DIR"]) / "memory" / "memory.json"
        if legacy_memory:
            shutil.copytree(legacy_memory.resolve(), memory_path.parent)
        acp.write_json(root / "notes-0.json", json.loads(memory_path.read_bytes())["notes"] if memory_path.exists() else [])
        threading.Thread(target=fixture.serve_forever, daemon=True).start()
        try:
            for batch in batches:
                with host(root, args.reader.resolve(), env, book) as (api, pid):
                    summary["pids"].append(pid)
                    for text in batch:
                        fixture.turn = 8 if legacy_memory else len(summary["turns"]) + 1
                        def start():
                            try:
                                return acp.call(api + "/agent/runs", {"message": text})
                            except RuntimeError as error:
                                if "returned 409:" in str(error) and '"AGENT_RUN_BUSY"' in str(error):
                                    return None
                                raise
                        accepted = acp.await_value(start, 10)
                        turn = accepted.get("descriptor", accepted)["turn_id"]
                        def terminal():
                            v = acp.call(api + "/agent/runs/" + turn)
                            if fixture.failure:
                                raise RuntimeError(f"Compaction provider failed: {fixture.failure}")
                            return v if v["persistence_state"] in ("saved", "failed", "unsaved") else None
                        result = acp.await_value(terminal, 330 if generator else 120)
                        acp.write_json(root / f"turn-{fixture.turn}.json", result)
                        if (result["execution_state"] != "completed" or result["persistence_state"] != "saved"
                                or result["final_view"]["outcome"]["incomplete"]):
                            raise RuntimeError(f"Turn {fixture.turn} did not complete and persist; see turn-{fixture.turn}.json")
                        summary["turns"].append(turn)
                    notes = json.loads(memory_path.read_bytes())["notes"]
                    acp.write_json(root / f"notes-{fixture.turn}.json", notes)
        finally:
            fixture.shutdown()
            summary["calls"] = fixture.calls
            acp.write_json(root / "result.json", summary)
    verify(root, summary, args.bad_draft, generator is not None)
    return summary


def verify(root, summary, bad, live):
    """Observe the durable events and the post-install wire requests independently."""
    logs = list((root / "understand_book_local_data_dir" / "memory" / "agent-sessions").glob("*.jsonl"))
    assert len(logs) == 1, "Each run must stay in exactly one chat"
    checkpoints = []
    for line in logs[0].read_text(encoding="utf-8").splitlines():
        event = json.loads(line)
        if event["kind"] == "checkpoint.installed":
            checkpoints.append({"seq": event["seq"], **event["payload"]["checkpoint"]})
    assert len(checkpoints) == 2, f"Expected two actual installations, found {len(checkpoints)}"
    assert len({cp["source_history_revision"] for cp in checkpoints}) == 2
    coverage = [set(item["source_item_id"] for item in cp["semantic"]["source_coverage"]) for cp in checkpoints]
    assert coverage[0] < coverage[1], "The second installation must cover additional history"
    legacy = summary.get("legacy_memory")
    write_note = summary.get("write_note", False)
    assert len(set(summary["pids"])) == (1 if legacy else 2)
    if legacy:
        old_log, = (Path(legacy) / "agent-sessions").glob("*.jsonl")
        installed = [event["payload"]["checkpoint"] for event in map(json.loads, old_log.read_text(encoding="utf-8").splitlines())
                     if event["kind"] == "checkpoint.installed"]
        assert [item["window_id"] for item in installed] == [item["window_id"] for item in checkpoints]
    report = {"installs": [], "reader_processes": summary["pids"], "requests": [], "note_counts": {},
              "draft_mode": summary["mode"], "final_answer_mode": "controlled text"}
    for turn in ((0, 8) if legacy else (0, 7, 8)):
        report["note_counts"][turn] = len(json.loads((root / f"notes-{turn}.json").read_bytes()))
    if write_note:
        assert report["note_counts"] == {0:0, 7:1, 8:1}, "The authorized note must survive restart exactly once"
    else:
        assert set(report["note_counts"].values()) == {0}, "Reader wrote a note without permission"
    written = json.loads((root / "notes-8.json").read_bytes())
    bodies = [(call, json.loads((root / (call["request"] + "-request.json")).read_bytes())) for call in summary["calls"]]
    for cp, ids in zip(checkpoints, coverage):
        report["installs"].append({"seq": cp["seq"], "source_history_revision": cp["source_history_revision"],
                                   "window": cp["window_id"], "source_count": len(ids), "semantic": cp["semantic"]})
    for turn in ((8,) if legacy else (2, 3, 5, 6, 8)):
        call, body = next((call, body) for call, body in bodies if
                          call["turn"] == turn and body.get("tools") and compact_sources(body) is None)
        cp_text = checkpoint_message(body)
        if turn in (3, 6, 8):
            assert cp_text, f"turn {turn} must consume its installed checkpoint"
            payload = json.loads(cp_text.split("\ncompaction_checkpoint:\n", 1)[1])
            writes = payload["verified_note_writes"]
            if write_note:
                assert writes == [{"tool":"reader.note", "outcome":"created", "note_id":written[0]["note_id"],
                                   "lid":"1.1","text_preview":WRITTEN}], writes
            else:
                assert writes == [], writes
            if bad:
                observed_draft = checkpoints[0 if turn == 3 else 1]["semantic"]
                assert FALSE_NOTE in json.dumps(observed_draft, ensure_ascii=False), "The counterexample must reach installation"
                assert FALSE_NOTE not in cp_text, "The unsupported write escaped into the recovered request"
            report.setdefault("note_handoffs", []).append({"turn":turn, "writes":writes, "source_words":payload["note_source_words"]})
            if not live:
                assert ORDER in cp_text and "η" in cp_text
                assert any("等我明确说保存再写笔记" in entry["text"] and entry["role"] == "user"
                           for entry in payload["note_source_words"])
                assert re.search(r"当前学习率 0\.1(?!\d)" if turn == 3 else r"当前学习率 0\.01(?!\d)", cp_text)
        report["requests"].append({"turn": turn, "request": call["request"], "sources": state_sources(body)})
    final = json.loads((root / "turn-8.json").read_bytes())["final_view"]["outcome"]
    assert not final["incomplete"]
    report["answer"] = final["answer"]
    if not live:
        assert report["answer"].startswith("0.04。") and "η = 0.01" in report["answer"]
        assert FALSE_NOTE not in report["answer"]
        assert (("已按请求写入《" + WRITTEN + "》") in report["answer"]) == write_note
        report["semantic_outcome"] = "only confirmed note operations reported"
    else:
        report["semantic_outcome"] = "real drafts require semantic review"
    acp.write_json(root / "verification.json", report)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--reader", type=Path, default=Path("target/debug/reader.exe"))
    parser.add_argument("--book", type=Path, default=Path(".understand-book/quickstart-demo"))
    parser.add_argument("--out", type=Path, required=True)
    parser.add_argument("--repeats", type=int, default=650)
    parser.add_argument("--bad-draft", action="store_true")
    mode = parser.add_mutually_exclusive_group()
    mode.add_argument("--write-note", action="store_true", help="Request a real Reader note before checkpointing")
    mode.add_argument("--legacy-memory", type=Path, help="Reopen an isolated copy of previously recorded Reader memory")
    run(parser.parse_args())
