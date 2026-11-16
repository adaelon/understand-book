"""Prepare the full native-PDF index and automatic recording-following edition."""
from pathlib import Path
from collections import defaultdict,Counter
import json,re,xml.etree.ElementTree as ET
import fitz
import numpy as np
from scipy.spatial import cKDTree

ROOT=Path(__file__).resolve().parents[2];WORK=ROOT/'.understand-book/erlkoenig';WS=WORK/'workspace';NS='http://www.w3.org/2000/svg'

def save(path,value):path.write_text(json.dumps(value,ensure_ascii=False,indent=2)+'\n',encoding='utf-8')

def pages(notes):
    doc=fitz.open(WORK/'inputs/score.pdf');mapped=0
    for page_index,page in enumerate(doc,1):
        root=ET.fromstring(page.get_svg_image(text_as_path=True));root.set('style','color:#000');parents={e:p for p in root.iter() for e in p};ns=[n for n in notes if n['page']==page_index]
        coordinate_notes=defaultdict(list)
        for n in ns:coordinate_notes[(round(n['x'],3),round(n['y'],3))].append(n)
        coordinates=list(coordinate_notes);tree=cKDTree(coordinates);paths=defaultdict(list)
        for e in root.iter():
            if not e.tag.endswith('path'):continue
            nums=re.findall(r'[-+]?\d*\.?\d+',e.get('d',''))[:2]
            if len(nums)!=2 or tuple(map(float,nums)) not in [(0,-39),(278,64),(198,133),(0,-115),(0,10),(107,292),(0,123),(133,-111),(69,-54)]:continue
            matrix=re.findall(r'[-+]?\d*\.?\d+',e.get('transform',''))
            if len(matrix)!=6:continue
            a,b,c,d,x,y=map(float,matrix)
            if abs(a-.0216)>.00001:continue
            distance,index=tree.query([x,y])
            if distance>.03:raise ValueError(f'Unmapped actual SVG notehead on page {page_index}: {x},{y}')
            paths[coordinates[index]].append(e)
        note_paths={}
        for coordinate,source_notes in coordinate_notes.items():
            elements=paths[coordinate]
            if len(elements)!=2*len(source_notes):raise ValueError(f'SVG/source count differs at {page_index}:{coordinate}')
            for i,n in enumerate(sorted(source_notes,key=lambda n:n['draw_order'])):note_paths[n['source_id']]=elements[i*2:i*2+2]
        for n in ns:
            matched=note_paths[n['source_id']]
            if len(matched)!=2 or parents[matched[0]] is not parents[matched[1]]:raise ValueError(f'SVG notehead mapping incomplete: {n["source_id"]}')
            rest=n['kind'].startswith('rest_')
            parent=parents[matched[0]];g=ET.Element(f'{{{NS}}}g',id=n['source_id'],attrib={'class':'rest' if rest else 'note','data-source-id':n['source_id']})
            head=ET.SubElement(g,f'{{{NS}}}g',{'class':'rest-glyph' if rest else 'notehead'});parent.insert(list(parent).index(matched[0]),g)
            for e in matched:
                parent.remove(e)
                if e.get('fill')!='none':e.set('fill','currentColor')
                if e.get('stroke'):e.set('stroke','currentColor')
                head.append(e)
            mapped+=not rest
        identifiers={e.get('id') for e in root.iter() if e.get('id') and not e.get('id').startswith(('pdf-head-','pdf-rest-'))}
        for e in root.iter():
            for key,value in list(e.attrib.items()):
                if key=='id' and value in identifiers:e.set(key,f'pdf{page_index}-{value}')
                elif value.startswith('#') and value[1:] in identifiers:e.set(key,f'#pdf{page_index}-{value[1:]}')
                elif 'url(#' in value:
                    for identifier in identifiers:value=value.replace(f'url(#{identifier})',f'url(#pdf{page_index}-{identifier})')
                    e.set(key,value)
        ET.register_namespace('',NS);ET.register_namespace('xlink','http://www.w3.org/1999/xlink')
        (WS/f'music/score/page-{page_index}.svg').write_bytes(ET.tostring(root,encoding='utf-8',xml_declaration=True))
    return mapped

