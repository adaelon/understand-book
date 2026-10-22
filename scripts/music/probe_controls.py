"""Compare the actual-pitch DTW with a wrong-pitch control and inspect ordering.

The control tests whether pitch content matters. It does not establish timing
accuracy; no human onset labels exist in this experiment.
"""
import json
import time
import numpy as np
import alignment_probe as p

started = time.perf_counter()
notes, measures, total = p.score_notes(p.ASSETS / "schubert-d780-3-noctave.musicxml")
audio, times, flux, audio_info = p.audio_features(p.ASSETS / "schubert-d780-3-vetter.mp3")
score = p.score_features(notes, total)
wrong = np.concatenate((np.roll(score[:12], 6, axis=0), np.roll(score[12:], 6, axis=0)), axis=0)
_, metrics = p.dtw(wrong, audio, .12)
result = json.loads((p.OUT / "candidates.json").read_text(encoding="utf-8"))
by_quarter = {}
for note in result["notes"]:
    if note["onset_seconds"] is not None:
        by_quarter.setdefault(note["quarter"], []).append(note["onset_seconds"])
ordered = [(q, float(np.median(x))) for q, x in sorted(by_quarter.items())]
reversals = [{"before": a, "after": b} for a, b in zip(ordered, ordered[1:]) if b[1] < a[1]]
report = {
    "negative_control": "Score chroma rotated six semitones, same actual recording/rhythm and DTW settings",
    "real_score_mean_cost": result["metrics"]["path_mean_chroma_cost"],
    "transposed_score_mean_cost": metrics["path_mean_chroma_cost"],
    "supports_pitch_specific_matching_only": metrics["path_mean_chroma_cost"] > result["metrics"]["path_mean_chroma_cost"],
    "distinct_score_onsets": len(ordered),
    "refined_time_reversals": reversals,
    "unresolved_notes": sum(n["onset_seconds"] is None for n in result["notes"]),
    "elapsed_seconds": time.perf_counter() - started,
    "status": "experimental_not_timing_accuracy_validation",
}
(p.OUT / "negative-control.json").write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
print(json.dumps(report))
