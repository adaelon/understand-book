"""Compile the actual changed-condition teaching example; no Provider calls."""
import json
import re
import subprocess
import tempfile
from pathlib import Path

HERE = Path(__file__).resolve().parent
output = json.loads((HERE / "sol/repair/independent-step/output.json").read_text(encoding="utf-8-sig"))
source = next(s for s in re.findall(r"```rust\n(.*?)```", output["answer"], re.S) if "fn main()" in s)
with tempfile.TemporaryDirectory(prefix="tb3-content-", dir="D:/codex-build/tb1-temp") as tmp:
    path = Path(tmp) / "copy_shared.rs"
    path.write_text(source, encoding="utf-8")
    result = subprocess.run(["rustup", "run", "1.95.0", "rustc", str(path), "--error-format=json", "-o", str(Path(tmp) / "example.exe")], text=True, capture_output=True, encoding="utf-8")
    diagnostics = [json.loads(line) for line in result.stderr.splitlines() if line.startswith("{")]
    codes = [d["code"]["code"] for d in diagnostics if d.get("code")]
    record = {"source_from":"sol/repair/independent-step/output.json", "source":source,
              "rust_toolchain":"1.95.0", "compile_exit":result.returncode,"diagnostic_codes":codes}
    (HERE / "content-check.json").write_text(json.dumps(record, ensure_ascii=False, indent=2)+"\n", encoding="utf-8")
    assert result.returncode != 0 and "E0594" in codes, record
    print("Copy shared-reference write: E0594 confirmed")
