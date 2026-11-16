"""Recover notation geometry from the user's vector PDF, preserving real marks.

This entry writes an inspectable intermediate; importing it as accepted music is
a separate step after pitch/rhythm and recording evidence have been reviewed.
"""
from __future__ import annotations
from collections import defaultdict, Counter
from pathlib import Path
import argparse
import bisect
import json
import math
import xml.etree.ElementTree as ET
import zipfile

import fitz
import numpy as np
from scipy.spatial import cKDTree

ROOT = Path(__file__).resolve().parents[2]
WORK = ROOT / '.understand-book/erlkoenig'
GLYPHS = {
    (0., -39.): 'head_black', (278., 64.): 'head_half', (198., 133.): 'head_whole',
    (441., -245.): 'clef_g', (343., -190.): 'clef_g',
    (582., -120.): 'clef_f', (509., -105.): 'clef_f',
    (20., 110.): 'flat', (136., 186.): 'sharp', (0., -188.): 'natural',
    (0., 120.): 'double_sharp', (280., 106.): 'double_flat',
    (0., -115.): 'rest_whole', (0., 10.): 'rest_half', (107., 292.): 'rest_quarter',
    (0., 123.): 'rest_eighth', (133., -111.): 'rest_16th', (69., -54.): 'rest_32nd',
    (179., -646.): 'flag_eighth', (179., 646.): 'flag_eighth',
    (206., -748.): 'flag_16th', (216., 730.): 'flag_16th',
    (0., -48.): 'dot_below', (0., 48.): 'dot_above',
    (161., 326.): 'tuplet_three',
}


def transform(record, x, y, heights):
    a,b,c,d,e,f = record['ctm']
    return [a*x+c*y+e, heights[record['page']-1]-(b*x+d*y+f)]


def records(pdf, cache):
    if cache.exists():
        return json.loads(cache.read_text())
    from pypdf import PdfReader
    from pypdf.generic import ContentStream
    def mul(a,b):
        return (a[0]*b[0]+a[1]*b[2], a[0]*b[1]+a[1]*b[3], a[2]*b[0]+a[3]*b[2], a[2]*b[1]+a[3]*b[3], a[4]*b[0]+a[5]*b[2]+b[4], a[4]*b[1]+a[5]*b[3]+b[5])
    result=[]; reader=PdfReader(pdf)
    for pi,page in enumerate(reader.pages,1):
        matrix=(1,0,0,1,0,0);stack=[];current=[]
        for args,op in ContentStream(page.get_contents(),reader).operations:
            values=tuple(float(x) for x in args) if op in (b'cm',b'm',b'l',b'c',b'v',b'y',b're') else ()
            if op==b'q':stack.append(matrix)
            elif op==b'Q':matrix=stack.pop()
            elif op==b'cm':matrix=mul(values,matrix)
            elif op in (b'm',b'l',b'c',b'v',b'y',b'h',b're'):current.append([values,op.decode()])
            elif op in (b'f',b'f*',b'B',b'B*',b'S',b's',b'n'):
                if current:result.append(dict(page=pi,paint=op.decode(),ctm=matrix,path=current))
                current=[]
    cache.write_text(json.dumps(result,separators=(',',':')))
    return result


def primitives(pdf):
    doc=fitz.open(pdf); heights=[page.rect.height for page in doc]
    data=records(pdf,WORK/'inspection/pdf-vectors.json')
    glyphs=[];lines=[]
    for order,r in enumerate(data):
        path=r['path']; first=tuple(path[0][0])
        if r['paint'] in ['f','f*'] and len(path)==18 and all(op in ['m','c','h'] for _,op in path):
            pts=[transform(r,*c[-2:],heights) for c,op in path if c]
            if pts:
                xs,ys=zip(*pts);w=max(xs)-min(xs);h=max(ys)-min(ys)
                if 1.7<w<2.7 and abs(w-h)<.1:
                    glyphs.append(dict(kind='dot_circle',page=r['page'],x=(max(xs)+min(xs))/2,y=(max(ys)+min(ys))/2,scale=0,draw_order=order))
        if r['paint'] in ['f','f*','B','B*'] and path[0][1]=='m' and first in GLYPHS:
            kind=GLYPHS[first]
            # Filled noteheads have four actual curves; reject a coincidentally
            # equal move in decorative text paths.
            if kind=='head_black' and len(path)!=6:continue
            x,y=transform(r,0,0,heights)
            glyphs.append(dict(kind=kind,page=r['page'],x=x,y=y,scale=abs(r['ctm'][0]),draw_order=order))
        if r['paint'] not in ['S','s']:continue
        last=None
        for coords,op in path:
            if op=='m':last=coords
            elif op=='l' and last is not None:
                p1=transform(r,*last,heights);p2=transform(r,*coords,heights)
                lines.append(dict(page=r['page'],x1=p1[0],y1=p1[1],x2=p2[0],y2=p2[1],draw_order=order))
                last=coords
            else:last=None
    return doc,glyphs,lines


