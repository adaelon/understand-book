"""Align native notation to the unchanged recording with bounded local tempo."""
from pathlib import Path
import json,time
import numpy as np
from scipy.ndimage import gaussian_filter1d
from scipy.signal import find_peaks
from alignment_probe import audio_features,normalize

ROOT=Path(__file__).resolve().parents[2];WORK=ROOT/'.understand-book/erlkoenig';STEP=1/12
ANCHORS=[(0,1.38),(4,2.7),(19*4,30),(35*4,56),(49*4,79),(68*4,110),(85*4,135),(103*4,166),(111*4,177.06),(112*4,178.84),(113*4,180.28),(114*4,181.58),(117*4,188),(136*4,218),(143*4,226),(144*4,230.66)]
REVIEWED_BAR_STARTS={4:2.7,111*4:177.06,112*4:178.84,113*4:180.28,114*4:181.58}
# Observed attacks from the source video and recording. No steady-tempo
# interpolation is used across the recitative or the long fermata.
REVIEWED_ATTACKS={
    **{i/3:t for i,t in enumerate([1.38,1.5,1.6,1.7,1.84,1.94,2.04,2.14,2.24,2.36,2.46,2.58])},4:2.7,
    111*4:177.06,112*4:178.84,113*4:180.28,114*4:181.58,
    **{144*4+i/3:t for i,t in enumerate([230.66,230.82,230.92,231.04,231.2,231.44,231.64,231.84,232.04,232.2,232.36,232.56])},
    145*4:232.76,145*4+1.25:236.56,145*4+1.5:236.92,145*4+1.75:237.2,145*4+2:237.5,145*4+3:239.5,145*4+3.5:240.5,
    146*4:241.82,146*4+1:244.56,146*4+1.5:248.86,146*4+2:250.66,146*4+3:254,147*4:255.44,
}

def bounded_dtw(score,audio,times):
    cost=np.maximum(0,1-score.T@audio).astype(np.float32);n,m=cost.shape
    trace=np.zeros((n,m),np.uint8);previous=np.full(m+1,np.inf,np.float32);previous[0]=0;older=previous.copy()
    hard_rows={round(q/STEP)+1:t for q,t in REVIEWED_BAR_STARTS.items()}
    # Each step consumes both notation and audio. An entire measure can no
    # longer disappear into an unrestricted run of score-only steps.
    for i in range(1,n+1):
        current=np.full(m+1,np.inf,np.float32)
        center=int(np.searchsorted(times,np.interp((i-1)*STEP,*np.array(ANCHORS).T)))
        if i in hard_rows:center=int(np.searchsorted(times,hard_rows[i]))
        band=2 if i in hard_rows else 350
        for j in range(max(1,center-band),min(m,center+band)+1):
            c=cost[i-1,j-1];best=previous[j-1]+c;step=1
            if j>=2:
                value=previous[j-2]+cost[i-1,j-2]+c+.008
                if value<best:best,step=value,2
            if j>=3:
                value=previous[j-3]+cost[i-1,j-3]+cost[i-1,j-2]+c+.02
                if value<best:best,step=value,3
            if i>=2 and i-1 not in hard_rows:
                value=older[j-1]+cost[i-2,j-1]+c+.008
                if value<best:best,step=value,4
            current[j]=best;trace[i-1,j-1]=step
        older,previous=previous,current
    if not np.isfinite(previous[m]):raise ValueError('Bounded recording alignment did not reach the reviewed ending')
    i,j=n,m;buckets=[[] for _ in range(n)];path=[]
    while i>0 and j>0:
        step=int(trace[i-1,j-1])
        if step in [1,2,3]:
            for k in range(j-step,j):buckets[i-1].append(k);path.append((i-1,k))
            i-=1;j-=step
        elif step==4:
            buckets[i-2].append(max(0,j-1.5));buckets[i-1].append(j-1);path.extend([(i-2,j-1),(i-1,j-1)]);i-=2;j-=1
        else:raise ValueError('Broken bounded alignment path')
    mapping=np.array([np.median(b) if b else np.nan for b in buckets]);good=np.flatnonzero(np.isfinite(mapping))
    mapping=np.interp(np.arange(n),good,mapping[good])
    return np.interp(mapping,np.arange(m),times),dict(mean_chroma_cost=float(np.mean([cost[i,j] for i,j in path])),path_pairs=len(path))

