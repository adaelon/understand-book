"""Attach inspectable rhythm candidates to native PDF noteheads.

The PDF stays the displayed score. Motor-voice rhythm supplies score-time
candidates for audio alignment; ambiguous notation remains explicitly marked.
"""
from pathlib import Path
from collections import Counter,defaultdict
import bisect,json,zipfile,xml.etree.ElementTree as ET
import numpy as np

ROOT=Path(__file__).resolve().parents[2];WORK=ROOT/'.understand-book/erlkoenig'
# Read against the two saved native-score review sheets. These include
# cross-staff triplets, simultaneous duplets/triplets, and the final rests.
REVIEWED_BEATS={
    **{m:[i/3 for i in range(12)] for m in [20,57,67,73,75,98,100,116,119,120,121,122,124,126,128,129,130]},
    96:[0,1/3,.5,2/3,1,4/3,1.5,5/3,2,7/3,8/3,3,10/3,11/3],
    117:[0,0,1/3,2/3,1,4/3,1.5,5/3,2,7/3,8/3,3,10/3,11/3],
    118:[0,1/3,2/3,1,4/3,1.5,5/3,2,7/3,8/3,3,10/3,3.5,11/3],
    144:[0,1/3,2/3,1,4/3,1.5,5/3,2,7/3,8/3,3,10/3,11/3],
    146:[0,1.25,1.5,1.75,2,3,3.5],147:[0,1,1.5,2,3],148:[0],
}
REVIEWED_TRIPLET_FLOW={20,57,67,73,75,98,100,116,119,120,121,122,124,126,128,129,130}

def group(items):
    result=[]
    for item in sorted(items,key=lambda c:c['x']):
        if not result or item['x']-result[-1][0]['x']>7:result.append([item])
        else:result[-1].append(item)
    return result

REST_QUARTERS={'rest_whole':4,'rest_half':2,'rest_quarter':1,'rest_eighth':.5,'rest_16th':.25,'rest_32nd':.125}

def source_rests(p):
    rests=[]
    for glyph in p['glyphs']:
        if glyph['kind'] not in REST_QUARTERS:continue
        system=min((s for s in p['systems'] if s['page']==glyph['page']),key=lambda s:min(abs(glyph['y']-np.mean(st)) for st in s['staves']))
        staff=min(range(2),key=lambda i:abs(glyph['y']-np.mean(system['staves'][i])))+1
        index=min(len(system['bars'])-2,max(0,bisect.bisect_right(system['bars'],glyph['x'])-1))
        rests.append(dict(glyph,measure=system['first_measure']+index,system=system['index'],staff=staff,source_id=f'pdf-rest-{glyph["draw_order"]}',nominal_quarters=REST_QUARTERS[glyph['kind']]))
    return sorted(rests,key=lambda r:(r['measure'],r['x'],r['staff']))

