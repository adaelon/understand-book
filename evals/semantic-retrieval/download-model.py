"""Download the public pinned ONNX model; inference is performed offline by spike.mjs."""
import json
import pathlib
import shutil
import sys
import urllib.request

root = pathlib.Path(sys.argv[1])
model = "Xenova/paraphrase-multilingual-MiniLM-L12-v2"
if len(sys.argv) > 2:
    revision = sys.argv[2]
else:
    with urllib.request.urlopen(f"https://huggingface.co/api/models/{model}", timeout=30) as response:
        revision = json.load(response)["sha"]
root.mkdir(parents=True, exist_ok=True)
(root / "model-metadata.json").write_text(json.dumps({"model": model, "revision": revision}, indent=2), encoding="utf-8")
for name in ["config.json", "tokenizer_config.json", "special_tokens_map.json", "tokenizer.json", "onnx/model_quantized.onnx"]:
    target = root / name
    target.parent.mkdir(parents=True, exist_ok=True)
    # A revision-specific directory supplied by the caller can be reused for offline runs.
    with urllib.request.urlopen(f"https://huggingface.co/{model}/resolve/{revision}/{name}", timeout=120) as response, target.open("wb") as output:
        shutil.copyfileobj(response, output)
    print(name, target.stat().st_size, flush=True)
print("revision", revision, flush=True)