def main():
    rhythm=json.loads((WORK/'transcription/rhythm-candidates.json').read_text(encoding='utf-8'));timing=json.loads((WORK/'transcription/audio-alignment.json').read_text(encoding='utf-8'));notes=rhythm['notes'];rests=rhythm['rests'];mapped=pages(notes+rests)
    manifest=json.loads((WS/'.build/music/import-candidate.json').read_text(encoding='utf-8'))
    manifest['book_id']='music-schubert-liszt-erlkoenig-native-score-v2';manifest['title']='舒伯特—李斯特 · 魔王';manifest['score']['title']='用户原谱 · 全曲 148 小节 · 原排版';manifest['score']['source']['revision']='user-pdf-20261103-native-v2'
    manifest['parts']=[dict(part_id='piano',name='钢琴',instrument='钢琴',group='键盘',voices=['1:whole','2:whole'])]
    manifest['score_objects']=[dict(source_id=n['source_id'],part_id='piano',measure=str(n['measure']),beat=n['beat'],duration_beats=n['duration_beats'],voice=f'{n["staff"]}:whole',midi_pitch=n['midi'],rest=False,tie=n['tie'],page_id=f'page-{n["page"]}',svg_element_id=n['source_id']) for n in notes]
    manifest['score_objects'] += [dict(source_id=r['source_id'],part_id='piano',measure=str(r['measure']),beat=r['beat'],duration_beats=r['duration_beats'],voice=f'{r["staff"]}:whole',midi_pitch=None,rest=True,tie='none',page_id=f'page-{r["page"]}',svg_element_id=r['source_id']) for r in rests]
    by_measure=defaultdict(list)
    for n in notes+rests:by_measure[n['measure']].append(n['source_id'])
    def mark(mid,measure,value,beat=0):return dict(mark_id=mid,part_id='piano',measure=str(measure),measure_index=measure,beat=beat,staff=None,score_object_ids=[n['source_id'] for n in notes+rests if n['measure']==measure and n['beat']==beat][:12],provenance_id='native-pdf-index',value=value)
    manifest['score_marks']=[mark(f'key-{m}',m,dict(kind='key_signature',fifths=k,encoded_mode=None)) for m,k in [(1,-2),(83,0),(131,-2)]]
    manifest['score_marks'].append(mark('common-time',1,dict(kind='time_signature',components=[dict(beats='4',beat_type=4)],symbol='common')))
    manifest['score_marks'] += [mark(f'direction-{m}',m,dict(kind='direction',words=words,metronome=None,sound_tempo_quarter_bpm=None),beat) for m,beat,words in [(1,0,['Presto agitato','dramatico']),(146,1,['Recit.']),(147,3,['Andante'])]]
    duration=manifest['recording']['duration_ms'];occurrences=[]
    for row in timing['measures']:
        m=row['measure'];occurrences.append(dict(occurrence_id=f'performance-m{m}',order=m,measure=str(m),start_ms=row['start_ms'],end_ms=min(duration,row['end_ms']),score_object_ids=by_measure[m]))
    manifest['occurrences']=occurrences
    manifest['note_alignments']=[dict(occurrence_id=f'performance-m{n["measure"]}',score_object_id=n['source_id'],start_ms=n['start_ms'],end_ms=min(duration,n['end_ms']),attack=n['attack'],unobserved=n['unobserved'],provenance_id='erlkoenig-spectrum-following') for n in timing['notes']]
    manifest['provenance']=[p for p in manifest['provenance'] if p['provenance_id'] not in ['native-pdf-index','erlkoenig-spectrum-following']]
    metrics=timing['metrics'];matched=metrics['matched_attacks'];interpolated=metrics['interpolated_attacks'];total=metrics['score_attacks'];matched_pct=matched/total*100
    manifest['provenance'] += [dict(provenance_id='native-pdf-index',kind='score',description='直接恢复 PDF 的 5309 个真实音头、45 组双谱表与 148 小节；依谱号、调号及临时升降号建立音高索引。24 个特殊节奏小节已对照原谱图核对。上下谱表作为阅读分层，保留原 PDF 排版。',source_url=None,processor='native PDF vector recovery; notation geometry; AI visual score review'),dict(provenance_id='erlkoenig-spectrum-following',kind='audio_analysis',description=f'原录音的 20 ms 频谱、低音色度与谱面动态对齐，再按起音顺序匹配频谱攻击峰。{total} 组谱面起音中 {matched} 组有匹配或结尾校对的攻击峰依据，{interpolated} 组由受节奏约束的谱面时间映射估计；这些未观测音标为 unobserved，不能当作独立检测出的起音。以原视频的谱面位置交叉核对结构及结尾。音尾按谱面音值映射，不表示实测松键或踏板释放。尚无独立人工逐音时间标注。',source_url=None,processor='bounded-slope native-score chroma/bass DTW + local spectral attacks; 20 ms analysis hop')]
    for p in manifest['provenance']:
        if p['provenance_id']=='user-pdf':p['description']='用户提供的 9 页 PDF 原谱；原件与排版保留，音符标识直接包裹原矢量音头。首轮 OMR 的错误音高和漏谱未直接采用。'
    complete=metrics['note_complete_measures']
    manifest['coverage']=[dict(start_ms=0,end_ms=duration,granularity='measure',method='audio_alignment',status='accepted',provenance_id='erlkoenig-spectrum-following',detail='全曲自动小节定位；逐音提示按各小节的起音依据完整程度单独标记。')]
    manifest['coverage'] += [dict(start_ms=row['start_ms'],end_ms=min(duration,row['end_ms']),granularity='note',method='audio_alignment',status=row['status'],provenance_id='erlkoenig-spectrum-following',detail=f'{matched}/{total} 组起音有攻击峰依据，可显示自动音符提示；{interpolated} 组未观测音不显示光点，对应位置保留小节定位。已观测的持续音可以跨过局部缺口继续显示。这是来源统计，不是准确率。') for row in timing['attack_coverage']]
    page_first=[1,19,35,50,70,86,101,118,132]
    for e in manifest['events']:
        if e['event_id'].startswith('page-guide-'):
            page=int(e['event_id'].split('-')[-1]);a=page_first[page-1];b=page_first[page]-1 if page<9 else 148
            e['start_ms']=occurrences[a-1]['start_ms'];e['end_ms']=occurrences[b-1]['end_ms']
        else:
            bounds={'exercise-pulse':(1,9),'exercise-recitando':(15,18),'exercise-contrast':(50,58),'exercise-layers':(86,93),'exercise-pressure':(118,131),'exercise-ending':(146,148)}
            a,b=bounds[e['event_id']];e['start_ms']=occurrences[a-1]['start_ms'];e['end_ms']=occurrences[b-1]['end_ms']
        selected=[o for o in occurrences if o['start_ms']<e['end_ms'] and o['end_ms']>e['start_ms']];e['occurrence_ids']=[o['occurrence_id'] for o in selected]
        e['evidence']=[dict(provenance_id='native-pdf-index',score_object_ids=selected[0]['score_object_ids'][:24],score_page_ids=list(dict.fromkeys(n['page_id'] for n in manifest['score_objects'] if n['source_id'] in selected[0]['score_object_ids'])),audio=None),dict(provenance_id='erlkoenig-spectrum-following',score_object_ids=[],audio=dict(recording=manifest['recording']['source'],start_ms=e['start_ms'],end_ms=e['end_ms']))]
    for s in manifest['structure']:
        selected=[e for e in manifest['events'] if e['event_id'] in s['event_ids']]
        if selected:s['start_ms']=min(e['start_ms'] for e in selected);s['end_ms']=max(e['end_ms'] for e in selected);s['coverage']='detailed'
    # Save a complete, editable note table alongside the original score.
    save(WS/'music/native-index.json',dict(version='erlkoenig_native_index.v2',notes=notes,rests=rests,measure_rhythm=rhythm['measures'],alignment_metrics=timing['metrics']))
    save(WS/'.build/music/import-candidate.json',manifest)
    guide='''# 舒伯特—李斯特《魔王》：全曲原谱与自动跟谱

作品为 Erlkönig, S.558/4。原声来自你提供的视频，文件说明将钢琴演奏者标为王羽佳。保留你提供的 9 页 PDF 原排版，建立全曲 148 小节、5309 个音头与 165 个休止符的数字索引，并与 4 分 19 秒原声音轨建立自动跟谱。

## 怎样使用

直接播放可跟随当前小节滚动。有起音依据的音符显示自动光点，持续音可跨过局部缺口继续显示。未观测音不显示光点，对应位置保留小节定位；界面标为“部分跟谱”。上谱表和下谱表用两种颜色区分，可单独关注其中一层。点选音符可回到相应原声位置；拖动进度、调整倍速、循环当前片段均沿原录音工作。手动浏览后点击“回到播放位置”恢复跟随。

第一遍只选一个观察点：持续运动怎样在最后让位于停顿和分句。听完余音，再从作品导览中选择一个问题重听。

## 作品导览
'''
    for e in manifest['events']:
        if e['event_id'].startswith('page-guide-'):guide+=f'\n### {e["title"]}\n\n{e["explanation"]}\n\n{e["listen_for"]}\n'
    guide+='\n## 回听练习\n'
    for e in manifest['events']:
        if e['event_id'].startswith('exercise-'):guide+=f'\n### {e["title"]}\n\n{e["explanation"]}\n\n{e["listen_for"]}\n'
    guide+=f'''
## 来源与精度

音轨从用户 MP4 原样提取 AAC，未重新编码、裁剪或改速。原 PDF 和所有原矢量音头保持原位。第 83、131 小节的调号变化、谱号变化、八度提示和可识别的延音线用于索引；特殊节奏保留了原谱观察图。

自动对齐综合谱面音高、原声频谱与受约束的局部节奏。{total} 组谱面起音中，{matched} 组有攻击峰依据，{interpolated} 组由谱面时间映射估计。未观测音不显示光点，对应区间保留小节定位，其他有依据的音符照常提示。这是来源统计，不是逐音准确率。

## 已知限制

跟谱适合伴听定位，尚未经过独立人工逐音标注认证；密集重复音、琶音和多声部交叠处仍可能出现局部偏差。光点音尾按谱面音值映射，不代表实测松键或踏板释放。上下谱表的阅读分层不等同于完成了旋律、父亲、孩子和魔王四个角色的逐句声部分离。

当前数字成果为原谱音符索引与时间映射；原 PDF 是显示与核对依据。录音和该谱本保存在本机，公开分发许可未确认。演奏者标注依据用户视频说明。
'''
    (WS/'source.txt').write_text(guide,encoding='utf-8')
    report=dict(version='music_native_pdf_build_report.v1',source_pages=9,pages=9,measures=148,indexed_noteheads=len(notes),indexed_rests=len(rests),mapped_native_svg_noteheads=mapped,note_alignments=len(manifest['note_alignments']),unobserved_noteheads=sum(n['unobserved'] for n in timing['notes']),rhythm_methods=dict(Counter(m['method'] for m in rhythm['measures'])),alignment=timing['metrics'],acceptance='full measure navigation; observed attack intervals only; no independent human timing certification',events=len(manifest['events']),audio_operation='unchanged AAC stream copy from user MP4',duration_ms=duration,model_calls=0,limitations=timing['limitations'])
    save(WS/'music/build-report.json',report);print(json.dumps(report,ensure_ascii=False))

if __name__=='__main__':main()
