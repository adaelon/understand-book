"""Inspect actual pitch-specific energy near all Schubert grace-note candidates.

This emits review plots and machine candidates. A plot is not a human listening
label, and the output never upgrades automatic candidates to accepted notes.
"""
from pathlib import Path
import json
import subprocess
import numpy as np
from scipy import signal
from scipy.ndimage import gaussian_filter1d
import matplotlib
matplotlib.use("Agg")
import matplotlib.pyplot as plt
import alignment_probe as p

OUT = p.ROOT / "evals/music/results/schubert-pitch-onsets-v2"


def main():
    OUT.mkdir(parents=True, exist_ok=True)
    coarse = json.loads((p.OUT / "candidates.json").read_text(encoding="utf-8"))
    pcm = subprocess.run(["ffmpeg", "-v", "error", "-i", str(p.ASSETS / "schubert-d780-3-vetter.mp3"), "-f", "f32le", "-ac", "1", "-ar", "22050", "pipe:1"], capture_output=True, check=True).stdout
    audio = np.frombuffer(pcm, dtype=np.float32)
    # Short window resolves close attacks. Harmonic bands distinguish upper
    # melody notes whose fundamentals may coexist with lower accompaniment.
    f, t, z = signal.stft(audio, fs=22050, nperseg=2048, noverlap=2048-110, nfft=8192, boundary="zeros")
    mag = np.abs(z)
    midis = np.arange(48, 90)
    templates = np.zeros((len(midis), len(f)), dtype=np.float32)
    for i, midi in enumerate(midis):
        fundamental = 440 * 2 ** ((midi-69)/12)
        for harmonic in range(1, 7):
            center = fundamental*harmonic
            if center > 8000:
                break
            templates[i] += np.exp(-.5*((f-center)/10)**2)/harmonic
        templates[i] /= max(np.linalg.norm(templates[i]), 1e-8)
    # A simultaneous nonnegative fit reduces upper-harmonic confusion; projected
    # gradient solves all frames together with a fixed acoustic template basis.
    gram = templates @ templates.T
    target = templates @ mag
    step = 1/np.linalg.norm(gram, 2)
    energy = np.maximum(target, 0)
    for _ in range(60):
        energy = np.maximum(0, energy-step*(gram @ energy-target))
    energy = gaussian_filter1d(energy, sigma=.65, axis=1)
    attack = np.maximum(np.diff(energy, axis=1, prepend=energy[:, :1]), 0)
    np.savez_compressed(OUT / "pitch-energy.npz", times=t, midi=midis, energy=energy, attack=attack)
    notes = coarse["notes"]
    reviews = []
    previous_grace = {}
    for note in notes:
        if not note["grace"]:
            continue
        following = next((n for n in notes if not n["grace"] and n["quarter"] == note["quarter"] and n["staff"] == note["staff"] and n["voice"] == note["voice"]), None)
        if following is None or following["dtw_seconds"] is None:
            reviews.append({"source_id": note["source_id"], "status": "gap_no_main_note_candidate"})
            continue
        center = following["dtw_seconds"]
        group = (note["quarter"], note["staff"], note["voice"])
        earlier = previous_grace.get(group)
        start = max(center-.16, earlier + .01 if earlier is not None else center-.16)
        region = np.flatnonzero((t >= start) & (t <= center+.20))
        gi, mi = note["midi"]-48, following["midi"]-48
        if not (0 <= gi < len(midis) and 0 <= mi < len(midis)):
            reviews.append({"source_id": note["source_id"], "status": "gap_outside_pitch_analysis_range"})
            continue
        g = attack[gi,region]
        peaks,_ = signal.find_peaks(g, prominence=max(g.max()*.15, 1e-8), distance=4)
        # Distinct pitch peak near the coarse attack is offered for review.
        # Prefer the nearest substantial attack, not a louder preceding note.
        if len(peaks):
            substantial = peaks[g[peaks] >= g[peaks].max()*.30]
            chosen = int(substantial[np.argmin(np.abs(t[region[substantial]]-center))])
        else:
            chosen = int(np.argmax(g))
        gframe = int(region[chosen])
        previous_grace[group] = float(t[gframe])
        main_region = region[(region >= gframe) & (t[region] <= t[gframe]+.22)]
        grace_pitches = {n['midi'] for n in notes if n['grace'] and (n['quarter'],n['staff'],n['voice']) == group}
        main_chord = [n for n in notes if not n['grace'] and (n['quarter'],n['staff'],n['voice']) == group]
        exclusive = [n for n in main_chord if n['midi'] not in grace_pitches and 48 <= n['midi'] < 90]
        witness = exclusive[0] if exclusive else following
        wi = witness['midi'] - 48
        mframe = int(main_region[np.argmax(attack[wi, main_region])]) if len(main_region) else gframe
        reviews.append({"source_id": note["source_id"], "measure": note["measure_index"], "quarter": note["quarter"], "grace_midi": note["midi"], "main_source_id": following["source_id"], "main_midi": following["midi"], "main_witness_midi": witness['midi'], "main_witness_source_id": witness['source_id'], "coarse_main_seconds": center, "grace_attack_candidate_seconds": round(float(t[gframe]),6), "main_attack_candidate_seconds": round(float(t[mframe]),6), "separation_ms": round(float((t[mframe]-t[gframe])*1000),2), "pitch_peak_count": len(peaks), "status": "unreviewed_pitch_candidate"})
    # Every grace event appears in a contact sheet, keeping raw pitch energy,
    # derivative and machine proposals visible instead of concealing ambiguity.
    for page in range((len(reviews)+11)//12):
        batch=reviews[page*12:(page+1)*12]
        fig, axes=plt.subplots(6,2,figsize=(16,18), constrained_layout=True)
        for ax, item in zip(axes.flat,batch):
            if "coarse_main_seconds" not in item:
                ax.set_title(item["source_id"]+" "+item["status"]); continue
            center=item["coarse_main_seconds"]
            region=np.flatnonzero((t>=center-.38)&(t<=center+.35))
            for midi,label,color in [(item["grace_midi"],"grace","#d95728"),(item["main_witness_midi"],"main chord witness","#145aaa")]:
                y=energy[midi-48,region]
                ax.plot(t[region],y/max(float(y.max()),1e-8),label=f"{label} MIDI {midi}",color=color)
            ax.axvline(item["grace_attack_candidate_seconds"],color="#d95728",linestyle="--")
            ax.axvline(item["main_attack_candidate_seconds"],color="#145aaa",linestyle="--")
            ax.axvline(center,color="gray",alpha=.5)
            ax.set_title(f"{item['source_id']}  separation {item['separation_ms']} ms")
            ax.set_ylim(-.05,1.1);ax.set_xlabel("original recording seconds");ax.legend(fontsize=8)
        for ax in list(axes.flat)[len(batch):]:ax.set_visible(False)
        fig.savefig(OUT/f"grace-review-{page+1}.png",dpi=110);plt.close(fig)
    summary={"method":"short_window_harmonic_nonnegative_fit_v2","status":"machine_candidates_not_accepted","sample_rate":22050,"window_samples":2048,"hop_samples":110,"time_step_ms":110/22050*1000,"events":reviews,"limitations":["Acoustic templates are approximate and include neither piano inharmonicity nor microphone calibration.","Candidate peak positions are spectral attack maxima, not calibrated physical note onsets.","Close peaks or same pitch energy cannot establish two distinct attacks; manual review required.","No independent human labels; no timing error claim."]}
    (OUT/"grace-candidates.json").write_text(json.dumps(summary,indent=2)+"\n",encoding="utf-8")
    print(json.dumps({"grace_events":len(reviews),"status":"all_unreviewed","zero_separation":sum(x.get('separation_ms')==0 for x in reviews),"review_pages":(len(reviews)+11)//12}))


if __name__ == "__main__": main()
