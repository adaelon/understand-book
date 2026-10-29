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


def load_model(model_dir):
    from faster_whisper import WhisperModel
    started = time.perf_counter()
    model = WhisperModel(model_dir, device="cpu", compute_type="int8", cpu_threads=8)
    return model, round((time.perf_counter()-started)*1000)


def recognize(model, audio, output, load_ms):
    started = time.perf_counter()
    segments, info = model.transcribe(audio, language="en", task="transcribe",
        beam_size=5, temperature=0, word_timestamps=True, condition_on_previous_text=False,
        vad_filter=True, vad_parameters={"min_silence_duration_ms": 500})
    raw = [asdict(s) for s in segments]
    result = {"engine": "faster-whisper", "engine_version": version("faster-whisper"),
        "model": "small.en", "language": "en", "segments": raw, "info": asdict(info),
        "load_ms": load_ms, "inference_ms": round((time.perf_counter()-started)*1000)}
    target = Path(output)
    temporary = target.with_suffix(".tmp")
    temporary.write_text(json.dumps(serializable(result), ensure_ascii=False, allow_nan=False), encoding="utf-8")
    temporary.replace(target)


def main():
    if sys.argv[1] == "--resident":
        model, load_ms = load_model(sys.argv[2])
        for line in sys.stdin:
            try:
                request = json.loads(line)
                recognize(model, request["audio"], request["output"], load_ms)
                response = {"ok": True}
            except Exception as error:
                response = {"ok": False, "error": str(error)}
            load_ms = 0
            print(json.dumps(response, ensure_ascii=True), flush=True)
    else:
        # The recovery evaluation also invokes the adapter for one saved request.
        request = json.loads(Path(sys.argv[1]).read_text(encoding="utf-8"))
        model, load_ms = load_model(request["model_dir"])
        recognize(model, request["audio"], sys.argv[2], load_ms)


if __name__ == "__main__":
    main()