def dedup(values,tolerance=.3):
    groups=[]
    for v in sorted(values):
        if not groups or v-groups[-1][-1]>tolerance:groups.append([v])
        else:groups[-1].append(v)
    return [sum(g)/len(g) for g in groups]


def staff_systems(doc,lines):
    systems=[];measure=1
    for pi,page in enumerate(doc,1):
        horizontal=defaultdict(list)
        for l in lines:
            if l['page']==pi and abs(l['y1']-l['y2'])<.03 and abs(l['x2']-l['x1'])>20:
                horizontal[round(l['y1'],2)].append(sorted([l['x1'],l['x2']]))
        ys=sorted(y for y,spans in horizontal.items() if sum(b-a for a,b in spans)>page.rect.width*.25)
        clusters=[]
        for y in ys:
            if not clusters or abs(y-clusters[-1][-1]-5.4)>.12:clusters.append([y])
            else:clusters[-1].append(y)
        clusters=[cluster for cluster in clusters if len(cluster)==5]
        if len(clusters)!=10:
            raise ValueError(f'Page {pi}: expected ten real five-line staves, found {clusters}')
        for si in range(5):
            upper,lower=clusters[si*2:si*2+2]
            spans=horizontal[upper[0]];left=min(s[0] for s in spans);right=max(s[1] for s in spans)
            candidates=[];upper_bars=[];lower_bars=[]
            for l in lines:
                if l['page']!=pi or abs(l['x1']-l['x2'])>.03:continue
                lo,hi=sorted([l['y1'],l['y2']])
                if abs(lo-upper[0])<.6 and abs(hi-lower[-1])<.6:candidates.append(l['x1'])
                if abs(lo-upper[0])<.2 and abs(hi-upper[-1])<.2:upper_bars.append(l['x1'])
                if abs(lo-lower[0])<.2 and abs(hi-lower[-1])<.2:lower_bars.append(l['x1'])
            candidates.extend(x for x in upper_bars if any(abs(x-y)<.15 for y in lower_bars))
            bars=[left]+[x for x in dedup(candidates,5) if x>left+5 and x<right-5]+[right]
            text=[]
            for word in page.get_text('words'):
                if word[4].isdigit() and word[0]<left+12 and abs(word[3]-upper[0])<20:text.append(word[4])
            if text and int(text[0])!=measure:raise ValueError(f'Page {pi} system {si+1}: printed {text[0]} != recovered {measure}, bars={bars}')
            systems.append(dict(page=pi,index=si+1,staves=[upper,lower],left=left,right=right,bars=bars,first_measure=measure))
            measure+=len(bars)-1
    if measure!=149:raise ValueError(f'Expected 148 measures, found {measure-1}')
    return systems


def omr_heads(path):
    result=defaultdict(list)
    with zipfile.ZipFile(path) as archive:
        for name in archive.namelist():
            if not name.startswith('sheet#') or not name.endswith('.xml'):continue
            root=ET.fromstring(archive.read(name));page=int(name.split('/')[0].split('#')[1])
            for head in root.iter('head'):
                box=head.find('bounds')
                if box is None:continue
                x=(float(box.get('x'))+float(box.get('w'))/2)*72/300
                y=(float(box.get('y'))+float(box.get('h'))/2)*72/300
                result[page].append(dict(id=head.get('id'),x=x,y=y,staff=int(head.get('staff')),shape=head.get('shape')))
    return result