def main():
    p=json.loads((WORK/'transcription/vector-score.json').read_text(encoding='utf-8'))
    byid={n['source_id']:n for n in p['notes']};measures=[];rests=source_rests(p)
    # Tie identities are accepted only between real matched heads; unrelated
    # slurs are retained visually by the original PDF and never become ties.
    byomr={(n['page'],n['omr_id']):n for n in p['notes'] if n['omr_id']}
    for n in p['notes']:n['tie']='none'
    with zipfile.ZipFile(WORK/'omr/score.omr') as z:
        for page in range(1,10):
            root=ET.fromstring(z.read(f'sheet#{page}/sheet#{page}.xml'))
            relations=list(root.iter('relation'))
            for slur in root.iter('slur'):
                if slur.get('tie')!='true':continue
                ends={r.find('slur-head').get('side'):byomr.get((page,r.get('target'))) for r in relations if r.get('source')==slur.get('id') and r.find('slur-head') is not None}
                a,b=ends.get('LEFT'),ends.get('RIGHT')
                if a and b and a['midi']==b['midi']:
                    a['tie']='continue' if a['tie']=='stop' else 'start';b['tie']='continue' if b['tie']=='start' else 'stop'
            for shift in root.iter('octave-shift'):
                line=shift.find('line');staff=int(shift.get('staff'));system=(staff-1)//2+1
                if line is None:continue
                a=float(line.find('p1').get('x'))*.24;b=float(line.find('p2').get('x'))*.24
                for n in p['notes']:
                    if n['page']==page and n['system']==system and n['staff']==(staff-1)%2+1 and a-4<=n['center_x']<=b+4:
                        n['octave_shift']=12 if shift.get('kind')=='ALTA' else -12;n['midi']+=n['octave_shift']
    for m in range(1,149):
        ns=[n for n in p['notes'] if n['measure']==m];cs=[c for c in p['chords'] if c['measure']==m];system=next(s for s in p['systems'] if s['first_measure']<=m<s['first_measure']+len(s['bars'])-1)
        left,right=system['bars'][m-system['first_measure']:m-system['first_measure']+2]
        choices=[];mr=[r for r in rests if r['measure']==m]
        for staff in [1,2]:
            short=[dict(c) for c in cs if c['staff']==staff and c['nominal_quarters']<=.75]
            short += [dict(r,id=r['source_id'],rest=True) for r in mr if r['staff']==staff and r['nominal_quarters']<=.75]
            groups=group(short)
            total=sum(min(c['nominal_quarters'] for c in g) for g in groups)
            error=min(abs(total-4),abs(total-6))
            choices.append((error,-len(groups),staff,groups,total))
        error,_,staff,groups,total=min(choices)
        if len(groups)>=7:
            scale=2/3 if abs(total-6)<abs(total-4) else 1
            # Candidate correction is recorded below, never hidden as a fully
            # reconstructed duration when engraving/voices leave an ambiguity.
            if abs(total*scale-4)>.05:scale=4/max(total,.1)
            xs=[];beats=[];cursor=0
            for g in groups:
                xs.append(float(np.median([c['x'] for c in g])));beats.append(cursor)
                cursor+=min(c['nominal_quarters'] for c in g)*scale
            xs.append(right-3);beats.append(4)
            method='native_motor_rhythm' if error<.01 else 'native_motor_rhythm_candidate'
        else:
            # The recitative/closing measures have no continuous motor voice.
            # Use their native note/rest spacing for alignment candidates only.
            g=group(cs);xs=[float(np.median([c['x'] for c in a])) for a in g]
            weights=[min(c['nominal_quarters'] for c in a) for a in g];cum=np.r_[0,np.cumsum(weights)]
            beats=(cum[:-1]*4/max(sum(weights),.1)).tolist();xs.append(right-3);beats.append(4);method='sparse_notation_candidate'
        override=REVIEWED_BEATS.get(m)
        reviewed_groups=group(cs)
        if override:
            if len(override)!=len(reviewed_groups):raise ValueError(f'Native review coordinates changed in measure {m}')
            method='native_score_visual_review'
            reviewed={c['id']:b for g,b in zip(reviewed_groups,override) for c in g}
        for c in cs:
            # Engraved seconds and multiple voices displace a head sideways.
            # Match the nearby rhythmic column before interpolating an extra
            # subdivision between two established motor attacks.
            closest=int(np.argmin(abs(np.array(xs[:-1])-c['x'])))
            beat=beats[closest] if abs(xs[closest]-c['x'])<7.1 else float(np.interp(c['x'],xs,beats))
            beat=reviewed[c['id']] if override else round(beat*12)/12
            c['beat']=min(beat,3.916666667)
        # Tuplets belong to individual rhythmic groups. Sustained melody and
        # bass notes keep their written values even above/below a triplet motor.
        triplets=set()
        for _,_,_,staff_groups,staff_total in choices:
            if len(staff_groups)==12 and abs(staff_total-6)<.01:
                triplets.update(c.get('id') for g in staff_groups for c in g if c['nominal_quarters']<=.75)
        beam_chords=defaultdict(list)
        for c in cs:
            for beam in c['beams']:beam_chords[beam].append(c)
        for members in beam_chords.values():
            members=sorted(members,key=lambda c:c['beat'])
            if len(members)<3 or any(c['nominal_quarters']>.75 for c in members):continue
            gaps=[b['beat']-a['beat'] for a,b in zip(members,members[1:])]
            if all(abs(gap-a['nominal_quarters']*2/3)<.01 for a,gap in zip(members,gaps)):
                triplets.update(c['id'] for c in members)
        for c in cs:
            beat=c['beat'];duration=c['nominal_quarters']*(2/3 if c['id'] in triplets else 1)
            if m in REVIEWED_TRIPLET_FLOW and c['nominal_quarters']<=.75:duration=1/3
            if m==146:duration=[1,.25,.25,.25,1,.5,.5][override.index(beat)]
            for nid in c['notes']:
                n=byid[nid];n['beat']=min(beat,3.916666667);n['duration_beats']=min(duration,4-n['beat']);n['rhythm_status']=method
        review_x=[float(np.median([c['x'] for c in g])) for g in reviewed_groups] if override else xs[:-1]
        review_b=override if override else beats[:-1]
        for rest in mr:
            nominal=rest['nominal_quarters']
            closest=int(np.argmin(abs(np.array(review_x)-rest['x'])))
            beat=review_b[closest] if abs(review_x[closest]-rest['x'])<7.1 else round(float(np.interp(rest['x'],review_x,review_b))*12)/12
            if rest['kind']=='rest_whole':beat=0
            if m==146:beat=1
            if m==147:beat=3.5
            if m==148:beat=1 if rest['kind']=='rest_quarter' else 2
            rest['beat']=beat;rest['duration_beats']=nominal*(2/3 if rest['source_id'] in triplets else 1)
            if m in REVIEWED_TRIPLET_FLOW and nominal<=.75:rest['duration_beats']=1/3
            if beat+rest['duration_beats']>4.001:raise ValueError(f'Rest exceeds measure {m}: {rest}')
        measures.append(dict(measure=m,page=system['page'],method=method,motor_staff=staff,motor_columns=len(groups),nominal_sum=total,error=error))
    result=dict(notes=p['notes'],rests=rests,measures=measures,version='erlkoenig_native_rhythm_candidates.v2')
    (WORK/'transcription/rhythm-candidates.json').write_text(json.dumps(result,ensure_ascii=False,separators=(',',':')),encoding='utf-8')
    print(json.dumps(dict(notes=len(p['notes']),methods=Counter(m['method'] for m in measures),ambiguous=[(m['measure'],m['motor_columns'],m['nominal_sum']) for m in measures if m['method']!='native_motor_rhythm'],ties=Counter(n['tie'] for n in p['notes']),octave_notes=sum('octave_shift' in n for n in p['notes']))))

if __name__=='__main__':main()
