"""Compare the two actual tool-definition captures; no model or network calls."""
import json
from pathlib import Path

root = Path(__file__).resolve().parent
before = json.loads((root / "before.json").read_text(encoding="utf-8"))
after = json.loads((root / "after.json").read_text(encoding="utf-8"))


def schema_bytes(value):
    return len(json.dumps(value, ensure_ascii=False, separators=(",", ":")).encode("utf-8"))


def constraints(value):
    if isinstance(value, dict):
        return {k: constraints(v) for k, v in value.items() if k != "description"}
    if isinstance(value, list):
        return [constraints(v) for v in value]
    return value


rows = []
for surface, old_tools in before.items():
    new_tools = {tool["name"]: tool for tool in after[surface]}
    assert [tool["name"] for tool in old_tools] == [tool["name"] for tool in after[surface]], surface
    for old in old_tools:
        name = old["name"]
        new = new_tools[name]
        old_constraints = constraints(old["inputSchema"])
        new_constraints = constraints(new["inputSchema"])
        if name in ("book.context", "book_context"):
            assert new_constraints["properties"]["granularity"]["default"] == "near"
            assert new_constraints["properties"]["k"]["default"] == 10
            del new_constraints["properties"]["granularity"]["default"]
            del new_constraints["properties"]["k"]["default"]
        assert old_constraints == new_constraints, (surface, name, "input constraints changed")
        if not name.startswith(("book.", "book_")) or name == "book_list":
            assert old == new, (surface, name, "unrelated tool changed")
        if surface == "remote_mcp":
            assert old["description"] == new["description"], (name, "remote transport guidance changed")
            for field in ("published_book_ref", "start_utf16", "page"):
                assert old["inputSchema"].get("properties", {}).get(field) == new["inputSchema"].get("properties", {}).get(field), (name, field)
        rows.append({
            "surface": surface,
            "tool": name,
            "schema_before_bytes": schema_bytes(old["inputSchema"]),
            "schema_after_bytes": schema_bytes(new["inputSchema"]),
            "description_before_bytes": len(old["description"].encode("utf-8")),
            "description_after_bytes": len(new["description"].encode("utf-8")),
        })

summary = []
for surface in before:
    selected = [row for row in rows if row["surface"] == surface]
    summary.append({"surface": surface, "tools": len(selected), **{
        key: sum(row[key] for row in selected)
        for key in ("schema_before_bytes", "schema_after_bytes", "description_before_bytes", "description_after_bytes")
    }})
(root / "sizes.json").write_text(json.dumps({"totals": summary, "tools": rows}, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
print(json.dumps({"checks": "tool order, input constraints, remote fields/guidance, unrelated tools unchanged", "totals": summary}, ensure_ascii=False, indent=2))
