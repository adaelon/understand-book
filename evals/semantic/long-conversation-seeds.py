"""Create two isolated seven-turn Reader histories, using a local scripted teacher.

Both arms can read the same book. In the long arm the reader attaches source
material from earlier discussions. The manifest identifies every excerpt;
these are controlled replay histories, not a natural eight-turn observation.
"""
import argparse
import json
import re
import runpy
import shutil
import threading
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
helpers = runpy.run_path(str(ROOT / "scripts/verify-long-conversation-compaction.py"))
acp, host = helpers["acp"], helpers["host"]
QUESTIONS = runpy.run_path(str(ROOT / "scripts/verify-long-conversation-acp.py"))["QUESTIONS"][:7]
READING_TURNS = (2, 3, 5, 6, 7)
SOURCE = ROOT / "output/spaces-llm/科学空间-大模型文章合集.md"
PREVIOUS = Path("docs/performance/ex11-local-demonstrations/ex11.7/batch1/runs/learning-1/history.json")
EXAMPLE = "一维学习率例子。在实数轴上取损失 L(w)=(w-2)^2，起点 w=0，每一步沿导数的反方向移动。"
NEUTRAL = "收到。我们可以接着讨论。"


def earlier_discussion():
    """Only the public question, its explanation and a typed read-only receipt."""
    session = json.loads((ROOT / PREVIOUS).read_bytes())["sessions"][0]
    turn = session["turns"][0]
    question = turn["user"].split("？", 1)[0]+"？"
    answer = re.search(r"^## 为什么[^\n]+\n(.*?)(?=\n## )", turn["outcome"]["answer"], re.M | re.S)
    if not answer:
        raise ValueError("Historical explanation section missing")
    for message in session["messages"]:
        if message["role"] != "Tool":
            continue
        receipt = json.loads(message["content"])
        if receipt.get("version") == "tool_search_history.v1":
            return {"question_excerpt": question, "answer_excerpt": answer[1].strip(),
                    "discovery_receipt": {key:receipt[key] for key in ("version", "activation_status", "matched_tools")}}
    raise ValueError("Historical discovery receipt missing")


def article_groups(path):
    text = path.read_text(encoding="utf-8-sig")
    begin = text.index("\n## 第4部分：")
    end = text.index("\n## 第5部分：", begin)
    contents = text[begin:end]
    start = list(re.finditer(r"^### ([^\n]+)\n\n> 作者：", contents, re.M))
    groups = [[] for _ in READING_TURNS]
    size = 0
    for index, match in enumerate(start):
        title = match[1]
        if not re.search("梯度|优化|学习率|动量|Adam|AdaFactor|SGD|训练稳定", title, re.I):
            continue
        stop = start[index + 1].start() if index + 1 < len(start) else len(contents)
        passage = contents[match.start():stop].strip()
        url = re.search(r"\[原文\]\((https?://[^)]+)\)", passage).group(1)
        group = min(groups, key=lambda values: sum(len(item[2]) for item in values))
        group.append((title, url, passage))
        size += len(passage)
        if size >= 330000:
            break
    if not all(groups):
        raise ValueError("Archive lacks the optimization discussions")
    return groups


def make_book(book, groups):
    book.mkdir()
    nodes = []
    texts = []
    offset = 0
    def leaf(text):
        nonlocal offset
        number = len(nodes) + 1
        begin = offset
        offset += len(text.encode("utf-16-le")) // 2
        lid = f"1.{number}"
        nodes.append({"lid": lid, "kind": "paragraph", "path": [1, number],
                      "children": [], "span": {"start": begin, "end": offset}})
        texts.append(text)
        offset += 1
        return lid
    leaf(EXAMPLE)
    manifest = []
    for turn, group in zip(READING_TURNS, groups):
        entries = []
        for title, url, passage in group:
            lids = [leaf(passage[i:i+3000]) for i in range(0, len(passage), 3000)]
            entries.append({"title": title, "url": url, "lids": lids, "characters": len(passage)})
        manifest.append({"turn": turn, "articles": entries})
    source = "\n".join(texts)
    book_id = "long-conversation-learning-rate"
    parent = {"lid": "1", "path": [1], "kind": "chapter", "children": [node["lid"] for node in nodes],
              "span": {"start": 0, "end": offset-1}}
    acp.write_json(book / "base.json", {"book_id": book_id, "lid_nodes": [parent]+nodes,
                                       "graph_nodes": [], "graph_edges": []})
    profile = json.loads((ROOT / ".understand-book/quickstart-demo/profile_metadata.json").read_bytes())
    profile["header"]["book_id"] = book_id
    acp.write_json(book / "profile_metadata.json", profile)
    (book / "source.txt").write_text(source, encoding="utf-8", newline="\n")
    return manifest


class Teacher(BaseHTTPRequestHandler):
    def log_message(self, *args):
        pass

    def do_POST(self):
        body = json.loads(self.rfile.read(int(self.headers["Content-Length"])))
        # Quoted source may contain verbs such as "remember". The seed reader
        # requested discussion; a parser invocation must preserve that intent.
        system = body["messages"][0].get("content", "")
        answer = '{"intent":"none"}' if "You extract one explicit" in system else NEUTRAL
        message = {"role": "assistant", "content": answer}
        self.server.calls += 1
        frame = {"id": "fixture", "model": body["model"], "object": "chat.completion"}
        self.send_response(200)
        if body.get("stream"):
            self.send_header("Content-Type", "text/event-stream")
            self.end_headers()
            for delta, reason in ((message, None), ({}, "stop")):
                data = {**frame, "choices": [{"index": 0, "delta": delta, "finish_reason": reason}]}
                self.wfile.write(("data: "+json.dumps(data)+"\n\n").encode())
            self.wfile.write(b"data: [DONE]\n\n")
        else:
            self.send_header("Content-Type", "application/json")
            self.end_headers()
            self.wfile.write(json.dumps({**frame, "choices": [{"index": 0, "message": message,
                                                               "finish_reason": "stop"}]}).encode())


