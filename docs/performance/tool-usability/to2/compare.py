"""Validate examples in final captures and measure their incremental input cost."""
import json
from pathlib import Path
from jsonschema import Draft7Validator

root = Path(__file__).resolve().parent
before = json.loads((root / "before.json").read_text(encoding="utf-8"))
after = json.loads((root / "after.json").read_text(encoding="utf-8"))


def size(value):
    return len(json.dumps(value, ensure_ascii=False, separators=(",", ":")).encode("utf-8"))


rows = []
for surface, old_tools in before.items():
    new_tools = after[surface]
    assert [t["name"] for t in old_tools] == [t["name"] for t in new_tools], surface
    for old, new in zip(old_tools, new_tools):
        name = new["name"]
        schema = new["inputSchema"]
        without_examples = {k: v for k, v in schema.items() if k != "examples"}
        assert old["inputSchema"] == without_examples, (surface, name, "input contract changed")
        assert old["description"] == new["description"], (surface, name, "description changed")
        examples = schema.get("examples", [])
        validator = Draft7Validator(schema)
        Draft7Validator.check_schema(schema)
        for i, example in enumerate(examples):
            validator.validate(example)
            assert example not in examples[:i], (surface, name, "duplicate example")
            if surface == "remote_mcp":
                assert "published_book_ref" in example, name
            else:
                assert "published_book_ref" not in example, name
        if name in ("book.text", "book_text"):
            broken = dict(schema, properties={k: v for k, v in schema["properties"].items() if k != "end_lid"})
            assert not Draft7Validator(broken).is_valid(examples[0]), (surface, "dropped field undetected")
        if examples and surface == "remote_mcp":
            missing_ref = {k: v for k, v in examples[0].items() if k != "published_book_ref"}
            assert not validator.is_valid(missing_ref), name
        rows.append(dict(surface=surface, tool=name, examples=len(examples),
                         schema_before_bytes=size(old["inputSchema"]), schema_after_bytes=size(schema),
                         description_bytes=len(new["description"].encode("utf-8"))))

totals = []
for surface in before:
    selected = [r for r in rows if r["surface"] == surface]
    totals.append(dict(surface=surface, tools=len(selected), **{
        k: sum(r[k] for r in selected) for k in ("examples", "schema_before_bytes", "schema_after_bytes", "description_bytes")
    }))
(root / "sizes.json").write_text(json.dumps(dict(totals=totals, tools=rows), ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
print(json.dumps(dict(checks="all final schemas/examples valid; duplicate, dropped field and missing publication detected; constraints, descriptions and tool inventory unchanged", totals=totals), ensure_ascii=False, indent=2))
