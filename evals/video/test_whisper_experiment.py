from dataclasses import dataclass
from pathlib import Path
import json
import tempfile
import unittest
import wave
import numpy as np

from whisper_experiment import chunk_plan, crop_wav, map_response, read, report, run_job, save


@dataclass
class Segment:
    start: float = 0.1
    end: float = 0.8
    text: str = " test"
    words: object = None


@dataclass
class Info:
    duration: float = 1


class ExperimentTests(unittest.TestCase):
    def test_unbounded_vad_duration_is_portable_json(self):
        with tempfile.TemporaryDirectory() as folder:
            path = Path(folder) / "raw.json"
            save(path, {"vad_options": {"max_speech_duration_s": float("inf")}})
            def reject(value):
                raise ValueError(f"Nonstandard JSON constant: {value}")
            result = json.loads(path.read_text(encoding="utf-8"), parse_constant=reject)
            self.assertEqual(result["vad_options"]["max_speech_duration_s"], "Infinity")

    def test_real_backend_numpy_scalars_are_saved_as_json_numbers(self):
        with tempfile.TemporaryDirectory() as folder:
            job = {"id": "numpy", "audio_path": "unused", "source_start_ms": 550000, "duration_ms": 1000}
            segment = Segment(words=[{"start": np.float64(0.1), "end": np.float64(0.8),
                "word": " test", "probability": np.float32(0.8)}])
            result, _ = run_job(job, {"transcribe": {}}, Path(folder), lambda *a, **k: (iter([segment]), Info()))
            stored = read(Path(folder) / "numpy.json")
            self.assertEqual(stored["status"], "completed")
            self.assertIs(stored["segments"][0]["words"][0]["owned_by_core"], True)

    def test_pcm_rounding_retains_available_samples(self):
        # Real VI0a WAVs can end up <1 ms shorter than requested container intervals.
        with tempfile.TemporaryDirectory() as folder:
            source, target = Path(folder) / "input.wav", Path(folder) / "output.wav"
            with wave.open(str(source), "wb") as out:
                out.setparams((1, 2, 44100, 0, "NONE", "not compressed"))
                out.writeframes(b"\x01\x00" * 44095)
            start, duration = crop_wav(source, target, 0, 1000)
            self.assertEqual(start, 0)
            self.assertAlmostEqual(duration, 44095 / 44.1)
            with wave.open(str(target), "rb") as out:
                self.assertEqual(out.readframes(44100), b"\x01\x00" * 44095)
            with self.assertRaises(ValueError):
                crop_wav(source, target, 0, 1010)

    def test_chunks_cover_final_partial_core_and_retain_context(self):
        plan = chunk_plan(65000, 30000, 2000)
        self.assertEqual([(p["core_start_ms"], p["core_end_ms"]) for p in plan],
                         [(0, 30000), (30000, 60000), (60000, 65000)])
        self.assertEqual((plan[1]["start_ms"], plan[1]["end_ms"]), (28000, 62000))
        with self.assertRaises(ValueError):
            chunk_plan(1000, 0)

    def test_timestamps_add_source_offset_and_own_boundary_once(self):
        raw = [{"text": "test", "start": 1, "end": 2, "words": [
            {"word": " test", "start": 1, "end": 2, "probability": 0.8}]}]
        first = {"source_start_ms": 550000, "duration_ms": 3000,
                 "core_start_ms": 550000, "core_end_ms": 551500}
        second = {**first, "core_start_ms": 551500, "core_end_ms": 553000}
        mapped = map_response(raw, first)[0]
        self.assertEqual(mapped["source_start_ms"], 551000)
        self.assertFalse(mapped["words"][0]["owned_by_core"])
        self.assertTrue(map_response(raw, second)[0]["words"][0]["owned_by_core"])
        with self.assertRaisesRegex(ValueError, "outside"):
            map_response(raw, {**first, "duration_ms": 1000})

    def test_failure_interruption_and_completed_resume(self):
        # Fault injection exercises local bookkeeping; real model integration is run separately.
        with tempfile.TemporaryDirectory() as folder:
            out = Path(folder)
            job = {"id": "test", "audio_path": "unused", "source_start_ms": 550000, "duration_ms": 1000}
            profile = {"transcribe": {}}
            def fail(*args, **kwargs):
                raise RuntimeError("interrupted inference")
            result, reused = run_job(job, profile, out, fail)
            self.assertEqual(result["status"], "failed")
            self.assertFalse(reused)
            def good(*args, **kwargs):
                return iter([Segment()]), Info()
            result, reused = run_job(job, profile, out, good)
            self.assertEqual(result["status"], "completed")
            result, reused = run_job(job, profile, out, fail)
            self.assertTrue(reused)  # A completed result does not call the failing backend.
            with self.assertRaisesRegex(ValueError, "different input"):
                run_job({**job, "source_start_ms": 600000}, profile, out, good)
            path = out / "test.json"
            state = read(path)
            state["status"] = "running"
            save(path, state)
            result, reused = run_job(job, profile, out, good)
            self.assertEqual(result["status"], "completed")
            self.assertFalse(reused)

    def test_failed_projection_keeps_raw_model_response(self):
        with tempfile.TemporaryDirectory() as folder:
            job = {"id": "out-of-range", "audio_path": "unused", "source_start_ms": 0, "duration_ms": 500}
            result, _ = run_job(job, {"transcribe": {}}, Path(folder), lambda *a, **k: (iter([Segment()]), Info()))
            self.assertEqual(result["status"], "failed")
            self.assertEqual(result["raw_response"]["segments"][0]["text"], " test")

    def test_report_does_not_treat_missing_failed_or_identical_audio_results_as_pass(self):
        with tempfile.TemporaryDirectory() as folder:
            out = Path(folder)
            names = ["review-overflow", "review-hidden", "silence"]
            jobs = [{"id": name, "source_start_ms": 0, "purpose": "test", "duration_ms": 1000} for name in names]
            save(out / "jobs.json", {"jobs": jobs})
            self.assertFalse(any(report(out)["checks"].values()))
            for job in jobs:
                save(out / "results" / (job["id"] + ".json"), {
                    "status": "completed", "text": "same", "elapsed_ms": 1,
                    "request": {"job": job}, "segments": []})
            checks = report(out)["checks"]
            self.assertTrue(checks["all_jobs_completed"])
            self.assertFalse(checks["changed_audio_changes_text"])
            self.assertFalse(checks["digital_silence_has_no_text"])
            path = out / "results" / "silence.json"
            record = read(path)
            record["status"] = "failed"
            save(path, record)
            self.assertFalse(report(out)["checks"]["all_jobs_completed"])


if __name__ == "__main__":
    unittest.main()
