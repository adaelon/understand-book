"""Check the frozen excerpt and executable content, without a model call."""
import json
import pathlib
import re
import subprocess
import tempfile

ROOT = pathlib.Path(__file__).resolve().parent


def main():
    source = (ROOT / "source.txt").read_text(encoding="utf-8")
    material = json.loads((ROOT / "material.json").read_text(encoding="utf-8"))
    original = pathlib.Path(material["source_path"])
    original_check = "not available; use committed excerpt"
    if original.exists():
        raw = original.read_bytes().decode("utf-8").encode("utf-16-le")
        bounds = material["original_utf16_range"]
        assert raw[bounds["start"] * 2:bounds["end"] * 2].decode("utf-16-le") == source
        original_check = "exact UTF-16 source slice matches"
    code = re.search(r"<code>(// borrowing_basics.rs.*?)</code>", source, re.S).group(1)
    cases = [
        ("original", code, None, "Foo is Foo(0)\nBar is Foo(0)\n"),
        ("move_without_ampersand", code.replace("let bar = &foo;", "let bar = foo;"), "E0382", None),
        ("write_through_shared_reference", code.replace("let bar = &foo;", "let bar = &foo;\n    bar.0 = 1;"), "E0594", None),
    ]
    results = []
    with tempfile.TemporaryDirectory(prefix="tb1-rust-") as directory:
        temp = pathlib.Path(directory)
        for name, text, error, stdout in cases:
            path = temp / f"{name}.rs"
            binary = temp / f"{name}.exe"
            path.write_text(text, encoding="utf-8")
            compiled = subprocess.run(["rustc", "+1.95.0", "--edition=2021", "--error-format=json", str(path), "-o", str(binary)], capture_output=True, text=True, encoding="utf-8")
            diagnostics = [json.loads(line) for line in compiled.stderr.splitlines() if line.startswith("{")]
            codes = [d["code"]["code"] for d in diagnostics if d.get("code")]
            actual_stdout = None
            if error:
                assert compiled.returncode != 0 and error in codes, (name, compiled.stderr)
            else:
                assert compiled.returncode == 0, compiled.stderr
                ran = subprocess.run([str(binary)], capture_output=True, text=True, encoding="utf-8", check=True)
                actual_stdout = ran.stdout
                assert actual_stdout == stdout, actual_stdout
            results.append({"case":name,"source":text,"compile_exit":compiled.returncode,"diagnostic_codes":codes,"stdout":actual_stdout})
    output = {"original_material":original_check,"rustc":subprocess.check_output(["rustc","+1.95.0","--version"], text=True).strip(),"cases":results}
    (ROOT / "content-check.json").write_text(json.dumps(output,ensure_ascii=False,indent=2)+"\n",encoding="utf-8")
    print(json.dumps({"original_material":original_check,"content_cases_passed":len(results)},ensure_ascii=False))


if __name__ == "__main__":
    main()
