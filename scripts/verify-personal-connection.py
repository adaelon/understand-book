"""Exercise the installed personal CLI and stdio MCP against a disposable Reader profile."""
import argparse
from contextlib import contextmanager
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import threading
import time
import urllib.error
import urllib.request


@contextmanager
def mcp_session(command, env):
    with subprocess.Popen(command, env=env, text=True, encoding="utf-8",
                          stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                          creationflags=getattr(subprocess, "CREATE_NO_WINDOW", 0)) as mcp:
        deadline = threading.Timer(30, mcp.kill)
        deadline.start()
        def send(method, params, request_id=None):
            request = {"jsonrpc": "2.0", "method": method, "params": params}
            if request_id is not None:
                request["id"] = request_id
            mcp.stdin.write(json.dumps(request) + "\n")
            mcp.stdin.flush()
            if request_id is not None:
                response = json.loads(mcp.stdout.readline())
                assert response["id"] == request_id
                return response
        try:
            init = send("initialize", {"protocolVersion": "2025-06-18", "capabilities": {},
                        "clientInfo": {"name": "verify", "version": "1"}}, 1)["result"]
            send("notifications/initialized", {})
            yield send, init
        finally:
            mcp.stdin.close()
            mcp.wait(timeout=10)
            deadline.cancel()


