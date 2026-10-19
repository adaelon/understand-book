"""VI0b local ASR experiment; raw audio results remain machine observations."""
import argparse
import html
from dataclasses import asdict
from datetime import datetime, timezone
from importlib.metadata import version
import json
import math
from pathlib import Path
import platform
import time
import wave
import numpy as np

HERE = Path(__file__).resolve().parent


def json_value(value):
    if isinstance(value, np.generic):
        value = value.item()
    if isinstance(value, float) and not math.isfinite(value):
        # Whisper's default VAD maximum is infinity; retain it in standard JSON.
        return "NaN" if math.isnan(value) else ("Infinity" if value > 0 else "-Infinity")
    if isinstance(value, dict):
        return {k: json_value(v) for k, v in value.items()}
    if isinstance(value, (list, tuple)):
        return [json_value(v) for v in value]
    return value


def save(path, value):
    path.parent.mkdir(parents=True, exist_ok=True)
    temp = path.with_suffix(path.suffix + ".tmp")
    temp.write_text(json.dumps(json_value(value), ensure_ascii=False, indent=2, allow_nan=False) + "\n", encoding="utf-8")
    temp.replace(path)


def read(path):
    return json.loads(path.read_text(encoding="utf-8"))


def chunk_plan(duration_ms, chunk_ms, context_ms=2000):
    if duration_ms <= 0 or chunk_ms <= 0 or context_ms < 0:
        raise ValueError("Duration/chunk must be positive; context must be nonnegative")
    return [{"core_start_ms": start, "core_end_ms": min(start + chunk_ms, duration_ms),
             "start_ms": max(0, start - context_ms),
             "end_ms": min(start + chunk_ms + context_ms, duration_ms)}
            for start in range(0, math.ceil(duration_ms), chunk_ms)]


def crop_wav(source, target, start_ms, end_ms):
    with wave.open(str(source), "rb") as src:
        rate = src.getframerate()
        first, last = round(start_ms * rate / 1000), round(end_ms * rate / 1000)
        # VI0a records container ranges; their PCM may be <1 ms shorter at the tail.
        if 0 < last - src.getnframes() < rate / 1000:
            last = src.getnframes()
        if not 0 <= first < last <= src.getnframes():
            raise ValueError("Audio range outside PCM input")
        src.setpos(first)
        with wave.open(str(target), "wb") as dst:
            dst.setparams(src.getparams())
            dst.writeframes(src.readframes(last - first))
    return first / rate * 1000, (last - first) / rate * 1000


def prepare(config, media, output, manifest):
    clips = {c["id"]: c for c in read(media / "replay.json")["clips"]}
    output.mkdir(parents=True, exist_ok=True)
    jobs = []
    for case in config["cases"]:
        if "silence_ms" in case:
            audio = output / (case["id"] + ".wav")
            with wave.open(str(audio), "wb") as dst:
                dst.setparams((1, 2, 16000, 0, "NONE", "not compressed"))
                dst.writeframes(b"\0\0" * (case["silence_ms"] * 16))
            jobs.append({"id": case["id"], "audio_path": str(audio.resolve()),
                         "source_id": "vi0b-digital-silence", "source_revision": "v1",
                         "source_start_ms": 0, "duration_ms": case["silence_ms"],
                         "purpose": case["purpose"], "origin": "generated zero PCM"})
            continue
        clip = clips[case["clip"]]
        if clip["source_id"] != manifest["source"]["id"]:
            raise ValueError("This experiment requires the primary lecture source")
        ranges = chunk_plan(clip["duration_ms"], case["chunk_ms"], case["context_ms"]) if "chunk_ms" in case else [
            {"start_ms": case.get("start_ms", 0), "end_ms": case.get("end_ms", clip["duration_ms"])}]
        for index, part in enumerate(ranges):
            name = case["id"] + (f"-{index:03}" if "chunk_ms" in case else "")
            audio = output / (name + ".wav")
            start, duration = crop_wav(media / clip["audio_path"], audio, part["start_ms"], part["end_ms"])
            job = {"id": name, "audio_path": str(audio.resolve()), "source_id": clip["source_id"],
                   "source_revision": manifest["source"]["revision"],
                   "source_start_ms": clip["source_start_ms"] + start, "duration_ms": duration,
                   "purpose": case["purpose"], "origin": clip["audio_path"]}
            if "core_start_ms" in part:
                job["core_start_ms"] = clip["source_start_ms"] + part["core_start_ms"]
                job["core_end_ms"] = clip["source_start_ms"] + part["core_end_ms"]
            jobs.append(job)
    if len({j["id"] for j in jobs}) != len(jobs):
        raise ValueError("Duplicate job IDs")
    return jobs


