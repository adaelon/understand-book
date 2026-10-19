import { isDeepStrictEqual } from 'node:util';
import type { VisualUnit } from './video-visual-units';
import type { TranscriptMap } from './generated/TranscriptMap';
import { CODEX_EXECUTOR_MEDIA_CAPABILITY_V1 as cap } from './build-executor-media';

export interface LectureSelection { vu_ids:string[]; segment_ids:string[] }
/** Include every visual unit and every spoken segment. A long neighbouring
 * sentence that cannot fit the joint window receives its own bounded text task. */
export function planVideoLectureSemantics(units: VisualUnit[], segments: TranscriptMap['segments']): LectureSelection[] {
  const selections:LectureSelection[]=[], covered=new Set<string>();
  function fits(group:VisualUnit[], text:typeof segments){
    const locations=[...group.map(u=>u.location),...text.map(s=>s.location)];
    if(!locations.length||Math.max(...locations.map(l=>l.end_ms))-Math.min(...locations.map(l=>l.start_ms))>60000)return false;
    const frames=group.flatMap(u=>u.representative_frames).filter((f,i,a)=>a.findIndex(p=>isDeepStrictEqual(p.frame,f.frame))===i);
    return frames.length<=6&&frames.reduce((n,f)=>n+f.png_bytes,0)<=cap.max_png_bytes
      &&frames.reduce((n,f)=>n+f.frame.width*f.frame.height,0)<=cap.max_pixels;
  }
  function add(group:VisualUnit[]){
    const first=Math.min(...group.map(u=>u.location.start_ms)),last=Math.max(...group.map(u=>u.location.end_ms));
    const text:typeof segments=[];
    for(const s of segments.filter(s=>s.location.start_ms<last&&s.location.end_ms>first))if(fits(group,[...text,s]))text.push(s);
    selections.push({vu_ids:group.map(u=>u.vu_id),segment_ids:text.map(s=>s.segment_id)});text.forEach(s=>covered.add(s.segment_id));
  }
  let group:VisualUnit[]=[];
  for(const unit of [...units].sort((a,b)=>a.location.start_ms-b.location.start_ms)){
    if(!fits([unit],[]))throw new Error(`Visual unit exceeds semantic input bound: ${unit.vu_id}`);
    if(group.length&&!fits([...group,unit],[])){add(group);group=[];}
    group.push(unit);
  }
  if(group.length)add(group);
  let text:typeof segments=[];
  for(const s of segments.filter(s=>!covered.has(s.segment_id))){
    if(!fits([],[s]))throw new Error(`Transcript segment exceeds semantic input bound: ${s.segment_id}`);
    if(text.length&&!fits([],[...text,s])){selections.push({vu_ids:[],segment_ids:text.map(s=>s.segment_id)});text=[];}
    text.push(s);
  }
  if(text.length)selections.push({vu_ids:[],segment_ids:text.map(s=>s.segment_id)});
  return selections;
}