def run_arm(root, arm, reader, book, manifest, discussion, originals):
    work = root / (arm+"-host")
    work.mkdir()
    turns, questions = [], []
    with ThreadingHTTPServer(("127.0.0.1", 0), Teacher) as teacher:
        teacher.calls = 0
        url = f"http://127.0.0.1:{teacher.server_port}/v1"
        env = acp.environment(work, url)
        env["UNDERSTAND_BOOK_WEB_DIST"] = str(ROOT / "packages/web/dist")
        config = {"revision": 0, "services": [{"id": "loop", "name": "Fixture", "base_url": url,
                  "auth": "none", "api_key": "", "headers": {}, "deployment": {"kind": "direct"}}],
                  "models": [{"id": "fixture", "name": "Fixture", "service_id": "loop", "model_id": "fixture",
                  "api_protocol": "openai_chat_completions", "mode": "native", "context_window_tokens": 1000000,
                  "output_token_limit": 4096, "capabilities": {"native_tools": True, "json_object": True}}],
                  "bindings": {"default_model": "fixture"}}
        acp.write_json(work / "model-settings.json", config)
        env["UNDERSTAND_BOOK_MODEL_SETTINGS"] = str(work / "model-settings.json")
        threading.Thread(target=teacher.serve_forever, daemon=True).start()
        try:
            with host(work, reader, env, book) as (api, pid):
                for index, question in enumerate(QUESTIONS, 1):
                    reads = []
                    if index == 3:
                        question += ("\n附上另一轮学习率讨论的原话及当时的工具发现回执，"
                                     "供对照当时的解释。\n"+json.dumps(discussion, ensure_ascii=False))
                    if index in READING_TURNS:
                        articles = manifest[READING_TURNS.index(index)]["articles"]
                        question += "\n相关阅读讨论：" + "、".join(article["title"] for article in articles)
                        if arm == "long":
                            question += "\n附我从同一书库读到的这些文章原文，作为相关阅读记录。\n"
                            for article in articles:
                                parts = []
                                for lid in article["lids"]:
                                    read = acp.call(api + "/book/text?lid=" + lid)
                                    parts.append(read["text"])
                                    reads.append({"lid": lid, "characters": len(read["text"])})
                                passage = "".join(parts)
                                if passage != originals[article["url"]]:
                                    raise RuntimeError(f"Reader source differs: {article['title']}")
                                question += passage + "\n"
                    accepted = acp.call(api + "/agent/runs", {"message": question})
                    desc = accepted.get("descriptor", accepted)
                    def terminal():
                        value = acp.call(api+"/agent/runs/"+desc["turn_id"])
                        return value if value["persistence_state"] in ("saved", "failed", "unsaved") else None
                    done = acp.await_value(terminal, 90)
                    if done["execution_state"] != "completed" or done["final_view"]["outcome"]["answer"] != NEUTRAL:
                        acp.write_json(work / f"failed-turn-{index}.json", done)
                        raise RuntimeError(f"Seed turn {index}: {done['final_view'].get('error')}")
                    turns.append({"turn": index, "id": desc["turn_id"], "chat": desc["session_id"],
                                  "question_characters": len(question), "reader_reads": reads})
                    questions.append(question)
            with host(work, reader, env, book) as (api, reopen_pid):
                history = acp.call(api+"/agent/history")
                saved = history["current"]
                if (saved["id"] != turns[0]["chat"] or [t["user"] for t in saved["turns"]] != questions
                        or any(t["status"] != "completed" for t in saved["turns"])):
                    raise ValueError("Reopened Reader did not recover the same seven submitted turns")
        finally:
            teacher.shutdown()
    memory = Path(env["UNDERSTAND_BOOK_LOCAL_DATA_DIR"]) / "memory"
    notes = json.loads((memory / "memory.json").read_bytes())["notes"]
    if notes or len(set(t["chat"] for t in turns)) != 1:
        raise ValueError("Seed state changed book, chat, or notes")
    shutil.copytree(memory, root / (arm+"-memory"), ignore=shutil.ignore_patterns("*.lock"))
    acp.write_json(root / (arm+".json"), {"arm": arm, "turns": turns, "initial_notes": len(notes),
                                         "controlled_teacher_calls": teacher.calls, "paid_calls": 0,
                                         "restored_questions_verified": len(questions), "reader_pids": [pid, reopen_pid]})


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--reader", type=Path, default=ROOT / "target/debug/reader.exe")
    parser.add_argument("--archive", type=Path, default=SOURCE)
    parser.add_argument("--out", required=True, type=Path)
    args = parser.parse_args()
    root = args.out.resolve()
    root.mkdir(parents=True, exist_ok=False)
    book = root / "book"
    groups = article_groups(args.archive)
    manifest = make_book(book, groups)
    originals = {url: passage for group in groups for _, url, passage in group}
    discussion = earlier_discussion()
    acp.write_json(root / "sources.json", {"archive": str(args.archive), "groups": manifest,
                  "shared_historical_discussion": {"source": PREVIOUS.as_posix(), **discussion}})
    for arm in ("short", "long"):
        run_arm(root, arm, args.reader.resolve(), book, manifest, discussion, originals)
        print(root / (arm+"-memory"), flush=True)