def map_response(segments, job):
    """Preserve model intervals, adding source offset; never silently clamp errors."""
    mapped = []
    for segment in segments:
        item = {"text": segment["text"], "start_ms": segment["start"] * 1000,
                "end_ms": segment["end"] * 1000, "words": []}
        for word in segment.get("words") or []:
            start, end = word["start"] * 1000, word["end"] * 1000
            midpoint = job["source_start_ms"] + (start + end) / 2
            item["words"].append({"text": word["word"], "start_ms": start, "end_ms": end,
                "source_start_ms": job["source_start_ms"] + start,
                "source_end_ms": job["source_start_ms"] + end, "probability": word["probability"],
                "owned_by_core": job.get("core_start_ms", -math.inf) <= midpoint < job.get("core_end_ms", math.inf)})
        for interval in [item, *item["words"]]:
            if not (0 <= interval["start_ms"] <= interval["end_ms"] <= job["duration_ms"] + 1):
                raise ValueError("Model timestamp outside submitted audio")
        item["source_start_ms"] = job["source_start_ms"] + item["start_ms"]
        item["source_end_ms"] = job["source_start_ms"] + item["end_ms"]
        mapped.append(item)
    return mapped


def run_job(job, profile, output, transcribe):
    """Question/correction reviews share exactly the same audio path and parameters."""
    path = output / (job["id"] + ".json")
    request = {"job": job, "profile": profile}
    if path.exists():
        previous = read(path)
        if previous["request"] != request:
            raise ValueError("Existing result uses different input/configuration; choose a new --out")
        if previous["status"] == "completed":
            return previous, True
    record = {"request": request, "status": "running", "started_at": datetime.now(timezone.utc).isoformat()}
    save(path, record)
    start = time.perf_counter()
    try:
        segments, info = transcribe(job["audio_path"], **profile["transcribe"])
        raw = [asdict(s) for s in segments]  # Exhaust generator: this is when inference actually runs.
        record["raw_response"] = {"segments": raw, "info": asdict(info)}
        save(path, record)  # Retain raw response even if timestamp projection fails.
        record["segments"] = map_response(raw, job)
        record["text"] = "".join(s["text"] for s in raw).strip()
        record["status"] = "completed"
        record["observation"] = "machine_audio_transcription_not_listening_truth"
    except Exception as error:
        record["status"] = "failed"
        record["error"] = f"{type(error).__name__}: {error}"
    record["elapsed_ms"] = round((time.perf_counter() - start) * 1000)
    save(path, record)
    return record, False


