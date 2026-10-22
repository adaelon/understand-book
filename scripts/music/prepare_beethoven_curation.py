"""Make the local-only orchestral sample from real sources and DTW candidates."""
from pathlib import Path
import json
import xml.etree.ElementTree as E

ROOT = Path(__file__).resolve().parents[2]
OUT = ROOT / "evals/music"
d = json.loads((OUT/"results/beethoven-chroma-dtw/candidates.json").read_text(encoding="utf-8"))
source = json.loads((OUT/"beethoven-sources.json").read_text(encoding="utf-8"))
r = E.parse(OUT/"assets/beethoven-op125-4-m93-164.musicxml").getroot()
objects=[]
for part in r.findall('part'):
    for mi,m in enumerate(part.findall('measure'),1):
        for ni,n in enumerate(m.findall('note'),1):
            objects.append({'id':f"{part.get('id')}-m{mi}-n{ni}",'part':part.get('id'),'measure':int(m.get('number')),'pitched':n.find('pitch') is not None})
measures = [m for m in d['measures'] if m['part_id']=='P8']
times={int(m['label']):m['recording_seconds']*1000 for m in measures}
times[165]=d['last_attack_seconds']*1000
def ids(start,end,parts=None):
    return [x['id'] for x in objects if start<=x['measure']<=end and (parts is None or x['part'] in parts)]
occurrences=[]
for i,m in enumerate(measures):
    number=int(m['label'])
    if times[number+1] <= times[number]:
        raise RuntimeError(f'DTW collapsed measure {number}; manual anchor required')
    occurrences.append({'occurrence_id':f'measure-{number}-occurrence-1','order':i,'measure':str(number),'start_ms':times[number],'end_ms':times[number+1],'score_object_ids':ids(number,number)})
prov='beethoven-score-source';alignment='beethoven-coates-chroma-candidate'
events=[]
for key,title,lo,hi,explanation,listen,roles,prior in [
    ('low-strings-theme','先跟低弦记住旋律',93,116,'这份总谱中，大提琴和倍大提琴先呈现这段旋律；其他保留的声部在开头休止。先用低弦谱线记住旋律轮廓。','尝试跟着低弦哼出前几个音，留意同一旋律怎样展开。',[('P35','主题线条'),('P36','与大提琴同走的低弦线条')],None),
    ('viola-handoff','主题转到中提琴',117,140,'从第117小节开始，中提琴进入主题线条，同时巴松和低弦形成其他活动线条。把注意力从原来的低弦谱线移到中提琴，比较角色的变化。','重听交接前后：旋律保持哪些形状，周围又多了哪些线条？',[('P34','接过主题'),('P8','伴随线条'),('P35','伴随线条'),('P36','低音支持')],'low-strings-theme'),
    ('violin-handoff','小提琴加入之后',141,164,'这份总谱中，第141小节第一小提琴进入主题；第二小提琴、中提琴、巴松和低弦同时活动。可以比较主题位置和伴随线条数量的变化。','先只跟第一小提琴的谱线，再听它周围的活动。',[('P32','接过主题'),('P33','伴随线条'),('P34','伴随线条'),('P8','伴随线条'),('P35','低音与伴随线条'),('P36','低音支持')],'viola-handoff')]:
    events.append({'event_id':key,'title':title,'explanation':explanation,'listen_for':listen,'start_ms':times[lo],'end_ms':times[hi+1],'roles':[{'part_id':p,'role':role} for p,role in roles],'evidence':[{'provenance_id':prov,'score_object_ids':ids(lo,hi),'audio':None}],'relations':[] if prior is None else [{'target_event_id':prior,'kind':'handoff','description':'对照前一段主题的声部位置。'}],'concept_tags':['主题','乐器','声部角色']})
names={'P8':('第一巴松','Bassoon 1','木管'),'P32':('第一小提琴','Violin 1','弦乐'),'P33':('第二小提琴','Violin 2','弦乐'),'P34':('中提琴','Viola','弦乐'),'P35':('大提琴','Violoncello','弦乐'),'P36':('倍大提琴','Contrabass','弦乐')}
parts=[]
for part in r.findall('part'):
    name,instrument,group=names[part.get('id')]
    voices=sorted({f"{n.findtext('staff','1')}:{n.findtext('voice','1')}" for n in part.findall('.//note')})
    parts.append({'part_id':part.get('id'),'name':name,'instrument':instrument,'group':group,'voices':voices})
