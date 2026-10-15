"""Local Whisper adapter. The Build Engine owns scheduling and admission."""
import json
import math
import sys
import time
from dataclasses import asdict
from importlib.metadata import version
from pathlib import Path


def serializable(value):
    if hasattr(value, "item"):
        value = value.item()
    if isinstance(value, float) and not math.isfinite(value):
        return str(value)
    if isinstance(value, dict):
        return {k: serializable(v) for k, v in value.items()}
    if isinstance(value, (list, tuple)):
        return [serializable(v) for v in value]
    return value


def main():
    from faster_whisper import WhisperModel
    request = json.loads(Path(sys.argv[1]).read_text(encoding="utf-8"))
    started = time.perf_counter()
    model = WhisperModel(request["model_dir"], device="cpu", compute_type="int8", cpu_threads=8)
    loaded = time.perf_counter()
    segments, info = model.transcribe(request["audio"], language="en", task="transcribe",
        beam_size=5, temperature=0, word_timestamps=True, condition_on_previous_text=False,
        vad_filter=True, vad_parameters={"min_silence_duration_ms": 500})
    raw = [asdict(s) for s in segments]
    result = {"engine": "faster-whisper", "engine_version": version("faster-whisper"),
        "model": "small.en", "language": "en", "segments": raw, "info": asdict(info),
        "load_ms": round((loaded-started)*1000), "inference_ms": round((time.perf_counter()-loaded)*1000)}
    target = Path(sys.argv[2])
    temporary = target.with_suffix(".tmp")
    temporary.write_text(json.dumps(serializable(result), ensure_ascii=False, allow_nan=False), encoding="utf-8")
    temporary.replace(target)


if __name__ == "__main__":
    main()
