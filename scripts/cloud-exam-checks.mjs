// Deterministic transport/context checks. The prose still requires a human
// semantic judgment; task completion alone never establishes teaching quality.
export function cloudExamChecks(row,records,video){
  const business=records.filter(r=>r.purpose==='business_loop');
  const states=business.map(r=>[...(r.request?.messages??[])].reverse().find(m=>
    typeof m.content==='string'&&m.content.startsWith('runtime_state_snapshot.v1'))?.content??'');
  const calendars=states.map(state=>{
    const raw=state.match(/workspace_goal\.v1\n([^\n]+)/)?.[1];
    return raw?JSON.parse(raw).exam_calendar:null;
  });
  const checks={
    execution_completed:row.status==='completed',
    delivery_not_incomplete:row.turn?.outcome?.incomplete===false,
    frozen_exam_calendar:calendars.length>0&&calendars.every(calendar=>
      calendar?.exam_date===row.sprint.settings.exam_date&&calendar?.remaining_days===row.sprint.remaining_days
      &&calendar?.workspace_goal_revision===row.sprint.goal_revision),
    no_negated_page_requirement:states.length>0&&states.every(state=>{
      const goal=state.split('key=agent.resident_goal\n')[1]?.split('\n\nkey=')[0]??'';
      return goal.length>0&&!goal.includes('[PresentationDelivery]');
    }),
  };
  if(row.id==='text-video-sources'){
    const sources=(row.sources??[]).map(source=>source.resolved?.result).filter(Boolean);
    checks.text_source_opens=sources.some(s=>s.can_open_in_reader===true&&s.stale===false
      &&!s.media&&typeof s.highlighted_quote==='string'&&s.highlighted_quote.length>0);
    checks.video_source_opens=sources.some(s=>s.can_open_in_reader===true&&s.stale===false
      &&s.media?.publication?.book_id===video.book_id&&s.media?.publication?.publication_id===video.publication_id
      &&s.media.transcript?.some(t=>t.location.start_ms===0&&t.location.end_ms===2000));
    checks.source_resolutions_succeeded=(row.sources??[]).length>0&&(row.sources??[]).every(s=>!s.error);
  }
  return {checks,all_passed:Object.values(checks).every(Boolean),exam_calendars:calendars,semantic_review:'required'};
}