def recover_rhythm_geometry(result, pdf):
    doc=fitz.open(pdf);heights=[p.rect.height for p in doc]
    beams=[]
    for order,r in enumerate(records(pdf,WORK/'inspection/pdf-vectors.json')):
        if r['paint'] not in ['f','f*']:continue
        points=[]
        for c,op in r['path']:
            if op=='re':
                x,y,w,h=c;points=[transform(r,x,y,heights),transform(r,x+w,y,heights),transform(r,x+w,y+h,heights),transform(r,x,y+h,heights)];break
            if op in ['m','l']:points.append(transform(r,*c,heights))
            elif op!='h':points=[];break
        if len(points)==5 and np.linalg.norm(np.array(points[0])-points[-1])<.01:points.pop()
        if len(points)!=4:continue
        points.sort(key=lambda p:(p[0],p[1]));left=points[:2];right=points[2:]
        if abs(left[0][0]-left[1][0])>.05 or abs(right[0][0]-right[1][0])>.05:continue
        width=right[0][0]-left[0][0];thickness=abs(left[0][1]-left[1][1])
        if width<3 or not 1.5<thickness<3.4:continue
        if abs(thickness-abs(right[0][1]-right[1][1]))>.15:continue
        beams.append(dict(page=r['page'],left=left[0][0],right=right[0][0],y1=sum(p[1] for p in left)/2,y2=sum(p[1] for p in right)/2,thickness=thickness,draw_order=order))
    verticals=[]
    for i,l in enumerate(result['lines']):
        if abs(l['x1']-l['x2'])<.02 and 12<abs(l['y2']-l['y1'])<110:
            verticals.append(dict(l,line_id=i))
    chords={}
    for n in result['notes']:
        options=[];width=(405 if n['kind']=='head_whole' else 314)*n['scale']
        if n['kind']!='head_whole':
            for l in verticals:
                if l['page']!=n['page']:continue
                dx=min(abs(l['x1']-n['x']-.27),abs(l['x1']-n['x']-width+.27))
                lo,hi=sorted([l['y1'],l['y2']])
                if dx<.32 and lo-1.3<n['y']<hi+1.3:
                    options.append((dx+min(abs(n['y']-lo),abs(n['y']-hi))*.0001,l))
        if options:
            l=min(options,key=lambda pair:pair[0])[1];n['stem_line']=l['line_id'];key=f'stem-{l["line_id"]}'
        else:
            key=f'open-{n["page"]}-{n["measure"]}-{n["staff"]}-{round(n["x"],1)}';l=None;n['stem_line']=None
        chord=chords.setdefault(key,dict(id=key,page=n['page'],measure=n['measure'],x=n['x'],staff=n['staff'],notes=[],stem=l))
        chord['notes'].append(n['source_id']);n['chord_id']=key
    notes={n['source_id']:n for n in result['notes']}
    for chord in chords.values():
        ns=[notes[i] for i in chord['notes']];stem=chord['stem'];linked=[]
        chord['stem_direction']='up' if stem and max(n['y'] for n in ns)>sum([stem['y1'],stem['y2']])/2 else 'down'
        if stem:
            tip=min(stem['y1'],stem['y2']) if chord['stem_direction']=='up' else max(stem['y1'],stem['y2'])
            for i,b in enumerate(beams):
                if b['page']!=chord['page'] or not b['left']-.4<=stem['x1']<=b['right']+.4:continue
                y=b['y1']+(stem['x1']-b['left'])/(b['right']-b['left'])*(b['y2']-b['y1'])
                if abs(y-tip)<11:linked.append(i)
        chord['beams']=linked
        chord['beam_count']=len(linked)
        if any(n['kind']=='head_whole' for n in ns):base=4
        elif any(n['kind']=='head_half' for n in ns):base=2
        elif linked:base=1/(2**len(linked))
        else:
            flags=[g for g in result['glyphs'] if stem and g['page']==chord['page'] and g['kind'].startswith('flag_') and abs(g['x']-stem['x1'])<.6 and min(abs(g['y']-stem['y1']),abs(g['y']-stem['y2']))<25]
            base=.25 if any(g['kind']=='flag_16th' for g in flags) else .5 if flags else 1
        dots=[]
        for n in ns:
            width=(405 if n['kind']=='head_whole' else 314)*n['scale']
            for g in result['glyphs']:
                if g['page']!=n['page'] or not g['kind'].startswith('dot_'):continue
                x=g['x']+48*g['scale'];y=g['y']
                if 1<x-n['x']-width<9 and min(abs(y-n['y']),abs(y-n['y']+2.7))<.5:dots.append(g['draw_order'])
        chord['dotted']=bool(dots);chord['nominal_quarters']=base*(1.5 if dots else 1)
        chord['stem_direction']='up' if stem and max(n['y'] for n in ns)>sum([stem['y1'],stem['y2']])/2 else 'down'
    result['beams']=beams;result['chords']=list(chords.values())
    return result


