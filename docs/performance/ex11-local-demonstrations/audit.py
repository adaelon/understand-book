"""Audit recorded production requests and untouched model calls for one EX11 run."""
import json
import sys
from pathlib import Path

root = Path(sys.argv[1])
marker = "Konva JavaScript Framework v10.7.0"
requests = []
calls = []
source_chunks = []
tokens = 0
usage_requests = set()
for path in sorted(root.glob("request-*.json")):
    text = path.read_text(encoding="utf-8")
    request = json.loads(text)
    for message in request.get("messages", []):
        if message.get("role") != "Tool":
            continue
        try:
            body = json.loads(message.get("content") or "{}").get("model_body") or {}
        except (ValueError, TypeError):
            continue
        if body.get("status") == "version_read" and isinstance(body.get("text"), str):
            end = body["offset"] + len(body["text"])
            source_chunks.append({"request":path.name,"offset":body["offset"],"characters":len(body["text"]),
                "next_offset":body.get("next_offset"),"contiguous":body.get("next_offset") == (end if end < body["total_characters"] else None)})
    requests.append({
        "file": path.name,
        "library_source_present": marker in text,
        "method": [x for x in request["instruction_assets"]
                   if x["asset_id"] == "resident-agent.skill.presentation-method"],
        "guidance_present": all(x in request["instructions"] for x in
                                ["hitFunc", "getRelativePointerPosition", "zero magnitude", "commitState()"]),
        "request_bytes": path.stat().st_size,
    })
for path in sorted(root.glob("response-*.json")):
    response = json.loads(path.read_text(encoding="utf-8"))
    tokens += response.get("usage") or 0
    if response.get("usage") is not None:
        usage_requests.add(path.name.replace("response-", "request-"))
    for call in response["tool_calls"]:
        if call["name"] != "presentation.author":
            continue
        args = json.loads(call["arguments"])
        calls.append({"file": path.name, "operation": args.get("operation"),
                      "libraries": args.get("libraries"), "based_on": args.get("based_on"),
                      "read_file": args.get("file"), "raw_bytes": len(call["arguments"].encode()),
                      "library_source_present": marker in call["arguments"]})
        if "html" in args:
            # Preserve exactly the model's page; no fixes or resource expansion.
            (root / f"{path.stem}-raw.html").write_text(args["html"], encoding="utf-8", newline="")
missing_usage = [r["file"] for r in requests if r["file"] not in usage_requests]
report = {"source_chunks":source_chunks,"source_continuations_contiguous":all(x["contiguous"] for x in source_chunks),"requests": requests, "calls": calls, "provider_reported_tokens": tokens,
          "provider_total_tokens": tokens if not missing_usage else None, "requests_without_usage": missing_usage,
          "library_source_absent": not any(x["library_source_present"] for x in requests + calls)}
if (root / "content.json").exists():
    content = json.loads((root / "content.json").read_text(encoding="utf-8"))
    report["delivered"] = {"entrypoint_bytes": len(content["content_files"][content["entrypoint"]].encode()),
                           "version_files": {k: len(v.encode()) for k, v in content["content_files"].items()},
                           "managed_references": content["content_files"][content["entrypoint"]].count("data-presentation-library")}
(root / "audit.json").write_text(json.dumps(report, ensure_ascii=False, indent=2), encoding="utf-8")
print(json.dumps({"requests": len(requests), "calls": len(calls), "reported_tokens": tokens, "requests_without_usage": missing_usage,
                  "library_source_absent": report["library_source_absent"],
                  "delivered": report.get("delivered")}, ensure_ascii=False))