curation={
 'version':'music_curation.v1','book_id':'music-beethoven-op125-4-coates-local','title':'贝多芬 · 第九交响曲：主题在乐器间传递',
 'work':{'work_id':'beethoven-op125','title':'Symphony No.9 in D minor, Op.125','composer':'Ludwig van Beethoven','movement':'IV. Finale · 选段总谱第93–164小节；保留完整乐章录音'},
 'score':{'source':{'score_id':'beethoven-op125-4-openscore-m93-164','revision':'hauptstimme-download-20261022-excerpt'},'title':'OpenScore Orchestra / Hauptstimme · 第93–164小节活动声部','musicxml_path':'assets/beethoven-op125-4-m93-164.musicxml'},
 'recording':{'source':{'recording_id':'beethoven-op125-4-coates-damian','revision':'transfer-20240507-concat-mp3-20261022'},'title':'Albert Coates · 1923演奏 / Damian J Rogan转录 · 第四乐章完整录音','performers':'Albert Coates; Symphony Orchestra, chorus and soloists. Transfer/remaster: Damian J Rogan.','media_path':'assets/beethoven-op125-4-coates-damian-full.mp3','kind':'performance'},
 'rights':[
  {'asset_kind':'recording','title':'Coates历史录音的现代修复转录','url':source['recording']['source_page'],'license':'原1924录音已过保护期；现代转录修复再发布未获明确许可','attribution':'Albert Coates, Symphony Orchestra, chorus and soloists; recorded 1923, issued May 1924 on HMV D 842–849. Transferred/remastered by Damian J Rogan, 2024. Both finale files concatenated and transcoded to MP3; originals retained.','distribution':'local_only'},
  {'asset_kind':'score','title':'OpenScore Orchestra / Hauptstimme','url':source['score']['repository'],'license':'Score: CC0 1.0 Universal; retained annotations: CC-By-SA (publisher does not state version)','attribution':'Ludwig van Beethoven; OpenScore Orchestra / Hauptstimme, Mark Gotham and project transcribers/annotators. Extracted bars93–164, omitting parts silent throughout this excerpt; source bar labels retained. Original score files and attributed annotations retained locally.','distribution':'local_only'}],
 'parts':parts,'occurrences':occurrences,'note_alignments':[],
 'coverage':[{'start_ms':times[93],'end_ms':times[165],'granularity':'measure','method':'audio_alignment','status':'pending','provenance_id':alignment,'detail':'真实Coates录音与选段谱chroma子序列DTW；待独立听核，不宣称逐音准确。'}],
 'provenance':[
  {'provenance_id':prov,'kind':'score','description':'直接读取第93–164小节音符、声部进入和休止；主题关注参考署名Hauptstimme主奏标注。此预构建内容不代表当前交互模型听过音频。','source_url':source['score']['repository'],'processor':'MusicXML reading; attributed Hauptstimme annotations'},
  {'provenance_id':alignment,'kind':'audio_analysis','description':'从本次Coates完整乐章前420秒的真实波形计算频谱chroma，与288四分拍选段做子序列DTW；原录音时间轴，全部待复核。','source_url':None,'processor':'scripts/music/beethoven_alignment_probe.py'}],
 'events':events,
 'structure':[{'section_id':'movement-4','parent_id':None,'title':'第四乐章全局位置','description':'完整24分钟历史录音。当前深入材料仅第93–164小节，其他区域只有播放导航。','start_ms':0,'end_ms':source['recording']['duration_seconds']*1000,'event_ids':[],'coverage':'navigation_only'},{'section_id':'selected-theme','parent_id':'movement-4','title':'器乐主题的三次呈现','description':'保留6个在选段内实际发声的声部；完整36声部原谱另留作来源。时间为待复核对齐候选。','start_ms':times[93],'end_ms':times[165],'event_ids':[x['event_id'] for x in events],'coverage':'detailed'}],
 'demonstration_requests':[],
 'guide':'# 贝多芬 · 第九交响曲：主题在乐器间传递\n\n从低弦开始记住旋律，再观察中提琴与小提琴怎样接过它。每次只换一个关注对象，回听同一段。\n\n当前总谱只展示第四乐章第93–164小节实际活动的6个声部，完整乐章录音保留原有连续时间。三个教学事件以谱面和署名分析为依据；录音时间是待核对的自动候选。\n\n## 素材与限制\n\n演奏：Albert Coates，1923；首次发行1924。现代转录与修复：Damian J Rogan，2024；本机试听使用，尚未获得该修复版的明确商业再发布许可。总谱来自OpenScore Orchestra / Hauptstimme，谱面CC0，保留分析标注CC-By-SA（来源未标明版本）。完整来源、原始文件、变更说明留在样本材料中。\n'
}
theme_parts = {"low-strings-theme": {"P35", "P36"}, "viola-handoff": {"P34"}, "violin-handoff": {"P32"}}
for event in curation["events"]:
    for role in event["roles"]:
        if role["part_id"] in theme_parts[event["event_id"]]:
            role["theme_id"] = "beethoven-op125-4-joy-theme"
curation["provenance"][0]["description"] += " 对照所选主题声部，各次开始前十个谱音的音级、时值与小节内起点一致；其余声部的角色独立标注。"
(OUT/'beethoven-curation.json').write_text(json.dumps(curation,ensure_ascii=False,indent=2)+'\n',encoding='utf-8')
print(json.dumps({'curation':'evals/music/beethoven-curation.json','events':[(x['event_id'],x['start_ms'],x['end_ms']) for x in events],'parts':len(parts),'occurrences':len(occurrences),'note_accuracy':'pending','distribution':'local_only'}))
