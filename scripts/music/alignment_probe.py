"""Real recording -> score chroma DTW experiment, never an accepted alignment.

Dependencies: numpy, scipy, ffmpeg. Uses actual audio spectra and MusicXML pitch
content. It emits coarse note-onset candidates, not fabricated gold timestamps.
Grace-note onsets and physical note-off times deliberately remain unresolved.
"""
from pathlib import Path
import argparse
import json
import subprocess
import time
import xml.etree.ElementTree as ET

import numpy as np
from scipy import signal
from scipy.ndimage import gaussian_filter1d

ROOT = Path(__file__).resolve().parents[2]
OUT = ROOT / "evals/music/results/schubert-chroma-dtw"
ASSETS = ROOT / "evals/music/assets"
STEP_QUARTERS = 1 / 32
SR = 22050
HOP = 441


def score_notes(path):
    root = ET.parse(path).getroot()
    notes, measures = [], []
    pitch_class = dict(C=0, D=2, E=4, F=5, G=7, A=9, B=11)
    for part in root.findall("part"):
        total, divisions = 0.0, 1.0
        for mi, measure in enumerate(part.findall("measure"), 1):
            cursor, end, prev, ni = 0.0, 0.0, 0.0, 0
            d = measure.findtext("attributes/divisions")
            if d:
                divisions = float(d)
            for item in measure:
                duration = float(item.findtext("duration", "0")) / divisions
                if item.tag == "backup":
                    cursor -= duration
                elif item.tag == "forward":
                    cursor += duration
                    end = max(end, cursor)
                elif item.tag == "note":
                    ni += 1
                    onset = prev if item.find("chord") is not None else cursor
                    pitch = item.find("pitch")
                    if pitch is not None:
                        midi = 12 * (int(pitch.findtext("octave")) + 1) + pitch_class[pitch.findtext("step")] + int(pitch.findtext("alter", "0"))
                        notes.append(dict(source_id=f"{part.get('id')}-m{mi}-n{ni}", measure_index=mi, measure_label=measure.get("number"), voice=item.findtext("voice"), staff=item.findtext("staff"), quarter=total + onset, duration_quarters=duration, midi=midi, grace=item.find("grace") is not None, tied_stop=any(t.get("type") == "stop" for t in item.findall("tie"))))
                    if item.find("chord") is None:
                        prev = cursor
                        cursor += duration
                    end = max(end, onset + duration, cursor)
            measures.append(dict(part_id=part.get("id"), index=mi, label=measure.get("number"), quarter=total, duration_quarters=end))
            total += end
    return notes, measures, max(m["quarter"] + m["duration_quarters"] for m in measures)


def normalize(a):
    return a / np.maximum(np.linalg.norm(a, axis=0, keepdims=True), 1e-8)


def audio_features(path):
    pcm = subprocess.run(["ffmpeg", "-v", "error", "-i", str(path), "-f", "f32le", "-ac", "1", "-ar", str(SR), "pipe:1"], capture_output=True, check=True).stdout
    audio = np.frombuffer(pcm, dtype=np.float32)
    f, t, z = signal.stft(audio, fs=SR, nperseg=4096, noverlap=4096-HOP, boundary="zeros")
    magnitude = np.abs(z)
    rms = np.sqrt(np.mean(magnitude**2, axis=0))
    active = np.flatnonzero(rms > np.max(rms) * .02)
    first, last = int(active[0]), int(active[-1]) + 1
    # Global per-bin whitening reduces microphone/piano coloration; all values
    # still come from the actual recording, not the score's expected pitches.
    white = np.sqrt(magnitude / np.maximum(np.mean(magnitude, axis=1, keepdims=True), 1e-5))
    features = np.zeros((24, magnitude.shape[1]), dtype=np.float32)
    for k in np.flatnonzero((f >= 55) & (f <= 4200)):
        pitch = 69 + 12 * np.log2(f[k] / 440)
        nearest = int(round(pitch))
        weight = np.exp(-.5 * ((pitch-nearest)/.3)**2) / max(f[k], 55)**.3
        features[nearest % 12] += white[k] * weight
        if f[k] <= 270:
            features[12 + nearest % 12] += white[k] * weight
    features = np.concatenate((normalize(features[:12]), normalize(features[12:]) * .45), axis=0)
    flux = np.maximum(np.diff(magnitude, axis=1, prepend=magnitude[:, :1]), 0).sum(axis=0)
    return normalize(features[:, first:last]), t[first:last], flux[first:last], dict(sample_rate=SR, hop_samples=HOP, frame_seconds=HOP/SR, trimmed_start_seconds=float(t[first]), trimmed_end_seconds=float(t[last-1]), duration_seconds=len(audio)/SR)


