"""Locate the selected orchestral score excerpt in the actual Coates recording.

This is subsequence DTW against audio chroma; returned times remain unaccepted.
The official alignment table is for other performances and is never substituted.
"""
from pathlib import Path
import json
import subprocess
import time
import numpy as np
from scipy import signal
import alignment_probe as p

OUT = p.ROOT / "evals/music/results/beethoven-chroma-dtw"


def main():
    started = time.perf_counter()
    OUT.mkdir(parents=True, exist_ok=True)
    p.STEP_QUARTERS = .125
    notes, measures, total = p.score_notes(p.ASSETS / "beethoven-op125-4-m93-164.musicxml")
    sr, hop = 11025, 551
    pcm = subprocess.check_output(["ffmpeg", "-v", "error", "-i", str(p.ASSETS/"beethoven-op125-4-coates-damian-full.mp3"), "-t", "420", "-f", "f32le", "-ac", "1", "-ar", str(sr), "pipe:1"])
    f, times, z = signal.stft(np.frombuffer(pcm, dtype=np.float32), fs=sr, nperseg=2048, noverlap=2048-hop, boundary="zeros")
    mag = np.abs(z)
    white = np.sqrt(mag/np.maximum(np.mean(mag, axis=1, keepdims=True), 1e-5))
    audio = np.zeros((24, len(times)), dtype=np.float32)
    for k in np.flatnonzero((f>=55)&(f<=4200)):
        pitch = 69+12*np.log2(f[k]/440); nearest = int(round(pitch))
        weight = np.exp(-.5*((pitch-nearest)/.3)**2)/max(f[k],55)**.3
        audio[nearest%12] += white[k]*weight
        if f[k]<=270: audio[12+nearest%12] += white[k]*weight
    audio = p.normalize(np.concatenate([p.normalize(audio[:12]),p.normalize(audio[12:])*.45]))
    score = p.score_features(notes,total)
    cost = np.maximum(0,1-score.T@audio).astype(np.float32)
    n,m = cost.shape
    acc = np.full((n+1,m+1),np.inf,dtype=np.float64);acc[0,:]=0
    penalty=.06
    # The prefix-min recurrence is equivalent to the three standard DTW steps.
    # It avoids a Python loop over every audio frame and retains the full matrix
    # for a traceable backtrace.
    for i in range(1,n+1):
        c=cost[i-1].astype(np.float64)
        base=c+np.minimum(acc[i-1,1:]+penalty,acc[i-1,:-1])
        prefix=np.cumsum(c+penalty)
        acc[i,1:]=prefix+np.minimum.accumulate(base-prefix)
    j=int(np.argmin(acc[n,1:]))+1;i=n;path=[]
    while i>0 and j>0:
        path.append((i-1,j-1))
        choices=[acc[i-1,j-1],acc[i-1,j]+penalty,acc[i,j-1]+penalty]
        step=int(np.argmin(choices))
        if step==0:i-=1;j-=1
        elif step==1:i-=1
        else:j-=1
    path.reverse()
    buckets=[[] for _ in range(n)]
    for i,j in path:buckets[i].append(j)
    mapping=np.array([np.median(x) if x else np.nan for x in buckets]);good=np.flatnonzero(np.isfinite(mapping));mapping=np.interp(np.arange(n),good,mapping[good])
    def stamp(quarter):return round(float(times[int(round(mapping[int(np.clip(round(quarter/.125),0,n-1))]))]),6)
    for note in notes:note.update(onset_seconds=stamp(note['quarter']),status="unreviewed_audio_alignment_candidate")
    for measure in measures:measure.update(recording_seconds=stamp(measure['quarter']),status="unreviewed_audio_alignment_candidate")
    result={"method":"subsequence_whitened_chroma_dtw_v1","status":"pending_not_accepted","recording":"../../assets/beethoven-op125-4-coates-damian-full.mp3","score":"../../assets/beethoven-op125-4-m93-164.musicxml","recording_original_timeline":True,"analysis_range_seconds":[0,420],"selected_score_measures":[93,164],"frame_seconds":hop/sr,"score_step_quarters":.125,"score_quarters":total,"start_seconds":stamp(0),"last_attack_seconds":stamp(total),"path_mean_chroma_cost":float(np.mean([cost[i,j] for i,j in path])),"elapsed_seconds":round(time.perf_counter()-started,3),"notes":notes,"measures":measures,"limitations":["No independent labels: chroma distance is not onset timing error.","Selected excerpt only; the rest of the movement is navigation only.","Mixed orchestral audio: matching does not prove each instrument is separately audible.","No physical note-off estimation or accepted note precision."]}
    (OUT/"candidates.json").write_text(json.dumps(result,indent=2)+"\n",encoding="utf-8")
    print(json.dumps({k:v for k,v in result.items() if k not in ['notes','measures']}))


if __name__ == '__main__':main()