def report(output):
    plan = read(output / "jobs.json")
    sections = []
    records = {}
    for job in plan["jobs"]:
        path = output / "results" / (job["id"] + ".json")
        if not path.exists():
            continue
        result = read(path)
        records[job["id"]] = result
        rows = []
        for segment in result.get("segments", []):
            rows.append(f'<tr><td>{segment["source_start_ms"] / 1000:.2f}–{segment["source_end_ms"] / 1000:.2f}s</td>'
                        f'<td>{html.escape(segment["text"])}</td></tr>')
        sections.append(f'<section><h2>{html.escape(job["id"])}</h2><p>{html.escape(job["purpose"])} · '
                        f'{job["source_start_ms"] / 1000:.3f}s 起 · {result["status"]}</p>'
                        f'<audio controls preload="metadata" src="audio/{html.escape(job["id"])}.wav"></audio>'
                        '<table><thead><tr><th>原片时间</th><th>机器转写候选</th></tr></thead>'
                        f'<tbody>{"".join(rows)}</tbody></table></section>')
    page = '<!doctype html><html lang="zh-CN"><meta charset="utf-8"><title>VI0b 原声复核</title>' \
        '<style>body{max-width:1000px;margin:40px auto;padding:0 24px;font:18px/1.6 system-ui;background:#f5f2eb;color:#202a32}' \
        'section{background:white;padding:24px;margin:20px 0;border-radius:12px}audio{width:100%}' \
        'table{border-collapse:collapse;width:100%}td,th{text-align:left;padding:8px;border-bottom:1px solid #ddd}' \
        'td:first-child{white-space:nowrap}</style><h1>VI0b · 原声与机器复核</h1>' \
        '<p>音频来自固定原片区间。以下文字及时间戳是 Whisper 识别候选，人工听核真值仍单独记录；silence 为生成的零采样对照。</p>' \
        + ''.join(sections) + '</html>'
    (output / "index.html").write_text(page, encoding="utf-8")
    completed = {name: result for name, result in records.items() if result["status"] == "completed"}
    checks = {
        "all_jobs_completed": len(completed) == len(plan["jobs"]),
        "changed_audio_changes_text": all(name in completed for name in ("review-overflow", "review-hidden"))
            and completed["review-overflow"]["text"] != completed["review-hidden"]["text"]
            and bool(completed["review-overflow"]["text"]) and bool(completed["review-hidden"]["text"]),
        "digital_silence_has_no_text": "silence" in completed and completed["silence"]["text"] == "",
    }
    evidence = {"checks": checks, "listening_truth": "tracked_separately_in_samples.json",
                "completed_audio_ms": sum(r["request"]["job"]["duration_ms"] for r in completed.values()),
                "inference_elapsed_ms": sum(r["elapsed_ms"] for r in completed.values()),
                "semantic_accuracy": "not_measured_without_listening_truth",
                "records": [{"id": name, "status": r["status"], "text": r.get("text"),
                             "elapsed_ms": r.get("elapsed_ms"), "error": r.get("error")}
                            for name, r in records.items()]}
    save(output / "verification.json", evidence)
    return evidence


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("command", choices=["prepare", "run", "status", "report"])
    parser.add_argument("--config", type=Path, default=HERE / "whisper-experiment.json")
    parser.add_argument("--media", type=Path, default=HERE / "generated")
    parser.add_argument("--out", type=Path, default=HERE / "generated" / "vi0b")
    parser.add_argument("--model-dir", type=Path, default=HERE.parent.parent / ".worktree-local" / "whisper-models" / "small.en")
    parser.add_argument("--only", nargs="+", help="Run selected job IDs, including failed/interrupted jobs")
    args = parser.parse_args()
    if args.command == "report":
        evidence = report(args.out)
        print(json.dumps(evidence["checks"]))
        raise SystemExit(0 if all(evidence["checks"].values()) else 1)
    config = read(args.config)
    if args.command == "prepare":
        jobs = prepare(config, args.media, args.out / "audio", read(HERE / "samples.json"))
        save(args.out / "jobs.json", {"config": config, "jobs": jobs})
        print(json.dumps({"jobs": len(jobs), "out": str(args.out)}))
        return
    plan = read(args.out / "jobs.json")
    if plan["config"] != config:
        raise ValueError("Configuration changed; prepare a new --out")
    jobs = plan["jobs"]
    if args.only:
        if set(args.only) - {j["id"] for j in jobs}:
            raise ValueError("Unknown job ID")
        jobs = [j for j in jobs if j["id"] in args.only]
    if args.command == "status":
        for job in jobs:
            path = args.out / "results" / (job["id"] + ".json")
            result = read(path) if path.exists() else {"status": "pending"}
            print(json.dumps({"id": job["id"], "status": result["status"], "elapsed_ms": result.get("elapsed_ms")}))
        return
    from faster_whisper import WhisperModel
    profile = {k: config[k] for k in ("model", "device", "compute_type", "cpu_threads", "transcribe")}
    profile["versions"] = {name: version(name) for name in ("faster-whisper", "ctranslate2", "av", "onnxruntime")}
    profile["model_directory"] = str(args.model_dir.resolve())
    # Local files only: model downloads and loading time are distinct from inference.
    loaded = time.perf_counter()
    model = WhisperModel(str(args.model_dir), device=config["device"], compute_type=config["compute_type"],
                         cpu_threads=config["cpu_threads"], local_files_only=True)
    summary = {"started_at": datetime.now(timezone.utc).isoformat(), "profile": profile,
               "python": platform.python_version(), "platform": platform.platform(),
               "model_load_ms": round((time.perf_counter() - loaded) * 1000),
               "remote_service_calls": 0, "api_cost": 0, "jobs": []}
    for job in jobs:
        result, reused = run_job(job, profile, args.out / "results", model.transcribe)
        item = {"id": job["id"], "status": result["status"], "reused": reused,
                "elapsed_ms": result["elapsed_ms"], "audio_duration_ms": job["duration_ms"]}
        summary["jobs"].append(item)
        print(json.dumps(item), flush=True)
        save(args.out / "run-summary.json", summary)
    if any(j["status"] != "completed" for j in summary["jobs"]):
        raise SystemExit(1)


if __name__ == "__main__":
    main()
