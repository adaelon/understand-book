"""Summarize retained TC3 runs without calling a provider."""
import json
from pathlib import Path

ROOT = Path(__file__).parent


def save(path, value):
    path.write_text(json.dumps(value, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")


groups = {}
for group in ("live", "live-v2", "live-v3"):
    rows = []
    for path in sorted((ROOT / group).glob("*/result.json")):
        result = json.loads(path.read_text(encoding="utf-8"))
        reply = result["reply"]
        errors = []
        for step in reply.get("trace", []):
            try:
                body = json.loads(step.get("result_digest", ""))
            except (ValueError, TypeError):
                continue
            if "error_code" in body:
                errors.append({"tool": step["tool"], "loop": step.get("model_tool_loop"), **body})
        events = result["events"]
        offers = [a["continuation"] for a in result["public_projection"]["activities"] if a.get("continuation")]
        (path.parent / "answer.md").write_text(reply.get("answer") or "", encoding="utf-8")
        save(path.parent / "public-continuations.json", offers)
        rows.append({"scenario": path.parent.name, "http_status": result["http_status"],
            "incomplete": reply.get("incomplete"), "warning": reply.get("warning"),
            "provider_calls": result["provider_calls"], "read_provider_calls": result["read_provider_calls"],
            "tokens_spent": reply.get("tokens_spent"), "errors": errors,
            "selected": sum(e["kind"] == "move_selected" for e in events),
            "delivered": sum(e["kind"] == "message_delivered" for e in events),
            "offered_sets": len(offers), "offered_options": sum(len(o["continuation"]["options"]) for o in offers)})
    if rows:
        groups[group] = rows
save(ROOT / "summary.json", groups)
print(json.dumps(groups, ensure_ascii=False, indent=2))