def match_attacks(attacks,target,score,audio,times,peaks):
    """Match nearby attack sequences without long score deletions or tempo jumps."""
    source=[q for q in attacks if q<576];expected=target[:len(source)]
    selected=peaks[times[peaks]<230.66];pt=times[selected];n,m=len(source),len(selected)
    sf=np.clip(np.rint(np.array(source)/STEP).astype(int),0,score.shape[1]-1)
    af=normalize(audio[:,selected]*.6+audio[:,np.minimum(selected+1,audio.shape[1]-1)]*.4)
    cost=np.maximum(0,1-score[:,sf].T@af)
    acc=np.full((n,m),np.inf,np.float32);trace=np.zeros((n,m,2),np.uint8)
    required=np.r_[0,np.cumsum([q in REVIEWED_ATTACKS for q in source])]
    for a in range(n):
        lower=max(0,int(np.searchsorted(pt,expected[a]-.6)));upper=min(m,int(np.searchsorted(pt,expected[a]+.6))+1)
        for b in range(lower,upper):
            exact=REVIEWED_ATTACKS.get(source[a])
            if exact is not None and abs(pt[b]-exact)>.011:continue
            local=cost[a,b]+abs(pt[b]-expected[a])*.25
            if a<3 and b<4 and required[a]==0:acc[a,b]=local+a*.4+b*.3
            for da in range(1,min(3,a)+1):
                if required[a]-required[a-da+1]>0:continue
                dq=source[a]-source[a-da]
                for db in range(1,min(4,b)+1):
                    elapsed=pt[b]-pt[b-db]
                    # The actual fast movement has neither 8 ms triplets nor
                    # multi-second stalls. Keep score skips local (at most two)
                    # and constrain elapsed time to the recording's local pace.
                    if not .15*dq<=elapsed<=1.2*dq:continue
                    value=acc[a-da,b-db]+local+(da-1)*.4+(db-1)*.3
                    if value<acc[a,b]:acc[a,b]=value;trace[a,b]=[da,db]
    endings=[(float(acc[a,b]+(n-1-a)*.4+(m-1-b)*.3),a,b) for a in range(max(0,n-3),n) for b in range(max(0,m-4),m)]
    value,a,b=min(endings)
    if not np.isfinite(value):
        missing=[(q,float(expected[a])) for a,q in enumerate(source) if q in REVIEWED_ATTACKS and not np.isfinite(acc[a]).any()]
        raise ValueError(f'The local attack sequence cannot reach reviewed boundaries: {missing[:5]}')
    matched={}
    while True:
        matched[source[a]]=float(pt[b]);da,db=map(int,trace[a,b])
        if not da:break
        a-=da;b-=db
    return matched

