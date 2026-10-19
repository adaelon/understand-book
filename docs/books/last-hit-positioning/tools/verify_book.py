"""检查本书的相对链接、代码摘录和用于正文的舍入结果。"""
from pathlib import Path
import json
import re
from urllib.parse import unquote

ROOT = Path(__file__).resolve().parents[1]
failures = []
links = 0
for source in [ROOT / "README.md", ROOT / "OUTLINE.md", ROOT / "SOURCES.md", ROOT / "book.md",
               *sorted((ROOT / "chapters").glob("*.md")), *sorted((ROOT / "appendices").glob("*.md"))]:
    content = source.read_text(encoding="utf-8")
    prose = re.sub(r"```[^\n]*\n.*?\n```", "", content, flags=re.S)
    for link in re.findall(r"!?\[[^\]]*\]\(([^)]+)\)", prose):
        if re.match(r"[a-zA-Z]+:", link) or link.startswith("#"):
            continue
        links += 1
        target = source.parent / unquote(link.split("#")[0])
        if not target.exists():
            failures.append(f"Missing link in {source.name}: {link}")

for appendix, script in [("B-计算接口与案例.md", "quickstart.py"), ("D-参考实现.md", "standalone_model.py")]:
    content = (ROOT / "appendices" / appendix).read_text(encoding="utf-8")
    block = re.search(r"```python\n(.*?)\n```", content, flags=re.S)
    if not block or block.group(1).strip() != (ROOT / "examples" / script).read_text(encoding="utf-8").strip():
        failures.append(f"Code excerpt differs: {appendix}")

data = json.loads((ROOT / "examples/results.json").read_text(encoding="utf-8"))
for name, expected in {"W1":"9.337409", "W2":"5.617315", "W3":"7.817081", "W4":"7.595921"}.items():
    if f"{data['window'][name]['result']['dist_ready']:.6f}" != expected:
        failures.append(f"Appendix B value changed: {name}")
if f"{data['points']['P']['enemy_distance']:.3f}" != "6.888":
    failures.append("Appendix B P distance changed")

parts = []
for directory in ("chapters", "appendices"):
    for source in sorted((ROOT / directory).glob("*.md")):
        lines = source.read_text(encoding="utf-8").splitlines()
        fence = False
        new = []
        for line in lines:
            if line.startswith("```"):
                fence = not fence
            if not fence and re.match(r"^#{1,5} ", line):
                line = "#" + line
            new.append(line)
        parts.append("\n".join(new).strip().replace("](../", "]("))
expected = "# 这一刀怎么补\n\n看懂对线站位与出手机会\n\n" + "\n\n".join(parts) + "\n"
if expected != (ROOT / "book.md").read_text(encoding="utf-8"):
    failures.append("Combined manuscript is stale")

result = {"relative_links":links,"code_excerpts":2,"numeric_rows":5,
          "combined_parts":len(parts),"failures":failures}
print(json.dumps(result, ensure_ascii=False, indent=2))
if failures:
    raise SystemExit(1)
