"""Keep model output; repair demonstrated restore and source-button defects."""
import json
from pathlib import Path

root = Path(__file__).resolve().parent
output = json.loads((root / "sol/author/output.json").read_text(encoding="utf-8"))
old = "if(typeof saved.values?.[k]==='boolean')s[k]=saved.values[k]"
new = "if(typeof saved.values?.page?.[k]==='boolean')s[k]=saved.values.page[k]"
assert output["write"]["html"].count(old) == 1
output["write"]["html"] = output["write"]["html"].replace(old, new)
style_old = "button{background:var(--accent);color:white}"
style_new = "button:not([data-source-ref]){background:var(--accent);color:white}"
assert output["write"]["html"].count(style_old) == 1
output["write"]["html"] = output["write"]["html"].replace(style_old, style_new)
(root / "artifact.json").write_text(json.dumps(output, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
(root / "page-repair.json").write_text(json.dumps({
    "origin": "root agent deterministic repair; no additional model experiment",
    "failure": "content-original.json state_restore.passed=false",
    "changed_field": "write.html", "old_text": old, "new_text": new,
    "style_repair": {
        "failure": "browser-before-source-color-fix/followup-desktop.png: authored button background equals host source-label foreground",
        "old_text": style_old, "new_text": style_new,
    },
    "teaching_text_changed": False,
}, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