def main():
    started=time.perf_counter();p=json.loads((WORK/'transcription/rhythm-candidates.json').read_text(encoding='utf-8'));notes=p['notes']
    cache=WORK/'transcription/audio-features.npz'
    if cache.exists():
        data=np.load(cache);audio,times,flux=data['audio'],data['times'],data['flux'];info=json.loads(str(data['info']))
    else:
        audio,times,flux,info=audio_features(WORK/'workspace/music/audio/recording.m4a');np.savez_compressed(cache,audio=audio,times=times,flux=flux,info=json.dumps(info))
    # The free-tempo ending has direct source observations. Align the preceding
    # 144 measures to its actual boundary, then join the observed ending.
    cutoff=144*4;n=round(cutoff/STEP);features=np.zeros((24,n),np.float32)
    for note in notes:
        lo=round(((note['measure']-1)*4+note['beat'])/STEP);hi=min(n,max(lo+1,round(((note['measure']-1)*4+note['beat']+note['duration_beats'])/STEP)))
        if lo>=n:continue
        features[note['midi']%12,lo:hi]+=1
        if note['midi']<60:features[12+note['midi']%12,lo:hi]+=1
    features=gaussian_filter1d(features,.55,axis=1)
    score=normalize(np.concatenate((normalize(features[:12]),normalize(features[12:])*.45),axis=0))
    audio_end=int(np.searchsorted(times,230.66));mapping,metrics=bounded_dtw(score,audio[:,:audio_end],times[:audio_end])
    tail=sorted((q,t) for q,t in REVIEWED_ATTACKS.items() if q>=cutoff)+[(592,info['duration_seconds'])]
    def score_time(q):return float(np.interp(q,*np.array(tail).T)) if q>=cutoff else float(np.interp(q/STEP,np.arange(n),mapping))
    attacks=sorted(set(round(((note['measure']-1)*4+note['beat'])*12)/12 for note in notes if note['tie'] not in ['continue','stop']))
    target=np.array([score_time(q) for q in attacks]);peaks,_=find_peaks(flux,distance=3,prominence=float(max(flux))*.003);peaks=np.unique(np.r_[0,peaks])
    matched=match_attacks(attacks,target,score,audio,times,peaks)
    # Only score attacks actually present in the notation count as observations.
    matched.update({q:t for q,t in REVIEWED_ATTACKS.items() if q in attacks})
    mq=sorted(matched);mt=[matched[q] for q in mq]
    onsets={q:matched.get(q,float(np.interp(q,mq,mt))) for q in attacks}
    bad=[(a,onsets[a],b,onsets[b]) for a,b in zip(attacks,attacks[1:]) if onsets[b]<=onsets[a]]
    if bad:raise ValueError(f'Distinct score attacks are not ordered in the recording: {bad[:6]}')
    # Unobserved attacks are interpolated only between the nearby sequence
    # matches; the matcher cannot skip a whole bar or create implausible tempo.
    def mapped(q):
        if q in onsets:return onsets[q]
        if q>=cutoff:return score_time(q)
        return float(np.interp(q,mq,mt))
    measures=[]
    for m in range(1,149):
        q=(m-1)*4;start=onsets.get(q,mapped(q));end=onsets.get(q+4,mapped(q+4)) if m<148 else info['duration_seconds']
        if m==1:start=0
        measures.append(dict(measure=m,start_ms=round(start*1000,3),end_ms=round(end*1000,3)))
    for k in range(147):measures[k]['end_ms']=measures[k+1]['start_ms']
    for row in measures[:144]:
        duration=row['end_ms']-row['start_ms']
        if not 600<duration<4000:raise ValueError(f'Unreviewed tempo anomaly in measure {row["measure"]}: {duration} ms')
    aligned=[]
    for note in notes:
        q=round(((note['measure']-1)*4+note['beat'])*12)/12;occurrence=measures[note['measure']-1]
        start=max(occurrence['start_ms'],min(occurrence['end_ms']-.1,onsets.get(q,mapped(q))*1000))
        end=max(start+40,mapped(q+note['duration_beats'])*1000);continuation=note['tie'] in ['continue','stop'];observed=q in matched
        aligned.append(dict(source_id=note['source_id'],measure=note['measure'],start_ms=round(start,3),end_ms=round(min(info['duration_seconds']*1000,end),3),attack=observed and not continuation,unobserved=not observed and not continuation,dtw_ms=round(score_time(q)*1000,3)))
    for row in measures:
        selected=[note for note in aligned if note['measure']==row['measure']];row['note_complete']=all(not note['unobserved'] for note in selected)
    attack_coverage=[]
    for k,q in enumerate(attacks):
        start=onsets[q]*1000 if k else 0
        end=onsets[attacks[k+1]]*1000 if k+1<len(attacks) else info['duration_seconds']*1000
        status='accepted' if q in matched else 'pending'
        if attack_coverage and attack_coverage[-1]['status']==status:attack_coverage[-1]['end_ms']=round(end,3)
        else:attack_coverage.append(dict(start_ms=round(start,3),end_ms=round(end,3),status=status))
    metrics.update(score_attacks=len(attacks),detected_attacks=len(peaks),matched_attacks=len(matched),interpolated_attacks=len(attacks)-len(matched),note_complete_measures=sum(row['note_complete'] for row in measures),seconds=round(time.perf_counter()-started,2))
    result=dict(version='erlkoenig_recording_alignment.v2',method='bounded_slope_chroma_bass_dtw_and_tempo_constrained_attack_sequence',audio=info,notes=aligned,measures=measures,attack_coverage=attack_coverage,metrics=metrics,limitations=['Automatic alignment without independent human onset labels.','Unobserved attacks are not highlighted; observed tones remain visible across local gaps.','Note ends map written durations; they are not measured key releases or pedal-off times.'])
    (WORK/'transcription/audio-alignment.json').write_text(json.dumps(result,ensure_ascii=False,separators=(',',':')),encoding='utf-8')
    print(json.dumps(dict(metrics=metrics,measures=[measures[m-1] for m in [1,6,28,32,108,112,114,144,146,147,148]])))

if __name__=='__main__':main()
