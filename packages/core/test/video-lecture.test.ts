import { expect, it } from 'vitest';
import { readFileSync } from 'node:fs';
import { planVideoLectureSemantics } from '../src/video-lecture';
import type { VisualUnit } from '../src/video-visual-units';
import type { TranscriptMap } from '../src/generated/TranscriptMap';

const fixture=JSON.parse(readFileSync(new URL('../../../crates/base-schema/fixtures/video-contract.json',import.meta.url),'utf8')).cases[0];
function unit(i:number,start:number,end:number):VisualUnit {
  const frame={...fixture.questions[0].frame,actual_pts_ms:start};
  return {vu_id:`vu-${i}`,analysis_id:'lecture',location:{video:fixture.manifest.material.video,start_ms:start,end_ms:end},
    representative_frames:[{id:`f${i}`,requested_ms:start,frame,path:`f${i}.png`,png_bytes:1000,content_id:`f${i}`}],
    screen_text:'',description:'screen',change:'changed',content_rect:[0,0,1,1],transcript_segment_ids:[],description_reused_from:null};
}
function segment(i:number,start:number,end:number):TranscriptMap['segments'][number] {
  return {...fixture.map.segments[0],segment_id:`s${i}`,location:{video:fixture.manifest.material.video,start_ms:start,end_ms:end}};
}
it('covers every image and spoken segment without exceeding native-image or 60-second inputs',()=>{
  const units=Array.from({length:20},(_,i)=>unit(i,i*10000,(i+1)*10000));
  const segments=[segment(0,5000,46000),segment(3,25000,85000),segment(1,65000,106000),segment(2,125000,166000)];
  const plans=planVideoLectureSemantics(units,segments);
  expect(plans.flatMap(p=>p.vu_ids).sort()).toEqual(units.map(u=>u.vu_id).sort());
  expect(new Set(plans.flatMap(p=>p.segment_ids))).toEqual(new Set(segments.map(s=>s.segment_id)));
  expect(plans.some(p=>p.vu_ids.length===0&&p.segment_ids.includes('s3'))).toBe(true);
  for(const p of plans){
    const chosen=units.filter(u=>p.vu_ids.includes(u.vu_id)), text=segments.filter(s=>p.segment_ids.includes(s.segment_id));
    const locations=[...chosen,...text].map(v=>v.location);
    expect(Math.max(...locations.map(l=>l.end_ms))-Math.min(...locations.map(l=>l.start_ms))).toBeLessThanOrEqual(60000);
    expect(chosen.flatMap(u=>u.representative_frames).length).toBeLessThanOrEqual(6);
  }
});
it('rejects an individually unbounded input instead of silently dropping it',()=>{
  expect(()=>planVideoLectureSemantics([unit(1,0,60001)],[])).toThrow('Visual unit exceeds');
  expect(()=>planVideoLectureSemantics([],[segment(1,0,60001)])).toThrow('Transcript segment exceeds');
});