def score_features(notes, total):
    frames = int(np.ceil(total/STEP_QUARTERS))
    a = np.zeros((24, frames), dtype=np.float32)
    for note in notes:
        if note["grace"]:
            continue
        lo = round(note["quarter"] / STEP_QUARTERS)
        hi = min(frames, max(lo + 1, round((note["quarter"] + note["duration_quarters"]) / STEP_QUARTERS)))
        a[note["midi"] % 12, lo:hi] += 1
        if note["midi"] < 60:
            a[12 + note["midi"] % 12, lo:hi] += 1
    a = gaussian_filter1d(a, sigma=.45, axis=1)
    return normalize(np.concatenate((normalize(a[:12]), normalize(a[12:]) * .45), axis=0))


def dtw(score, audio, band_fraction):
    cost = np.maximum(0, 1 - score.T @ audio).astype(np.float32)
    n, m = cost.shape
    acc = np.full((n+1, m+1), np.inf, dtype=np.float32)
    steps = np.zeros((n, m), dtype=np.uint8)
    acc[0, 0] = 0
    band = max(20, int(m * band_fraction))
    for i in range(1, n+1):
        center = round(i*m/n)
        for j in range(max(1, center-band), min(m, center+band)+1):
            up, left, diag = acc[i-1, j] + .03, acc[i, j-1] + .03, acc[i-1, j-1]
            if diag <= up and diag <= left:
                best, step = diag, 0
            elif up <= left:
                best, step = up, 1
            else:
                best, step = left, 2
            acc[i, j] = cost[i-1, j-1] + best
            steps[i-1, j-1] = step
    i, j, path = n-1, m-1, []
    while i >= 0 and j >= 0:
        path.append((i, j))
        step = steps[i, j]
        if step == 0:
            i -= 1; j -= 1
        elif step == 1:
            i -= 1
        else:
            j -= 1
    path.reverse()
    mapping = np.full(n, np.nan)
    for k in range(n):
        matches = [j for i, j in path if i == k]
        if matches:
            mapping[k] = np.median(matches)
    good = np.flatnonzero(np.isfinite(mapping))
    mapping = np.interp(np.arange(n), good, mapping[good])
    return mapping, dict(path_mean_chroma_cost=float(np.mean([cost[i,j] for i,j in path])), path_pairs=len(path), score_frames=n, audio_frames=m)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--band", type=float, default=.12)
    args = parser.parse_args()
    started = time.perf_counter()
    OUT.mkdir(parents=True, exist_ok=True)
    notes, measures, total = score_notes(ASSETS/"schubert-d780-3-noctave.musicxml")
    audio, times, flux, audio_info = audio_features(ASSETS/"schubert-d780-3-vetter.mp3")
    score = score_features(notes, total)
    mapping, metrics = dtw(score, audio, args.band)
    candidates = []
    for note in notes:
        if note["grace"] or note["tied_stop"]:
            candidates.append({**note, "onset_seconds": None, "status": "unresolved_grace_or_tie", "evidence": "score_only"})
            continue
        frame = int(np.clip(round(note["quarter"]/STEP_QUARTERS), 0, len(mapping)-1))
        aframe = int(np.clip(round(mapping[frame]), 0, len(times)-1))
        # Optional local spectral attack nearest the DTW prediction. This is a
        # candidate, not proof that a particular overlapping note started there.
        lo, hi = max(0, aframe-5), min(len(flux), aframe+6)
        peak = lo + int(np.argmax(flux[lo:hi]))
        candidates.append({**note, "onset_seconds": round(float(times[peak]), 6), "dtw_seconds": round(float(times[aframe]), 6), "status": "unreviewed_candidate", "evidence": "recording_spectral_chroma_dtw_then_local_flux"})
    measure_candidates = []
    for measure in measures:
        idx = int(np.clip(round(measure["quarter"]/STEP_QUARTERS), 0, len(mapping)-1))
        measure_candidates.append({**measure, "recording_seconds": round(float(times[int(round(mapping[idx]))]), 6), "status": "unreviewed_candidate"})
    result = dict(method="whitened_stft_chroma_bass_dtw_v1", status="experimental_candidates_not_accepted", recording="../../assets/schubert-d780-3-vetter.mp3", score="../../assets/schubert-d780-3-noctave.musicxml", audio=audio_info, score_quarters=total, band_fraction=args.band, metrics=metrics, elapsed_seconds=round(time.perf_counter()-started, 3), notes=candidates, measures=measure_candidates, limitations=["No independent human onset labels; chroma cost is not a timing accuracy measurement.", "Grace-note onset and tied continuation remain unresolved.", "Local flux may identify another simultaneous voice; candidates require review before note-level publication.", "Band assumes same chronological score/performance route; structural mismatch must be corrected before use."])
    (OUT/"candidates.json").write_text(json.dumps(result, ensure_ascii=False, indent=2)+"\n",encoding="utf-8")
    (OUT/"measure-candidates.tsv").write_text("measure\tscore_quarter\trecording_seconds\n"+"".join(f"{m['index']}\t{m['quarter']}\t{m['recording_seconds']}\n" for m in measure_candidates),encoding="utf-8")
    print(json.dumps({k:v for k,v in result.items() if k not in ["notes","measures"]}))


if __name__ == "__main__":
    main()