def recover(pdf):
    doc,glyphs,lines=primitives(pdf);systems=staff_systems(doc,lines)
    original_heads=omr_heads(WORK/'omr/score.omr')
    trees={p:cKDTree([[h['x'],h['y']] for h in heads]) for p,heads in original_heads.items()}
    notes=[];accidental_issues=[]
    for system in systems:
        page=system['page'];top=system['staves'][0][0];bottom=system['staves'][1][-1]
        gs=[g for g in glyphs if g['page']==page and top-35<g['y']<bottom+35]
        heads=[g for g in gs if g['kind'].startswith('head_')]
        # Restrict each head to its physical grand staff; adjacent systems are
        # separated by much more than the widest ledger excursion here.
        mids=[sum(staff)/5 for staff in system['staves']]
        for h in heads:
            if not (top-28<h['y']<bottom+28):continue
            xcenter=h['x']+(405 if h['kind']=='head_whole' else 314)*h['scale']/2
            distance,index=trees[page].query([xcenter,h['y']])
            detected=original_heads[page][index]
            staff=(detected['staff']-1)%2 if distance<1.2 and (detected['staff']-1)//2==system['index']-1 else min(range(2),key=lambda s:abs(h['y']-mids[s]))
            h=dict(h,center_x=xcenter,staff=staff+1,omr_id=detected['id'] if distance<1.2 else None,omr_distance=float(distance))
            bi=min(len(system['bars'])-2,max(0,bisect.bisect_right(system['bars'],h['x']+.05)-1))
            h['measure']=system['first_measure']+bi
            h['system']=system['index'];h['system_top']=top
            notes.append(h)
    # Deduplicate ledger excursions that were considered by adjacent system windows.
    unique={n['draw_order']:n for n in notes};notes=list(unique.values())
    for system in systems:
        selected=[n for n in notes if n['page']==system['page'] and n['system']==system['index']]
        for staff_index in range(2):
            staff=system['staves'][staff_index];mid=sum(staff)/5
            clefs=[]
            for g in glyphs:
                if g['page']!=system['page'] or g['kind'] not in ['clef_g','clef_f']:continue
                expected=staff[-1]-5.4 if g['kind']=='clef_g' else staff[0]+5.4
                if abs(g['y']-expected)<.3:clefs.append(g)
            clefs.sort(key=lambda g:g['x'])
            if not clefs:raise ValueError(f'Missing clef: p{system["page"]}, system {system["index"]}, staff {staff_index+1}')
            for measure in range(system['first_measure'],system['first_measure']+len(system['bars'])-1):
                entries=sorted([n for n in selected if n['staff']==staff_index+1 and n['measure']==measure],key=lambda n:(n['x'],n['y']))
                state={}
                for n in entries:
                    clef=next((c for c in reversed(clefs) if c['x']<n['x']),clefs[0])
                    anchor=32 if clef['kind']=='clef_g' else 24
                    step=anchor-round((n['y']-clef['y'])/2.7)
                    n['step']=step;n['letter']='CDEFGAB'[step%7];n['octave']=step//7
                    local=[]
                    for g in glyphs:
                        if g['page']!=n['page'] or g['kind'] not in ['flat','sharp','natural','double_sharp','double_flat']:continue
                        if abs(g['y']-n['y'])<.4 and .5<n['x']-g['x']<15 and g['x']>system['bars'][measure-system['first_measure']]+.5:
                            # Header flats precede the clef's following first note by
                            # at least 20 points in this engraving; the local window
                            # only contains a written accidental belonging to a note.
                            between=any(other['x']>g['x']+2 and other['x']<n['x']-1 and abs(other['y']-n['y'])<.4 for other in entries)
                            if not between:local.append(g)
                    if local:
                        g=max(local,key=lambda g:g['x']);state[step]={'flat':-1,'sharp':1,'natural':0,'double_sharp':2,'double_flat':-2}[g['kind']]
                        n['accidental']=g['kind']
                    n['alter']=state.get(step,-1 if n['letter'] in 'BE' and (measure<83 or measure>=131) else 0)
                    n['midi']=12*(n['octave']+1)+[0,2,4,5,7,9,11][step%7]+n['alter']
                    n['clef']=clef['kind']
    notes.sort(key=lambda n:(n['measure'],n['x'],n['staff'],n['y']))
    for i,n in enumerate(notes,1):n['source_id']=f'pdf-head-{i}'
    output=WORK/'transcription';output.mkdir(exist_ok=True)
    result=dict(version='erlkoenig_pdf_vectors.v1',source=str(pdf),systems=systems,notes=notes,glyphs=glyphs,lines=lines)
    recover_rhythm_geometry(result,pdf)
    (output/'vector-score.json').write_text(json.dumps(result,ensure_ascii=False,separators=(',',':')),encoding='utf-8')
    print(json.dumps(dict(pages=len(doc),systems=len(systems),measures=sum(len(s['bars'])-1 for s in systems),notes=len(notes),heads=Counter(n['kind'] for n in notes),unmatched=sum(n['omr_id'] is None for n in notes),opening=[(n['letter'],n['octave'],n['alter']) for n in notes[:6]],chords=len(result['chords']),beams=len(result['beams']),durations=Counter(c['nominal_quarters'] for c in result['chords']))))
    return result


if __name__=='__main__':
    parser=argparse.ArgumentParser();parser.add_argument('--pdf',type=Path,default=WORK/'inputs/score.pdf');args=parser.parse_args()
    recover(args.pdf)