def verify(cli, visitor, fixture, query, temp, pdf=None):
    profile = temp / "profile"
    config_home = temp / "original-home"
    config_home.mkdir()
    (config_home / "data-location.json").write_text(json.dumps(str(profile)), encoding="utf-8")
    book = profile / "library" / ".understand-book" / "demo"
    shutil.copytree(fixture, book)
    env = os.environ.copy()
    for name in ("UNDERSTAND_BOOK_READER_URL", "UNDERSTAND_BOOK_PERSONAL_TOKEN", "UNDERSTAND_BOOK_ADDR"):
        env.pop(name, None)
    env["UNDERSTAND_BOOK_LOCAL_DATA_DIR"] = str(config_home)
    hidden = {"creationflags": getattr(subprocess, "CREATE_NO_WINDOW", 0)}
    local = urllib.request.build_opener(urllib.request.ProxyHandler({}))

    def command(*args, data=None, success=True):
        proc = subprocess.run([cli, *args], env=env, text=True, encoding="utf-8",
                              input=data, capture_output=True, timeout=20, **hidden)
        if success:
            if proc.returncode:
                raise RuntimeError(f"{args[0]}: {proc.stderr}")
            return json.loads(proc.stdout)
        assert proc.returncode != 0, f"{args[0]} unexpectedly succeeded"
        return proc.stderr

    @contextmanager
    def host():
        with (temp / "reader.log").open("ab") as log:
            proc = subprocess.Popen([cli, "serve", book], env=env, stdin=subprocess.DEVNULL,
                                    stdout=log, stderr=log, **hidden)
            try:
                for _ in range(200):
                    if proc.poll() is not None:
                        raise RuntimeError((temp / "reader.log").read_text(encoding="utf-8"))
                    record = profile / "reader-host.json"
                    if record.is_file():
                        address = json.loads(record.read_bytes())
                        if address["pid"] == proc.pid:
                            break
                    time.sleep(0.1)
                else:
                    raise RuntimeError("Reader did not publish its listening address")
                def configure(**body):
                    request = urllib.request.Request(address["url"] + "/api/personal/connection",
                        data=json.dumps(body).encode(), headers={"Content-Type": "application/json"})
                    try:
                        with local.open(request, timeout=10) as response:
                            return json.load(response)
                    except urllib.error.HTTPError as error:
                        raise RuntimeError(f"Reader HTTP {error.code}: {error.read().decode()}") from error
                yield configure
            finally:
                proc.terminate()
                proc.wait(timeout=10)

    def tool(name, **args):
        return command("call", name, "-", data=json.dumps(args))

    with host() as configure:
        assert "already owns" in command("serve", book, success=False)
        token = configure(action="enable", write=True, navigate=True)["token"]
        assert command("connect", "--token-stdin", data=token)["connected"]
        assert (profile / "personal-cli.json").is_file() and not (config_home / "personal-cli.json").exists()
        materials = command("material", "list")["materials"]
        assert len(materials) == 1
        book_id = materials[0]["book_id"]
        assert command("paper", "metadata", book_id)["book_id"] == book_id
        search = command("book", "search", book_id, query)
        at = search["occurrences"][0]["ranges"][0]["source_ref"]
        text = command("book", "text", book_id, at["lid"])
        assert query in text["text"]
        assert text["source_ref"]["source_revision"] == at["source_revision"]
        ticket = tool("reader_goto", source_ref=at)
        assert ticket["status"] == "requested"
        assert tool("reader_navigation_result", id=ticket["id"])["displayed"] is None
        passage = json.dumps(at)
        note_args = ["note", "add", "--at", "-", "--text", "CLI source note", "--operation-id", "verify-note"]
        mark_args = ["highlight", "add", "--at", "-", "--operation-id", "verify-highlight"]
        note = command(*note_args, data=passage)
        highlight = command(*mark_args, data=passage)
        assert note["note_id"] == command(*note_args, data=passage)["note_id"]
        assert highlight["highlight_id"] == command(*mark_args, data=passage)["highlight_id"]
        question = {"source_ref": at, "text": "Which passage supports this question?", "operation_id": "verify-task"}
        task = tool("task_save", **question)
        assert task["task"]["external_question"]["note_id"] == task["note"]["note_id"]

    with host() as configure:
        notes = tool("note_list", book_id=book_id)["notes"]
        assert {item["note_id"] for item in notes} == {note["note_id"], task["note"]["note_id"]}
        saved_note = tool("note_read", book_id=book_id, note_id=note["note_id"])
        assert saved_note["value"]["record"]["content"] == "CLI source note"
        assert command(*note_args, data=passage)["note_id"] == note["note_id"]
        highlights = command("highlight", "list", book_id)["highlights"]
        assert len(highlights) == 1 and highlights[0]["highlight_id"] == highlight["highlight_id"]
        assert highlights[0]["source_ref"] == at and highlights[0]["text"] == query
        assert command(*mark_args, data=passage)["highlight_id"] == highlight["highlight_id"]
        assert tool("task_save", **question)["task"] == task["task"]
        tasks = command("task", "list", book_id)["tasks"]
        assert len(tasks) == 1 and tasks[0]["question"] == question["text"]
        with mcp_session([visitor, book], env) as (send, _):
            visitors = {t["name"] for t in send("tools/list", {}, 2)["result"]["tools"]}
            assert "book_text" in visitors and not visitors.intersection({"note_list", "task_list", "note_add"})
            public_text = send("tools/call", {"name":"book_text", "arguments":{"lid": at["lid"]}}, 3)["result"]
            assert not public_text["isError"] and query in public_text["content"][0]["text"]
            assert send("tools/call", {"name":"note_list", "arguments":{"book_id":book_id}}, 4)["result"]["isError"]
        with mcp_session([cli, "mcp"], env) as (send, init):
            tools = {t["name"] for t in send("tools/list", {}, 2)["result"]["tools"]}
            assert {"note_add", "task_save", "reader_goto", "book_search_text"} <= tools
            result = send("tools/call", {"name":"book_text", "arguments":{
                "book_id":book_id, "lid":at["lid"], "range":at["range"]}}, 3)["result"]
            assert result["structuredContent"]["text"] == query
            configure(action="revoke")
            assert "error" in send("tools/list", {}, 4)
            denied = send("tools/call", {"name":"book_text", "arguments":{
                "book_id":book_id, "lid":at["lid"]}}, 5)["result"]
            assert denied["isError"] and "structuredContent" not in denied
            command("material", "list", success=False)
            read_token = configure(action="enable")["token"]
            assert command("connect", "--token-stdin", data=read_token)["connected"]
            read_tools = {t["name"] for t in command("tools")["tools"]}
            assert "paper_metadata" in read_tools and "note_add" not in read_tools
            assert command("paper", "metadata", book_id)["book_id"] == book_id
            command(*note_args, data=passage, success=False)
            assert "error" in send("tools/list", {}, 6)
        paper_result = None
        if pdf:
            manage_token = configure(action="enable", write=True, manage=True)["token"]
            assert command("connect", "--token-stdin", data=manage_token)["connected"]
            imported = command("paper", "import", str(pdf))
            metadata = command("paper", "metadata", imported["book_id"])
            assert metadata["book_id"] == imported["book_id"]
            draft = Path(imported["dir"])
            assert (draft / "paper.pdf").stat().st_size == pdf.stat().st_size
            for submitted in imported["jobs"]:
                job_file = draft / ".build" / "jobs" / (submitted["job_id"] + ".json")
                deadline = time.monotonic() + 120
                while time.monotonic() < deadline:
                    job = json.loads(job_file.read_bytes())
                    if job["status"] == "done":
                        break
                    if job["status"] == "failed":
                        raise RuntimeError(str(job.get("failure_summary")))
                    time.sleep(0.3)
                else:
                    raise RuntimeError("Imported PDF did not finish source reconciliation")
            paper_result = {"book_id":metadata["book_id"], "copied_pdf_bytes":pdf.stat().st_size,
                            "readiness":imported["readiness"]["route"],
                            "preparation_jobs": len(imported["jobs"]),
                            "metadata_status":metadata.get("status", "projected")}
        configure(action="revoke")
        assert command("disconnect")["saved_token_removed"]
    return {"book_id":book_id, "source_lid":at["lid"], "selected_text":query,
            "passage_note_count_after_restart":len(notes), "task_count_after_restart":len(tasks),
            "highlight_count_after_restart":len(highlights), "highlight_retained_exact_range":True,
            "note_retry_same_id":True, "highlight_retry_same_id":True, "task_retry_same_reference":True,
            "second_host_excluded":True, "mcp_protocol":init["protocolVersion"], "mcp_tool_count":len(tools),
            "relocated_profile_discovered":True,
            "headless_navigation_waits_for_display": True,
            "visitor_tool_count":len(visitors), "visitor_reads_book_only":True,
            "revoked_live_mcp_denied":True, "revoked_cli_denied":True,
            "readonly_metadata_allowed": True, "readonly_write_denied": True,
            **({"paper_import": paper_result} if paper_result else {})}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cli", type=Path, required=True)
    parser.add_argument("--visitor", type=Path, help="Visitor executable; defaults to book_mcp beside --cli")
    parser.add_argument("--book", type=Path, default=Path(".understand-book/quickstart-demo"))
    parser.add_argument("--query", default="知识图谱")
    parser.add_argument("--pdf", type=Path, help="Also import a real PDF and read its metadata using the returned ID")
    parser.add_argument("--out", type=Path, help="Optional JSON evidence file")
    args = parser.parse_args()
    with tempfile.TemporaryDirectory(prefix="ub-personal-") as temp:
        cli = args.cli.resolve()
        visitor = args.visitor.resolve() if args.visitor else cli.with_name("book_mcp" + cli.suffix)
        result = verify(cli, visitor, args.book.resolve(), args.query, Path(temp), args.pdf.resolve() if args.pdf else None)
    report = json.dumps(result, ensure_ascii=False, indent=2) + "\n"
    if args.out:
        args.out.parent.mkdir(parents=True, exist_ok=True)
        args.out.write_text(report, encoding="utf-8")
    print(report, end="")


if __name__ == "__main__":
    main()
